use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

pub type SessionHandle = Arc<DoraSessionManager>;

/// Seconds to wait for `dora up` to exit on its own. A quick non-zero
/// exit means startup failed; a still-running child means the session
/// is held open in the foreground (normal for dora up).
const UP_EXIT_TIMEOUT: Duration = Duration::from_secs(3);
const DOWN_TIMEOUT: Duration = Duration::from_secs(10);
const START_POLL_ATTEMPTS: usize = 30;
const START_POLL_INTERVAL: Duration = Duration::from_millis(500);
const LOG_LIMIT: usize = 500;

pub struct DoraSessionManager {
    child: Mutex<Option<Child>>,
    logs: Mutex<Vec<String>>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub status: String,
    pub running: bool,
    pub coordinator_connected: bool,
    pub coordinator_status: String,
    pub pid: Option<u32>,
    pub version: String,
    pub lifecycle_supported: bool,
    pub dataflow_count: u32,
    pub message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyDaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

/// Projects a session status onto the legacy `/api/daemon/*` response
/// shape so old clients keep working unchanged.
pub fn legacy_daemon_status(status: &SessionStatus) -> LegacyDaemonStatus {
    LegacyDaemonStatus {
        running: status.running,
        pid: status.pid,
    }
}

enum CoordinatorProbe {
    Connected { dataflow_count: u32 },
    Unavailable,
    Unknown,
}

impl CoordinatorProbe {
    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

#[derive(serde::Deserialize)]
struct SessionListEntry {
    #[serde(default)]
    status: String,
}

/// dora 1.0 emits `dora list --format json` as JSON Lines: one JSON
/// object per line, so N dataflows are N separate lines (not an
/// array). Accept JSON Lines, a JSON array, and a single object.
fn parse_list_entries(stdout: &str) -> Option<Vec<SessionListEntry>> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    let lines: Result<Vec<SessionListEntry>, _> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str)
        .collect();
    if let Ok(entries) = lines {
        return Some(entries);
    }
    if let Ok(entries) = serde_json::from_str::<Vec<SessionListEntry>>(trimmed) {
        return Some(entries);
    }
    serde_json::from_str::<SessionListEntry>(trimmed)
        .ok()
        .map(|entry| vec![entry])
}

impl DoraSessionManager {
    pub fn new() -> SessionHandle {
        Arc::new(Self {
            child: Mutex::new(None),
            logs: Mutex::new(Vec::new()),
        })
    }

    pub async fn status(&self) -> SessionStatus {
        let version = crate::dora_env::dora_version().await;
        let probe = self.coordinator_probe().await;
        let pid = self
            .child
            .lock()
            .await
            .as_ref()
            .and_then(|child| child.id());

        let lifecycle_supported = crate::dora_env::lifecycle_supported(&version);
        let (status, running, coordinator_connected, coordinator_status, dataflow_count, message) =
            match probe {
                CoordinatorProbe::Connected { dataflow_count } => (
                    "running",
                    true,
                    true,
                    "connected",
                    dataflow_count,
                    "Coordinator is reachable.",
                ),
                CoordinatorProbe::Unavailable => (
                    "stopped",
                    false,
                    false,
                    "unavailable",
                    0,
                    "Coordinator is unavailable.",
                ),
                CoordinatorProbe::Unknown => (
                    "unknown",
                    false,
                    false,
                    "unknown",
                    0,
                    "Coordinator state is unknown.",
                ),
            };

        SessionStatus {
            status: status.to_string(),
            running,
            coordinator_connected,
            coordinator_status: coordinator_status.to_string(),
            pid,
            version,
            lifecycle_supported,
            dataflow_count,
            message: message.to_string(),
        }
    }

