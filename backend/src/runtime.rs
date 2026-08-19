use std::{path::PathBuf, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

use crate::models::{LogEntry, RuntimeState};

pub type RuntimeHandle = Arc<RuntimeManager>;

/// How long `dora start` (non-interactive detach mode) may take before
/// the submission is considered failed.
const START_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const START_RETRY_ATTEMPTS: u32 = 3;
const START_RETRY_DELAY: Duration = Duration::from_millis(800);
/// Coordinator-side rejection when the previous dataflow with the same
/// name has not been released yet.
const NAME_CONFLICT_MARKER: &str = "there is already a running dataflow with name";
const LOG_LIMIT: usize = 500;

pub struct RuntimeManager {
    child: Mutex<Option<Child>>,
    logs: Mutex<Vec<RuntimeLogLine>>,
    state: Mutex<RuntimeState>,
    dataflow_name: Mutex<Option<String>>,
}

struct RuntimeLogLine {
    source: &'static str,
    message: String,
}

/// The coordinator-facing name of a Studio-started dataflow. Attempt 0
/// uses the stable base name; retries after a name-conflict use a
/// suffixed name so a not-yet-released name never blocks the restart.
fn dataflow_name(dataflow_id: &str, attempt: u32) -> String {
    if attempt == 0 {
        format!("studio-{dataflow_id}")
    } else {
        format!("studio-{dataflow_id}-{attempt}")
    }
}

/// A failed `dora start` is retried only when the coordinator still
/// holds the previous dataflow under the same name and attempts remain.
fn should_retry_start(stderr: &str, attempt: u32) -> bool {
    attempt + 1 < START_RETRY_ATTEMPTS && stderr.contains(NAME_CONFLICT_MARKER)
}

fn build_start_command(binary: &str, dataflow_path: &std::path::Path, name: &str) -> Command {
    let mut command = Command::new(binary);
    command
        .arg("start")
        .arg(dataflow_path)
        .arg("--name")
        .arg(name)
        // dora start attaches to the dataflow when stdin is a terminal;
        // a null stdin forces the non-interactive detach path.
        .stdin(std::process::Stdio::null());
    command
}

fn build_stop_command(binary: &str, name: &str) -> Command {
    let mut command = Command::new(binary);
    command.arg("stop").arg("--name").arg(name);
    command
}

fn unavailable_state(dataflow_id: String, relative_path: String, version: String) -> RuntimeState {
    RuntimeState {
        status: "unavailable".to_string(),
        pid: None,
        last_message: format!("Lifecycle operations require dora 1.x (detected {version})."),
        dataflow_id: Some(dataflow_id),
        dataflow_path: Some(relative_path),
    }
}

async fn read_child_stderr(child: &mut Child) -> String {
    let Some(mut stderr) = child.stderr.take() else {
        return String::new();
    };
    let mut buffer = Vec::new();
    let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut buffer).await;
    String::from_utf8_lossy(&buffer).to_string()
}

const STDERR_SUFFIX_LIMIT: usize = 500;

fn stderr_suffix(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let truncated: String = trimmed.chars().take(STDERR_SUFFIX_LIMIT).collect();
    let ellipsis = if trimmed.chars().count() > STDERR_SUFFIX_LIMIT {
        "…"
    } else {
        ""
    };
    format!(": {truncated}{ellipsis}")
}

impl RuntimeManager {
    pub fn new() -> RuntimeHandle {
        Arc::new(Self {
            child: Mutex::new(None),
            logs: Mutex::new(Vec::new()),
            state: Mutex::new(RuntimeState {
                status: "stopped".to_string(),
                pid: None,
                last_message: "Dataflow has not been started from Studio.".to_string(),
                dataflow_id: None,
                dataflow_path: None,
            }),
            dataflow_name: Mutex::new(None),
        })
    }

    pub async fn status(&self) -> RuntimeState {
        self.state.lock().await.clone()
    }

    pub async fn logs(&self) -> Vec<LogEntry> {
        self.logs
            .lock()
            .await
            .iter()
            .enumerate()
            .map(|(index, line)| parse_log_entry(index, line))
            .collect()
    }

