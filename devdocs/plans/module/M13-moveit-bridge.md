# M13: MoveIt Bridge Tool

**Layer**: 5 — Tool Slot System
**Depends on**: M11 (tool slot protocol), M06 (3D viewport), M01 (canvas)
**Effort**: 1.5 weeks

## Revision R1 (2026-08-14, source-audit re-verification + student decisions)

Audit re-verified against `/home/dora/dora-moveit2` (see OVERVIEW risk
register); corrections and decisions below override the body of this plan.

### Audit corrections (verified against source)

- **plan_status payload has 7 keys** — `plan_id, success, planning_time,
  path_length, num_waypoints, num_nodes, message`; the error branch sends
  only `{plan_id, success: false, message}` with NO metadata. Parser must
  be tolerant; `success` is the only required field.
- **execution_status has 5 keys** — adds `execution_count`,
  `total_waypoints` to the 3 in the plan.
- **joint_positions is float64** (mujoco qpos, full `model.nq`, may
  include freejoint on mobile models). ToolPayload has no f64 channel;
  feed.ts keeps full precision on the `json` channel — the tool reads
  `json` first for this port, `f32` as fallback (rendering tolerance).
- **scene_update also carries `timestamp`** (wall clock);
  **attached_objects have NO `color`** (default gray `[0.5,0.5,0.5,1]`).