    pub async fn start(self: &Arc<Self>) -> SessionStatus {
        let version = crate::dora_env::dora_version().await;
        if !crate::dora_env::lifecycle_supported(&version) {
            return unavailable_status(version);
        }

        self.logs.lock().await.clear();

        let mut command = Command::new(crate::dora_env::resolve_dora_bin());
        command
            .arg("up")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return failed_status(version, format!("dora up failed to spawn: {error}"));
            }
        };

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_reader(stderr);
        }

        let up_failed = match tokio::time::timeout(UP_EXIT_TIMEOUT, child.wait()).await {
            Ok(Ok(status)) if !status.success() => Some(status.code()),
            Ok(Err(_)) => Some(None),
            Ok(Ok(_)) | Err(_) => None,
        };

        if let Some(code) = up_failed {
            self.child.lock().await.take();
            return failed_status(
                version,
                format!("dora up failed (exit code {})", exit_code_label(code)),
            );
        }

        *self.child.lock().await = Some(child);

        // Coordinator startup is asynchronous; wait for it to become
        // reachable before reporting the session as running.
        for _ in 0..START_POLL_ATTEMPTS {
            if self.coordinator_probe().await.is_connected() {
                break;
            }
            tokio::time::sleep(START_POLL_INTERVAL).await;
        }

        self.status().await
    }

    pub async fn stop(self: &Arc<Self>) -> SessionStatus {
        let version = crate::dora_env::dora_version().await;
        if !crate::dora_env::lifecycle_supported(&version) {
            return unavailable_status(version);
        }

        let mut command = Command::new(crate::dora_env::resolve_dora_bin());
        command
            .arg("down")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return failed_status(version, format!("dora down failed to spawn: {error}"));
            }
        };

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_reader(stderr);
        }

        let down_failed = match tokio::time::timeout(DOWN_TIMEOUT, child.wait()).await {
            Ok(Ok(status)) if !status.success() => Some(status.code()),
            Ok(Err(_)) | Err(_) => Some(None),
            Ok(Ok(_)) => None,
        };

        self.child.lock().await.take();

        let mut result = self.status().await;
        if let Some(code) = down_failed {
            result.status = "failed".to_string();
            result.message = format!("dora down failed (exit code {})", exit_code_label(code));
        }
        result
    }

    async fn coordinator_probe(&self) -> CoordinatorProbe {
        let output = Command::new(crate::dora_env::resolve_dora_bin())
            .args(["list", "--format", "json"])
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // dora 1.0 prints nothing at all when no dataflow is
                // running — that is a reachable coordinator, not an
                // unknown state.
                if stdout.trim().is_empty() {
                    return CoordinatorProbe::Connected { dataflow_count: 0 };
                }
                match parse_list_entries(&stdout) {
                    Some(entries) => CoordinatorProbe::Connected {
                        dataflow_count: entries
                            .iter()
                            .filter(|entry| entry.status.eq_ignore_ascii_case("running"))
                            .count() as u32,
                    },
                    None => CoordinatorProbe::Unknown,
                }
            }
            Ok(_) | Err(_) => CoordinatorProbe::Unavailable,
        }
    }

    fn spawn_log_reader<R>(self: &Arc<Self>, stream: R)
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut logs = manager.logs.lock().await;
                logs.push(line);
                if logs.len() > LOG_LIMIT {
                    logs.remove(0);
                }
            }
        });
    }
}

