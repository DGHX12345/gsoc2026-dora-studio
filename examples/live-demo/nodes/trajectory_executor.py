"""trajectory_executor — demo arm executor for the live demo (M15 B5).

Honest demo stand-in for moveit2's trajectory_executor + mujoco pair:
on each 50ms tick it quintic-interpolates the arm pose toward the
current planner target and emits joint_commands / joint_positions /
execution_status. joint_positions ECHO the commands (there is no
physics feedback in this dataflow — labeled demo, not simulated
physics; the real dora-mujoco mirror is Phase C).

The arm pose rule is real kinematics-by-direction (azimuth tracking),
not full IK: q1 = 0.5 * atan2(target_y, target_x), the wrist sweeps
slowly so the model visibly moves.
"""

import json
import math
import time

import pyarrow as pa
from dora import Node

TICK_S = 0.05
MOVE_DURATION_S = 1.5
# Nano-model-friendly home: the mirror renders the NANO robot, so the
# demo poses stay well inside plausible nano arm ranges.
HOME = [0.0, 0.4, -0.8, 0.8, 0.4, 0.2, 0.03]


def quintic_step(s, q0, q1):
    """Quintic (zero end velocity/acceleration) interpolation."""
    blend = 10 * s**3 - 15 * s**4 + 6 * s**5
    return [a + (b - a) * blend for a, b in zip(q0, q1)]


def pose_toward_target(target_xy):
    """7-joint pose tracking the target azimuth with gentle nano-friendly
    ranges (the mirror renders the NANO model — B601-scale angles would
    fold it into impossible poses)."""
    x, y = target_xy
    azimuth = 0.3 * math.atan2(y, x)
    return [
        azimuth,
        0.4 + 0.1 * math.sin(time.time()),
        -0.8 + 0.15 * math.cos(time.time() * 0.7),
        0.8,
        0.4,
        0.2,
        0.03,
    ]


def home_return(q_current):
    """The Stop behavior: a quintic move from the current pose back to
    HOME (not an instant hold — the console expects Stop to return the
    arm to its starting pose)."""
    return plan_samples(q_current, HOME, MOVE_DURATION_S, TICK_S)


def plan_samples(q_from, q_to, duration_s, tick_s):
    """Sample the quintic move from q_from to q_to, endpoints included."""
    n = max(1, round(duration_s / tick_s))
    return [quintic_step(k / n, q_from, q_to) for k in range(n + 1)]


def progress_fraction(step_index, total_steps):
    if total_steps <= 0:
        return 0.0
    return min(1.0, step_index / total_steps)


def parse_execute_command(payload):
    """B6: {"command": "execute"|"stop"} -> command string, else None."""
    if not isinstance(payload, dict):
        return None
    command = payload.get("command")
    if command in ("execute", "stop"):
        return command
    return None


def trajectory_envelope(samples):
    """The {waypoints: [[q...], ...]} JSON envelope (M13 port-collision
    policy: unambiguous against dviz xyz paths on the trajectory port)."""
    return {"waypoints": samples}


def main():
    node = Node()
    target = (0.55, 0.0)
    q_current = list(HOME)
    samples = plan_samples(HOME, pose_toward_target(target), MOVE_DURATION_S, TICK_S)
    step = 0
    execution_count = 1
    node.send_output(
        "trajectory",
        pa.array(
            list(json.dumps(trajectory_envelope(samples)).encode()), type=pa.uint8()
        ),
        {"num_waypoints": len(samples), "num_joints": 7},
    )
    while True:
        event = node.try_recv()
        if event is not None and event.get("type") == "STOP":
            break
        if event is not None and event.get("type") == "INPUT":
            if event["id"] == "target":
                value = event.get("value")
                if value is not None:
                    arr = value.to_pylist()
                    if len(arr) >= 2:
                        new_target = (float(arr[0]), float(arr[1]))
                        if math.hypot(new_target[0] - target[0], new_target[1] - target[1]) > 0.02:
                            target = new_target
                            samples = plan_samples(
                                q_current, pose_toward_target(target), MOVE_DURATION_S, TICK_S
                            )
                            step = 0
                            execution_count += 1
                            # The planned joint path for this move (MoveIt EE
                            # path), envelope form per the M13 port policy.
                            envelope = trajectory_envelope(samples)
                            node.send_output(
                                "trajectory",
                                pa.array(
                                    list(json.dumps(envelope).encode()), type=pa.uint8()
                                ),
                                {"num_waypoints": len(samples), "num_joints": 7},
                            )
            elif event["id"] == "execute":
                value = event.get("value")
                if value is not None:
                    try:
                        command = json.loads(bytes(value.to_pylist()).decode("utf-8"))
                    except (UnicodeDecodeError, json.JSONDecodeError):
                        command = None
                    parsed = parse_execute_command(command)
                    if parsed == "stop":
                        # Stop returns the arm to HOME, then holds.
                        samples = home_return(q_current)
                        step = 0
                    elif parsed == "execute" and not samples:
                        samples = plan_samples(
                            q_current, pose_toward_target(target), MOVE_DURATION_S, TICK_S
                        )
                        step = 0
                        execution_count += 1

        if not samples:
            # Stopped (console Stop): hold the pose, report idle honestly.
            node.send_output("joint_commands", pa.array(q_current))
            node.send_output("joint_positions", pa.array(q_current))
            status = {
                "is_executing": False,
                "current_waypoint": 0,
                "progress": 1.0,
                "execution_count": execution_count,
                "total_waypoints": 0,
            }
            node.send_output(
                "execution_status",
                pa.array(list(json.dumps(status).encode()), type=pa.uint8()),
            )
            time.sleep(TICK_S)
            continue

        q = samples[min(step, len(samples) - 1)]
        q_current = list(q)
        node.send_output("joint_commands", pa.array(q))
        node.send_output("joint_positions", pa.array(q))
        status = {
            "is_executing": step < len(samples) - 1,
            "current_waypoint": step,
            "progress": round(progress_fraction(step, len(samples) - 1), 3),
            "execution_count": execution_count,
            "total_waypoints": len(samples) - 1,
        }
        node.send_output(
            "execution_status",
            pa.array(list(json.dumps(status).encode()), type=pa.uint8()),
        )
        if step < len(samples) - 1:
            step += 1
        time.sleep(TICK_S)


if __name__ == "__main__":
    main()
