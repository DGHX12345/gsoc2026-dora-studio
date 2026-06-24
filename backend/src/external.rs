use crate::models::{DvizStatus, MoveitStatus};

fn dviz_binary_path() -> Option<std::path::PathBuf> {
    let candidates = ["/home/dora/Desktop/dviz/target/release/dviz"];

    for path in candidates {
        let p = std::path::Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // Fallback: try to find dviz on PATH
    std::process::Command::new("which")
        .arg("dviz")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    Some(std::path::PathBuf::from(s))
                } else {
                    None
                }
            } else {
                None
            }
        })
}

pub fn query_dviz() -> DvizStatus {
    match dviz_binary_path() {
        Some(path) => {
            // Check if dviz process is running
            let running = is_process_running("dviz");
            DvizStatus {
                installed: true,
                running,
                binary_path: Some(path.display().to_string()),
                message: if running {
                    "dviz is running and publishing Zenoh data.".to_string()
                } else {
                    "dviz is installed but not running. Start with: cargo run -p dviz-shell --release"
                        .to_string()
                },
            }
        }
        None => DvizStatus {
            installed: false,
            running: false,
            binary_path: None,
            message: "dviz is not installed. Clone from the dviz repository and build with: cargo build --release"
                .to_string(),
        },
    }
}

pub fn query_moveit() -> MoveitStatus {
    let installed =
        is_python_package_installed("dora-moveit") || is_python_package_installed("dora_moveit");
    MoveitStatus {
        installed,
        running: false, // Will be updated when coordinator shows moveit nodes
        message: if installed {
            "dora-moveit2 is installed. Start moveit nodes in a Dora dataflow to begin.".to_string()
        } else {
            "dora-moveit2 is not installed. Install with: pip install -e dora_moveit/".to_string()
        },
    }
}

fn is_process_running(name: &str) -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn is_python_package_installed(name: &str) -> bool {
    std::process::Command::new("pip")
        .args(["show", name])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}