    pub async fn start_dataflow(
        self: &Arc<Self>,
        dataflow_id: String,
        dataflow_path: PathBuf,
        relative_path: String,
    ) -> RuntimeState {
        let version = crate::dora_env::dora_version().await;
        if !crate::dora_env::lifecycle_supported(&version) {
            *self.state.lock().await = unavailable_state(dataflow_id, relative_path, version);
            return self.status().await;
        }

        {
            let child = self.child.lock().await;
            if child.is_some() {
                return self.status().await;
            }
        }

        self.logs.lock().await.clear();

        let mut attempt: u32 = 0;
        let failure: Option<String> = loop {
            let name = dataflow_name(&dataflow_id, attempt);
            let mut command =
                build_start_command(&crate::dora_env::resolve_dora_bin(), &dataflow_path, &name);
            command
                .current_dir(repo_root())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(error) => break Some(format!("Failed to start dora start: {error}")),
            };

            if let Some(stdout) = child.stdout.take() {
                self.spawn_log_reader(stdout, "stdout");
            }

            let (exited, exit_code, timed_out) = {
                let mut child_guard = self.child.lock().await;
                *child_guard = Some(child);
                let active = child_guard.as_mut().expect("child stored above");
                match tokio::time::timeout(START_TIMEOUT, active.wait()).await {
                    Ok(Ok(status)) => (true, status.code(), false),
                    Ok(Err(_)) => (true, None, false),
                    Err(_) => {
                        let _ = active.kill().await;
                        let _ = active.wait().await;
                        (true, None, true)
                    }
                }
            };

            let stderr_text = {
                let mut child_guard = self.child.lock().await;
                match child_guard.as_mut() {
                    Some(active) => read_child_stderr(active).await,
                    None => String::new(),
                }
            };
            self.append_stderr_logs(&stderr_text).await;
            self.child.lock().await.take();

            if exited && exit_code == Some(0) {
                *self.dataflow_name.lock().await = Some(name.clone());
                *self.state.lock().await = RuntimeState {
                    status: "running".to_string(),
                    pid: None,
                    last_message: format!(
                        "Started {relative_path} through dora start as '{name}'."
                    ),
                    dataflow_id: Some(dataflow_id.clone()),
                    dataflow_path: Some(relative_path.clone()),
                };
                break None;
            }

            if should_retry_start(&stderr_text, attempt) {
                attempt += 1;
                tokio::time::sleep(START_RETRY_DELAY).await;
                continue;
            }

            let reason = if timed_out {
                "timed out".to_string()
            } else {
                format!(
                    "exit code {}",
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                )
            };
            break Some(format!(
                "dora start failed ({reason}){}",
                stderr_suffix(&stderr_text)
            ));
        };

        if let Some(message) = failure {
            *self.state.lock().await = RuntimeState {
                status: "failed".to_string(),
                pid: None,
                last_message: message,
                dataflow_id: Some(dataflow_id),
                dataflow_path: Some(relative_path),
            };
        }

