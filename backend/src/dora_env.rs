use std::{collections::HashMap, sync::Arc, time::Duration};

pub(crate) fn resolve_dora_bin() -> String {
    std::env::var("DORA_STUDIO_DORA_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "dora".to_string())
}

pub(crate) fn normalize_dora_version(raw: &str) -> String {
    let line = raw.lines().next().unwrap_or("").trim();
    let candidate = line
        .strip_prefix("dora-cli ")
        .or_else(|| line.strip_prefix("dora "))
        .unwrap_or(line);
    let mut parts = candidate.split_whitespace();
    let version = parts.next().unwrap_or("");
    let valid = is_complete_semver(version) && parts.next().is_none();

    if valid {
        format!("dora {version}")
    } else {
        "unknown".to_string()
    }
}

fn is_complete_semver(version: &str) -> bool {
    let core = version
        .split_once(['-', '+'])
        .map(|(core, _)| core)
        .unwrap_or(version);
    let components: Vec<_> = core.split('.').collect();
    if components.len() != 3
        || components.iter().any(|component| {
            component.is_empty()
                || !component.chars().all(|c| c.is_ascii_digit())
                || (component.len() > 1 && component.starts_with('0'))
        })
    {
        return false;
    }

    let Some(suffix) = version.strip_prefix(core) else {
        return false;
    };
    if suffix.is_empty() {
        return true;
    }

    let is_prerelease = suffix.starts_with('-');
    let suffix = &suffix[1..];
    suffix
        .split_once('+')
        .map(|(prerelease, build)| {
            is_valid_semver_identifiers(prerelease, true)
                && is_valid_semver_identifiers(build, false)
        })
        .unwrap_or_else(|| is_valid_semver_identifiers(suffix, is_prerelease))
}

fn is_valid_semver_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
                && (!reject_numeric_leading_zero
                    || identifier.len() == 1
                    || !identifier.chars().all(|c| c.is_ascii_digit())
                    || !identifier.starts_with('0'))
        })
}

pub(crate) fn lifecycle_supported(version: &str) -> bool {
    let normalized = normalize_dora_version(version);
    normalized != "unknown"
        && normalized
            .strip_prefix("dora ")
            .and_then(|version| version.split('.').next())
            == Some("1")
}

static DORA_VERSION_CACHE: tokio::sync::OnceCell<tokio::sync::Mutex<HashMap<String, Arc<String>>>> =
    tokio::sync::OnceCell::const_new();

async fn dora_version_cache() -> &'static tokio::sync::Mutex<HashMap<String, Arc<String>>> {
    DORA_VERSION_CACHE
        .get_or_init(|| async { tokio::sync::Mutex::new(HashMap::new()) })
        .await
}

pub(crate) async fn dora_version() -> String {
    let binary = resolve_dora_bin();
    let cache = dora_version_cache().await;

    if let Some(version) = cache.lock().await.get(&binary).cloned() {
        return (*version).clone();
    }

    let version = match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new(&binary)
            .arg("--version")
            .output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => {
            let stdout_version = normalize_dora_version(&String::from_utf8_lossy(&output.stdout));
            if stdout_version != "unknown" {
                stdout_version
            } else {
                // dora 0.5 prints version info to stderr; only fall back
                // when stdout holds nothing recognizable.
                normalize_dora_version(&String::from_utf8_lossy(&output.stderr))
            }
        }
        _ => "unknown".to_string(),
    };

    cache.lock().await.insert(binary, Arc::new(version.clone()));
    version
}

