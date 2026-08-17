"""moveit_console — command channel node for the moveit live demo (M15 C1).

Polls the Studio backend command queue (same B6 protocol as
examples/live-demo/nodes/studio_console.py) and maps console commands to
moveit requests:

  plan    -> target [x y z] or [x y z roll pitch yaw]
             -> ik_request (float32 pose) to ik_solver
             -> on ik_solution: plan_request {start, goal} to the planner
  execute -> execute_command {"command": "execute"} (gated executor)
  stop    -> stop_command    {"command": "stop"}
  auto    -> resume_command  {"command": "auto"}
  scene   -> scene_command {"action", "object"} (planning_scene)

The plan start config is the latest arm-filtered joint_positions from the
arm_joints node (mujoco qpos[7:13]).
"""

import json
import time
import urllib.error
import urllib.request

import numpy as np
import pyarrow as pa
from dora import Node

DEFAULT_BACKEND_URL = "http://127.0.0.1:3001/api/live/ingest"
POLL_INTERVAL_S = 0.5


def parse_target(text):
    """'x y z' (pad roll/pitch/yaw = 0) or 'x y z r p y' -> 6 floats, else None."""
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
    """6D pose [x, y, z, roll, pitch, yaw] -> float list for ik_request."""
    return [float(v) for v in pose]


def build_plan_request(start, goal, planner="rrt_connect", max_time=5.0):
    return {
        "start": [float(v) for v in start],
        "goal": [float(v) for v in goal],
        "planner": planner,
        "max_time": float(max_time),
    }


def build_scene_command(action, obj):
    """B6 console scene command → moveit scene command. The moveit
    planning scene reads the remove target from the top-level `name`
    (B6 nests it under `object`)."""
    if action == "remove":
        name = obj.get("name") if isinstance(obj, dict) else None
        if name is None:
            return None
        return {"action": "remove", "name": name}
    return {"action": action, "object": obj}


def build_gate_command(kind):
    return {"command": kind}


def plan_ready(current_joints, solution):
    """plan_request when both the joint state and the IK solution are
    present; None while either is still missing."""
    if current_joints is None or solution is None:
        return None
    return build_plan_request(current_joints, solution[:6])


def parse_ik_solution(payload):
    """Array payload -> list of joint angles (at least 6), else None."""
    try:
        arr = payload.to_numpy()
    except Exception:
        return None
    if arr.size < 6:
        return None
    return arr.tolist()


def command_url(backend_url):
    if backend_url.endswith("/api/live/ingest"):
        return backend_url[: -len("/api/live/ingest")] + "/api/live/command"
    return backend_url


def advance_watermark(since_seq, next_seq):
    """Backend restart detection (same as studio_console)."""
    if since_seq >= next_seq:
        return 0
    return since_seq


def initial_watermark(body):
    """First poll: adopt the backend's next_seq minus one (the watermark
    means "last consumed seq"), so historical commands are not replayed on
    every console restart. Exactly-next_seq would trip the restart
    detection in advance_watermark (since >= next -> reset to 0)."""
    return max(0, body.get("next_seq", 1) - 1)


def send_json(node, output_id, payload):
    encoded = json.dumps(payload).encode()
    node.send_output(output_id, pa.array(list(encoded), type=pa.uint8()))


def main():
    node = Node()
    url = command_url(
        node.node_config().get("env", {}).get("STUDIO_BACKEND_URL", DEFAULT_BACKEND_URL)
    )

    current_joints = None
    pending_solution = None
    since_seq = 0
    skip_first = True

    while True:
        event = node.try_recv()
        if event is not None:
            if event.get("type") == "STOP":
                break
            if event.get("type") == "INPUT":
                input_id = event["id"]
                if input_id == "joint_positions":
                    try:
                        current_joints = event["value"].to_numpy().tolist()
                    except Exception:
                        pass
                    request = plan_ready(current_joints, pending_solution)
                    if request is not None:
                        send_json(node, "plan_request", request)
                        pending_solution = None
                        print(f"[Console] plan_request sent: goal={request['goal'][:3]}...")
                elif input_id == "ik_solution":
                    solution = parse_ik_solution(event["value"])
                    if solution is None:
                        print("[Console] Invalid ik_solution payload, skipped")
                    else:
                        pending_solution = solution
                        request = plan_ready(current_joints, solution)
                        if request is not None:
                            send_json(node, "plan_request", request)
                            pending_solution = None
                            print(f"[Console] plan_request sent: goal={request['goal'][:3]}...")
                        else:
                            print("[Console] IK solved; waiting for joint state")
                elif input_id == "ik_status":
                    pass  # informational only

        try:
            req = urllib.request.Request(f"{url}?since_seq={since_seq}")
            with urllib.request.urlopen(req, timeout=2.0) as resp:
                body = json.loads(resp.read().decode("utf-8"))
            next_seq = body.get("next_seq", 0)
            if skip_first:
                since_seq = initial_watermark(body)
                skip_first = False
                continue
            since_seq = advance_watermark(since_seq, next_seq)
            for command in body.get("commands", []):
                seq = command.get("seq", since_seq)
                kind = command.get("kind")
                if kind == "plan":
                    target = command.get("target")
                    pose = parse_target(" ".join(str(v) for v in target)) if isinstance(target, list) else None
                    if pose is None:
                        print(f"[Console] Invalid plan target: {target!r}")
                    else:
                        pending_target = pose
                        node.send_output(
                            "ik_request",
                            pa.array(build_ik_request(pose), type=pa.float32()),
                        )
                        # goal marker for the dviz path tool in the viewport
                        node.send_output(
                            "target_point", pa.array(pose[:3], type=pa.float32())
                        )
                        print(f"[Console] ik_request sent for {pose}")
                elif kind in ("execute", "stop"):
                    send_json(node, "execute_command", build_gate_command(kind))
                elif kind == "auto":
                    send_json(node, "resume_command", build_gate_command("auto"))
                elif kind == "scene":
                    action = command.get("action")
                    obj = command.get("object")
                    scene = build_scene_command(action, obj) if action else None
                    if scene is None:
                        print("[Console] Invalid scene command, skipped")
                    else:
                        send_json(node, "scene_command", scene)
                since_seq = max(since_seq, seq)
        except (urllib.error.URLError, OSError, json.JSONDecodeError):
            pass  # backend not up yet — keep polling

        time.sleep(POLL_INTERVAL_S)


if __name__ == "__main__":
    main()
