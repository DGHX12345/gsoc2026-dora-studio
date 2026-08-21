# M15: Live Planning & Simulation (rviz-style)

**Layer**: 5+ — Tool Slot System / Live Data
**Depends on**: M13 (MoveIt bridge), M12 (dviz path tool), M03 (runtime)
**Effort**: 2.5 weeks (Phase B 1.5 + Phase C 1.0)
**Status**: CONFIRMED ESSENTIAL (2026-08-14, student decision) — route
simulation is part of the project scope; scheduled after M13 (Phase A rides
on M13), Phase B before/after M14 per weekly-sync prioritization.
**Phase B delivered 2026-08-16; Phase C delivered 2026-08-17** (C1 mujoco
mirror, C2 collision, C3 acceptance, 50ms feed — see the completion
records below; the per-link-sphere collision limitation is deferred).

## Requirement (verbatim intent)

Load a scene (with obstacles) and a robot into the 3D view, implant a
planning algorithm, start simulation: the robot follows the planned path
and avoids obstacles — "very realistic" simulation. Copying the mature
rviz solution is allowed.

## Architecture answer (what "copying rviz" actually means)

rviz alone does NOT plan or simulate. The mature complete solution is the
ROS trio: **rviz (frontend) + MoveIt (planning/collision) + Gazebo/MuJoCo
(physics)**. The dora-ecosystem equivalents all exist on this machine:

| Role | ROS classic | dora ecosystem | Status |
|------|-------------|----------------|--------|
| Frontend | rviz | dora-studio (M12 tool + M13) | in progress |
| Planning | MoveIt move_group | `/home/dora/dora-moveit2` (planner, planning_scene, ik_solver, trajectory_executor) | exists |
| Physics sim | Gazebo/MuJoCo | `/home/dora/dora-mujoco` (physics node, 50+ robots, sensor output) | exists |
| Collision checking | FCL/Bullet in MoveIt | **stubbed** in dora-moveit2 (`return True`) | must fix (external repo) |

So the requirement is satisfiable without inventing anything new: Studio
plays rviz's role; we build the *wiring* (live data path + sim mirror).

## Verified facts (2026-08-14)

- **Environment upgrade is feasible**: python3.11 and python3.12 already
  installed (`/usr/bin/python3.11`), plus `uv` and `conda`. Only the
  default `python3` is 3.10. Known pip mismatch (dora CLI v0.8 vs
  dora-rs python message v0.6) is fixable with a uv venv pinning matching
  versions — needs a verification spike.
- **Live path gap**: the viewport's only data source is .drec replay
  (ReplayScene). No Zenoh/WS data bridge exists; backend must stay
  dependency-light (Rust 1.75). Recommended: a `studio_bridge` dora node
  (Python, inside the dataflow) forwarding subscribed ports to the backend
  over local HTTP; backend buffers; frontend consumes via SSE or 100ms
  polling. NO new Rust dependencies.
- **Registry reuse**: toolRegistry.broadcastBatch already accepts arbitrary
  batch sources — a LiveFeed can feed the same M12/M13 tools unchanged.
- **MuJoCo boundary (existing decision)**: physics stays in the
  dora-mujoco node; Studio mirrors `joint_positions` in the viewport.
- Collision checking must be completed in dora-moveit2 (local repo) for
  true avoidance; alternatively user algorithms can avoid using the M12
  costmap (rendered in-viewport for free).

## Deliverables

### Phase A — replay-style planning visualization (covered by M13)

Planning scene obstacles + trajectory + ghost poses + execution status on
replay data. No new work beyond M13; this is the "static demo" baseline.

### Phase B — live data path + real dataflow (1.5 wk)

- **B1 environment spike**: uv venv with python3.11 + matching dora-rs
  python package; run `dora up`/example dataflow end-to-end locally.
- **B2 studio_bridge node** (new: `backend/scripts/studio_bridge.py` or
  examples/): subscribes to configurable ports via dora python API,
  POSTs `{node_id, output_id, timestamp, payload}` to backend
  `POST /api/live/ingest` (localhost-only, ring buffer per stream).
- **B3 backend live API** (`backend/src/live.rs`): ingest + ring buffer +
  `GET /api/live/recent?stream=&since_ts=`; SSE endpoint preferred
  (axum 0.6 supports), 100ms polling fallback.
- **B4 LiveFeed** (frontend): converts live frames to ToolBatch and calls
  `toolRegistry.broadcastBatch` — M12 path tool and M13 moveit tool render
  live data with zero tool changes.
- **B5 end-to-end demo**: dataflow `simple_planner → studio_bridge` with
  Studio attached: paths/target/costmap update live while the dataflow
  runs.
- **B6 Motion Planner console wiring** (module-role decision 2026-08-14):
  MotionPlannerView evolves from placeholder to the planning *console* —
  planner algorithm implantation UI (select/configure the planner node of
  the running dataflow), goal/target setting, Plan/Execute/Stop wired to
  real moveit requests, scene object management (add/remove planning-scene
  obstacles). Its 3D part stays a read-only mirror; all rendering/sim
  presentation stays in the Visualization viewport. Each module keeps an
  irreplaceable role: console vs renderer.

