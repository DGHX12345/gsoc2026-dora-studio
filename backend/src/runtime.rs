use std::{path::PathBuf, sync::Arc};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

use crate::models::{LogEntry, RuntimeState};

pub type RuntimeHandle = Arc<RuntimeManager>;

pub struct RuntimeManager {
    child: Mutex<Option<Child>>,
    logs: Mutex<Vec<RuntimeLogLine>>,
    state: Mutex<RuntimeState>,
}

struct RuntimeLogLine {
    source: &'static str,
    message: String,
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
        {
            let mut child = self.child.lock().await;
            if child.is_some() {
                return self.status().await;
            }

            let mut command = Command::new("dora");
            command
                .arg("run")
                .arg(&dataflow_path)
                .current_dir(repo_root())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            match command.spawn() {
                Ok(mut process) => {
                    let pid = process.id();
                    self.logs.lock().await.clear();

                    if let Some(stdout) = process.stdout.take() {
                        self.spawn_log_reader(stdout, "stdout");
                    }

                    if let Some(stderr) = process.stderr.take() {
                        self.spawn_log_reader(stderr, "stderr");
                    }

                    *self.state.lock().await = RuntimeState {
                        status: "running".to_string(),
                        pid,
                        last_message: format!("Started {relative_path} through dora run."),
                        dataflow_id: Some(dataflow_id),
                        dataflow_path: Some(relative_path),
                    };
                    *child = Some(process);
                }
                Err(error) => {
                    *self.state.lock().await = RuntimeState {
                        status: "failed".to_string(),
                        pid: None,
                        last_message: format!("Failed to start dora run: {error}"),
                        dataflow_id: Some(dataflow_id),
                        dataflow_path: Some(relative_path),
                    };
                }
            }
        }

        self.status().await
    }

    pub async fn start(self: &Arc<Self>) -> RuntimeState {
        self.start_dataflow(
            "robot-perception-test".to_string(),
            repo_root().join("examples/robot-perception-test/dataflow.yml"),
            "examples/robot-perception-test/dataflow.yml".to_string(),
        )
        .await
    }

    pub async fn stop(&self) -> RuntimeState {
        let previous = self.state.lock().await.clone();
        let mut child = self.child.lock().await;
        if let Some(mut process) = child.take() {
            match process.kill().await {
                Ok(()) => {
                    *self.state.lock().await = RuntimeState {
                        status: "stopped".to_string(),
                        pid: None,
                        last_message: "Stopped dataflow process from Studio.".to_string(),
                        dataflow_id: previous.dataflow_id.clone(),
                        dataflow_path: previous.dataflow_path.clone(),
                    };
                }
                Err(error) => {
                    *self.state.lock().await = RuntimeState {
                        status: "failed".to_string(),
                        pid: None,
                        last_message: format!("Failed to stop dataflow process: {error}"),
                        dataflow_id: previous.dataflow_id.clone(),
                        dataflow_path: previous.dataflow_path.clone(),
                    };
                }
            }
        } else {
            *self.state.lock().await = RuntimeState {
                status: "stopped".to_string(),
                pid: None,
                last_message: "No running dataflow process.".to_string(),
                dataflow_id: previous.dataflow_id,
                dataflow_path: previous.dataflow_path,
            };
        }

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
                if logs.len() > 500 {
                    logs.remove(0);
                }
            }
        });
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
