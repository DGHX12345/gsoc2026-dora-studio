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

#[derive(Debug, Clone, PartialEq)]
pub struct RobotProfile {
    pub robot_name: String,
    pub fields: FieldAliases,
    pub joint_mapping: JointMapping,
}

#[derive(Debug)]
pub enum ProfileError {
    Parse(String),
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
}