/// Shared lock for every test that mutates the process-global
/// `DORA_STUDIO_DORA_BIN` variable. Parallel test modules each need
/// their own lock; a single shared lock keeps them from stomping on
/// each other's environment mid-test.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::{dora_version, lifecycle_supported, normalize_dora_version, resolve_dora_bin};
    use std::{
        ffi::OsString,
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::TEST_ENV_LOCK as ENV_LOCK;

    struct DoraBinEnvGuard {
        previous: Option<OsString>,
    }

    impl DoraBinEnvGuard {
        fn set(value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os("DORA_STUDIO_DORA_BIN");
            std::env::set_var("DORA_STUDIO_DORA_BIN", value);
            Self { previous }
        }

        fn remove() -> Self {
            let previous = std::env::var_os("DORA_STUDIO_DORA_BIN");
            std::env::remove_var("DORA_STUDIO_DORA_BIN");
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

    #[test]
    fn explicit_binary_overrides_path_fallback() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = DoraBinEnvGuard::set("/opt/dora-1.0/bin/dora");
        assert_eq!(resolve_dora_bin(), "/opt/dora-1.0/bin/dora");
    }

    #[test]
    fn empty_binary_uses_path_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = DoraBinEnvGuard::set("");
        assert_eq!(resolve_dora_bin(), "dora");
    }

    #[test]
    fn missing_binary_uses_path_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = DoraBinEnvGuard::remove();
        assert_eq!(resolve_dora_bin(), "dora");
    }

    #[test]
    fn normalizes_supported_outputs() {
        assert_eq!(normalize_dora_version("dora 1.0.0\n"), "dora 1.0.0");
        assert_eq!(
            normalize_dora_version("dora-cli 1.0.0-rc.4\n"),
            "dora 1.0.0-rc.4"
        );
        assert_eq!(normalize_dora_version("1.0.0\n"), "dora 1.0.0");
    }

    #[test]
    fn rejects_invalid_outputs() {
        for raw in [
            "",
            "\n",
            "not dora",
            "dora",
            "dora latest",
            "version 1.0.0",
            "dora 1.0.0-",
            "dora 1.0.0+",
            "dora 1.0.0-!!!",
            "dora 1.0.0-rc..1",
            "dora 01.2.3",
            "dora 1.02.3",
            "dora 1.2.03",
            "dora 1.2.3-01",
        ] {
            assert_eq!(normalize_dora_version(raw), "unknown", "input: {raw:?}");
        }
    }

    #[test]
    fn accepts_only_nonempty_ascii_semver_suffix_identifiers() {
        for raw in [
            "dora 1.0.0-rc.4",
            "dora 1.0.0-alpha-1+build.7",
            "dora 1.0.0+build.7",
            "dora 1.0.0+01",
        ] {
            assert_ne!(normalize_dora_version(raw), "unknown", "input: {raw:?}");
        }
    }

    /// dora 0.5 prints `dora-cli 0.5.0` to stderr; a version on stderr
    /// must still be detected so the gate can reject it explicitly.
    #[tokio::test(flavor = "current_thread")]
    async fn version_query_reads_stderr_when_stdout_is_empty() {
        let _lock = ENV_LOCK.lock().unwrap();
        let script = version_script_on_stderr("0.5.0");
        let _scripts = TempScriptsGuard::new([script.clone()]);
        let _env = DoraBinEnvGuard::set(&script);
        assert_eq!(dora_version().await, "dora 0.5.0");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn version_cache_is_keyed_by_resolved_binary() {
        let _lock = ENV_LOCK.lock().unwrap();
        let first = version_script("1.0.0");
        let second = version_script("2.0.0");
        let _scripts = TempScriptsGuard::new([first.clone(), second.clone()]);

        let _first_env = DoraBinEnvGuard::set(&first);
        assert_eq!(dora_version().await, "dora 1.0.0");

        let _second_env = DoraBinEnvGuard::set(&second);
        assert_eq!(dora_version().await, "dora 2.0.0");
    }

    struct TempScriptsGuard {
        paths: Vec<PathBuf>,
    }

    impl TempScriptsGuard {
        fn new(paths: impl IntoIterator<Item = PathBuf>) -> Self {
            Self {
                paths: paths.into_iter().collect(),
            }
        }
    }

    impl Drop for TempScriptsGuard {
        fn drop(&mut self) {
            for path in &self.paths {
                let _ = fs::remove_file(path);
            }
        }
    }

    #[test]
    fn temp_scripts_are_removed_during_unwind() {
        let path = version_script("1.0.0");
        let result = std::panic::catch_unwind(|| {
            let _scripts = TempScriptsGuard::new([path.clone()]);
            panic!("test unwind");
        });

        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn only_dora_one_is_lifecycle_supported() {
        assert!(lifecycle_supported("dora 1.0.0"));
        assert!(lifecycle_supported("dora 1.0.0-rc.4"));
        assert!(!lifecycle_supported("dora 0.5.0"));
        assert!(!lifecycle_supported("dora 2.0.0"));
        assert!(!lifecycle_supported("dora 1"));
        assert!(!lifecycle_supported("dora 1.invalid"));
        assert!(!lifecycle_supported("dora 1.0 unexpected"));
        assert!(!lifecycle_supported("unknown"));
    }

    fn version_script(version: &str) -> PathBuf {
        let path = unique_script_path("dora-version-test");
        fs::write(&path, format!("#!/bin/sh\nprintf 'dora {version}\\n'\n"))
            .expect("write version script");
        make_executable(&path);
        path
    }

    fn version_script_on_stderr(version: &str) -> PathBuf {
        let path = unique_script_path("dora-version-stderr-test");
        fs::write(
            &path,
            format!("#!/bin/sh\nprintf 'dora {version}\\n' >&2\n"),
        )
        .expect("write version script");
        make_executable(&path);
        path
    }

    fn unique_script_path(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{unique}", std::process::id()))
    }

    fn make_executable(path: &PathBuf) {
        let mut permissions = fs::metadata(path)
            .expect("read script metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("make version script executable");
    }
}
