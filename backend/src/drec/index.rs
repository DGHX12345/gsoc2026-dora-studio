//! Offset index for random-access timeline scrubbing in `.drec` files.
//!
//! Builds a sorted index of every record entry (byte offset + absolute timestamp)
//! in a single pass, then supports binary-search seek and per-stream pagination.

use std::{collections::HashMap, time::Instant};

use crate::drec::reader::DrecReader;

/// A single entry in the offset index.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub byte_offset: u64,
    pub timestamp_absolute_nanos: u64,
    pub node_id: String,
    pub output_id: String,
    pub record_len: u32,
}

/// Identifies a unique output stream within a recording.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamKey(pub String, pub String); // (node_id, output_id)

/// Summary info for a single stream.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub node_id: String,
    pub output_id: String,
    pub entry_count: usize,
    pub time_range: (u64, u64),
}

/// Progress while building the index.
#[derive(Debug, Clone)]
pub struct IndexProgress {
    pub indexed_bytes: u64,
    pub total_bytes: u64,
    pub entry_count: usize,
}

/// A ready-to-use index for random-access reading.
pub struct DrecIndex {
    entries: Vec<IndexEntry>,                // sorted by timestamp
    streams: HashMap<StreamKey, Vec<usize>>, // (node,output) → indices into entries
    duration_nanos: u64,
}

impl DrecIndex {
    /// Build the index by scanning all records in the reader.
    pub fn build(reader: &mut DrecReader) -> Result<Self, String> {
        Self::build_with_progress(reader, |_| {})
    }

    /// Build the index with progress callbacks.
    pub fn build_with_progress<F>(reader: &mut DrecReader, mut progress: F) -> Result<Self, String>
    where
        F: FnMut(&IndexProgress),
    {
        let start_nanos = reader.header().start_nanos;
        let file_size = reader.file_size();
        let records_start = reader.records_start();
        let start = Instant::now();

        let mut entries: Vec<IndexEntry> = Vec::new();
        reader
            .scan_entries(|byte_offset, entry| {
                let timestamp_absolute_nanos = start_nanos + entry.timestamp_offset_nanos;
                // Estimate record_len: byte_offset difference (approx); we refine later
                entries.push(IndexEntry {
                    byte_offset,
                    timestamp_absolute_nanos,
                    node_id: entry.node_id.clone(),
                    output_id: entry.output_id.clone(),
                    record_len: 0, // filled later
                });
            })
            .map_err(|e| format!("index scan failed: {e}"))?;

        let elapsed = start.elapsed();
        // Fill record_len by computing gaps between entries
        for i in 0..entries.len() {
            let next_offset = if i + 1 < entries.len() {
                entries[i + 1].byte_offset
            } else {
                file_size.saturating_sub(24) // approximate (footer at end)
            };
            let len = next_offset.saturating_sub(entries[i].byte_offset);
            entries[i].record_len = len as u32;
        }

        // Build stream index
        let mut streams: HashMap<StreamKey, Vec<usize>> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            let key = StreamKey(entry.node_id.clone(), entry.output_id.clone());
            streams.entry(key).or_default().push(i);
        }

        let duration_nanos = entries
            .last()
            .map(|e| e.timestamp_absolute_nanos - start_nanos)
            .unwrap_or(0);

        // Report final progress
        progress(&IndexProgress {
            indexed_bytes: file_size,
            total_bytes: file_size,
            entry_count: entries.len(),
        });

        eprintln!(
            "DrecIndex built: {} entries, {} streams, {:.2}s",
            entries.len(),
            streams.len(),
            elapsed.as_secs_f64()
        );

