use std::sync::Arc;

use tokio::{
    process::{Child, Command},
    sync::Mutex,
};

pub type DaemonHandle = Arc<DaemonManager>;

pub struct DaemonManager {
    child: Mutex<Option<Child>>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

impl DaemonManager {
    pub fn new() -> DaemonHandle {
        Arc::new(Self {
            child: Mutex::new(None),
        })
    }

    pub async fn status(&self) -> DaemonStatus {
        let child = self.child.lock().await;
        DaemonStatus {
            running: child.is_some(),
            pid: child.as_ref().and_then(|c| c.id()),
        }
    }

    pub async fn start(self: &Arc<Self>) -> DaemonStatus {
        let mut child = self.child.lock().await;
        if child.is_some() {
            return DaemonStatus {
                running: true,
                pid: None,
            };
        }

        match Command::new("dora")
            .arg("daemon")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(process) => {
                let pid = process.id();
                *child = Some(process);
                DaemonStatus { running: true, pid }
            }
            Err(_) => DaemonStatus {
                running: false,
                pid: None,
            },
        }
    }

    pub async fn stop(&self) -> DaemonStatus {
        let mut child = self.child.lock().await;
        if let Some(mut process) = child.take() {
            let _ = process.kill().await;
        }
        DaemonStatus {
            running: false,
            pid: None,
        }
    }
}
