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
    logs: Mutex<Vec<String>>,
    state: Mutex<RuntimeState>,
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
            .map(|(index, line)| LogEntry {
                time: extract_log_time(line),
                node: "dora-run".to_string(),
                level: classify_log_level(line).to_string(),
                message: format!("#{index}: {line}"),
            })
            .collect()
    }

    pub async fn start(self: &Arc<Self>) -> RuntimeState {
        {
            let mut child = self.child.lock().await;
            if child.is_some() {
                return self.status().await;
            }

            let dataflow_path = repo_root().join("examples/robot-perception-test/dataflow.yml");
            let mut command = Command::new("dora");
            command
                .arg("run")
                .arg(dataflow_path)
                .current_dir(repo_root())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            match command.spawn() {
                Ok(mut process) => {
                    let pid = process.id();
                    self.logs.lock().await.clear();

                    if let Some(stdout) = process.stdout.take() {
                        self.spawn_log_reader(stdout);
                    }

                    if let Some(stderr) = process.stderr.take() {
                        self.spawn_log_reader(stderr);
                    }

                    *self.state.lock().await = RuntimeState {
                        status: "running".to_string(),
                        pid,
                        last_message: "Started examples/robot-perception-test/dataflow.yml through dora run.".to_string(),
                    };
                    *child = Some(process);
                }
                Err(error) => {
                    *self.state.lock().await = RuntimeState {
                        status: "failed".to_string(),
                        pid: None,
                        last_message: format!("Failed to start dora run: {error}"),
                    };
                }
            }
        }

        self.status().await
    }

    pub async fn stop(&self) -> RuntimeState {
        let mut child = self.child.lock().await;
        if let Some(mut process) = child.take() {
            match process.kill().await {
                Ok(()) => {
                    *self.state.lock().await = RuntimeState {
                        status: "stopped".to_string(),
                        pid: None,
                        last_message: "Stopped dataflow process from Studio.".to_string(),
                    };
                }
                Err(error) => {
                    *self.state.lock().await = RuntimeState {
                        status: "failed".to_string(),
                        pid: None,
                        last_message: format!("Failed to stop dataflow process: {error}"),
                    };
                }
            }
        } else {
            *self.state.lock().await = RuntimeState {
                status: "stopped".to_string(),
                pid: None,
                last_message: "No running dataflow process.".to_string(),
            };
        }

        self.status().await
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
                if logs.len() > 500 {
                    logs.remove(0);
                }
            }
        });
    }
}

fn extract_log_time(line: &str) -> String {
    line.split_whitespace()
        .find(|part| {
            let bytes = part.as_bytes();
            bytes.len() == 8
                && bytes[2] == b':'
                && bytes[5] == b':'
                && bytes.iter().enumerate().all(|(index, byte)| {
                    index == 2 || index == 5 || byte.is_ascii_digit()
                })
        })
        .unwrap_or("live")
        .to_string()
}

fn classify_log_level(line: &str) -> &'static str {
    if line.contains("ERROR") || line.contains(" error") || line.contains("failed") {
        "error"
    } else if line.contains("WARN") || line.contains("warning") || line.contains("pending") {
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