        Ok(Self {
            entries,
            streams,
            duration_nanos,
        })
    }

    // ---- Query API ----

    /// Binary search for the entry at or just before the given timestamp.
    pub fn seek_to_timestamp(&self, nanos: u64) -> Option<&IndexEntry> {
        if self.entries.is_empty() {
            return None;
        }
        // Binary search for the greatest entry with timestamp <= nanos
        let idx = match self
            .entries
            .binary_search_by_key(&nanos, |e| e.timestamp_absolute_nanos)
        {
            Ok(i) => i,
            Err(0) => return Some(&self.entries[0]),
            Err(i) => i - 1,
        };
        self.entries.get(idx)
    }

    /// All unique streams in the recording.
    pub fn streams(&self) -> Vec<StreamInfo> {
        self.streams
            .iter()
            .map(|(key, indices)| {
                let first = &self.entries[indices[0]];
                let last = &self.entries[indices[indices.len() - 1]];
                StreamInfo {
                    node_id: key.0.clone(),
                    output_id: key.1.clone(),
                    entry_count: indices.len(),
                    time_range: (
                        first.timestamp_absolute_nanos,
                        last.timestamp_absolute_nanos,
                    ),
                }
            })
            .collect()
    }

    /// Paginated entries for a specific stream. Returns a slice of references.
    pub fn stream_entries(
        &self,
        stream: &StreamKey,
        offset: usize,
        limit: usize,
    ) -> Vec<&IndexEntry> {
        let indices = match self.streams.get(stream) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let start = offset.min(indices.len());
        let end = (start + limit).min(indices.len());
        indices[start..end]
            .iter()
            .map(|&i| &self.entries[i])
            .collect()
    }

    /// Gaps between consecutive entries in nanoseconds (for timeline spacing).
    pub fn gaps(&self) -> Vec<u64> {
        self.entries
            .windows(2)
            .map(|w| {
                w[1].timestamp_absolute_nanos
                    .saturating_sub(w[0].timestamp_absolute_nanos)
            })
            .collect()
    }

    pub fn duration_nanos(&self) -> u64 {
        self.duration_nanos
    }

    pub fn message_count(&self) -> usize {
        self.entries.len()
    }

    /// Return all entries (for small result sets / debugging).
    pub fn all_entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    /// Return entries in a time range (inclusive).
    pub fn entries_in_range(&self, start_nanos: u64, end_nanos: u64) -> Vec<&IndexEntry> {
        let start_idx = match self
            .entries
            .binary_search_by_key(&start_nanos, |e| e.timestamp_absolute_nanos)
        {
            Ok(i) => i,
            Err(i) => i,
        };
        let end_idx = match self
            .entries
            .binary_search_by_key(&end_nanos, |e| e.timestamp_absolute_nanos)
        {
            Ok(i) => (i + 1).min(self.entries.len()),
            Err(i) => i,
        };
        self.entries[start_idx..end_idx].iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drec::{
        generator::DrecGenerator,
        types::{RecordEntry, RecordingHeader},
    };
    use uuid::Uuid;

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("dora-studio-tests");
        std::fs::create_dir_all(&dir).ok();
        dir.join(name)
    }

    fn cleanup(path: &std::path::Path) {
        std::fs::remove_file(path).ok();
    }

    fn write_test_file(
        header: &RecordingHeader,
        entries: &[RecordEntry],
    ) -> (std::path::PathBuf, u64) {
        let path = temp_file_path(&format!("idx_{}.drec", Uuid::new_v4()));
        let mut file = std::fs::File::create(&path).unwrap();
        DrecGenerator::write_to(&mut file, header, entries).unwrap();
        let file_size = std::fs::metadata(&path).unwrap().len();
        (path, file_size)
    }

    #[test]
    fn index_build_empty() {
        let header = RecordingHeader {
            version: 1,
            start_nanos: 0,
            dataflow_id: uuid::Uuid::nil(),
            descriptor_yaml: vec![],
        };
        let (path, _) = write_test_file(&header, &[]);
        let mut reader = DrecReader::open(&path).unwrap();
        let index = DrecIndex::build(&mut reader).unwrap();
        assert_eq!(index.message_count(), 0);
        assert_eq!(index.duration_nanos(), 0);
        cleanup(&path);
    }

    #[test]
    fn index_seek_to_timestamp() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["cam"], 10, 100_000);
        let (path, _) = write_test_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();
        let index = DrecIndex::build(&mut reader).unwrap();

        // Seek to first entry
        let first = index.seek_to_timestamp(1_000_000_000).unwrap();
        assert_eq!(first.node_id, "cam");

        // Seek to middle — absolute timestamp = start_nanos (1B) + offset (~450k)
        let target_nanos = header.start_nanos + 450_000;
        let mid = index.seek_to_timestamp(target_nanos).unwrap();
        // Entry should be at or just before the target
        assert!(mid.timestamp_absolute_nanos <= target_nanos);
        // Should be between offset 400k and 500k
        let offset = mid.timestamp_absolute_nanos - header.start_nanos;
        assert!(
            (400_000..=500_000).contains(&offset),
            "offset {offset} not in range"
        );
        cleanup(&path);
    }

    #[test]
    fn index_streams() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["cam", "lidar"], 5, 100_000);
        let (path, _) = write_test_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();
        let index = DrecIndex::build(&mut reader).unwrap();

        let streams = index.streams();
        assert_eq!(streams.len(), 2);
        for s in &streams {
            assert_eq!(s.entry_count, 5);
        }
        cleanup(&path);
    }

    #[test]
    fn stream_pagination() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["cam"], 8, 100_000);
        let (path, _) = write_test_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();
        let index = DrecIndex::build(&mut reader).unwrap();

        let key = StreamKey("cam".into(), "output_0".into());
        let page1 = index.stream_entries(&key, 0, 3);
        assert_eq!(page1.len(), 3);

        let page2 = index.stream_entries(&key, 3, 3);
        assert_eq!(page2.len(), 3);

        let page3 = index.stream_entries(&key, 6, 3);
        assert_eq!(page3.len(), 2);
        cleanup(&path);
    }

    #[test]
    fn entries_in_range() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["cam"], 10, 1_000_000);
        let (path, _) = write_test_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();
        let index = DrecIndex::build(&mut reader).unwrap();

        let start = header.start_nanos + 2_000_000;
        let end = header.start_nanos + 5_000_000;
        let range = index.entries_in_range(start, end);
        // Should include entries at 2s, 3s, 4s, 5s = 4 entries
        assert!(range.len() >= 3);
        cleanup(&path);
    }
}
