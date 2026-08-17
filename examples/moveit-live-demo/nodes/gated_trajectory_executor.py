"""gated_trajectory_executor — console-gated trajectory execution (M15 C1).

Adapted from dora-moveit2's trajectory_executor
(dora_moveit/trajectory_execution/trajectory_executor.py); the execution
loop and interpolation are unchanged. The addition is an ExecutionGate:
trajectories from the planner are held until the console sends an execute
command (or resume/auto mode starts them immediately), so the Motion
Planner console keeps its B6 semantics: Plan plans once, Execute runs,
Stop returns HOME, Auto auto-executes new plans.

Idle behavior matches the original: HOME commands keep the arm stable.
"""

import json
from typing import List, Optional

import numpy as np
import pyarrow as pa
from dora import Node
from dora_moveit.config import load_config


def reshape_trajectory(flat, num_waypoints, num_joints):
    """Flat array -> list of per-waypoint joint arrays, or None if the
    length does not divide evenly."""
    arr = np.asarray(flat, dtype=np.float64)
    n_way = num_waypoints if num_waypoints is not None else len(arr) // num_joints
    if n_way <= 0 or n_way * num_joints != len(arr):
        return None
    return [arr[i * num_joints:(i + 1) * num_joints] for i in range(n_way)]


class ExecutionGate:
    """Console-controlled execution gate.

    on_trajectory / on_execute / on_resume return the trajectory to
    execute (consuming the pending slot) or None. Manual mode holds a new
    trajectory until execute; auto mode starts every new one at once.
    """

    def __init__(self):
        self.auto = False
        self.pending = None
        self.running = False

    def on_trajectory(self, trajectory):
        self.pending = trajectory
        if self.auto:
            return self._start()
        self.running = False
        return None

    def on_execute(self):
        if self.running:
            return None
        return self._start()

    def on_stop(self):
        self.running = False
        return True

    def on_resume(self):
        self.auto = True
        return self._start()

    def on_complete(self):
        self.running = False
        if self.auto:
            return self._start()
        return None

    def _start(self):
        trajectory = self.pending
        if trajectory is None:
            return None
        self.pending = None
        self.running = True
        return trajectory


class TrajectoryExecutor:
    """Execution loop copied from dora-moveit2 (see module docstring)."""

    def __init__(self, num_joints: int = 7):
        self.num_joints = num_joints
        self._home_config = load_config().HOME_CONFIG
        self.trajectory: List[np.ndarray] = []
        self.current_waypoint_idx = 0
        self.prev_waypoint: Optional[np.ndarray] = None

        self.interpolation_progress = 0.0
        # Demo divergence from the moveit executor (0.1): 0.05 gives the
        # heavily-damped mujoco arm ~1s per waypoint so the physics can
        # track the commands instead of lagging behind the interpolation.
        self.interpolation_speed = 0.05

        self.is_executing = False
        self.execution_count = 0

        self.current_joints: Optional[np.ndarray] = None
        self.last_command: Optional[np.ndarray] = None

    def set_trajectory(self, trajectory: List[np.ndarray]):
        self.trajectory = trajectory
        self.interpolation_progress = 0.0
        self.is_executing = True
        self.execution_count += 1

        if len(trajectory) > 0:
            self.prev_waypoint = trajectory[0]
            self.current_waypoint_idx = 1 if len(trajectory) > 1 else 0
            self.last_command = trajectory[0].copy()
            print(f"[GatedExecutor] New trajectory with {len(trajectory)} waypoints")

    def stop(self):
        self.is_executing = False
        self.trajectory = []
        self.prev_waypoint = None

    def update_current_joints(self, joints: np.ndarray):
        self.current_joints = joints[:self.num_joints].copy()

    def step(self) -> Optional[np.ndarray]:
        """One interpolation step; HOME while idle (original behavior)."""
        if not self.is_executing or len(self.trajectory) == 0:
            return self._home_config.copy()

        if self.prev_waypoint is None:
            return self._home_config.copy()

        target = self.trajectory[self.current_waypoint_idx]
        self.interpolation_progress += self.interpolation_speed

        if self.interpolation_progress >= 1.0:
            self.prev_waypoint = target
            self.current_waypoint_idx += 1
            self.interpolation_progress = 0.0

            if self.current_waypoint_idx >= len(self.trajectory):
                self.is_executing = False
                print(f"[GatedExecutor] Trajectory #{self.execution_count} complete!")

                if self.current_joints is not None:
                    self.last_command = self.current_joints.copy()
                    return self.current_joints.copy()

                return self.last_command

            target = self.trajectory[self.current_waypoint_idx]

        t = min(self.interpolation_progress, 1.0)
        command = self.prev_waypoint + t * (target - self.prev_waypoint)
        self.last_command = command.copy()
        return command

    def get_status(self) -> dict:
        return {
            "is_executing": self.is_executing,
            "execution_count": self.execution_count,
            "current_waypoint": self.current_waypoint_idx,
            "total_waypoints": len(self.trajectory),
            "progress": self.interpolation_progress,
        }


