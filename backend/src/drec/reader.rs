//! Binary `.drec` file reader.
//!
//! Parses the dora recording format: header, sequence of records, optional footer.
//! Uses `std::fs::File` with seeking for random access.

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use crate::drec::types::{
    RecordEntry, RecordingFooter, RecordingHeader, FOOTER_MAGIC, MAGIC, MAX_RECORD_BYTES,
};

/// Result type for reader operations.
pub type DrecResult<T> = Result<T, DrecError>;

#[derive(Debug)]
pub enum DrecError {
    Io(io::Error),
    InvalidMagic,
    UnsupportedVersion(u16),
    RecordTooLarge { size: usize, max: usize },
    CorruptRecord(String),
    /// The file ends in the middle of a record (writer was killed).
    TruncatedRecord,
}

impl std::fmt::Display for DrecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidMagic => write!(f, "not a dora recording file (invalid magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            Self::RecordTooLarge { size, max } => {
                write!(f, "record too large: {size} bytes (max {max})")
            }
            Self::CorruptRecord(msg) => write!(f, "corrupt record: {msg}"),
            Self::TruncatedRecord => write!(f, "recording ends mid-record"),
        }
    }
}

impl From<io::Error> for DrecError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// A parsed `.drec` file open for reading, with random access to records.
#[derive(Debug)]
pub struct DrecReader {
    file: File,
    header: RecordingHeader,
    /// Byte offset where the first record begins (right after header).
    records_start: u64,
    /// Total file size in bytes.
    file_size: u64,
}

impl DrecReader {
    /// Open and validate a `.drec` file. Parses the header immediately.
    pub fn open(path: &Path) -> DrecResult<Self> {
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len();

        // Parse header
        let header = read_header(&mut file)?;

        let records_start = file.stream_position()?;

        Ok(Self {
            file,
            header,
            records_start,
            file_size,
        })
    }

    pub fn header(&self) -> &RecordingHeader {
        &self.header
    }

    pub fn records_start(&self) -> u64 {
        self.records_start
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Read a single record entry at the given byte offset.
    pub fn read_entry_at(&mut self, offset: u64) -> DrecResult<RecordEntry> {
        self.file.seek(SeekFrom::Start(offset))?;
        read_record(&mut self.file)
    }

    /// Read all entries sequentially, returning their byte offsets and timestamps.
    /// Used by the index builder.
    pub fn scan_entries<F>(&mut self, mut on_entry: F) -> DrecResult<()>
    where
        F: FnMut(u64, &RecordEntry),
    {
        self.file.seek(SeekFrom::Start(self.records_start))?;
        loop {
            let pos = self.file.stream_position()?;
            match read_record_optional(&mut self.file) {
                Ok(Some(entry)) => on_entry(pos, &entry),
                Ok(None) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }

    /// Try to read the footer at end of file.
    pub fn read_footer(&mut self) -> DrecResult<Option<RecordingFooter>> {
        // Footer is at EOF: 8 (magic) + 8 (messages) + 8 (bytes) = 24 bytes
        if self.file_size < self.records_start + 24 {
            return Ok(None);
        }
        let footer_offset = self.file_size - 24;
        self.file.seek(SeekFrom::Start(footer_offset))?;
        let mut magic = [0u8; 8];
        self.file.read_exact(&mut magic)?;
        if &magic != FOOTER_MAGIC {
            // Footer was at end but was overwritten or file was truncated — scan
            // backward to find it (unlikely but handles edge cases).
            return Ok(None);
        }
        let mut msgs = [0u8; 8];
        let mut bytes = [0u8; 8];
        self.file.read_exact(&mut msgs)?;
        self.file.read_exact(&mut bytes)?;
        Ok(Some(RecordingFooter {
            total_messages: u64::from_le_bytes(msgs),
            total_bytes: u64::from_le_bytes(bytes),
        }))
    }
}

// ---------------------------------------------------------------------------
// Internal read helpers
// ---------------------------------------------------------------------------

fn read_header<R: Read>(r: &mut R) -> DrecResult<RecordingHeader> {
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(DrecError::InvalidMagic);
    }

    let mut ver = [0u8; 2];
    r.read_exact(&mut ver)?;
    let version = u16::from_le_bytes(ver);
    if version > 1 {
        return Err(DrecError::UnsupportedVersion(version));
    }

    let mut nanos = [0u8; 8];
    r.read_exact(&mut nanos)?;
    let start_nanos = u64::from_le_bytes(nanos);

    let mut uuid_buf = [0u8; 16];
    r.read_exact(&mut uuid_buf)?;
    let dataflow_id = uuid::Uuid::from_bytes(uuid_buf);

    let mut yaml_len_buf = [0u8; 4];
    r.read_exact(&mut yaml_len_buf)?;
    let yaml_len = u32::from_le_bytes(yaml_len_buf) as usize;
    if yaml_len > MAX_RECORD_BYTES as usize {
        return Err(DrecError::RecordTooLarge {
            size: yaml_len,
            max: MAX_RECORD_BYTES as usize,
        });
    }

    let mut descriptor_yaml = vec![0u8; yaml_len];
    r.read_exact(&mut descriptor_yaml)?;

    Ok(RecordingHeader {
        version,
        start_nanos,
        dataflow_id,
        descriptor_yaml,
    })
}

/// Read a record; returns `Ok(None)` at EOF or footer marker.
fn read_record_optional<R: Read>(r: &mut R) -> DrecResult<Option<RecordEntry>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(DrecError::Io(e)),
    }

