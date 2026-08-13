# LeRobot Robot Profiles

Profile YAML files map robot-specific LeRobot dataset column names to
dora-studio's attribution schema. Switching robots is swapping a file —
no code changes.

## Format

```yaml
robot: <name>
angle_unit: radians                 # or: degrees (converted for 3D preview)
fields:
  state: [<candidate column names, first hit wins>]
  action: [<...>]
  task: [<...>]
  timestamp: [<...>]
  frame_index: [<...>]
joint_mapping:
  arm_joints: [0, 1, 2, 3, 4, 5]   # action/state indices for arm joints
  gripper: 6                        # optional gripper index
```

## angle_unit

`radians` (default) or `degrees`. The B601 dataset records angles in
degrees; the studio converts to radians for the 3D viewport preview.
The detail card shows the raw values with their unit.

## Field semantics

| Field | Meaning | Attribution step |
|-------|---------|------------------|
| state | robot observation vector | SensorFrame (metadata) |
| action | commanded action vector | ParsedAction |
| task | task index column | Prompt text lookup |
| timestamp | seconds (episode-relative or wall clock) | timeline alignment |
| frame_index | frame counter (falls back to row order) | frame identity |

## Aliases

Each field lists candidate column names; the first that exists in the
dataset wins. This handles naming drift between dataset versions, e.g.
`observation.state` ↔ `observations/state` ↔ `obs.state`.

## Auto-detect

The backend scores each profile by the fraction of its semantic fields
that match the dataset's actual columns and suggests the best one
(score ≥ 0.5).

## Adding a robot

Copy a template profile, change `robot:` and adjust the alias lists to
the dataset's column names. Filename convention: `lerobot_profile_<id>.yaml`.