        self.status().await
    }

    /// Run a YAML string by writing it to a temp file and starting it
    /// through the coordinator.
    pub async fn run_yaml(self: &Arc<Self>, yaml: &str, name: &str) -> RuntimeState {
        let dir = repo_root().join(".dora-studio-tmp");
        let _ = std::fs::create_dir_all(&dir);

        let path = dir.join(format!("{name}.yml"));
        if let Err(e) = std::fs::write(&path, yaml) {
            return RuntimeState {
                status: "failed".to_string(),
                pid: None,
                last_message: format!("Failed to write YAML to temp file: {e}"),
                dataflow_id: Some(name.to_string()),
                dataflow_path: Some(format!(".dora-studio-tmp/{name}.yml")),
            };
        }

        self.start_dataflow(
            name.to_string(),
            path,
            format!(".dora-studio-tmp/{name}.yml"),
        )
        .await
    }

    pub async fn start(self: &Arc<Self>) -> RuntimeState {
        self.start_dataflow(
            "robot-perception-test".to_string(),
            repo_root().join("examples/robot-perception-test/dataflow.yml"),
            "examples/robot-perception-test/dataflow.yml".to_string(),
        )
        .await
    }

    pub async fn stop(self: &Arc<Self>) -> RuntimeState {
        let previous = self.state.lock().await.clone();

        // Interrupt a still-running `dora start` submission.
        {
            let mut child_guard = self.child.lock().await;
            if let Some(mut process) = child_guard.take() {
                let _ = process.kill().await;
            }
        }

        let Some(name) = self.dataflow_name.lock().await.clone() else {
            *self.state.lock().await = RuntimeState {
                status: "stopped".to_string(),
                pid: None,
                last_message: "No running dataflow process.".to_string(),
                dataflow_id: previous.dataflow_id,
                dataflow_path: previous.dataflow_path,
            };
            return self.status().await;
        };

        let version = crate::dora_env::dora_version().await;
        if !crate::dora_env::lifecycle_supported(&version) {
            *self.state.lock().await = unavailable_state(
                previous.dataflow_id.unwrap_or_default(),
                previous.dataflow_path.unwrap_or_default(),
                version,
            );
            return self.status().await;
        }

        let mut command = build_stop_command(&crate::dora_env::resolve_dora_bin(), &name);
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                *self.state.lock().await = RuntimeState {
                    status: "failed".to_string(),
                    pid: None,
                    last_message: format!("Failed to start dora stop: {error}"),
                    dataflow_id: previous.dataflow_id,
                    dataflow_path: previous.dataflow_path,
                };
                return self.status().await;
            }
        };

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout, "stdout");
        }

        let (exited, exit_code, timed_out) =
            match tokio::time::timeout(STOP_TIMEOUT, child.wait()).await {
                Ok(Ok(status)) => (true, status.code(), false),
                Ok(Err(_)) => (true, None, false),
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    (true, None, true)
                }
            };
        let stderr_text = read_child_stderr(&mut child).await;
        self.append_stderr_logs(&stderr_text).await;

        if exited && exit_code == Some(0) {
            *self.dataflow_name.lock().await = None;
            *self.state.lock().await = RuntimeState {
                status: "stopped".to_string(),
                pid: None,
                last_message: format!("Stopped dataflow '{name}' through dora stop."),
                dataflow_id: previous.dataflow_id,
                dataflow_path: previous.dataflow_path,
            };
            return self.status().await;
        }

        // A failed stop is only honest to report when the dataflow is
        // still registered with the coordinator; if it vanished, the
        // goal (nothing running) was reached anyway.
        let still_running = crate::coordinator::query_coordinator()
            .await
            .dataflows
            .iter()
            .any(|dataflow| {
                dataflow.name == name && dataflow.status.eq_ignore_ascii_case("running")
            });
        if still_running {
            let reason = if timed_out {
                "timed out".to_string()
            } else {
                format!(
                    "exit code {}",
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                )
            };
            *self.state.lock().await = RuntimeState {
                status: "failed".to_string(),
                pid: None,
                last_message: format!("dora stop failed ({reason}){}", stderr_suffix(&stderr_text)),
                dataflow_id: previous.dataflow_id,
                dataflow_path: previous.dataflow_path,
            };
            return self.status().await;
        }

        *self.dataflow_name.lock().await = None;
        *self.state.lock().await = RuntimeState {
            status: "stopped".to_string(),
            pid: None,
            last_message: format!("Dataflow '{name}' is no longer running."),
            dataflow_id: previous.dataflow_id,
            dataflow_path: previous.dataflow_path,
        };
        self.status().await
    }

    fn spawn_log_reader<R>(self: &Arc<Self>, stream: R, source: &'static str)
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut logs = manager.logs.lock().await;
                logs.push(RuntimeLogLine {
                    source,
                    message: line,
                });
                if logs.len() > LOG_LIMIT {
                    logs.remove(0);
                }
            }
        });
    }

    async fn append_stderr_logs(&self, stderr_text: &str) {
        if stderr_text.trim().is_empty() {
            return;
        }
        let mut logs = self.logs.lock().await;
        for line in stderr_text.lines() {
            logs.push(RuntimeLogLine {
                source: "stderr",
                message: line.to_string(),
            });
            if logs.len() > LOG_LIMIT {
                logs.remove(0);
            }
        }
    }
}

fn parse_log_entry(index: usize, log: &RuntimeLogLine) -> LogEntry {
    let line = log.message.as_str();
    let timestamp = extract_timestamp(line).unwrap_or_else(|| format!("live+{index:03}"));
    let time = timestamp
        .split('T')
        .nth(1)
        .and_then(|part| part.get(0..8))
        .or_else(|| line.split_whitespace().find(|part| is_clock_time(part)))
        .unwrap_or("live")
        .to_string();
    let source_location = extract_source_location(line);

    LogEntry {
        time,
        timestamp,
        node: extract_node(line),
        level: classify_log_level(line).to_string(),
        message: clean_log_message(line),
        raw_message: line.to_string(),
        source: extract_source(line, log.source),
        source_file: source_location.as_ref().map(|location| location.0.clone()),
        source_line: source_location.map(|location| location.1),
    }
}

