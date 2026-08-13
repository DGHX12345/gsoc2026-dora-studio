//! LeRobot robot profile — maps dataset column names to attribution fields.

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAliases {
    pub state: Vec<String>,
    pub action: Vec<String>,
    pub task: Vec<String>,
    pub timestamp: Vec<String>,
    pub frame_index: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JointMapping {
    pub arm_joints: Vec<usize>,
    pub gripper: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    Radians,
    Degrees,
}

impl AngleUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Radians => "radians",
            Self::Degrees => "degrees",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotProfile {
    pub robot_name: String,
    pub fields: FieldAliases,
    pub joint_mapping: JointMapping,
    pub angle_unit: AngleUnit,
}

#[derive(Debug)]
pub enum ProfileError {
    Parse(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "profile error: {msg}"),
        }
    }
}

pub fn parse_profile_yaml(text: &str) -> Result<RobotProfile, ProfileError> {
    let err = |msg: &str| ProfileError::Parse(msg.to_string());

    let mut robot_name = None;
    let mut state = Vec::new();
    let mut action = Vec::new();
    let mut task = Vec::new();
    let mut timestamp = Vec::new();
    let mut frame_index = Vec::new();
    let mut arm_joints = Vec::new();
    let mut gripper = None;
    let mut angle_unit = AngleUnit::Radians;
    let mut section = "";

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(sec) = line.strip_suffix(':') {
            let sec = sec.trim();
            if sec == "fields" || sec == "joint_mapping" {
                section = sec;
                continue;
            }
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| err(&format!("expected 'key: value', got '{line}'")))?;
        let key = key.trim();
        let value = value.trim().trim_end_matches('#').trim();

        if section.is_empty() && key == "robot" {
            robot_name = Some(value.to_string());
            continue;
        }
        if key == "angle_unit" {
            angle_unit = match value {
                "degrees" => AngleUnit::Degrees,
                "radians" => AngleUnit::Radians,
                other => return Err(err(&format!("unknown angle_unit '{other}'"))),
            };
            continue;
        }

        match (section, key) {
            ("fields", "state") => state = parse_str_list(value).map_err(|e| err(&e))?,
            ("fields", "action") => action = parse_str_list(value).map_err(|e| err(&e))?,
            ("fields", "task") => task = parse_str_list(value).map_err(|e| err(&e))?,
            ("fields", "timestamp") => timestamp = parse_str_list(value).map_err(|e| err(&e))?,
            ("fields", "frame_index") => frame_index = parse_str_list(value).map_err(|e| err(&e))?,
            ("joint_mapping", "arm_joints") => {
                arm_joints = parse_usize_list(value).map_err(|e| err(&e))?
            }
            ("joint_mapping", "gripper") => {
                gripper = Some(value.parse().map_err(|e| err(&format!("gripper: {e}")))?)
            }
            _ => {}
        }
    }

    let robot_name =
        robot_name.ok_or_else(|| err("profile is missing 'robot:' line"))?;

    Ok(RobotProfile {
        robot_name,
        fields: FieldAliases {
            state,
            action,
            task,
            timestamp,
            frame_index,
        },
        joint_mapping: JointMapping {
            arm_joints,
            gripper,
        },
        angle_unit,
    })
}

fn parse_str_list(value: &str) -> Result<Vec<String>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| format!("expected list like [a, b], got '{value}'"))?;
    Ok(inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn parse_usize_list(value: &str) -> Result<Vec<usize>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| format!("expected list like [0, 1], got '{value}'"))?;
    inner
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.trim()
                .parse::<usize>()
                .map_err(|e| format!("expected integer, got '{}': {e}", s.trim()))
        })
        .collect()
}

pub fn match_columns(profile: &RobotProfile, columns: &[String]) -> (usize, usize) {
    let fields = [
        &profile.fields.state,
        &profile.fields.action,
        &profile.fields.task,
        &profile.fields.timestamp,
        &profile.fields.frame_index,
    ];
    let total = fields.len();
    let mut matched = 0;
    for aliases in fields {
        if aliases
            .iter()
            .any(|a| columns.iter().any(|c| c == a))
        {
            matched += 1;
        }
    }
    (matched, total)
}

pub fn profile_score(profile: &RobotProfile, columns: &[String]) -> f32 {
    let (matched, total) = match_columns(profile, columns);
    matched as f32 / total.max(1) as f32
}

pub struct ProfileManager {
    dir: PathBuf,
}

