"""costmap_source — synthetic costmap + planning-scene source (M15 B5).

SYNTHETIC DEMO DATA (clearly labeled): generates a 24x24 grid costmap
with a slowly drifting gaussian obstacle blob, the matching
scene_update box (so the yellow collision wireframe and the costmap
blob tell the same story), and a moving target that occasionally ends
up INSIDE the obstacle (the planner then honestly reports "no path").
A real deployment replaces this node with sensor/costmap producers.
"""

import json
import math
import time

import pyarrow as pa
from dora import Node

GRID = 24
RESOLUTION = 0.05
OBSTACLE_THRESHOLD = 80.0
CADENCE_S = 0.5


def obstacle_xy(t):
    """Blob center in world coords, drifting slowly."""
    return (0.30 + 0.05 * math.sin(t / 20.0), 0.08 * math.cos(t / 25.0))


def make_costmap(t, user_objects=None):
    x0, y0 = obstacle_xy(t)
    cx, cy = round(x0 / RESOLUTION), round(y0 / RESOLUTION)
    blobs = [(cx, cy, 8.0)]
    for obj in user_objects or []:
        position = obj.get("position") or [0, 0, 0]
        blobs.append(
            (
                round(position[0] / RESOLUTION),
                round(position[1] / RESOLUTION),
                3.0,
            )
        )
    values = []
    for i in range(GRID):
        for j in range(GRID):
            value = 0.0
            for bx, by, sigma2 in blobs:
                d2 = (j - bx) ** 2 + (i - by) ** 2
                value = max(value, 100.0 * math.exp(-d2 / sigma2))
            values.append(round(value, 1))
    return {
        "width": GRID,
        "height": GRID,
        "resolution": RESOLUTION,
        "values": values,
    }


def make_scene(t, user_objects=None):
    x, y = obstacle_xy(t)
    world_objects = [
        {
            "name": "box_obstacle",
            "type": "box",
            "position": [round(x, 3), round(y, 3), 0.15],
            "dimensions": [0.10, 0.10, 0.30],
        }
    ]
    for obj in user_objects or []:
        world_objects.append(
            {
                "name": obj.get("name", "user_object"),
                "type": obj.get("type", "box"),
                "position": obj.get("position", [0.0, 0.0, 0.15]),
                "dimensions": obj.get("dimensions", [0.10, 0.10, 0.30]),
            }
        )
    return {
        "version": int(t),
        "world_objects": world_objects,
        "attached_objects": [],
        "robot_state": {"joint_positions": [], "gripper_state": 0},
    }


def apply_mode_command(auto, command):
    """B6: console-first semantics — the orbit target only runs in auto
    mode; a console Plan command disables it."""
    if not isinstance(command, dict):
        return auto
    kind = command.get("command")
    if kind == "plan":
        return False
    if kind == "auto":
        return True
    return auto


def apply_scene_command(user_objects, command):
    """B6: add/remove user objects; returns the updated list."""
    action = command.get("action")
    obj = command.get("object") or {}
    if action == "add" and obj.get("name"):
        return [o for o in user_objects if o.get("name") != obj.get("name")] + [obj]
    if action == "remove":
        name = obj.get("name")
        return [o for o in user_objects if o.get("name") != name]
    return user_objects


def target_xy(t):
    """Orbits the grid; the orbit crosses the obstacle blob, so the
    planner periodically reports a blocked goal."""
    return (
        0.45 + 0.35 * math.cos(t / 30.0),
        0.15 + 0.35 * math.sin(t / 30.0),
    )


def main():
    node = Node()
    user_objects = []
    auto = False
    t0 = time.time()
    while True:
        event = node.try_recv()
        if event is not None and event.get("type") == "STOP":
            break
        if event is not None and event.get("type") == "INPUT":
            value = event.get("value")
            if value is not None and event["id"] in ("scene", "mode", "resume"):
                try:
                    command = json.loads(bytes(value.to_pylist()).decode("utf-8"))
                    if event["id"] == "scene":
                        user_objects = apply_scene_command(user_objects, command)
                    else:
                        auto = apply_mode_command(auto, command)
                except (UnicodeDecodeError, json.JSONDecodeError):
                    pass
        t = time.time() - t0

        node.send_output(
            "costmap",
            pa.array(
                list(json.dumps(make_costmap(t, user_objects)).encode()),
                type=pa.uint8(),
            ),
        )
        node.send_output(
            "scene_update",
            pa.array(
                list(json.dumps(make_scene(t, user_objects)).encode()),
                type=pa.uint8(),
            ),
        )
        if auto:
            x, y = target_xy(t)
            node.send_output("target_point", pa.array([x, y, 0.30]))

        time.sleep(CADENCE_S)


if __name__ == "__main__":
    main()