### Phase C — physics sim mirror + avoidance (1.0 wk)

- **C1 mujoco mirror**: dataflow adds `dora-mujoco`; `joint_positions`
  (already parsed by M13) stream through the bridge into
  NanoRobotViewer — the robot moves per physics in the viewport.
- **C2 avoidance**: complete collision checking in dora-moveit2 (external
  repo, ~small FCL integration) OR demo avoidance with a user algorithm
  consuming the M12 costmap.
- **C3 realistic acceptance**: robot follows planned path in viewport with
  physics-accurate joint states; obstacles from the planning scene render
  in 3D (M13 D3); avoidance visibly works in sim.

## Acceptance Criteria

- [ ] Phase B: real dora dataflow runs on this machine (python3.11 env)
- [ ] Live paths/target render in viewport with <200ms latency, no tool changes
- [ ] Phase C: robot in viewport mirrors dora-mujoco physics at ≥20Hz
- [ ] Avoidance demonstrable: path visibly avoids scene obstacles
- [ ] No new Rust dependencies in backend; no viewport architecture change
- [ ] Motion Planner page: Plan/Execute/Stop issues real requests; planner
  selection (algorithm implantation) works end-to-end

## Risks

| Risk | Prob | Mitigation |
|------|------|-----------|
| dora pip/CLI version mismatch not fixable via uv | Medium | spike B1 first; fallback: run dataflow on a remote/container host, bridge over HTTP |
| SSE/frontend polling latency | Low | localhost; 50ms tick tolerates 100ms polling |
| moveit2 collision stub blocks C2 | Medium | FCL integration in local dora-moveit2 repo (student owns both repos) |
| Scope creep beyond GSoC deadline | Medium | B is the milestone; C only if time remains after M13+M14 |

## Revision B6 feedback (2026-08-16, student decisions)

- **B6 delivered**: live command channel (backend queue + studio_console
  node + console-first planner semantics + Motion Planner console UI).
  Student-verified: Plan/Execute/Stop/Auto/scene commands, smoothed A*
  paths, mode pill, command feedback.