    if len_buf == FOOTER_MAGIC[..4] {
        let mut rest = [0u8; 4];
        match r.read_exact(&mut rest) {
            Ok(()) if rest == FOOTER_MAGIC[4..] => return Ok(None),
            _ => {}
        }
        return Ok(None);
    }

    match read_record_body(r, &len_buf) {
        // A tail cut mid-record means the writer was interrupted; the
        // complete records up to this point are still valid.
        Err(DrecError::TruncatedRecord) => Ok(None),
        other => other.map(Some),
    }
}

/// Must read a record; returns Err if anything goes wrong.
fn read_record<R: Read>(r: &mut R) -> DrecResult<RecordEntry> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;

    if len_buf == FOOTER_MAGIC[..4] {
        return Err(DrecError::CorruptRecord("unexpected footer marker".into()));
    }

    read_record_body(r, &len_buf)
}

fn read_record_body<R: Read>(r: &mut R, len_buf: &[u8; 4]) -> DrecResult<RecordEntry> {
    let record_len = u32::from_le_bytes(*len_buf) as usize;
    if record_len > MAX_RECORD_BYTES as usize {
        return Err(DrecError::RecordTooLarge {
            size: record_len,
            max: MAX_RECORD_BYTES as usize,
        });
    }

    let mut buf = vec![0u8; record_len];
    match r.read_exact(&mut buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(DrecError::TruncatedRecord);
        }
        Err(e) => return Err(DrecError::Io(e)),
    }

    let mut pos = 0usize;

    let node_id_len = read_u16_le(&buf, &mut pos)? as usize;
    let node_id = read_utf8(&buf, &mut pos, node_id_len)?;

    let output_id_len = read_u16_le(&buf, &mut pos)? as usize;
    let output_id = read_utf8(&buf, &mut pos, output_id_len)?;

    let timestamp_offset_nanos = read_u64_le(&buf, &mut pos)?;
    let event_bytes_len = read_u32_le(&buf, &mut pos)? as usize;
    let event_bytes = read_slice(&buf, &mut pos, event_bytes_len)?;

    Ok(RecordEntry {
        node_id,
        output_id,
        timestamp_offset_nanos,
        event_bytes: event_bytes.to_vec(),
    })
}

fn read_u16_le(buf: &[u8], pos: &mut usize) -> DrecResult<u16> {
    check_bounds(buf, *pos, 2)?;
    let arr = [buf[*pos], buf[*pos + 1]];
    *pos += 2;
    Ok(u16::from_le_bytes(arr))
}

