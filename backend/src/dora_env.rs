use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

// --- M17: user-level settings store (~/.config/dora-studio/settings.json) ---

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoraSettings {
    #[serde(default)]
    pub dora_bin: Option<String>,
    #[serde(default)]
    pub candidates: Vec<String>,
    #[serde(default)]
    pub project_dirs: Vec<String>,
    #[serde(default)]
    pub manual_nodes: Vec<ManualNode>,
}

// --- M18: manual node definitions + user project dirs (dataflow scanning) ---

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualPort {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub urn: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualNode {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub inputs: Vec<ManualPort>,
    #[serde(default)]
    pub outputs: Vec<ManualPort>,
}

/// In-memory settings state; loaded lazily on first use and updated
/// in place on switch. None = not loaded yet.
static SETTINGS_STATE: RwLock<Option<DoraSettings>> = RwLock::new(None);

pub(crate) fn settings_path() -> PathBuf {
    std::env::var("DORA_STUDIO_SETTINGS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config/dora-studio/settings.json")
        })
}

fn save_settings_to(path: &Path, settings: &DoraSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create settings directory: {error}"))?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("failed to serialize settings: {error}"))?;
    std::fs::write(path, text).map_err(|error| format!("failed to write settings: {error}"))
}

fn save_settings(settings: &DoraSettings) -> Result<(), String> {
    save_settings_to(&settings_path(), settings)
}

/// Load, mutate, persist, and refresh the in-memory settings cache in
/// one step. Every settings mutation goes through here so the file and
/// `SETTINGS_STATE` never drift apart.
fn mutate_settings(mutator: impl FnOnce(&mut DoraSettings)) -> Result<DoraSettings, String> {
    let mut settings = load_or_seed_settings();
    mutator(&mut settings);
    save_settings(&settings)?;
    *SETTINGS_STATE.write().expect("settings lock") = Some(settings.clone());
    Ok(settings)
}

pub(crate) fn project_dirs() -> Vec<String> {
    load_or_seed_settings().project_dirs
}

pub(crate) fn add_project_dir(path: &str) -> Result<Vec<String>, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("invalid project directory {path}: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let canonical = canonical.to_string_lossy().to_string();
    let settings = mutate_settings(|settings| {
        if !settings
            .project_dirs
            .iter()
            .any(|existing| existing == &canonical)
        {
            settings.project_dirs.push(canonical);
        }
    })?;
    Ok(settings.project_dirs)
}

pub(crate) fn remove_project_dir(path: &str) -> Result<Vec<String>, String> {
    // Fall back to the raw path so a project dir that was already deleted
    // on disk can still be removed from settings.
    let canonical = std::fs::canonicalize(path)
        .map(|canonical| canonical.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
    let settings = mutate_settings(|settings| {
        settings
            .project_dirs
            .retain(|existing| existing != &canonical);
    })?;
    Ok(settings.project_dirs)
}

pub(crate) fn manual_nodes() -> Vec<ManualNode> {
    load_or_seed_settings().manual_nodes
}

pub(crate) fn add_manual_node(mut node: ManualNode) -> Result<(), String> {
    node.id = node.id.trim().to_string();
    node.path = node.path.trim().to_string();
    if node.id.is_empty() || node.path.is_empty() {
        return Err("manual node requires id and path".to_string());
    }
    mutate_settings(|settings| {
        settings
            .manual_nodes
            .retain(|existing| existing.id != node.id);
        settings.manual_nodes.push(node);
    })?;
    Ok(())
}

pub(crate) fn load_or_seed_settings() -> DoraSettings {
    if let Some(settings) = SETTINGS_STATE.read().expect("settings lock").as_ref() {
        return settings.clone();
    }
    let path = settings_path();
    let settings = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => {
            let seeded = DoraSettings {
                dora_bin: None,
                candidates: seed_candidates(),
                ..Default::default()
            };
            // Best-effort persist; a read-only HOME falls back to the
            // in-memory state.
            let _ = save_settings_to(&path, &seeded);
            seeded
        }
    };
    *SETTINGS_STATE.write().expect("settings lock") = Some(settings.clone());
    settings
}

#[cfg(test)]
pub(crate) fn reset_settings_state_for_tests() {
    *SETTINGS_STATE.write().expect("settings lock") = None;
}