- **50ms cadence is dataflow-driven** (`dora/timer/millis/50` in the
  example YAMLs), not executor-internal. The executor also emits HOME
  joint commands continuously while idle ("ALWAYS output HOME position
  when idle") — the panel must key off `execution_status.is_executing`.
- **.drec format has NO metadata field** (M04 format, drec/types.rs) —
  `num_waypoints`/`num_joints` metadata is unavailable in replay.
  Trajectory reshape must use the D3 joint config count. (Live M15-B
  bridge events DO carry metadata — flat reshape then uses it.)
- Real .drec event bytes are **Arrow IPC** (pa.array serialization), not
  JSON. Per student decision: honest `unsupported` (M12 policy), no
  Arrow IPC reader. M15-B live bridge sends JSON — the gap disappears.

### Student decisions (2026-08-14)

- **D4 = Option B**: MoveItTool owns a Three.js URDF loader (joint tree +
  STLLoader, package:// remap). One mechanism covers UR5e/GEN72/B601
  (B601 exists only as URDF). FK comes free from the URDF joint tree.
  NanoRobotViewer stays untouched. Option A's premise fails: no
  robot_descriptions on this machine, no URDF→MJCF converter exists.
- **D1 subscribePorts + plan_status** (6 ports total): trajectory,
  joint_positions, joint_commands, scene_update, execution_status,
  plan_status. Panel shows plan success/failure.
- **Backend demo generator**: add `generate_moveit_demo` (moveit-semantics
  streams: joint-angle trajectory, scene_update, execution_status,
  plan_status, joint_commands, joint_positions) + tests, written to
  `/tmp/dora-studio-tests/moveit_demo.drec` — required for manual
  verification.
- **Port-collision policy (trajectory)**: DvizPathTool also subscribes
  `/^trajectory$/i` (any node). Flat joint-angle arrays and dviz xyz
  paths are mutually ambiguous (6N divisible by 3). Resolution: the
  moveit demo writes trajectory as a JSON object envelope
  `{waypoints: [[q...], ...]}` (unambiguous — dviz parsers reject
  objects); the moveit demo file omits the dviz `trajectory` stream
  (waypoints/target/costmap suffice for D7 co-visualization). The flat
  reshape path remains implemented + unit-tested for M15-B live data
  (metadata available there), gated by joint-count divisibility and
  |q| ≤ 2π plausibility.
- **float64 note**: joint_positions parsed from `json` channel when
  present (feed.ts exposes both f32 and json for arrays).

### D4 implementation decisions (2026-08-14, student confirmed)

- **B601 placement**: copied to `models/b601/` locally (URDF +
  meshes_b601_gripper/, ~27 MB), `models/b601/` in .gitignore — served by
  the existing `/models` static route (Nano precedent). Not committed.
- **Option B tool-owned loader**: minimal XML tokenizer parser (testable
  in node; no DOMParser dependency) → URDF link/joint tree → THREE
  Object3D hierarchy (revolute/continuous rotate around `<axis>`,
  prismatic translate, fixed static). FK = matrix world update on the
  tree. STL meshes via three STLLoader.parse with an injectable
  byte resolver (fetch in browser, fs in tests).
- **package:// remap**: strip everything through `description/` — the
  remainder resolves relative to the model dir.
- **Auto-load**: on attach the tool loads the single available model
  (b601) when present; honest `unavailable` otherwise (chart remains the
  fallback). The D6 panel adds the model selector on top.
- **D4.1 M10 linkage**: MoveItTool exposes `previewPose(values)` — when
  the LeRobot source's profile associates a model name and the moveit
  tool is attached+loaded, the AttributionBar "Show in 3D" drives the
  B601 model (hint label removed); otherwise the Nano preview fallback
  stays.
- **Gripper mapping**: dataset value 7 (0-56.8, degrees) maps linearly
  to the URDF prismatic limit (0-0.0715 m — corrected from the handoff's
  0.0285 after reading the actual URDF), clamped; applied to
  gripper_joint1 + gripper_joint2.

### D7 implementation note (2026-08-14)

Co-visualization is delivered by construction: the moveit demo file carries
the dviz streams (simple_planner waypoints/target, costmap_node costmap),
both tools attach to the same scene, and the z-layers never fight —
costmap plane 0.02 < dviz paths 0.05 < the moveit EE path at true FK
heights (3D). Cross-tool port isolation is regression-tested
(co-visualization.test.ts): the moveit envelope is invisible to dviz
parsers, dviz waypoints/costmaps are invisible to the moveit tool, and a
dviz xyz trajectory (120 values) is rejected by the b601 7-joint reshape.
Edge case left documented: with a 6-joint robot selected (ur5e) a dviz
xyz stream whose length is divisible by 6 could false-positive; M15-B
live data carries metadata and is unaffected.

### Known issue (2026-08-14): B601 playback lag

B601 STLs carry ~1.65M vertices; the live model plus 5 full-detail ghost
clones ≈ 10M vertices per frame — laggy on integrated GPUs. Student
accepted deferral. Fix options ranked: (1) offline mesh decimation of
models/b601 (one-time asset step); (2) skeleton/stick-figure ghosts
(D6 "ghost detail" option); (3) hide ghosts during playback, show on
pause. Per-frame rebuild churn is already fixed (e5363bb).

## Module Roles (2026-08-14, student decision — every module keeps an irreplaceable role)

- **Visualization 页面(视口)**: the only 3D world renderer — dviz paths,
  costmap, MoveIt trajectory/collision objects, physics mirror (M15),
  live simulation presentation. All render tools mount here via the tool
  slot protocol.
- **Motion Planner 页面**: the planning *console* — planner algorithm
  implantation, goal setting, Plan/Execute/Stop requests, scene object
  management. Stays UNTOUCHED in M13 (its 3D part remains a read-only
  mirror); its control-plane wiring lands in M15-B (needs live dataflow).
- **ToolPanel sidebars (M13 D6)**: tool-scoped *playback* controls
  (trajectory player, joint table, ghost sliders) — viewing side, not
  request side. No overlap with the Motion Planner console.

## Source Audit Summary

dora-moveit2 is pure Python. **No structured Arrow schemas exist.** Every
payload is either flat float32/f64 array or JSON-as-uint8.

**Actual port names and formats** (from dora_moveit/):

| Operator | Output Port | Format | Metadata |
|----------|-------------|--------|----------|
| planner | `trajectory` | flat float32, row-major waypoints: `[q1_1..q1_n, q2_1..q2_n, ...]` | `num_waypoints`, `num_joints` |
| planner | `plan_status` | JSON-as-uint8: `{"success": bool, "message": str}` | `success: bool` |
| planning_scene | `scene_update` | JSON-as-uint8: `{"version": int, "world_objects": [...], "attached_objects": [...], "robot_state": {...}}` | `version: int` |
| ik_solver | `ik_solution` | flat float32: N joint angles | `encoding: "jointstate"` |
| trajectory_executor | `joint_commands` | flat float32: joint positions (one cmd per 50ms tick) | (none) |
| trajectory_executor | `execution_status` | JSON: `{"is_executing": bool, "current_waypoint": int, "progress": float}` | (none) |
| mujoco | `joint_positions` | flat float64: full qpos | `encoding: "jointstate"` |

**Critical missing data:**
- **No joint names on wire** — must be read from config files or dataflow YAML
- **No URDF emission** — must be loaded from disk (`models/` or `robot_descriptions`)
- **No timestamps in trajectory** — executor linearly interpolates at fixed 50ms tick
- **Collision checking is stubbed** (`return True` in planner)
- **Planning scene objects**: sphere/box/cylinder only, no meshes

## Deliverables

### D1: Flat trajectory parser

Arrow port subscription (corrected):
```typescript
subscribePorts: [
  { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
  { nodeIdPattern: /.*/, outputIdPattern: /^joint_positions$/i },
  { nodeIdPattern: /.*/, outputIdPattern: /^joint_commands$/i },
  { nodeIdPattern: /.*/, outputIdPattern: /^scene_update$/i },
  { nodeIdPattern: /.*/, outputIdPattern: /^execution_status$/i },
]
```

Parser logic:
- `trajectory`: reshape `flat[N*M]` → `[N waypoints, M joints]` using metadata
- `joint_positions` / `joint_commands`: direct float array
- `scene_update`: JSON bytes → utf8 → parse into `PlanningScene`
- `execution_status`: JSON bytes → `ExecutionStatus`

### D2: Trajectory renderer

New file: `frontend/src/tools/moveit/MoveItTool.ts`

- End-effector path: compute FK from joint angles (use robot config kinematics)
  - If FK not available: render joint-space as colored bar chart or parallel coordinates
  - Color gradient: blue (start) → red (end) along trajectory
- Ghost poses: semi-transparent robot silhouettes at evenly-spaced waypoints
  - Configurable count (default: 5 ghost poses)
- Current pose: solid robot model at current time
- Joint angle table: per-joint readout (need joint names from config)

### D3: Joint name configuration

New file: `frontend/src/tools/moveit/joint-config.ts`

Since joint names are not on the wire, load them from configuration:

- Read from dataflow YAML's `env.ROBOT_CONFIG_MODULE` — but we can't import Python
- Alternative: **manual joint name maps** for known robots:
  ```typescript
  const KNOWN_ROBOTS: Record<string, string[]> = {
    'ur5e': ['shoulder_pan_joint', 'shoulder_lift_joint', 'elbow_joint',
             'wrist_1_joint', 'wrist_2_joint', 'wrist_3_joint'],
    'gen72': ['joint_1', 'joint_2', 'joint_3', 'joint_4', 'joint_5', 'joint_6',
              'gripper_left', 'gripper_right'],
    // ... more as needed
  };
  ```
- Robot type detected from dataflow YAML node config or model name
- Fallback: generic labels `J0, J1, J2, ...`
- User can override via settings panel

### D4: URDF model loading

dviz ships a URDF parser (`dviz-urdf` crate, Rust). Since we can't link Rust
crates (see M00), we have two options:

**Option A (recommended)**: Reuse existing NanoRobotViewer (MuJoCo XML-based).
Convert URDF → MuJoCo XML using `robot_descriptions` or manual mapping.
Most dora-moveit2 examples already ship MuJoCo XML files.

**Option B**: Build a minimal Three.js URDF loader.
Parse URDF XML → extract link/joint hierarchy → create Three.js Object3D tree.
Joint types: Fixed, Revolute, Continuous, Prismatic.

- URDF path: read from dataflow YAML `env.MODEL_NAME` or scan `models/`
- Load time target: <3s for 6DOF arm
- Cache loaded models in memory
- Fallback: stick-figure skeleton if model loading fails

### D4.1: B601 模型接入（M10 联动，2026-08-13）

模型文件已获取（Seeed reBot Arm B601-DM，学生已本地克隆，验证通过）:

- **源路径**: `/home/dora/reBotArmController_ROS2/src/rebotarm_bringup/description/`
- **主 URDF**: `urdf/reBot_B601_DM_with_gripper.urdf` — 6 旋转关节
  （joint1-joint6）+ 夹爪（prismatic 手指关节）
- **网格**: `meshes_b601_gripper/*.STL`（URDF 内引用
  `package://rebotarm_bringup/...`，加载时需重映射到本地目录）
- **许可**: CC BY-NC-SA 4.0（个人/研究免费）。分发需署名 Seeed Studio；
  或沿用 Nano 模型先例仅本地放置（models/ 不提交）——实施时与学生确认

**接入要点**（归 D4 实施）:

1. 复制到 `models/b601/`（本地，默认不提交）
2. D4 选型影响: Option A（URDF→MuJoCo XML）需支持夹爪 prismatic 关节；
   Option B（Three.js URDF 解析器）需 `package://` 路径重映射 +
   STLLoader（NanoRobotViewer 已有 STL 加载经验）
3. **M10 联动**: AttributionBar 的 "在 3D 中查看" 当前在 Nano 模型上
   预览（详情卡有提示标签）。B601 模型接入后，lerobot 源预览切换为
   B601 模型（profile 关联模型名），提示标签移除
4. **关节对齐**: action 前 6 值 → joint1-6；第 7 值（夹爪）→ gripper
   手指关节（URDF 限位 0-0.0285m，数据集为 0-56.8° 开合，实施时确认
   映射比例）
5. **验收**: 加载 B601 URDF <3s；lerobot 动作向量在 B601 模型上预览，
   与 b601_pilot_v1 真实采集姿态一致（可抽查对照）

### D5: Collision scene overlay

Parse `scene_update` JSON → render collision objects as wireframe overlays:
- `world_objects`: spheres → WireframeGeometry(SphereGeometry), boxes → BoxGeometry, cylinders → CylinderGeometry
- Color: yellow wireframe (same as RViz convention)
- Toggle collision scene visibility
- Simple bounding-sphere self-collision check (not full mesh collision)

### D6: MoveIt control panel

Sidebar when tool attached:
- Trajectory player: Play/Pause animation (independent of main timeline)
  - Speed: 0.5x / 1x / 2x
  - Frame step: forward/back one waypoint
  - "Sync to timeline" toggle
- End-effector position readout (x, y, z) — requires FK
- Joint angle table (name → current angle, updated during playback)
- Collision scene toggle
- Ghost pose count slider (1-20)
- Robot model selector (if multiple URDFs available)

### D7: dviz co-visualization

- MoveIt trajectory + dviz global path in same viewport
- Layer ordering:
  1. Costmap (bottom) — dviz
  2. Global path (middle) — dviz
  3. Robot trajectory (top) — MoveIt
- Z-fighting prevention: slight Y-offset per layer (0.01 units)
- Both share same TF tree and timeline

## Acceptance Criteria

- [ ] Correctly parse flat float32 `trajectory` → reshape to waypoints × joints
- [ ] End-effector path renders (or joint-space chart if FK unavailable)
- [ ] Ghost poses render at configurable intervals
- [ ] URDF/MuJoCo model loads in <3s, robot moves correctly
- [ ] B601 URDF loads; LeRobot 源的 "在 3D 中查看" 在 B601 模型上预览（M10 联动，D4.1）
- [ ] Collision objects visible as yellow wireframes
- [ ] dviz paths + MoveIt trajectory overlay without z-fighting
- [ ] Joint angle table updates during trajectory playback
- [ ] Trajectory player: play/pause/scrub independent of main timeline
- [ ] `npm run build` passes

## Exposed Interfaces

```typescript
// frontend/src/tools/moveit/MoveItTool.ts
// (signatures corrected per M12 Revision R1 — actual M11 protocol)
import type { Component } from 'vue';

export class MoveItTool implements ViewportTool {
  readonly id = 'moveit-bridge';
  readonly displayName = 'MoveIt Bridge';
  readonly category = 'planning';
  readonly subscribePorts = [
    { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^joint_positions$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^joint_commands$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^scene_update$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^execution_status$/i },
  ];

  onAttach(context: ToolContext): void;
  onBatch(batch: ToolBatch, tf?: TfTree): void;
  onTimelineSeek?(timestampNs: number): void;
  onDetach(): void;
  panelComponent?: Component; // MoveItPanel.vue, bound in registerBuiltinTools
}
```

```typescript
// Planning scene (parsed from JSON)
interface PlanningScene {
  version: number;
  world_objects: SceneObject[];
  attached_objects: AttachedObject[];
  robot_state: { joint_positions: number[]; gripper_state: number };
}

interface SceneObject {
  name: string;
  type: 'sphere' | 'box' | 'cylinder';
  position: [number, number, number];
  dimensions: number[];  // [r] | [sx,sy,sz] | [r,h]
  color: [number, number, number, number];
}
```