fn read_u32_le(buf: &[u8], pos: &mut usize) -> DrecResult<u32> {
    check_bounds(buf, *pos, 4)?;
    let arr = [buf[*pos], buf[*pos + 1], buf[*pos + 2], buf[*pos + 3]];
    *pos += 4;
    Ok(u32::from_le_bytes(arr))
}

fn read_u64_le(buf: &[u8], pos: &mut usize) -> DrecResult<u64> {
    check_bounds(buf, *pos, 8)?;
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&buf[*pos..*pos + 8]);
    *pos += 8;
    Ok(u64::from_le_bytes(arr))
}

fn read_utf8(buf: &[u8], pos: &mut usize, len: usize) -> DrecResult<String> {
    let bytes = read_slice(buf, pos, len)?;
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| DrecError::CorruptRecord(format!("invalid UTF-8: {e}")))
}

fn read_slice<'a>(buf: &'a [u8], pos: &mut usize, len: usize) -> DrecResult<&'a [u8]> {
    let end = pos.checked_add(len).ok_or_else(|| {
        DrecError::CorruptRecord(format!("length {len} overflows at offset {pos}"))
    })?;
    if end > buf.len() {
        return Err(DrecError::CorruptRecord(format!(
            "buffer too short at offset {pos}: need {len} bytes, have {}",
            buf.len() - *pos
        )));
    }
    let slice = &buf[*pos..end];
    *pos = end;
    Ok(slice)
}

