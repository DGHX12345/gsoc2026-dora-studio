use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

pub type RecordingHandle = Arc<RecordingController>;

/// A healthy `dora record` keeps running for the lifetime of the
/// recording; a quick exit means submission failed.
const RECORD_EXIT_TIMEOUT: Duration = Duration::from_secs(3);
const LOG_LIMIT: usize = 500;

pub struct RecordingController {
    child: Mutex<Option<Child>>,
    active: Mutex<Option<ActiveRecording>>,
    logs: Mutex<Vec<String>>,
    output_dir: PathBuf,
}

#[derive(Clone)]
struct ActiveRecording {
    path: PathBuf,
    dataflow_path: String,
    started_at_millis: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub status: String,
    pub output_path: Option<String>,
    pub dataflow_path: Option<String>,
    pub started_at_millis: Option<u64>,
    pub frame_count: Option<u64>,
    pub message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEntry {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub created_at_millis: u64,
    pub frame_count: Option<u64>,
}

fn build_record_command(binary: &str, dataflow_path: &Path, output_path: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .arg("record")
        .arg(dataflow_path)
        .arg("-o")
        .arg(output_path)
        // A terminal stdin flips dora record into interactive mode;
        // recordings run non-interactively.
        .stdin(std::process::Stdio::null());
    command
}

fn recording_output_path(output_dir: &Path, now_millis: u64) -> PathBuf {
    output_dir.join(format!("{now_millis}.drec"))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Frame count of a recording file: the footer when present (exact),
/// otherwise a sequential scan (tolerates truncated files).
pub fn count_frames(path: &Path) -> Result<u64, String> {
    let mut reader =
        crate::drec::reader::DrecReader::open(path).map_err(|error| error.to_string())?;
    if let Some(footer) = reader.read_footer().map_err(|error| error.to_string())? {
        return Ok(footer.total_messages);
    }
    let mut count = 0u64;
    reader
        .scan_entries(|_, _| count += 1)
        .map_err(|error| error.to_string())?;
    Ok(count)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend lives under repository root")
        .to_path_buf()
}

fn idle_status(message: String) -> RecordingStatus {
    RecordingStatus {
        status: "idle".to_string(),
        output_path: None,
        dataflow_path: None,
        started_at_millis: None,
        frame_count: None,
        message,
    }
}

impl RecordingController {
    pub fn new() -> RecordingHandle {
        let output_dir = repo_root().join("out/recordings");
        let _ = std::fs::create_dir_all(&output_dir);
        Arc::new(Self {
            child: Mutex::new(None),
            active: Mutex::new(None),
            logs: Mutex::new(Vec::new()),
            output_dir,
        })
    }

    pub async fn status(&self) -> RecordingStatus {
        match self.active.lock().await.clone() {
            Some(active) => RecordingStatus {
                status: "recording".to_string(),
                output_path: Some(active.path.to_string_lossy().to_string()),
                dataflow_path: Some(active.dataflow_path),
                started_at_millis: Some(active.started_at_millis),
                frame_count: None,
                message: "Recording in progress.".to_string(),
            },
            None => idle_status("No active recording.".to_string()),
        }
    }

    pub async fn capture(self: &Arc<Self>, dataflow_path: String) -> RecordingStatus {
        if self.active.lock().await.is_some() {
            return self.status().await;
        }

        let version = crate::dora_env::dora_version().await;
        if !crate::dora_env::lifecycle_supported(&version) {
            return RecordingStatus {
                status: "unavailable".to_string(),
                output_path: None,
                dataflow_path: Some(dataflow_path),
                started_at_millis: None,
                frame_count: None,
                message: format!("Lifecycle operations require dora 1.x (detected {version})."),
            };
        }

        let started_at_millis = now_millis();
        let output_path = recording_output_path(&self.output_dir, started_at_millis);
        let mut command = build_record_command(
            &crate::dora_env::resolve_dora_bin(),
            Path::new(&dataflow_path),
            &output_path,
        );
        command
            .current_dir(repo_root())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return RecordingStatus {
                    status: "failed".to_string(),
                    output_path: Some(output_path.to_string_lossy().to_string()),
                    dataflow_path: Some(dataflow_path),
                    started_at_millis: Some(started_at_millis),
                    frame_count: None,
                    message: format!("dora record failed to spawn: {error}"),
                };
            }
        };

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_reader(stderr);
        }

        let exited = match tokio::time::timeout(RECORD_EXIT_TIMEOUT, child.wait()).await {
            Ok(Ok(status)) => Some(status.code()),
            Ok(Err(_)) => Some(None),
            Err(_) => None,
        };