def main():
    print("=== Gated Trajectory Executor (M15 C1) ===")

    node = Node()
    config = load_config()
    executor = TrajectoryExecutor(num_joints=config.NUM_JOINTS)
    gate = ExecutionGate()

    executor.current_joints = config.SAFE_CONFIG.copy()
    executor.last_command = config.SAFE_CONFIG.copy()
    print(f"Initialized with {config.NUM_JOINTS}-DOF safe config")

    first_tick = True

    for event in node:
        if event["type"] == "INPUT":
            input_id = event["id"]

            if input_id in ("trajectory", "cartesian_trajectory"):
                try:
                    traj_flat = event["value"].to_numpy()
                    metadata = event.get("metadata", {})
                    num_waypoints = metadata.get("num_waypoints")
                    num_joints = metadata.get("num_joints", executor.num_joints)
                    trajectory = reshape_trajectory(traj_flat, num_waypoints, num_joints)
                    if trajectory is None:
                        print(f"[GatedExecutor] Trajectory length {len(traj_flat)} "
                              f"not divisible by {num_joints}, skipped")
                    else:
                        if executor.current_joints is not None:
                            trajectory.insert(0, executor.current_joints.copy())
                        to_run = gate.on_trajectory(trajectory)
                        if to_run is not None:
                            executor.set_trajectory(to_run)
                except Exception as e:
                    print(f"[GatedExecutor] Trajectory error: {e}")

            elif input_id == "joint_positions":
                executor.update_current_joints(event["value"].to_numpy())

            elif input_id == "execute_command":
                to_run = gate.on_execute()
                if to_run is not None:
                    executor.set_trajectory(to_run)
                else:
                    print("[GatedExecutor] Execute: nothing pending")

            elif input_id == "stop_command":
                gate.on_stop()
                executor.stop()
                print("[GatedExecutor] Stopped, returning HOME")

            elif input_id == "resume_command":
                to_run = gate.on_resume()
                if to_run is not None:
                    executor.set_trajectory(to_run)
                print("[GatedExecutor] Auto mode on")

            elif input_id == "tick":
                try:
                    if first_tick:
                        node.send_output(
                            "joint_commands",
                            pa.array(executor.last_command, type=pa.float32()),
                        )
                        first_tick = False

                    command = executor.step()

                    if command is not None:
                        node.send_output(
                            "joint_commands",
                            pa.array(command, type=pa.float32()),
                        )

                    status_bytes = json.dumps(executor.get_status()).encode("utf-8")
                    node.send_output(
                        "execution_status",
                        pa.array(list(status_bytes), type=pa.uint8()),
                    )
                except Exception as e:
                    print(f"[GatedExecutor] Error in tick: {e}")

        elif event["type"] == "STOP":
            print("Gated executor stopping...")
            break


if __name__ == "__main__":
    main()