fn exit_code_label(code: Option<i32>) -> String {
    code.map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn unavailable_status(version: String) -> SessionStatus {
    SessionStatus {
        status: "unavailable".to_string(),
        running: false,
        coordinator_connected: false,
        coordinator_status: "unknown".to_string(),
        pid: None,
        version: version.clone(),
        lifecycle_supported: false,
        dataflow_count: 0,
        message: format!("Lifecycle operations require dora 1.x (detected {version})."),
    }
}

fn failed_status(version: String, message: String) -> SessionStatus {
    SessionStatus {
        status: "failed".to_string(),
        running: false,
        coordinator_connected: false,
        coordinator_status: "unknown".to_string(),
        pid: None,
        version,
        lifecycle_supported: true,
        dataflow_count: 0,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::{legacy_daemon_status, DoraSessionManager, SessionStatus};
    use std::{
        ffi::OsString,
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::dora_env::TEST_ENV_LOCK as ENV_LOCK;

    #[tokio::test(flavor = "current_thread")]
    async fn unsupported_version_rejects_lifecycle_without_running_commands() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("0.5.0").with_session_running(false);
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = DoraSessionManager::new();

        let start = manager.start().await;
        let stop = manager.stop().await;

        assert_eq!(start.status, "unavailable");
        assert_eq!(stop.status, "unavailable");
        assert!(!start.running);
        assert!(!stop.running);
        assert!(!start.lifecycle_supported);
        assert!(!stop.lifecycle_supported);
        assert!(!fake.invocations().iter().any(|line| line == "up"));
        assert!(!fake.invocations().iter().any(|line| line == "down"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn supported_version_runs_dora_up_and_down() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0-rc.4").with_session_running(false);
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = DoraSessionManager::new();

        let started = manager.start().await;
        let stopped = manager.stop().await;

        assert_eq!(started.status, "running");
        assert!(started.running);
        assert!(started.coordinator_connected);
        assert_eq!(started.dataflow_count, 1);
        assert_eq!(stopped.status, "stopped");
        assert!(!stopped.running);
        assert!(fake.invocations().iter().any(|line| line == "up"));
        assert!(fake.invocations().iter().any(|line| line == "down"));
        assert!(fake
            .invocations()
            .iter()
            .any(|line| line == "list --format json"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_distinguishes_connected_unavailable_and_unknown() {
        let _lock = ENV_LOCK.lock().unwrap();

        let connected_fake = FakeDora::new("1.0.0").with_session_running(true);
        let _connected_env = DoraBinEnvGuard::set(connected_fake.path());
        let connected = DoraSessionManager::new().status().await;
        assert_eq!(connected.status, "running");
        assert_eq!(connected.coordinator_status, "connected");
        assert!(connected.coordinator_connected);
        assert_eq!(connected.dataflow_count, 1);
        drop(_connected_env);

        let unavailable_fake = FakeDora::new("1.0.0").with_session_running(false);
        let _unavailable_env = DoraBinEnvGuard::set(unavailable_fake.path());
        let unavailable = DoraSessionManager::new().status().await;
        assert_eq!(unavailable.status, "stopped");
        assert_eq!(unavailable.coordinator_status, "unavailable");
        assert!(!unavailable.coordinator_connected);
        drop(_unavailable_env);

        let unknown_fake = FakeDora::new("1.0.0").with_invalid_list_output();
        let _unknown_env = DoraBinEnvGuard::set(unknown_fake.path());
        let unknown = DoraSessionManager::new().status().await;
        assert_eq!(unknown.status, "unknown");
        assert_eq!(unknown.coordinator_status, "unknown");
        assert!(!unknown.running);
    }

    /// dora 1.0 prints nothing for `dora list --format json` when no
    /// dataflow is running — an empty successful output means the
    /// coordinator is reachable with zero running dataflows.
    #[tokio::test(flavor = "current_thread")]
    async fn empty_list_output_means_connected_with_no_dataflows() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0").with_empty_list_output();
        let _env = DoraBinEnvGuard::set(fake.path());

        let status = DoraSessionManager::new().status().await;

        assert_eq!(status.status, "running");
        assert!(status.running);
        assert!(status.coordinator_connected);
        assert_eq!(status.coordinator_status, "connected");
        assert_eq!(status.dataflow_count, 0);
    }

    /// dora 1.0 emits JSON Lines: N dataflows = N separate JSON
    /// object lines, never a single array.
    #[test]
    fn parses_json_lines_list_output() {
        let lines = concat!(
            "{\"uuid\":\"a\",\"name\":\"one\",\"status\":\"Running\",\"nodes\":1}\n",
            "{\"uuid\":\"b\",\"name\":\"two\",\"status\":\"Finished\",\"nodes\":0}\n",
        );
        let entries = super::parse_list_entries(lines).expect("JSON Lines parse");
        assert_eq!(entries.len(), 2);

        let array = format!("[{{\"status\":\"Running\"}}, {{\"status\":\"Finished\"}}]");
        assert_eq!(super::parse_list_entries(&array).unwrap().len(), 2);

        let single = "{\"status\":\"Running\"}";
        assert_eq!(super::parse_list_entries(single).unwrap().len(), 1);

        assert!(super::parse_list_entries("not-json").is_none());
        assert_eq!(super::parse_list_entries("").unwrap().len(), 0);
    }

    /// dora 1.0 prints a single JSON object (not an array) for
    /// `dora list --format json` when exactly one dataflow is
    /// registered — that must count as connected, not unknown.
    #[tokio::test(flavor = "current_thread")]
    async fn single_object_list_output_counts_as_connected() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0")
            .with_session_running(true)
            .with_single_list_output();
        let _env = DoraBinEnvGuard::set(fake.path());

        let status = DoraSessionManager::new().status().await;

        assert_eq!(status.status, "running");
        assert!(status.coordinator_connected);
        assert_eq!(status.coordinator_status, "connected");
        assert_eq!(status.dataflow_count, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn externally_started_session_is_visible_without_studio_child() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0").with_session_running(true);
        let _env = DoraBinEnvGuard::set(fake.path());

        let status = DoraSessionManager::new().status().await;

        assert_eq!(status.status, "running");
        assert!(status.running);
        assert!(status.coordinator_connected);
        assert!(!fake.invocations().iter().any(|line| line == "up"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_and_stop_failures_are_not_reported_as_success() {
        let _lock = ENV_LOCK.lock().unwrap();

        let start_fake = FakeDora::new("1.0.0").with_lifecycle_failure("up", 7);
        let _start_env = DoraBinEnvGuard::set(start_fake.path());
        let start = DoraSessionManager::new().start().await;
        assert_eq!(start.status, "failed");
        assert!(!start.running);
        assert!(start.message.contains("dora up failed"));
        drop(_start_env);

        let stop_fake = FakeDora::new("1.0.0")
            .with_session_running(true)
            .with_lifecycle_failure("down", 9);
        let _stop_env = DoraBinEnvGuard::set(stop_fake.path());
        let stop = DoraSessionManager::new().stop().await;
        assert_eq!(stop.status, "failed");
        assert!(stop.running);
        assert!(stop.message.contains("dora down failed"));
    }

    #[test]
    fn legacy_daemon_status_preserves_response_shape() {
        let status = SessionStatus {
            status: "running".to_string(),
            running: true,
            coordinator_connected: true,
            coordinator_status: "connected".to_string(),
            pid: None,
            version: "dora 1.0.0".to_string(),
            lifecycle_supported: true,
            dataflow_count: 2,
            message: "Coordinator is reachable.".to_string(),
        };

        assert_eq!(
            serde_json::to_value(legacy_daemon_status(&status)).unwrap(),
            serde_json::json!({ "running": true, "pid": null })
        );
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
        state_path: PathBuf,
        invocation_path: PathBuf,
    }

    impl FakeDora {
        fn new(version: &str) -> Self {
            Self::write(version, "valid", None)
        }

        fn with_session_running(self, running: bool) -> Self {
            if running {
                fs::write(&self.state_path, b"running").unwrap();
            } else {
                let _ = fs::remove_file(&self.state_path);
            }
            self
        }

        fn with_invalid_list_output(self) -> Self {
            let replacement = Self::write("1.0.0", "invalid", None);
            drop(self);
            replacement
        }

        fn with_empty_list_output(self) -> Self {
            let replacement = Self::write("1.0.0", "empty", None);
            drop(self);
            replacement
        }

        fn with_single_list_output(self) -> Self {
            let replacement = Self::write("1.0.0", "single", None);
            if self.state_path.exists() {
                fs::write(&replacement.state_path, b"running").unwrap();
            }
            drop(self);
            replacement
        }

        fn with_lifecycle_failure(self, command: &str, code: i32) -> Self {
            let replacement = Self::write("1.0.0", "valid", Some((command, code)));
            if self.state_path.exists() {
                fs::write(&replacement.state_path, b"running").unwrap();
            }
            drop(self);
            replacement
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

        fn write(version: &str, list_mode: &str, lifecycle_failure: Option<(&str, i32)>) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let base = std::env::temp_dir()
                .join(format!("dora-session-test-{}-{unique}", std::process::id()));
            let path = base.with_extension("sh");
            let state_path = base.with_extension("state");
            let invocation_path = base.with_extension("args");
            let failure_case = lifecycle_failure
                .map(|(command, code)| format!("\nif [ \"$1\" = \"{command}\" ]; then printf '{command} failed\\n' >&2; exit {code}; fi\n"))
                .unwrap_or_default();
            let list_body = match list_mode {
                "invalid" => "printf 'not-json\\n'; exit 0".to_string(),
                "empty" => "exit 0".to_string(),
                "single" => format!(
                    "if [ -f '{state}' ]; then printf '{{\"uuid\":\"df-1\",\"name\":\"external-flow\",\"status\":\"Running\",\"nodes\":2}}\\n'; exit 0; fi\nprintf 'coordinator unavailable\\n' >&2\nexit 1",
                    state = state_path.display()
                ),
                _ => format!(
                    "if [ -f '{state}' ]; then printf '[{{\"uuid\":\"df-1\",\"name\":\"external-flow\",\"status\":\"Running\",\"nodes\":2}}]\\n'; exit 0; fi\nprintf 'coordinator unavailable\\n' >&2\nexit 1",
                    state = state_path.display()
                ),
            };
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{args}'\nif [ \"$1\" = \"--version\" ]; then printf 'dora {version}\\n'; exit 0; fi\n{failure_case}\nif [ \"$1\" = \"up\" ]; then printf running > '{state}'; exit 0; fi\nif [ \"$1\" = \"down\" ]; then rm -f '{state}'; exit 0; fi\nif [ \"$1\" = \"list\" ] && [ \"$2\" = \"--format\" ] && [ \"$3\" = \"json\" ]; then {list_body}; fi\nexit 2\n",
                args = invocation_path.display(),
                state = state_path.display(),
            );
            fs::write(&path, script).unwrap();
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
            Self {
                path,
                state_path,
                invocation_path,
            }
        }
    }

    impl Drop for FakeDora {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(&self.state_path);
            let _ = fs::remove_file(&self.invocation_path);
        }
    }
}