impl ProfileManager {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    /// 返回去前缀（lerobot_profile_）、去 .yaml 后缀的 profile 名列表。
    pub fn list(&self) -> Result<Vec<String>, ProfileError> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.dir)
            .map_err(|e| ProfileError::Parse(e.to_string()))?
        {
            let name = entry
                .map_err(|e| ProfileError::Parse(e.to_string()))?
                .file_name();
            let name = name.to_string_lossy().to_string();
            if let Some(stem) = name
                .strip_prefix("lerobot_profile_")
                .and_then(|s| s.strip_suffix(".yaml"))
            {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> Result<RobotProfile, ProfileError> {
        let path = self.dir.join(format!("lerobot_profile_{name}.yaml"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ProfileError::Parse(format!("profile '{name}': {e}")))?;
        parse_profile_yaml(&text)
    }

    /// 按 profile_score 返回最高分建议；无 profile 得分 ≥ 0.5 时返回 None。
    pub fn autodetect(&self, columns: &[String]) -> Result<Option<(String, f32)>, ProfileError> {
        let mut best: Option<(String, f32)> = None;
        for name in self.list()? {
            let profile = self.load(&name)?;
            let score = profile_score(&profile, columns);
            if best.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                best = Some((name, score));
            }
        }
        Ok(best.filter(|(_, s)| *s >= 0.5))
    }
}

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    const B601_YAML: &str = r#"
# B601 profile
robot: B601
fields:
  state: [observation.state, observations/state, obs.state]
  action: [action]
  task: [task_index]
  timestamp: [timestamp]
  frame_index: [frame_index]
joint_mapping:
  arm_joints: [0, 1, 2, 3, 4, 5]
  gripper: 6
"#;

    #[test]
    fn parses_profile_fields_and_aliases() {
        let p = parse_profile_yaml(B601_YAML).unwrap();
        assert_eq!(p.robot_name, "B601");
        assert_eq!(
            p.fields.state,
            vec!["observation.state", "observations/state", "obs.state"]
        );
        assert_eq!(p.fields.action, vec!["action"]);
        assert_eq!(p.joint_mapping.arm_joints, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(p.joint_mapping.gripper, Some(6));
    }

    #[test]
    fn rejects_missing_robot_line() {
        assert!(parse_profile_yaml("fields:\n  state: [a]\n").is_err());
    }

    #[test]
    fn alias_matching_prefers_first_hit() {
        let p = parse_profile_yaml(B601_YAML).unwrap();
        let columns = vec![
            "action".to_string(),
            "observations/state".to_string(),
            "task_index".to_string(),
            "timestamp".to_string(),
        ];
        let (matched, total) = match_columns(&p, &columns);
        assert_eq!((matched, total), (4, 5)); // state 命中第二个别名；frame_index 未命中
    }

    #[test]
    fn autodetect_scores_and_suggests_best_profile() {
        let a = parse_profile_yaml(B601_YAML).unwrap();
        let b = parse_profile_yaml(
            "robot: OTHER\nfields:\n  state: [qpos]\n  action: [qvel]\njoint_mapping:\n  arm_joints: [0]\n",
        )
        .unwrap();
        let columns = vec![
            "observation.state".to_string(),
            "action".to_string(),
            "task_index".to_string(),
            "timestamp".to_string(),
            "frame_index".to_string(),
        ];
        // score = matched / profile 定义的字段数；a 应高于 b
        assert!(profile_score(&a, &columns) > profile_score(&b, &columns));
    }

    #[test]
    fn profile_manager_lists_and_loads_profiles() {
        let dir = std::env::temp_dir().join("dora-studio-tests/profiles_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lerobot_profile_testa.yaml"), B601_YAML).unwrap();
        std::fs::write(dir.join("not_a_profile.txt"), "x").unwrap();
        let mgr = ProfileManager::new(&dir);
        let names = mgr.list().unwrap();
        assert_eq!(names, vec!["testa".to_string()]);
        let p = mgr.load("testa").unwrap();
        assert_eq!(p.robot_name, "B601");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_angle_unit_degrees_and_defaults_to_radians() {
        let deg = parse_profile_yaml(&format!("{B601_YAML}\nangle_unit: degrees\n")).unwrap();
        assert_eq!(deg.angle_unit, AngleUnit::Degrees);
        let rad = parse_profile_yaml(B601_YAML).unwrap();
        assert_eq!(rad.angle_unit, AngleUnit::Radians);
    }

    #[test]
    fn profile_manager_autodetect_suggests_best_match() {
        let dir = std::env::temp_dir().join("dora-studio-tests/profiles_test2");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lerobot_profile_best.yaml"), B601_YAML).unwrap();
        std::fs::write(
            dir.join("lerobot_profile_worst.yaml"),
            "robot: X\nfields:\n  state: [qpos]\n  action: [qvel]\njoint_mapping:\n  arm_joints: [0]\n",
        )
        .unwrap();
        let mgr = ProfileManager::new(&dir);
        let columns = vec![
            "observation.state".to_string(),
            "action".to_string(),
            "task_index".to_string(),
            "timestamp".to_string(),
            "frame_index".to_string(),
        ];
        let (name, score) = mgr.autodetect(&columns).unwrap().expect("suggestion");
        assert_eq!(name, "best");
        assert!(score > 0.9);
        std::fs::remove_dir_all(&dir).ok();
    }
}