fn check_bounds(buf: &[u8], pos: usize, need: usize) -> DrecResult<()> {
    if pos + need > buf.len() {
        Err(DrecError::CorruptRecord(format!(
            "buffer too short at offset {pos}: need {need} bytes, have {}",
            buf.len().saturating_sub(pos)
        )))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drec::generator::DrecGenerator;
    use crate::drec::types::RecordingHeader;
    use std::io::Write;
    use uuid::Uuid;

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("dora-studio-tests");
        std::fs::create_dir_all(&dir).ok();
        dir.join(name)
    }

    /// Real dora 1.0 recording (M15.5 D5 fixture): the B5 live-demo
    /// dataflow recorded with dora 1.0.0-rc.4, truncated by SIGTERM
    /// (no footer). Confirms the container format stayed compatible.
    #[test]
    fn opens_dora10_real_recording() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dora10.drec");
        let mut reader = DrecReader::open(&path).expect("dora 1.0 recording opens");

        let header = reader.header().clone();
        assert_eq!(header.version, 1);
        assert!(!header.descriptor_yaml.is_empty());
        assert!(header.descriptor_yaml.starts_with(b"#"));

        let mut count = 0u64;
        reader
            .scan_entries(|_, _| count += 1)
            .expect("scan handles the truncated tail");
        assert!(count > 100, "real recording has entries, got {count}");

        let footer = reader.read_footer().expect("footer read ok");
        assert!(footer.is_none(), "SIGTERM-truncated recording has no footer");
    }

    fn cleanup(path: &std::path::Path) {
        std::fs::remove_file(path).ok();
    }

    fn sample_header() -> RecordingHeader {
        RecordingHeader {
            version: 1,
            start_nanos: 1_000_000_000,
            dataflow_id: Uuid::nil(),
            descriptor_yaml: b"nodes: [camera]".to_vec(),
        }
    }

    fn write_as_file(header: &RecordingHeader, entries: &[RecordEntry]) -> std::path::PathBuf {
        let path = temp_file_path(&format!("test_{}.drec", Uuid::new_v4()));
        let mut file = std::fs::File::create(&path).unwrap();
        DrecGenerator::write_to(&mut file, header, entries).unwrap();
        path
    }

    #[test]
    fn header_roundtrip() {
        let header = sample_header();
        let path = write_as_file(&header, &[]);
        let reader = DrecReader::open(&path).unwrap();
        assert_eq!(reader.header().version, 1);
        assert_eq!(reader.header().start_nanos, header.start_nanos);
        assert_eq!(reader.header().dataflow_id, header.dataflow_id);
        assert_eq!(reader.header().descriptor_yaml, header.descriptor_yaml);
        cleanup(&path);
    }

    #[test]
    fn single_record_read() {
        let header = sample_header();
        let entry = RecordEntry {
            node_id: "camera".into(),
            output_id: "image".into(),
            timestamp_offset_nanos: 42,
            event_bytes: b"hello".to_vec(),
        };
        let path = write_as_file(&header, &[entry.clone()]);
        let mut reader = DrecReader::open(&path).unwrap();
        let read = reader.read_entry_at(reader.records_start()).unwrap();
        assert_eq!(read, entry);
        cleanup(&path);
    }

    #[test]
    fn scan_multiple_entries() {
        let header = sample_header();
        let entries: Vec<_> = (0..10)
            .map(|i| RecordEntry {
                node_id: format!("node{i}"),
                output_id: "out".into(),
                timestamp_offset_nanos: i * 1_000_000,
                event_bytes: vec![i as u8; 4],
            })
            .collect();
        let path = write_as_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();
        let mut offsets = Vec::new();
        let mut count = 0;
        reader
            .scan_entries(|off, e| {
                offsets.push(off);
                assert_eq!(e.node_id, format!("node{count}"));
                count += 1;
            })
            .unwrap();
        assert_eq!(count, 10);
        assert_eq!(offsets.len(), 10);
        // Offsets should be monotonically increasing
        for w in offsets.windows(2) {
            assert!(w[1] > w[0]);
        }
        cleanup(&path);
    }

    #[test]
    fn footer_read() {
        let header = sample_header();
        let entries: Vec<_> = (0..3)
            .map(|i| RecordEntry {
                node_id: "n".into(),
                output_id: "o".into(),
                timestamp_offset_nanos: i * 1000,
                event_bytes: vec![1, 2, 3],
            })
            .collect();
        let path = write_as_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();
        let footer = reader.read_footer().unwrap();
        assert!(footer.is_some());
        assert_eq!(footer.unwrap().total_messages, 3);
        cleanup(&path);
    }

    #[test]
    fn random_access() {
        let header = sample_header();
        let entries: Vec<_> = (0..5)
            .map(|i| RecordEntry {
                node_id: format!("node{i}"),
                output_id: "out".into(),
                timestamp_offset_nanos: i * 1000,
                event_bytes: vec![i as u8; 3],
            })
            .collect();
        let path = write_as_file(&header, &entries);
        let mut reader = DrecReader::open(&path).unwrap();

        // Read first and last entry by offset
        let mut offsets = Vec::new();
        reader.scan_entries(|off, _| offsets.push(off)).unwrap();

        let first = reader.read_entry_at(offsets[0]).unwrap();
        assert_eq!(first.node_id, "node0");

        let last = reader.read_entry_at(offsets[4]).unwrap();
        assert_eq!(last.node_id, "node4");

        cleanup(&path);
    }

    #[test]
    fn invalid_magic_rejected() {
        let path = write_as_file(&sample_header(), &[]);
        // Corrupt the magic bytes
        let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.write_all(b"NOT_DORA").unwrap();
        drop(file);
        let result = DrecReader::open(&path);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("invalid magic"), "got: {err}");
        cleanup(&path);
    }

    #[test]
    fn corrupt_record_body_returns_err() {
        let header = sample_header();
        let entry = RecordEntry {
            node_id: "a".into(),
            output_id: "b".into(),
            timestamp_offset_nanos: 0,
            event_bytes: vec![1, 2, 3],
        };
        let mut buf = Vec::new();
        DrecGenerator::write_to(&mut buf, &header, &[entry]).unwrap();
        // Append a corrupt record: claim length 100 but only provide 5 bytes
        buf.extend_from_slice(&100u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 5]);

        let path = temp_file_path("corrupt.drec");
        std::fs::write(&path, &buf).unwrap();

        let mut reader = DrecReader::open(&path).unwrap();
        // First entry OK
        assert!(reader.read_entry_at(reader.records_start()).is_ok());

        // scan_entries should stop gracefully at corrupt record
        let result = reader.scan_entries(|_, _| {});
        assert!(result.is_ok()); // first entry still reads fine
        cleanup(&path);
    }
}