fn explicit_env_bin() -> Option<String> {
    std::env::var("DORA_STUDIO_DORA_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Whether DORA_STUDIO_DORA_BIN currently overrides settings/PATH
/// (the UI disables switching in this case).
pub(crate) fn env_bin_overrides() -> bool {
    explicit_env_bin().is_some()
}

/// First-run candidate seeding: the explicit env binary, the PATH
/// `dora`, and the conventional venv / local install locations.
fn seed_candidates_from(
    explicit_bin: Option<String>,
    path_entries: &[PathBuf],
    home_dir: Option<&Path>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(explicit) = explicit_bin {
        candidates.push(explicit);
    }
    for entry in path_entries {
        let candidate = entry.join("dora");
        if candidate.exists() {
            candidates.push(candidate.to_string_lossy().to_string());
        }
    }
    if let Some(home) = home_dir {
        let local = home.join(".local/bin/dora");
        if local.exists() {
            candidates.push(local.to_string_lossy().to_string());
        }
        if let Ok(entries) = std::fs::read_dir(home.join(".venvs")) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("bin/dora");
                if candidate.exists() {
                    candidates.push(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.clone()));
    candidates
}

fn seed_candidates() -> Vec<String> {
    let path_entries: Vec<PathBuf> = std::env::var("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    let home_dir = std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty());
    seed_candidates_from(explicit_env_bin(), &path_entries, home_dir.as_deref())
}

pub(crate) fn resolve_dora_bin() -> String {
    // 1. explicit env var wins over everything
    if let Some(explicit) = explicit_env_bin() {
        return explicit;
    }
    // 2. user settings (M17)
    if let Some(configured) = load_or_seed_settings().dora_bin {
        return configured;
    }
    // 3. PATH lookup by Command
    "dora".to_string()
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

/// Query `<binary> --version` with a strict 3s timeout; any failure
/// yields "unknown".
pub(crate) async fn query_version_of(binary: &str) -> String {
    match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new(binary)
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
    }
}

pub(crate) async fn dora_version() -> String {
    let binary = resolve_dora_bin();
    let cache = dora_version_cache().await;

    if let Some(version) = cache.lock().await.get(&binary).cloned() {
        return (*version).clone();
    }

    let version = query_version_of(&binary).await;
    cache.lock().await.insert(binary, Arc::new(version.clone()));
    version
}

/// Drop all cached version lookups so the next query re-runs against
/// the newly selected binary.
pub(crate) async fn invalidate_version_cache() {
    dora_version_cache().await.lock().await.clear();
}

// --- M17: detection and switching ---

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoraVersionItem {
    pub path: String,
    pub version: String,
    pub compatible: bool,
    pub active: bool,
}

/// Probe every configured candidate; unreachable binaries are
/// silently skipped.
pub(crate) async fn detect_versions() -> Vec<DoraVersionItem> {
    let settings = load_or_seed_settings();
    let active = resolve_dora_bin();
    let mut items = Vec::new();
    for candidate in settings.candidates {
        let version = query_version_of(&candidate).await;
        if version == "unknown" {
            continue;
        }
        items.push(DoraVersionItem {
            path: candidate.clone(),
            compatible: lifecycle_supported(&version),
            active: candidate == active,
            version,
        });
    }
    items
}

/// Validate a binary (exists + answers --version) and make it the
/// active dora for all future subprocesses. The in-memory settings,
/// the persisted settings file, and the version cache all update.
pub(crate) async fn switch_dora_bin(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err(format!("binary not found: {path}"));
    }
    if query_version_of(&path).await == "unknown" {
        return Err(format!("not a working dora binary: {path}"));
    }

    let mut settings = load_or_seed_settings();
    settings.dora_bin = Some(path.clone());
    if !settings.candidates.contains(&path) {
        settings.candidates.push(path.clone());
    }
    save_settings(&settings)?;
    *SETTINGS_STATE.write().expect("settings lock") = Some(settings);
    invalidate_version_cache().await;
    Ok(())
}

pub(crate) fn add_candidate(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err(format!("binary not found: {path}"));
    }
    let mut settings = load_or_seed_settings();
    if !settings.candidates.contains(&path) {
        settings.candidates.push(path.clone());
    }
    save_settings(&settings)?;
    *SETTINGS_STATE.write().expect("settings lock") = Some(settings);
    Ok(())
}

pub(crate) fn delete_candidate(path: String) -> Result<(), String> {
    let mut settings = load_or_seed_settings();
    settings.candidates.retain(|candidate| candidate != &path);
    if settings.dora_bin.as_deref() == Some(&path) {
        settings.dora_bin = None;
    }
    save_settings(&settings)?;
    *SETTINGS_STATE.write().expect("settings lock") = Some(settings);
    Ok(())
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
        path::{Path, PathBuf},
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
        let settings = SettingsEnvGuard::with_clean_dir("dora-settings-empty-bin");
        super::reset_settings_state_for_tests();
        let _env = DoraBinEnvGuard::set("");
        assert_eq!(resolve_dora_bin(), "dora");
        drop(settings);
    }

    #[test]
    fn missing_binary_uses_path_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let settings = SettingsEnvGuard::with_clean_dir("dora-settings-missing-bin");
        super::reset_settings_state_for_tests();
        let _env = DoraBinEnvGuard::remove();
        assert_eq!(resolve_dora_bin(), "dora");
        drop(settings);
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

    // --- M17: settings-based resolution, detection, switching ---

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    struct SettingsEnvGuard {
        previous: Option<OsString>,
        dir: PathBuf,
    }

    impl SettingsEnvGuard {
        fn set(path: &Path) -> Self {
            let previous = std::env::var_os("DORA_STUDIO_SETTINGS");
            std::env::set_var("DORA_STUDIO_SETTINGS", path);
            Self {
                previous,
                dir: PathBuf::new(),
            }
        }

        fn with_clean_dir(prefix: &str) -> Self {
            let dir = unique_temp_dir(prefix);
            let previous = std::env::var_os("DORA_STUDIO_SETTINGS");
            std::env::set_var("DORA_STUDIO_SETTINGS", dir.join("settings.json"));
            Self { previous, dir }
        }
    }

    impl Drop for SettingsEnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("DORA_STUDIO_SETTINGS", value),
                None => std::env::remove_var("DORA_STUDIO_SETTINGS"),
            }
            super::reset_settings_state_for_tests();
            if !self.dir.as_os_str().is_empty() {
                let _ = fs::remove_dir_all(&self.dir);
            }
        }
    }

    #[test]
    fn settings_path_uses_env_override() {
        let _lock = ENV_LOCK.lock().unwrap();
        let custom = std::env::temp_dir().join("custom-settings.json");
        let _settings = SettingsEnvGuard::set(&custom);
        super::reset_settings_state_for_tests();
        assert_eq!(super::settings_path(), custom);
    }

    #[test]
    fn resolve_prefers_env_over_settings_over_path() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-resolve");
        super::reset_settings_state_for_tests();
        let settings_file = guard.dir.join("settings.json");
        fs::write(
            &settings_file,
            r#"{"doraBin":"/opt/from-settings","candidates":[]}"#,
        )
        .unwrap();
        let _settings = SettingsEnvGuard::set(&settings_file);

        let _env = DoraBinEnvGuard::set("/opt/from-env");
        assert_eq!(resolve_dora_bin(), "/opt/from-env");
        drop(_env);

        assert_eq!(resolve_dora_bin(), "/opt/from-settings");

        fs::write(&settings_file, "{}").unwrap();
        super::reset_settings_state_for_tests();
        assert_eq!(resolve_dora_bin(), "dora");
        let _ = fs::remove_dir_all(&guard.dir);
    }

    #[test]
    fn missing_settings_seeds_candidates() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-seed");
        super::reset_settings_state_for_tests();
        let home = guard.dir.join("home");
        let venv10 = home.join(".venvs/dora10/bin/dora");
        let venv05 = home.join(".venvs/dora05/bin/dora");
        let local = home.join(".local/bin/dora");
        for path in [&venv10, &venv05, &local] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "#!/bin/sh\n").unwrap();
        }
        let _home_env = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);
        let _env = DoraBinEnvGuard::remove();
        let settings_file = guard.dir.join("settings.json");

        let settings = super::load_or_seed_settings();
        assert!(settings.dora_bin.is_none());
        assert!(settings
            .candidates
            .contains(&venv10.to_string_lossy().to_string()));
        assert!(settings
            .candidates
            .contains(&venv05.to_string_lossy().to_string()));
        assert!(settings
            .candidates
            .contains(&local.to_string_lossy().to_string()));
        assert!(settings_file.exists(), "seeding persists the settings file");

        match _home_env {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn seeded_candidates_are_deduplicated() {
        let explicit = "/opt/dora/bin/dora".to_string();
        let path_entries = [
            PathBuf::from("/opt/dora/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        let home = std::env::temp_dir().join("dora-seed-home");
        let candidates =
            super::seed_candidates_from(Some(explicit.clone()), &path_entries, Some(&home));
        let mut counts = std::collections::HashMap::new();
        for candidate in &candidates {
            *counts.entry(candidate).or_insert(0usize) += 1;
        }
        assert!(
            counts.values().all(|&count| count == 1),
            "no duplicates: {candidates:?}"
        );
        assert!(candidates.contains(&explicit));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn detect_versions_marks_compatible_and_active() {
        let _lock = ENV_LOCK.lock().unwrap();
        let v10 = version_script("1.0.0-rc.4");
        let v05 = version_script("0.5.0");
        let _scripts = TempScriptsGuard::new([v10.clone(), v05.clone()]);
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-detect");
        super::reset_settings_state_for_tests();
        let settings_file = guard.dir.join("settings.json");
        fs::write(
            &settings_file,
            format!(
                r#"{{"doraBin":"{}","candidates":["{}","{}"]}}"#,
                v10.display(),
                v10.display(),
                v05.display()
            ),
        )
        .unwrap();
        let _settings = SettingsEnvGuard::set(&settings_file);
        let _env = DoraBinEnvGuard::remove();

        let items = super::detect_versions().await;
        assert_eq!(items.len(), 2);
        let item10 = items
            .iter()
            .find(|item| item.path == v10.to_string_lossy())
            .unwrap();
        assert_eq!(item10.version, "dora 1.0.0-rc.4");
        assert!(item10.compatible);
        assert!(item10.active);
        let item05 = items
            .iter()
            .find(|item| item.path == v05.to_string_lossy())
            .unwrap();
        assert_eq!(item05.version, "dora 0.5.0");
        assert!(!item05.compatible);
        assert!(!item05.active);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn switch_validates_and_updates_resolution() {
        let _lock = ENV_LOCK.lock().unwrap();
        let v10 = version_script("1.0.0");
        let v05 = version_script("0.5.0");
        let _scripts = TempScriptsGuard::new([v10.clone(), v05.clone()]);
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-switch");
        super::reset_settings_state_for_tests();
        let settings_file = guard.dir.join("settings.json");
        fs::write(
            &settings_file,
            format!(
                r#"{{"doraBin":"{}","candidates":["{}"]}}"#,
                v10.display(),
                v10.display()
            ),
        )
        .unwrap();
        let _settings = SettingsEnvGuard::set(&settings_file);
        let _env = DoraBinEnvGuard::remove();

        assert_eq!(resolve_dora_bin(), v10.to_string_lossy());
        assert_eq!(dora_version().await, "dora 1.0.0");

        assert!(super::switch_dora_bin("/does/not/exist".to_string())
            .await
            .is_err());

        super::switch_dora_bin(v05.to_string_lossy().to_string())
            .await
            .expect("switch to valid binary");
        assert_eq!(resolve_dora_bin(), v05.to_string_lossy());
        assert_eq!(dora_version().await, "dora 0.5.0");

        let persisted = fs::read_to_string(&settings_file).unwrap();
        assert!(persisted.contains(&v05.to_string_lossy().to_string()));
    }

    #[test]
    fn add_and_delete_candidates_persist() {
        let _lock = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-candidates");
        super::reset_settings_state_for_tests();
        let extra = guard.dir.join("extra-dora");
        fs::write(&extra, "#!/bin/sh\n").unwrap();
        let active = guard.dir.join("active-dora");
        fs::write(&active, "#!/bin/sh\n").unwrap();
        let settings_file = guard.dir.join("settings.json");
        fs::write(
            &settings_file,
            format!(
                r#"{{"doraBin":"{}","candidates":["{}"]}}"#,
                active.display(),
                active.display()
            ),
        )
        .unwrap();
        let _settings = SettingsEnvGuard::set(&settings_file);
        let _env = DoraBinEnvGuard::remove();

        super::add_candidate(extra.to_string_lossy().to_string()).unwrap();
        let settings = super::load_or_seed_settings();
        assert!(settings
            .candidates
            .contains(&extra.to_string_lossy().to_string()));

        super::delete_candidate(extra.to_string_lossy().to_string()).unwrap();
        let settings = super::load_or_seed_settings();
        assert!(!settings
            .candidates
            .contains(&extra.to_string_lossy().to_string()));

        // 删除当前生效的 binary → 回退到 PATH 解析
        super::delete_candidate(active.to_string_lossy().to_string()).unwrap();
        let settings = super::load_or_seed_settings();
        assert!(settings.dora_bin.is_none());
        assert_eq!(resolve_dora_bin(), "dora");
    }

    // --- M18: project dirs + manual nodes ---

    #[test]
    fn settings_seed_has_empty_project_dirs_and_manual_nodes() {
        let _guard = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-m18-seed");
        let settings_file = guard.dir.join("settings.json");
        fs::write(
            &settings_file,
            r#"{"doraBin":"/opt/from-settings","candidates":[]}"#,
        )
        .unwrap();
        super::reset_settings_state_for_tests();
        let settings = super::load_or_seed_settings();
        assert!(settings.project_dirs.is_empty());
        assert!(settings.manual_nodes.is_empty());
    }

    #[test]
    fn add_project_dir_dedupes_and_persists() {
        let _guard = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-m18-proj");
        let target = guard.dir.join("proj-a");
        fs::create_dir_all(&target).unwrap();
        super::reset_settings_state_for_tests();
        let list = super::add_project_dir(target.to_string_lossy().as_ref()).unwrap();
        assert_eq!(list.len(), 1);
        // duplicate add is a no-op
        let list2 = super::add_project_dir(target.to_string_lossy().as_ref()).unwrap();
        assert_eq!(list2.len(), 1);
        // persisted: fresh load sees it
        super::reset_settings_state_for_tests();
        assert_eq!(super::project_dirs().len(), 1);
    }

    #[test]
    fn add_manual_node_persists_and_dedupes() {
        let _guard = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-m18-manual");
        super::reset_settings_state_for_tests();
        let node = super::ManualNode {
            id: "my-converter".into(),
            path: "/tmp/convert.py".into(),
            description: "RGB to BGR".into(),
            inputs: vec![super::ManualPort {
                name: "image".into(),
                urn: "std/media/v1/Image".into(),
            }],
            outputs: vec![super::ManualPort {
                name: "image".into(),
                urn: "std/media/v1/Image".into(),
            }],
        };
        super::add_manual_node(node.clone()).unwrap();
        super::add_manual_node(node).unwrap(); // duplicate id replaced
        let nodes = super::manual_nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "my-converter");
        // persisted: a fresh load sees the nested node (id + ports) again
        super::reset_settings_state_for_tests();
        assert_eq!(super::manual_nodes().len(), 1);
    }

    #[test]
    fn remove_project_dir_works_when_dir_gone_from_disk() {
        let _guard = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-m18-proj-gone");
        super::reset_settings_state_for_tests();
        let target = guard.dir.join("proj-gone");
        std::fs::create_dir_all(&target).unwrap();
        let canonical = std::fs::canonicalize(&target)
            .unwrap()
            .to_string_lossy()
            .to_string();
        super::add_project_dir(canonical.as_ref()).unwrap();
        std::fs::remove_dir_all(&target).unwrap();
        // dir no longer exists on disk — removal must still succeed
        let list = super::remove_project_dir(canonical.as_ref()).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn add_project_dir_rejects_missing_or_non_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-m18-proj-err");
        super::reset_settings_state_for_tests();
        assert!(super::add_project_dir("/definitely/not/a/real/dir").is_err());
        let file = guard.dir.join("plain-file");
        std::fs::write(&file, b"x").unwrap();
        assert!(super::add_project_dir(file.to_string_lossy().as_ref()).is_err());
    }

    #[test]
    fn add_manual_node_rejects_empty_id_or_path() {
        let _guard = ENV_LOCK.lock().unwrap();
        let guard = SettingsEnvGuard::with_clean_dir("dora-settings-m18-manual-err");
        super::reset_settings_state_for_tests();
        let empty_id = super::ManualNode {
            id: "  ".into(),
            path: "/tmp/a.py".into(),
            ..Default::default()
        };
        assert!(super::add_manual_node(empty_id).is_err());
        let empty_path = super::ManualNode {
            id: "node".into(),
            path: "".into(),
            ..Default::default()
        };
        assert!(super::add_manual_node(empty_path).is_err());
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