- **Dedicated arm viewer (NEW, deferred)**: the console mirror must NOT
  share the MoveIt tool with the Visualization viewport — one tool
  instance, one scene, whichever page attaches first wins and the other
  breaks (observed: Visualization's nano was replaced by B601). Module
  roles confirmed by student: **Motion Planner = arm viewer (robot arm,
  joint states, EE)**, **Visualization = base/chassis motion view**.
  Plan: a dedicated arm viewer component for the Motion Planner page
  that reuses the moveit URDF loader modules (tools/moveit/urdf: meshes
  loadUrdfRobot, robot buildRobotModel, parseUrdf) directly — no tool
  registry involvement — with live joint_positions driving plus model
  selector (models/* discovery already exists via GET /api/models).
  Nano-model live joint driving is also reverted (B601 angles fold the
  nano arm); the mirror currently shows the static nano seed pose.
- **Path smoothing**: greedy line-of-sight simplification after A*
  (planner emits clean polylines; detours preserved).

## Phase C Implementation Plan (Revision C, 2026-08-16)

Student decisions (2026-08-16): ur5e arm demo via the moveit pipeline;
un-stub the dora-moveit2 collision check (no FCL needed — the repo already
has a pure-numpy collision lib + SimpleFK); new `examples/moveit-live-demo/`
directory (B5 demo untouched); LiveFeed poll 100ms → 50ms to meet the
≥20Hz acceptance.

### Recon findings (verified 2026-08-16)

- `dora-mujoco` is NOT installed in the M15 venv. It ships a console script
  (`dora-mujoco = dora_mujoco.main:main`) — the YAML pattern is
  `path: dora-mujoco` (see its demo.yml). Deps: mujoco>=3.1.6 +
  robot_descriptions. Entry point `dora_mujoco/main.py`: subscribes
  `control_input` (joint position targets, 0.2s cmd timeout), steps once per
  INPUT event, outputs `joint_positions` (float64 qpos, metadata
  `encoding: jointstate`), `joint_velocities`, `actuator_controls`,
  `sensor_data`. Launches a passive mujoco viewer (needs a display).
- Collision stub: `planner_ompl_with_collision_op.py:174`
  (`is_state_valid` → `return True  # Temporarily disable all collision
  checking`). The planner class already holds `self.fk = SimpleFK(num_joints)`
  (line 138) and `self.collision_checker` with
  `set_robot_links(links)` (line 152). `collision_lib.py` has a complete
  primitive checker (`is_state_valid(link_transforms)` →
  `check_robot_self_collision` + `check_robot_environment_collision`,
  sphere/box/cylinder, pure numpy). ur5e config has NUM_JOINTS=6 +
  LINK_TRANSFORMS (`examples/move_group_demo/move_group_demo/config/ur5e.py`).
- dora_moveit is editable-installed from `/home/dora/dora-moveit2/dora_moveit`
  — external-repo edits take effect immediately. `move_group_demo` is NOT
  installed; ROBOT_CONFIG_MODULE is a dotted module path.
- Wire formats: `plan_request` = JSON `{"start": [...], "goal": [...],
  "planner": "rrt_connect", "max_time": 5.0}`; `ik_request` = float32 array
  `[x, y, z, roll, pitch, yaw]`; `scene_command` = JSON
  `{"action": "add|remove|clear", "object": {"name", "type": "box"|"sphere",
  "position", "dimensions"|"radius", "color"?}}` (move_group.py:645,
  planning_scene_op.py:420).
- Studio side needs almost nothing: `studio_bridge` auto-subscribes whatever
  is listed in its `inputs`; MoveItTool already drives the loaded URDF model
  from live `joint_positions` (MoveItTool.ts:290-298); LiveFeed interval is
  a single constant `intervalMs = 100` (live-feed.ts:58).

### Task C-1: Environment spike (no commit)

- [ ] Install dora-mujoco into the venv:
      `/home/dora/.venvs/dora-studio/bin/pip install -e /home/dora/dora-moveit/dora-mujoco`
      (pip resolves robot_descriptions from PyPI; the `[tool.uv.sources]` git
      pin only applies to uv). Expected: `dora-mujoco` console script in
      `/home/dora/.venvs/dora-studio/bin/`.
- [ ] Verify imports:
      `/home/dora/.venvs/dora-studio/bin/python -c "import mujoco, robot_descriptions; print(mujoco.__version__)"`
- [ ] Check what robot_descriptions provides for ur5e:
      `/home/dora/.venvs/dora-studio/bin/python -c "from robot_descriptions import ur5e_description, ur5e_mj_description; print(ur5e_description.URDF_PATH); print(ur5e_mj_description.XML_SCENE)"`
      Record paths; these feed Task C-3 (viewport URDF) and the dataflow
      MODEL_NAME. If `ur5e_mj_description` is absent, fall back to copying
      `/home/dora/dora-moveit2/examples/move_group_demo/models/ur5e.xml` +
      `ur5e_assets/` into `examples/moveit-live-demo/models/` (check the
      dora-moveit2 license before committing assets; otherwise keep the
      robot_descriptions MJCF name in MODEL_NAME).
- [ ] Headless smoke test (no display needed):
      `/home/dora/.venvs/dora-studio/bin/python -c "import mujoco; m = mujoco.MjModel.from_xml_path('/home/dora/dora-moveit2/examples/move_group_demo/models/ur5e.xml'); d = mujoco.MjData(m); mujoco.mj_step(m, d); print('nq', m.nq, 'nu', m.nu)"`
      Expected: nq 6, nu 6.

### Task C-2: Un-stub collision checking (external repo /home/dora/dora-moveit2, TDD)

Files: modify `dora_moveit/dora_moveit/motion_planner/planner_ompl_with_collision_op.py`;
create `/home/dora/dora-moveit2/tests/test_planner_collision.py` (unittest,
no pytest dependency). Commits go to the dora-moveit2 repo (English, no AI
signature).

- [ ] Step 1: Read the planner class init (lines ~100-160 of
      planner_ompl_with_collision_op.py) and record: class name, init
      signature, how robot links are built (create_robot_link calls),
      SimpleFK init arg. Also read `create_scene_with_obstacles.py` if links
      come from there.
- [ ] Step 2: Write the failing test (sketch — adjust class/init names per
      Step 1):

```python
# /home/dora/dora-moveit2/tests/test_planner_collision.py
import unittest
import numpy as np

from dora_moveit.motion_planner.planner_ompl_with_collision_op import OMPLPlanner  # per Step 1


class TestPlannerCollision(unittest.TestCase):
    def setUp(self):
        self.planner = OMPLPlanner(num_joints=6)  # per Step 1 signature
        # Block the arm's workspace with a big box in front of the base.
        from dora_moveit.collision_detection.collision_lib import create_box
        self.planner.add_obstacle(create_box(
            "wall", np.array([0.4, 0.0, 0.3]), np.array([0.1, 1.0, 1.0])))

    def test_config_inside_obstacle_is_invalid(self):
        # Elbow-forward config pushes links toward x=+0.4.
        bad = np.array([0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        # A config clearly inside the box (validated by link transforms).
        self.assertFalse(self.planner.is_state_valid(bad))

    def test_clear_config_is_valid(self):
        clear = np.array([0.0, -1.57, 1.57, 0.0, 0.0, 0.0])
        self.assertTrue(self.planner.is_state_valid(clear))

    def test_motion_through_obstacle_is_invalid(self):
        a = np.array([0.0, -1.57, 1.57, 0.0, 0.0, 0.0])
        b = np.array([0.0, 1.57, -1.57, 0.0, 0.0, 0.0])
        # Straight interpolation sweeps the arm through the wall region.
        self.assertFalse(self.planner.is_motion_valid(a, b, resolution=0.1))


if __name__ == "__main__":
    unittest.main()
```

      NOTE: the exact valid/invalid configs depend on the link geometry from
      Step 1 — pick configs whose FK link positions provably intersect the
      obstacle (print `self.fk.compute_link_transforms(cfg)` while writing
      the test to confirm). The test above is a template; finalize values
      with the printed transforms so the assertions are grounded, not lucky.
- [ ] Step 3: Run to confirm it fails with the stub:
      `cd /home/dora/dora-moveit2 && /home/dora/.venvs/dora-studio/bin/python -m unittest tests.test_planner_collision -v`
      Expected: 3 failures — `is_state_valid` returns True for everything.
- [ ] Step 4: Implement — replace the stub body:

```python
    def is_state_valid(self, config: np.ndarray) -> bool:
        link_transforms = self.fk.compute_link_transforms(config)
        valid, _ = self.collision_checker.is_state_valid(link_transforms)
        return valid
```

      (Joint-limit checks stay out of scope — sampling already respects
      limits; only collision is re-enabled.)
- [ ] Step 5: Run the tests again — all 3 pass. Also run the repo's existing
      tests if any: `cd /home/dora/dora-moveit2 && /home/dora/.venvs/dora-studio/bin/python -m unittest discover -s tests -v`
- [ ] Step 6: Commit in dora-moveit2 (English, no AI signature):
      `git add dora_moveit/dora_moveit/motion_planner/planner_ompl_with_collision_op.py tests/test_planner_collision.py`
      `git commit -m "fix: enable collision checking in planner is_state_valid"`
- [ ] Step 7: PAUSE — let the student manually verify (run the move_group_demo
      example with an obstacle and confirm the plan detours).

### Task C-3: examples/moveit-live-demo/ (studio repo, TDD)

New directory layout:

```
examples/moveit-live-demo/
  pyproject.toml                  # package moveit_live_demo (pip install -e)
  dataflow.yml
  moveit_live_demo/__init__.py
  moveit_live_demo/config/__init__.py
  moveit_live_demo/config/ur5e.py     # copy of move_group_demo config ur5e.py
  nodes/__init__.py
  nodes/planner.py                # wrapper: from dora_moveit.motion_planner.planner_ompl_with_collision_op import main; main()
  nodes/ik_solver.py              # wrapper: from dora_moveit.ik_solver.ik_op import main; main()
  nodes/planning_scene.py         # wrapper: from dora_moveit.motion_planner.planning_scene_op import main; main()
  nodes/gated_trajectory_executor.py   # adapted moveit executor + console gate
  nodes/moveit_console.py         # console → ik/plan/scene/gate mapping
  nodes/test_gated_executor.py
  nodes/test_moveit_console.py
```

Steps:

- [ ] Step 1: pyproject.toml (copy the move_group_demo pyproject shape,
      rename to moveit_live_demo, no dependencies beyond dora-rs — dora_moveit
      is already editable-installed):

```toml
[project]
name = "moveit-live-demo"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = ["dora-rs >= 0.5.0", "numpy", "pyarrow"]

[build-system]
requires = ["setuptools>=61"]
build-backend = "setuptools.build_meta"

[tool.setuptools]
packages = ["moveit_live_demo", "moveit_live_demo.config"]
```

- [ ] Step 2: copy `config/ur5e.py` from
      `/home/dora/dora-moveit2/examples/move_group_demo/move_group_demo/config/ur5e.py`
      (record the dora-moveit2 license in a README note if it has one; the
      example package is part of the student-owned repo).
- [ ] Step 3: the three wrapper nodes (3 lines each, as sketched above).
- [ ] Step 4: install the package editable into the venv:
      `/home/dora/.venvs/dora-studio/bin/pip install -e examples/moveit-live-demo`
      then `/home/dora/.venvs/dora-studio/bin/python -c "import moveit_live_demo.config.ur5e"`
- [ ] Step 5: `moveit_console.py` — TDD. First the pure functions
      (test_moveit_console.py), then the event loop (mirror the B6
      studio_console polling protocol: GET `{backend}/api/live/command`
      with `since_seq`/`next_seq`; see examples/live-demo/nodes/studio_console.py).

```python
# nodes/moveit_console.py
"""Console node for the moveit live demo.

Maps Studio console commands (B6 protocol) to moveit requests:
  plan    -> target_point [x,y,z] or [x,y,z,roll,pitch,yaw]
             -> ik_request (float32 pose) -> ik_solution
             -> plan_request {start: current mujoco joints, goal: ik solution}
  execute -> execute_command to the gated executor
  stop    -> stop_command to the gated executor
  auto    -> resume_command to the gated executor (auto-execute new plans)
  scene   -> scene_command to planning_scene
"""
import json
import urllib.request

import numpy as np
import pyarrow as pa
from dora import Node

BACKEND_URL = "http://127.0.0.1:3001/api/live"


def parse_target(text):
    """'x y z' (pad roll/pitch/yaw=0) or 'x y z r p y' -> 6 floats, else None."""
    if not isinstance(text, str):
        return None
    parts = text.replace(",", " ").split()
    if len(parts) not in (3, 6):
        return None
    try:
        values = [float(p) for p in parts]
    except ValueError:
        return None
    if len(values) == 3:
        values += [0.0, 0.0, 0.0]
    return values


def build_ik_request(pose):
    """6D pose [x,y,z,roll,pitch,yaw] -> float32 list for ik_request."""
    return [float(v) for v in pose]


def build_plan_request(start, goal, planner="rrt_connect", max_time=5.0):
    return {
        "start": [float(v) for v in start],
        "goal": [float(v) for v in goal],
        "planner": planner,
        "max_time": float(max_time),
    }


def build_scene_command(action, obj):
    return {"action": action, "object": obj}


def build_gate_command(kind):
    return {"kind": kind}


def parse_ik_solution(payload):
    """float32 array payload -> list of joint angles, else None."""
    try:
        arr = payload.to_numpy()
    except Exception:
        return None
    if arr.size < 6:
        return None
    return arr.tolist()
```

      Event-loop behavior (implemented after the pure-function tests pass):
      poll the command queue; `plan` sends ik_request and remembers the
      pending target; on `ik_solution` input build plan_request from the
      latest `joint_positions` (mujoco_sim) and the solution; `execute` /
      `stop` / `auto` forward gate commands; `scene` forwards to
      planning_scene via scene_command. All sends use
      `pa.array(..., type=pa.float32())` for ik and JSON bytes for dicts
      (same encodings as move_group.py).
- [ ] Step 6: run the console tests:
      `cd examples/moveit-live-demo/nodes && /home/dora/.venvs/dora-studio/bin/python -m unittest test_moveit_console -v`
- [ ] Step 7: `gated_trajectory_executor.py` — TDD. Step 7a: read
      `/home/dora/dora-moveit2/dora_moveit/dora_moveit/trajectory_execution/`
      (the executor op source) and record the execution loop + HOME-idle
      behavior. Step 7b: write the gate state machine as a pure class with
      tests:

```python
class ExecutionGate:
    """Console-controlled execution gate for the moveit executor.

    States: IDLE (no trajectory), READY (trajectory received, awaiting
    execute), RUNNING, AUTO (resume: every new trajectory starts at once).
    """

    def __init__(self):
        self.auto = False
        self.pending = None
        self.running = False

    def on_trajectory(self, trajectory):
        self.pending = trajectory
        if self.auto:
            return self.on_execute()
        self.running = False
        return False

    def on_execute(self):
        if self.pending is None or self.running:
            return False
        self.running = True
        return True

    def on_stop(self):
        self.running = False
        return True

    def on_resume(self):
        self.auto = True
        return self.on_execute()

    def on_complete(self):
        self.running = False
        if self.auto:
            return self.on_execute()
        return False
```

      Step 7c: the node adapts the moveit executor loop: subscribe
      `trajectory` (planner), `joint_positions` (mujoco), `tick`
      (dora/timer/millis/50), `execute_command`/`stop_command`/
      `resume_command` (console); on trajectory → gate.on_trajectory; on
      execute/stop/resume → gate; while gate.running emit interpolated
      `joint_commands` per tick (same interpolation as the moveit executor);
      when idle emit HOME commands only if the moveit executor does
      (match its behavior); `execution_status` JSON per the M13-audited
      5-key format.
- [ ] Step 8: run gate tests:
      `cd examples/moveit-live-demo/nodes && /home/dora/.venvs/dora-studio/bin/python -m unittest test_gated_executor -v`
- [ ] Step 9: dataflow.yml (paths relative to the dataflow file; the bridge
      lists every stream the tools consume):

```yaml
# M15 Phase C — live planning + mujoco physics mirror demo.
#
# REAL: dora-moveit2 planner (collision-checked RRT) plans arm trajectories,
# the gated executor drives dora-mujoco (ur5e), and joint_positions flow
# through studio_bridge into the Studio viewport (≥20Hz).
# Console: the Motion Planner page sends plan/execute/stop/auto/scene
# commands; plan targets are [x y z] or [x y z roll pitch yaw] (meters/rad).
#
# Run (M15 venv active, from this directory):
#   dora up
#   dora start dataflow.yml --name moveit-live
# Then in Studio: Visualization -> Live -> enable Live Feed -> mount the
# MoveIt tool -> pick the ur5e model; Motion Planner page drives the arm.
#
# Tests: /home/dora/.venvs/dora-studio/bin/python -m unittest \
#   test_moveit_console test_gated_executor   (from nodes/)

nodes:
  - id: mujoco_sim
    path: dora-mujoco
    inputs:
      tick: dora/timer/millis/10
      control_input: gated_executor/joint_commands
    outputs:
      - joint_positions
      - joint_velocities
      - actuator_controls
      - sensor_data
    env:
      MODEL_NAME: ur5e_mj_description   # or models/ur5e.xml per Task C-1
      MUJOCO_GL: "glfw"

  - id: planning_scene
    path: nodes/planning_scene.py
    inputs:
      robot_state: mujoco_sim/joint_positions
      scene_command: moveit_console/scene_command
      tick: dora/timer/millis/5000
    outputs:
      - scene_update
      - command_result
    env:
      ROBOT_CONFIG_MODULE: "moveit_live_demo.config.ur5e"

  - id: planner
    path: nodes/planner.py
    inputs:
      plan_request: moveit_console/plan_request
      scene_update: planning_scene/scene_update
    outputs:
      - trajectory
      - plan_status
    env:
      ROBOT_CONFIG_MODULE: "moveit_live_demo.config.ur5e"

  - id: ik_solver
    path: nodes/ik_solver.py
    inputs:
      ik_request: moveit_console/ik_request
      joint_state: mujoco_sim/joint_positions
    outputs:
      - ik_solution
      - ik_status
    env:
      ROBOT_CONFIG_MODULE: "moveit_live_demo.config.ur5e"

  - id: gated_executor
    path: nodes/gated_trajectory_executor.py
    inputs:
      trajectory: planner/trajectory
      joint_positions: mujoco_sim/joint_positions
      execute_command: moveit_console/execute_command
      stop_command: moveit_console/stop_command
      resume_command: moveit_console/resume_command
      tick: dora/timer/millis/50
    outputs:
      - joint_commands
      - execution_status
    env:
      ROBOT_CONFIG_MODULE: "moveit_live_demo.config.ur5e"

  - id: moveit_console
    path: nodes/moveit_console.py
    inputs:
      joint_positions: mujoco_sim/joint_positions
      ik_solution: ik_solver/ik_solution
      ik_status: ik_solver/ik_status
    outputs:
      - target_point
      - plan_request
      - ik_request
      - execute_command
      - stop_command
      - resume_command
      - scene_command
    env:
      STUDIO_BACKEND_URL: http://127.0.0.1:3001/api/live

  - id: studio_bridge
    path: ../../backend/scripts/studio_bridge.py
    inputs:
      joint_positions: mujoco_sim/joint_positions
      joint_velocities: mujoco_sim/joint_velocities
      joint_commands: gated_executor/joint_commands
      execution_status: gated_executor/execution_status
      trajectory: planner/trajectory
      plan_status: planner/plan_status
      scene_update: planning_scene/scene_update
      ik_solution: ik_solver/ik_solution
      ik_status: ik_solver/ik_status
    env:
      STUDIO_BACKEND_URL: http://127.0.0.1:3001/api/live/ingest
```

- [ ] Step 10: viewport model. Copy the ur5e URDF + meshes from
      robot_descriptions (Task C-1 recorded paths) into `models/ur5e/`
      (URDF at models/ur5e/ur5e.urdf or similar; meshes under
      models/ur5e/meshes/... so the M13 `package://` remap resolves).
      Confirm models/ur5e/ is covered by the existing models gitignore
      (b601 precedent) — add a line if not. Verify the MoveIt tool model
      selector lists ur5e (backend GET /api/models scans models/*/*.urdf).
      Verify the mujoco qpos joint order (from ur5e_mj_description or
      ur5e.xml) equals the URDF joint order (shoulder_pan, shoulder_lift,
      elbow, wrist_1, wrist_2, wrist_3); if not, document the mapping and
      reorder in the bridge? (Do NOT reorder — instead note it; canonical
      order is expected for both.)
- [ ] Step 11: commits (studio repo, English, no AI signature):
      1. `feat: add moveit-live-demo package skeleton and node wrappers (M15 C1)`
      2. `feat: add moveit console node with tests (M15 C1)`
      3. `feat: add gated trajectory executor with tests (M15 C1)`
      4. `feat: add moveit live planning dataflow with mujoco mirror (M15 C1)`
      (models/ur5e/ stays local — never `git add` it.)
- [ ] Step 12: PAUSE — let the student run the dataflow end-to-end.

### Task C-4: LiveFeed 50ms (frontend, TDD)

Files: `frontend/src/live-feed.ts` (+ `frontend/src/live-feed.test.ts` if it
asserts the 100ms interval).

- [ ] Step 1: read live-feed.ts around line 58 — change `intervalMs = 100`
      to `intervalMs = 50` (if it is a constructor default, update the
      constructor signature default too).
- [ ] Step 2: update live-feed.test.ts assertions that pin 100ms timing
      (fake timers advance 100 → 50) so the suite stays green.
- [ ] Step 3: run `npm --prefix frontend run test:tools` and
      `npm --prefix frontend run build`. Expected: all green (224 tools
      tests + build).
- [ ] Step 4: commit `feat: poll live feed at 50ms for 20Hz physics mirror (M15 C3)`
- [ ] Step 5: PAUSE — user verifies live motion smoothness in Studio.

### Task C-5: C3 acceptance + full regression

- [ ] Manual end-to-end (student, with display): dora up (VIRTUAL_ENV
      active) → start dataflow.yml → Studio Live mode → MoveIt tool + ur5e
      model → console: add box obstacle → Plan (path detours around the
      obstacle wireframe) → Execute (arm moves per physics, ≥20Hz, joint
      table live) → Stop returns HOME.
- [ ] Honesty check: every synthetic/adapted piece is labeled (dataflow
      header comments; no fabricated data).
- [ ] Full regression (baseline cannot drop):
      - `cargo test --manifest-path backend/Cargo.toml` (153 + 1)
      - `cd examples/live-demo/nodes && /home/dora/.venvs/dora-studio/bin/python -m unittest test_costmap_source test_simple_planner test_trajectory_executor test_studio_console` (46)
      - `cd examples/moveit-live-demo/nodes && /home/dora/.venvs/dora-studio/bin/python -m unittest test_moveit_console test_gated_executor` (new)
      - `npm --prefix frontend run test:tools` (224) + `npm --prefix frontend run build`
- [ ] Update HANDOFF-M15.txt / HANDOFF.md with the Phase C record (local
      files, never committed).

### Task C-1 results (2026-08-16, spike done)

- dora-mujoco installed editable into the M15 venv via
  `VIRTUAL_ENV=... uv pip install -e /home/dora/dora-moveit/dora-mujoco`
  (the venv has uv, NOT pip). Deps landed: mujoco 3.11.0,
  robot-descriptions 3.1.0, glfw, pyopengl. Console script `dora-mujoco`
  present.
- robot_descriptions lazy cache: `~/.cache/robot_descriptions/ur_description`
  cloned fully (URDF + meshes, BSD-3-Clause + UR mesh terms);
  `mujoco_menagerie` clone FAILED (network, 2424 files) — do not rely on
  `ur5e_mj_description`. Use the local MJCF instead.
- Headless smoke PASSED: ur5e.xml (dora-moveit2 examples) loads and steps.
  KEY LAYOUT: nq=21 = ball_joint(7) + arm(6) + gripper(8); arm qpos is
  `qpos[7:13]`; nu=7 with actuators 0-5 = the 6 arm joints (6-value
  joint_commands drive the arm correctly).
- CONSEQUENCE for the dataflow: every consumer that wants arm joints must
  slice qpos[7:13]. The move_group.py `_extract_arm_joints` 13-offset
  heuristic is hunter-specific and WRONG for this ur5e.xml (it would slice
  the gripper region) — do not reuse it. Add a `arm_joints` filter node
  (see Task C-3 revision below).
- Asset strategy (b601 precedent, local-only): copy the ur5e MJCF + assets
  (24K + 33MB) into examples/moveit-live-demo/models/ (gitignored) for
  dora-mujoco MODEL_NAME; copy the ur5e URDF + meshes from the cached
  ur_description into models/ur5e/ (gitignored) for the viewport loader.

### Task C-3 revision (2026-08-16): arm_joints filter node

Add `nodes/arm_joints.py` — subscribes `mujoco_sim/joint_positions` (21
values), outputs `joint_positions` (6 arm values, qpos[7:13]).
`ARM_JOINT_START` env (default 7) keeps the offset explicit. Tested with
`test_arm_joints.py` (pure slice helper: 21→6, short input → None,
offset env respected). Dataflow wiring changes:

- `planning_scene.robot_state: arm_joints/joint_positions` (6 values → the
  `len >= 20` branch in planning_scene_op never triggers; no external code
  change needed)
- `moveit_console.joint_positions: arm_joints/joint_positions` (plan start)
- `gated_executor.joint_positions: arm_joints/joint_positions`
- `studio_bridge.joint_positions: arm_joints/joint_positions` (MoveItTool
  applies first-N values — a raw 21-value stream would mis-apply the
  ball_joint qpos to the 6-joint model)
- The raw mujoco stream stays available as `joint_positions_raw` from
  mujoco_sim for honesty (bridge does NOT subscribe it).

### Task C-3 completion record (2026-08-16)

End-to-end verified via the backend live feed (curl): Plan → IK →
plan_request → collision-checked RRT (12 waypoints) → gated executor →
joint_commands → dora-mujoco physics → joint_positions → arm_joints
filter → studio_bridge → backend. The arm moved per physics and returned
HOME. Commits (studio repo): c75d601..8d17a07; dora-moveit2: 1a9dd61 +
ad1a1c5; dora-mujoco (bundle, no git): local fix.

Bugs found and fixed along the way:

- Backend live.rs `recent()` returned the OLDEST frames (ascending sort +
  truncate) — the API contract is newest-first; fixed with TDD (95fd6c3).
  This measurement artifact masqueraded as "routing failures" for most of
  the session; the daemon was fine.
- studio_bridge crashed on mujoco sim-time float timestamps
  (fromisoformat TypeError) — falls back to receive time (a565a6d).
- MoveItTool setRobot overwrote robotId before the loaded-model guard, so
  the model selector never reloaded (ee43a99).
- dora-moveit2: planner is_state_valid stubbed (1a9dd61); SimpleFK
  hardcoded GEN72 geometry ignoring config LINK_TRANSFORMS — folded poses
  self-collided and cut the joint space; create_robot_link 2cm padding
  inflated radii; planning_scene_op robot_state handler referenced
  undefined `config`; collision_lib prints referenced non-existent
  CollisionResult fields (ad1a1c5).
- dora-mujoco (bundle repo, local fix): 6-value commands on the 7-actuator
  ur5e model crashed step_simulation (broadcast error) — write
  min(len, nu) slots. load_model() runs twice (init + main).
- ur5e_example_mujoco.yml mujoco path (main_viewer.sh) does not exist —
  fixed to `path: dora-mujoco`.
- Left unfixed (documented): move_group.py `_extract_arm_joints` 13-offset
  heuristic is hunter-specific and slices the gripper region for ur5e
  (our demo sidesteps it with the arm_joints filter node).
- Console UX: box position is now editable (b9ca4c2); console holds the IK
  solution until joint state arrives (8d17a07).

Demo quirks (documented, honest):
- The demo config diverges from upstream ur5e: HOME/SAFE = upright zero
  pose (upstream folded home self-collides under the planner FK), base
  collision sphere 0.065 → 0.05.
- Every console restart replays the backend command history (B6 seq
  watermark design) — old plans re-run once at startup.
- B5 live-planning and moveit-live must not run simultaneously: both
  console nodes consume the shared backend command queue and the B5 demo
  joint_positions would drive the ur5e model in the viewport.

### Task C-3 follow-up fixes (2026-08-17, user testing feedback)

- Execution cut short: two causes. (a) The console replayed the whole
  command history on every restart (initial watermark = next_seq tripped
  the restart detection, since >= next -> reset 0) and old stop commands
  killed executions; fixed with next_seq-1 semantics (e13199f).
  (b) The interpolation ran 0.5s/waypoint — the damped mujoco arm lagged
  and turned HOME mid-path; slowed to ~1s/waypoint (demo divergence from
  the moveit executor's 0.1).
- Scene remove did nothing: the moveit planning scene reads the remove
  target from the top-level `name` while B6 nests it under `object`;
  the console now normalizes (e13199f). Add already worked; the default
  box position floated above the arm (z=0.36 vs the real-FK link plane
  z~0.16) — default moved to [0.45, 0.15, 0.16].
- Box position row overflowed the 340px panel — stacked layout
  (.console-field-stack).
- CONFIRMED dora daemon bug (community issue material): event routing
  degrades with daemon uptime + repeated dataflow starts — a node's
  outputs stop reaching some subscribers while other routes keep working
  (observed twice: the Aug-15 daemon after a day of starts, and the
  Aug-16 daemon after ~18h + ~10 starts). `dora destroy && dora up`
  restores routing. Symptom check: executor ticks but mujoco never
  receives joint_commands (arm frozen) while arm_joints -> bridge still
  flows.

### Known limitation (deferred, 2026-08-17, user decision)

The planner's collision model is one sphere per link (config
COLLISION_GEOMETRY) driven by the DH link transforms — it covers the
real link volumes only approximately. Demonstrated case: obstacle at
(0.45, -0.4, 0.16), target (-0.1, 0.5, 0.2) — the gripper tail clears the
obstacle but the joint-3 arm (elbow/forearm) penetrates it in the mujoco
simulation while the planner's spheres miss it. User deferred this as
complex. Future options (ranked): (1) per-link capsule/box collision
geometry driven by the robot config, with a per-robot link-index audit
(which sphere covers which real volume); (2) FCL integration. Recorded
for the post-GSoC backlog; the demo remains honest about the simplified
collision model.

## Acceptance Criteria — final status (2026-08-17)

- [x] Phase B: real dora dataflow runs on this machine (python3.11 env)
- [x] Live paths/target render in viewport with <200ms latency, no tool changes
- [x] Phase C: robot in viewport mirrors dora-mujoco physics at ≥20Hz
      (50ms LiveFeed; mujoco emits at 100Hz)
- [x] Avoidance demonstrable: path visibly avoids scene obstacles
      (collision-checked RRT; known limitation: one sphere per link,
      real volumes only approximated — deferred per student decision)
- [x] No new Rust dependencies in backend; no viewport architecture change
- [x] Motion Planner page: Plan/Execute/Stop issues real requests; planner
      selection works end-to-end
