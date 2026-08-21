# M04: .drec Parser + Offset Index

**Layer**: 2 — .drec Replay
**Depends on**: M00 (drec types)
**Effort**: 1.5 weeks

## Purpose

dora's `RecordingReader` is sequential-only with no seek or mmap. Build
our own reader with an offset index to enable random-access timeline
scrubbing. Parse the binary .drec format directly using the types from M00.

## Deliverables

### D1: Binary format reader

New file: `backend/src/drec/reader.rs`

- `DrecReader::open(path) -> Result<Self>`
  - Memory-map the file (memmap2 crate or std `fs::File` + read_at)
  - Validate MAGIC header (8 bytes: `DORAREC\x00`)
  - Parse `RecordingHeader`: version, start_nanos, dataflow_id, descriptor_yaml
- `read_entry_at(offset: u64) -> Result<RecordEntry>`
  - Read record_len, then node_id, output_id, timestamp_offset, event_bytes
  - Validate against MAX_RECORD_BYTES
- `validate_footer() -> Result<RecordingFooter>` — optional, at end of file
- Graceful handling of torn trailing records (crash mid-write)

### D2: Offset index builder

New file: `backend/src/drec/index.rs`

- `DrecIndex::build(reader: &DrecReader) -> Result<Self>`
  - Single-pass sequential scan of the file
  - Record byte offset + absolute timestamp (header.start_nanos + offset) for every entry
  - Store as `Vec<IndexEntry>` sorted by timestamp

```rust
pub struct DrecIndex {
    entries: Vec<IndexEntry>,       // sorted by timestamp
    streams: HashMap<StreamKey, Vec<usize>>, // (node,port) → indices into entries
}

pub struct IndexEntry {
    pub byte_offset: u64,
    pub timestamp_absolute_nanos: u64,
    pub node_id: String,
    pub output_id: String,
    pub record_len: u32,
}

pub struct StreamKey(pub String, pub String); // (node_id, output_id)
```

- `build_async(path)`: run index build in tokio `spawn_blocking`
- Progress reporting: `IndexProgress { indexed_bytes, total_bytes, entry_count }`
- Target: 1GB file indexed in <3s

### D3: Seek and query API

```rust
impl DrecIndex {
    /// Binary search for entry at or just before timestamp
    pub fn seek_to_timestamp(&self, nanos: u64) -> Option<&IndexEntry>;

    /// All streams in the recording
    pub fn streams(&self) -> Vec<StreamInfo>;

    /// Paginated entries for a specific stream
    pub fn stream_entries(&self, stream: &StreamKey, offset: usize, limit: usize) -> &[IndexEntry];

    /// Total duration in nanoseconds
    pub fn duration_nanos(&self) -> u64;

    /// Total message count
    pub fn message_count(&self) -> usize;
}

pub struct StreamInfo {
    pub node_id: String,
    pub output_id: String,
    pub entry_count: usize,
    pub time_range: (u64, u64),
}
```

### D4: Recording service

New file: `backend/src/drec/service.rs`

- `RecordingHandle` — open recording + built index, stored in AppState
- `RecordingManager` — manage multiple open recordings by ID
- Open: `POST /api/recording/open { path }` → `{ id, header, streams, duration_nanos, message_count }`
- Index progress: `GET /api/recording/:id/index-progress` → `{ done, pct }`
- Seek: `GET /api/recording/:id/seek?timestamp=T` → single entry
- Paginated: `GET /api/recording/:id/entries?stream=N&offset=0&limit=100` → entries list
- Stream list: `GET /api/recording/:id/streams` → `Vec<StreamInfo>`
- Close: `POST /api/recording/:id/close`

### D5: Synthetic .drec generator (dev tool)

New file: `backend/src/drec/generator.rs` (dev-only, behind `#[cfg(test)]` or feature flag)

- Generate valid .drec files for testing
- Configurable: N nodes, M entries per node, timestamp range
- Used when no real .drec files are available
- Enables development of M05-M06 without real dora recordings

## Dependencies Added

```toml
[dependencies]
memmap2 = "0.9"     # memory-mapped file I/O
uuid = { version = "1", features = ["serde", "v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

## Acceptance Criteria

- [ ] Parse a real (or generated) .drec file, extract header + all entries
- [ ] 1GB+ file: index builds in <3s, progress reported via API
- [ ] `seek_to_timestamp` returns correct entry within 200ms (binary search + disk read)
- [ ] Per-stream pagination returns correct entries
- [ ] Multiple concurrent recordings open (separate IDs)
- [ ] Synthetic generator creates valid .drec roundtrips
- [ ] `cargo test` includes: header parse, index build, seek, pagination, stream grouping
- [ ] `npm run build` passes

## Exposed Interfaces

```rust
// backend/src/drec/types.rs (from M00)
pub use self::{RecordingHeader, RecordEntry, RecordingFooter, MAGIC, FOOTER_MAGIC};

// backend/src/drec/index.rs
pub use self::{DrecIndex, IndexEntry, StreamKey, StreamInfo, IndexProgress};

// backend/src/drec/service.rs
pub struct RecordingHandle { reader: DrecReader, index: DrecIndex }
pub struct RecordingManager { recordings: HashMap<Uuid, RecordingHandle> }
impl RecordingManager {
    pub async fn open(&mut self, path: &Path) -> Result<Uuid>;
    pub fn get(&self, id: Uuid) -> Option<&RecordingHandle>;
    pub fn close(&mut self, id: Uuid);
}
```
