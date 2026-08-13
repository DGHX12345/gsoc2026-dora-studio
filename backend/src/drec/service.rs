//! Recording manager service — manages open `.drec` recordings.
//!
//! Provides open/close lifecycle, index progress, and query endpoints.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::drec::{
    index::{DrecIndex, IndexEntry, IndexProgress, StreamInfo, StreamKey},
    reader::DrecReader,
    types::RecordEntry,
};

/// A fully-loaded recording with reader + index.
pub struct RecordingHandle {
    pub id: Uuid,
    pub path: PathBuf,
    pub header: crate::drec::types::RecordingHeader,
    pub index: DrecIndex,
}

impl RecordingHandle {
    /// Read the event_bytes for a single entry at the given byte offset.
    pub fn read_event_bytes(&self, byte_offset: u64) -> Result<Vec<u8>, String> {
        let mut reader =
            DrecReader::open(&self.path).map_err(|e| format!("failed to open reader: {e}"))?;
        let entry = reader
            .read_entry_at(byte_offset)
            .map_err(|e| format!("failed to read entry: {e}"))?;
        Ok(entry.event_bytes)
    }
}

/// Manages multiple open recordings by ID.
pub struct RecordingManager {
    recordings: Mutex<HashMap<Uuid, Arc<RecordingHandle>>>,
}

impl RecordingManager {
    pub fn new() -> Self {
        Self {
            recordings: Mutex::new(HashMap::new()),
        }
    }

    /// Open a `.drec` file, parse the header, and build the index.
    pub async fn open(&self, path: &Path) -> Result<Arc<RecordingHandle>, String> {
        let path = path.to_path_buf();
        let path_clone = path.clone();

        // Offload I/O to blocking thread
        let handle = tokio::task::spawn_blocking(move || -> Result<RecordingHandle, String> {
            let mut reader = DrecReader::open(&path_clone)
                .map_err(|e| format!("failed to open recording: {e}"))?;
            let header = reader.header().clone();
            let index =
                DrecIndex::build(&mut reader).map_err(|e| format!("failed to build index: {e}"))?;

            Ok(RecordingHandle {
                id: Uuid::new_v4(),
                path: path_clone,
                header,
                index,
            })
        })
        .await
        .map_err(|e| format!("blocking task panicked: {e}"))??;

        let handle = Arc::new(handle);
        let mut recordings = self.recordings.lock().await;
        recordings.insert(handle.id, Arc::clone(&handle));
        Ok(handle)
    }

    pub async fn get(&self, id: &Uuid) -> Option<Arc<RecordingHandle>> {
        self.recordings.lock().await.get(id).cloned()
    }

    pub async fn close(&self, id: &Uuid) {
        self.recordings.lock().await.remove(id);
    }

    pub async fn list_ids(&self) -> Vec<Uuid> {
        self.recordings.lock().await.keys().cloned().collect()
    }
}

// Convenience methods on RecordingHandle for the API layer.
impl RecordingHandle {
    pub fn seek(&self, timestamp_nanos: u64) -> Option<&IndexEntry> {
        self.index.seek_to_timestamp(timestamp_nanos)
    }

    pub fn streams(&self) -> Vec<StreamInfo> {
        self.index.streams()
    }

    pub fn stream_entries(
        &self,
        node: &str,
        output: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<&IndexEntry> {
        let key = StreamKey(node.to_string(), output.to_string());
        self.index.stream_entries(&key, offset, limit)
    }

    pub fn duration_nanos(&self) -> u64 {
        self.index.duration_nanos()
    }

    pub fn message_count(&self) -> usize {
        self.index.message_count()
    }

    pub fn entries_in_range(&self, start: u64, end: u64) -> Vec<&IndexEntry> {
        self.index.entries_in_range(start, end)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drec::generator::DrecGenerator;
    use crate::drec::types::{RecordEntry, RecordingHeader};

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("dora-studio-tests");
        std::fs::create_dir_all(&dir).ok();
        dir.join(name)
    }

    fn write_test_file(path: &Path, header: &RecordingHeader, entries: &[RecordEntry]) {
        let mut file = std::fs::File::create(path).unwrap();
        DrecGenerator::write_to(&mut file, header, entries).unwrap();
    }

    #[tokio::test]
    async fn open_and_query() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["cam", "lidar"], 5, 100_000);
        let file_path = temp_file_path("svc_open.drec");
        write_test_file(&file_path, &header, &entries);

        let mgr = RecordingManager::new();
        let handle = mgr.open(&file_path).await.unwrap();

        assert_eq!(handle.message_count(), 10);
        assert_eq!(handle.streams().len(), 2);

        let entry = handle.seek(header.start_nanos);
        assert!(entry.is_some());
        assert!(entry.unwrap().node_id == "cam" || entry.unwrap().node_id == "lidar");
        std::fs::remove_file(&file_path).ok();
    }

    #[tokio::test]
    async fn open_close_lifecycle() {
        let (header, entries) = DrecGenerator::generate_multi_stream(&["cam"], 2, 100_000);
        let file_path = temp_file_path("svc_close.drec");
        write_test_file(&file_path, &header, &entries);

        let mgr = RecordingManager::new();
        let handle = mgr.open(&file_path).await.unwrap();
        let id = handle.id;

        assert!(mgr.get(&id).await.is_some());
        mgr.close(&id).await;
        assert!(mgr.get(&id).await.is_none());
        std::fs::remove_file(&file_path).ok();
    }
}