        if let Some(exit_code) = exited {
            self.logs.lock().await.clear();
            if exit_code == Some(0) {
                return RecordingStatus {
                    status: "idle".to_string(),
                    output_path: Some(output_path.to_string_lossy().to_string()),
                    dataflow_path: Some(dataflow_path),
                    started_at_millis: Some(started_at_millis),
                    frame_count: count_frames(&output_path).ok(),
                    message: "Recording process finished on its own.".to_string(),
                };
            }
            let code = exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return RecordingStatus {
                status: "failed".to_string(),
                output_path: Some(output_path.to_string_lossy().to_string()),
                dataflow_path: Some(dataflow_path),
                started_at_millis: Some(started_at_millis),
                frame_count: None,
                message: format!("dora record failed (exit code {code})"),
            };
        }

        *self.child.lock().await = Some(child);
        *self.active.lock().await = Some(ActiveRecording {
            path: output_path,
            dataflow_path,
            started_at_millis,
        });
        self.status().await
    }

    pub async fn stop(&self) -> RecordingStatus {
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        let Some(active) = self.active.lock().await.take() else {
            return idle_status("No active recording.".to_string());
        };

        let frame_count = count_frames(&active.path).ok();
        RecordingStatus {
            status: "idle".to_string(),
            output_path: Some(active.path.to_string_lossy().to_string()),
            dataflow_path: Some(active.dataflow_path),
            started_at_millis: Some(active.started_at_millis),
            frame_count,
            message: match frame_count {
                Some(count) => format!("Recording stopped: {count} frames captured."),
                None => "Recording stopped; frame count unavailable.".to_string(),
            },
        }
    }

    pub async fn list(&self) -> Vec<RecordingEntry> {
        let mut entries = Vec::new();
        let Ok(read_dir) = std::fs::read_dir(&self.output_dir) else {
            return entries;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("drec") {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            entries.push(RecordingEntry {
                name: name.clone(),
                path: path.to_string_lossy().to_string(),
                size_bytes: metadata.len(),
                created_at_millis: name
                    .strip_suffix(".drec")
                    .and_then(|stamp| stamp.parse::<u64>().ok())
                    .unwrap_or(0),
                frame_count: count_frames(&path).ok(),
            });
        }
        entries.sort_by(|a, b| b.created_at_millis.cmp(&a.created_at_millis));
        entries
    }

    fn spawn_log_reader<R>(self: &Arc<Self>, stream: R)
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        let controller = Arc::clone(self);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut logs = controller.logs.lock().await;
                logs.push(line);
                if logs.len() > LOG_LIMIT {
                    logs.remove(0);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{build_record_command, count_frames, recording_output_path, RecordingController};
    use std::{
        ffi::OsString,
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::dora_env::TEST_ENV_LOCK as ENV_LOCK;

    fn fixture_drec() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dora10.drec")
    }

    #[test]
    fn build_record_command_uses_record_with_output() {
        let command = build_record_command(
            "/opt/dora",
            Path::new("/tmp/demo.yml"),
            Path::new("/tmp/out/recording.drec"),
        );
        let std_command = command.as_std();
        assert_eq!(std_command.get_program(), "/opt/dora");
        let args: Vec<_> = std_command.get_args().collect();
        assert_eq!(
            args,
            vec![
                std::ffi::OsStr::new("record"),
                std::ffi::OsStr::new("/tmp/demo.yml"),
                std::ffi::OsStr::new("-o"),
                std::ffi::OsStr::new("/tmp/out/recording.drec"),
            ]
        );
    }

    #[test]
    fn recording_output_path_lands_in_recordings_dir() {
        let dir = PathBuf::from("/repo/out/recordings");
        let first = recording_output_path(&dir, 1_700_000_000_000);
        let second = recording_output_path(&dir, 1_700_000_000_001);
        assert_eq!(first.parent().unwrap(), dir);
        assert_eq!(first.extension().unwrap(), "drec");
        assert_ne!(first, second);
    }

    /// Real dora 1.0 recording truncated by SIGTERM (no footer): the
    /// frame count must come from scanning, not fail.
    #[test]
    fn count_frames_scans_truncated_recording() {
        let count = count_frames(&fixture_drec()).expect("fixture counts");
        assert!(count > 100, "fixture has entries, got {count}");
    }

    struct DoraBinEnvGuard {
        previous: Option<OsString>,
    }

    impl DoraBinEnvGuard {
        fn set(value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os("DORA_STUDIO_DORA_BIN");
            std::env::set_var("DORA_STUDIO_DORA_BIN", value);
            Self { previous }
        }
    }

    impl Drop for DoraBinEnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("DORA_STUDIO_DORA_BIN", value),
                None => std::env::remove_var("DORA_STUDIO_DORA_BIN"),
            }
        }
    }

    struct FakeDora {
        path: PathBuf,
        invocation_path: PathBuf,
    }

    impl FakeDora {
        fn new(version: &str, record_mode: &str) -> Self {
            Self::write(version, record_mode)
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn invocations(&self) -> Vec<String> {
            fs::read_to_string(&self.invocation_path)
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect()
        }

        fn write(version: &str, record_mode: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let base = std::env::temp_dir().join(format!(
                "dora-recording-test-{}-{unique}",
                std::process::id()
            ));
            let path = base.with_extension("sh");
            let invocation_path = base.with_extension("args");
            let record_body = match record_mode {
                "fail" => "printf 'record failed\\n' >&2; exit 5".to_string(),
                // Real dora record keeps running; simulate a recording in
                // progress by copying the fixture and holding the session.
                // `exec` replaces the shell so the kill lands on sleep.
                _ => format!(
                    "cp '{fixture}' \"$4\"; exec sleep 300",
                    fixture = fixture_drec().display()
                ),
            };
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{args}'\nif [ \"$1\" = \"--version\" ]; then printf 'dora {version}\\n'; exit 0; fi\nif [ \"$1\" = \"record\" ]; then if [ -t 0 ]; then printf 'stdin-tty\\n' >> '{args}'; else printf 'stdin-null\\n' >> '{args}'; fi; {record_body}; fi\nexit 2\n",
                args = invocation_path.display(),
            );
            fs::write(&path, script).unwrap();
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
            Self {
                path,
                invocation_path,
            }
        }
    }

    impl Drop for FakeDora {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(&self.invocation_path);
        }
    }

    struct OutputCleanup(Vec<PathBuf>);

    impl Drop for OutputCleanup {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = fs::remove_file(path);
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn capture_spawns_dora_record_into_recordings_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0-rc.4", "hold");
        let _env = DoraBinEnvGuard::set(fake.path());
        let controller = RecordingController::new();

        let started = controller
            .capture("examples/live-demo/dataflow.yml".to_string())
            .await;
        assert_eq!(started.status, "recording");
        let output = started.output_path.expect("output path present");
        let _cleanup = OutputCleanup(vec![PathBuf::from(&output)]);
        assert!(output.contains("/out/recordings/"), "path: {output}");
        assert!(output.ends_with(".drec"));
        assert!(fake
            .invocations()
            .iter()
            .any(|line| line.starts_with("record examples/live-demo/dataflow.yml -o ")));
        // Regression: a terminal stdin flips dora record into
        // interactive mode; Studio must pass a non-terminal stdin.
        assert!(fake.invocations().iter().any(|line| line == "stdin-null"));
        assert!(!fake.invocations().iter().any(|line| line == "stdin-tty"));

        let stopped = controller.stop().await;
        assert_eq!(stopped.status, "idle");
        assert!(stopped.frame_count.unwrap_or(0) > 100);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsupported_version_rejects_capture_without_running_commands() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("0.5.0", "hold");
        let _env = DoraBinEnvGuard::set(fake.path());
        let controller = RecordingController::new();

        let started = controller.capture("examples/demo.yml".to_string()).await;
        assert_eq!(started.status, "unavailable");
        assert!(started.message.contains("dora 0.5.0"));
        assert!(!fake
            .invocations()
            .iter()
            .any(|line| line.starts_with("record ")));

        let stopped = controller.stop().await;
        assert_eq!(stopped.status, "idle");
        assert!(stopped.frame_count.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn capture_failure_is_not_reported_as_recording() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0", "fail");
        let _env = DoraBinEnvGuard::set(fake.path());
        let controller = RecordingController::new();

        let started = controller.capture("examples/demo.yml".to_string()).await;
        assert_eq!(started.status, "failed");
        assert!(started.message.contains("dora record failed"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_name_size_time_and_frame_count() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0", "hold");
        let _env = DoraBinEnvGuard::set(fake.path());
        let controller = RecordingController::new();

        let started = controller
            .capture("examples/live-demo/dataflow.yml".to_string())
            .await;
        let output = started.output_path.expect("output path present");
        let _cleanup = OutputCleanup(vec![PathBuf::from(&output)]);
        controller.stop().await;

        let entries = controller.list().await;
        let entry = entries
            .iter()
            .find(|entry| entry.path == output)
            .expect("recording listed");
        assert!(entry.name.ends_with(".drec"));
        assert!(entry.size_bytes > 0);
        assert!(entry.created_at_millis > 0);
        assert!(entry.frame_count.unwrap_or(0) > 100);
    }
}