fn extract_timestamp(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|part| part.contains('T') && part.len() >= 19)
        .map(|part| part.trim_matches(|ch| matches!(ch, '[' | ']')).to_string())
}

fn extract_node(line: &str) -> String {
    const KNOWN_NODES: [&str; 5] = ["camera", "detector", "planner", "logger", "robot_bridge"];

    KNOWN_NODES
        .iter()
        .find(|node| line.contains(**node))
        .unwrap_or(&"dora-run")
        .to_string()
}

fn extract_source(line: &str, captured_source: &str) -> String {
    if line.contains("stdout") {
        "stdout".to_string()
    } else if line.contains("stderr") {
        "stderr".to_string()
    } else if captured_source == "stdout" || captured_source == "stderr" {
        captured_source.to_string()
    } else {
        "runtime".to_string()
    }
}

fn extract_source_location(line: &str) -> Option<(String, String)> {
    line.split_whitespace()
        .map(strip_log_token)
        .find_map(|part| {
            let (file, line) = part.rsplit_once(':')?;
            if !file.contains('.') || !line.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }

            Some((file.to_string(), line.to_string()))
        })
}

fn clean_log_message(line: &str) -> String {
    line.split_whitespace()
        .filter(|part| {
            let stripped = strip_log_token(part);
            !is_timestamp_like(stripped)
                && stripped != "stdout"
                && stripped != "stderr"
                && extract_source_location(stripped).is_none()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_log_token(part: &str) -> &str {
    part.trim_matches(|ch: char| matches!(ch, '[' | ']' | '(' | ')' | ',' | ';'))
}

fn is_timestamp_like(part: &str) -> bool {
    part.contains('T') && part.len() >= 19
}

fn is_clock_time(part: &str) -> bool {
    let bytes = part.as_bytes();
    bytes.len() == 8
        && bytes[2] == b':'
        && bytes[5] == b':'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || index == 5 || byte.is_ascii_digit())
}

fn classify_log_level(line: &str) -> &'static str {
    let normalized = line.to_lowercase();
    if normalized.contains("error") || normalized.contains("failed") {
        "error"
    } else if normalized.contains("warn")
        || normalized.contains("warning")
        || normalized.contains("pending")
    {
        "warn"
    } else {
        "info"
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend lives under repository root")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{
        build_start_command, build_stop_command, dataflow_name, should_retry_start, stderr_suffix,
    };
    use std::{
        ffi::OsString,
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::dora_env::TEST_ENV_LOCK as ENV_LOCK;

    #[test]
    fn build_start_command_uses_dora_start_with_name() {
        let command = build_start_command("/opt/dora", Path::new("/tmp/demo.yml"), "studio-demo");
        let std_command = command.as_std();
        assert_eq!(std_command.get_program(), "/opt/dora");
        let args: Vec<_> = std_command.get_args().collect();
        assert_eq!(
            args,
            vec![
                std::ffi::OsStr::new("start"),
                std::ffi::OsStr::new("/tmp/demo.yml"),
                std::ffi::OsStr::new("--name"),
                std::ffi::OsStr::new("studio-demo"),
            ]
        );
    }

    #[test]
    fn build_stop_command_uses_dora_stop_with_name() {
        let command = build_stop_command("/opt/dora", "studio-demo");
        let std_command = command.as_std();
        assert_eq!(std_command.get_program(), "/opt/dora");
        let args: Vec<_> = std_command.get_args().collect();
        assert_eq!(
            args,
            vec![
                std::ffi::OsStr::new("stop"),
                std::ffi::OsStr::new("--name"),
                std::ffi::OsStr::new("studio-demo"),
            ]
        );
    }

    #[test]
    fn dataflow_name_uses_stable_base_name_on_first_attempt() {
        assert_eq!(
            dataflow_name("robot-perception-test", 0),
            "studio-robot-perception-test"
        );
    }

    #[test]
    fn dataflow_name_is_unique_per_attempt() {
        let names: Vec<_> = (0..3)
            .map(|attempt| dataflow_name("demo", attempt))
            .collect();
        assert_eq!(names, vec!["studio-demo", "studio-demo-1", "studio-demo-2"]);
    }

    #[test]
    fn dataflow_name_is_unique_per_dataflow_id() {
        assert_ne!(dataflow_name("alpha", 0), dataflow_name("beta", 0));
    }

    #[test]
    fn stderr_suffix_truncates_long_output() {
        let long = "x".repeat(2000);
        let suffix = stderr_suffix(&long);
        assert!(
            suffix.len() <= 505,
            "suffix stays bounded, got {}",
            suffix.len()
        );
        assert!(suffix.ends_with('…'));
        assert_eq!(stderr_suffix(""), "");
        assert_eq!(stderr_suffix("  short  "), ": short");
    }

    #[test]
    fn should_retry_start_only_on_name_conflict_with_attempts_left() {
        let conflict = "error: there is already a running dataflow with name `studio-demo`\n";
        assert!(should_retry_start(conflict, 0));
        assert!(should_retry_start(conflict, 1));
        assert!(!should_retry_start(conflict, 2));
        assert!(!should_retry_start("failed to read dataflow", 0));
        assert!(!should_retry_start("", 0));
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
        conflict_flag_path: PathBuf,
    }

    impl FakeDora {
        fn new(version: &str, start_mode: &str, stop_mode: &str) -> Self {
            Self::write(version, start_mode, stop_mode)
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

        fn write(version: &str, start_mode: &str, stop_mode: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let base = std::env::temp_dir()
                .join(format!("dora-runtime-test-{}-{unique}", std::process::id()));
            let path = base.with_extension("sh");
            let state_path = base.with_extension("state");
            let invocation_path = base.with_extension("args");
            let conflict_flag_path = base.with_extension("conflict");
            let start_body = match start_mode {
                "conflict_once" => format!(
                    "if [ ! -f '{flag}' ]; then touch '{flag}'; printf 'there is already a running dataflow with name `%s`\\n' \"$4\" >&2; exit 1; fi\nprintf '%s' \"$4\" > '{state}'; exit 0",
                    flag = conflict_flag_path.display(),
                    state = state_path.display(),
                ),
                "fail" => "printf 'start failed\\n' >&2; exit 3".to_string(),
                _ => format!(
                    "printf '%s' \"$4\" > '{state}'; exit 0",
                    state = state_path.display()
                ),
            };
            let stop_body = match stop_mode {
                "fail" => "printf 'stop failed\\n' >&2; exit 4".to_string(),
                "fail_and_vanish" => format!(
                    "rm -f '{state}'; printf 'stop failed\\n' >&2; exit 4",
                    state = state_path.display()
                ),
                _ => format!("rm -f '{state}'; exit 0", state = state_path.display()),
            };
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{args}'\nif [ \"$1\" = \"--version\" ]; then printf 'dora {version}\\n'; exit 0; fi\nif [ \"$1\" = \"start\" ]; then if [ -t 0 ]; then printf 'stdin-tty\\n' >> '{args}'; else printf 'stdin-null\\n' >> '{args}'; fi; {start_body}; fi\nif [ \"$1\" = \"stop\" ]; then {stop_body}; fi\nif [ \"$1\" = \"list\" ] && [ \"$2\" = \"--format\" ] && [ \"$3\" = \"json\" ]; then if [ -f '{state}' ]; then printf '[{{\"uuid\":\"df-1\",\"name\":\"%s\",\"status\":\"Running\",\"nodes\":2}}]\\n' \"$(cat '{state}')\"; exit 0; fi; printf '[]\\n'; exit 0; fi\nexit 2\n",
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
                conflict_flag_path,
            }
        }
    }

    impl Drop for FakeDora {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(&self.state_path);
            let _ = fs::remove_file(&self.invocation_path);
            let _ = fs::remove_file(&self.conflict_flag_path);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn supported_version_starts_and_stops_through_coordinator() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0-rc.4", "success", "success");
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = super::RuntimeManager::new();
        let path = PathBuf::from("/tmp/demo.yml");

        let started = manager
            .start_dataflow(
                "robot-perception-test".to_string(),
                path.clone(),
                "examples/demo.yml".to_string(),
            )
            .await;
        assert_eq!(started.status, "running");
        assert!(fake
            .invocations()
            .iter()
            .any(|line| line == "start /tmp/demo.yml --name studio-robot-perception-test"));
        // Regression: a terminal stdin makes dora start attach instead
        // of detaching; Studio must always pass a non-terminal stdin.
        assert!(fake.invocations().iter().any(|line| line == "stdin-null"));
        assert!(!fake.invocations().iter().any(|line| line == "stdin-tty"));

        let stopped = manager.stop().await;
        assert_eq!(stopped.status, "stopped");
        assert!(fake
            .invocations()
            .iter()
            .any(|line| line == "stop --name studio-robot-perception-test"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsupported_version_rejects_start_and_idle_stop() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("0.5.0", "success", "success");
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = super::RuntimeManager::new();

        let started = manager
            .start_dataflow(
                "demo".to_string(),
                PathBuf::from("/tmp/demo.yml"),
                "examples/demo.yml".to_string(),
            )
            .await;
        assert_eq!(started.status, "unavailable");
        assert!(started.last_message.contains("dora 0.5.0"));
        assert!(!fake
            .invocations()
            .iter()
            .any(|line| line.starts_with("start ")));

        let stopped = manager.stop().await;
        assert_eq!(stopped.status, "stopped");
        assert!(!fake
            .invocations()
            .iter()
            .any(|line| line.starts_with("stop ")));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsupported_version_rejects_stop_of_started_dataflow() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake10 = FakeDora::new("1.0.0", "success", "success");
        let _env10 = DoraBinEnvGuard::set(fake10.path());
        let manager = super::RuntimeManager::new();
        manager
            .start_dataflow(
                "demo".to_string(),
                PathBuf::from("/tmp/a.yml"),
                "examples/a.yml".to_string(),
            )
            .await;
        drop(_env10);

        let fake05 = FakeDora::new("0.5.0", "success", "success");
        let _env05 = DoraBinEnvGuard::set(fake05.path());
        let stopped = manager.stop().await;
        assert_eq!(stopped.status, "unavailable");
        assert!(!fake05
            .invocations()
            .iter()
            .any(|line| line.starts_with("stop ")));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn name_conflict_retries_with_new_name() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0", "conflict_once", "success");
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = super::RuntimeManager::new();

        let started = manager
            .start_dataflow(
                "demo".to_string(),
                PathBuf::from("/tmp/b.yml"),
                "examples/b.yml".to_string(),
            )
            .await;
        assert_eq!(started.status, "running");
        let invocations = fake.invocations();
        assert!(invocations
            .iter()
            .any(|line| line == "start /tmp/b.yml --name studio-demo"));
        assert!(invocations
            .iter()
            .any(|line| line == "start /tmp/b.yml --name studio-demo-1"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_failure_without_name_conflict_is_not_retried() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0", "fail", "success");
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = super::RuntimeManager::new();

        let started = manager
            .start_dataflow(
                "demo".to_string(),
                PathBuf::from("/tmp/c.yml"),
                "examples/c.yml".to_string(),
            )
            .await;
        assert_eq!(started.status, "failed");
        let start_calls = fake
            .invocations()
            .iter()
            .filter(|line| line.starts_with("start "))
            .count();
        assert_eq!(start_calls, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stop_failure_with_still_running_dataflow_reports_failed() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0", "success", "fail");
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = super::RuntimeManager::new();
        manager
            .start_dataflow(
                "demo".to_string(),
                PathBuf::from("/tmp/d.yml"),
                "examples/d.yml".to_string(),
            )
            .await;

        let stopped = manager.stop().await;
        assert_eq!(stopped.status, "failed");
        assert!(stopped.last_message.contains("dora stop failed"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stop_failure_with_vanished_dataflow_reports_stopped() {
        let _lock = ENV_LOCK.lock().unwrap();
        let fake = FakeDora::new("1.0.0", "success", "fail_and_vanish");
        let _env = DoraBinEnvGuard::set(fake.path());
        let manager = super::RuntimeManager::new();
        manager
            .start_dataflow(
                "demo".to_string(),
                PathBuf::from("/tmp/e.yml"),
                "examples/e.yml".to_string(),
            )
            .await;

        let stopped = manager.stop().await;
        assert_eq!(stopped.status, "stopped");
    }
}
