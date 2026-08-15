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


def make_costmap(t):
    x0, y0 = obstacle_xy(t)
    cx, cy = round(x0 / RESOLUTION), round(y0 / RESOLUTION)
    values = []
    for i in range(GRID):
        for j in range(GRID):
            d2 = (j - cx) ** 2 + (i - cy) ** 2
            values.append(round(100.0 * math.exp(-d2 / 8.0), 1))
    return {
        "width": GRID,
        "height": GRID,
        "resolution": RESOLUTION,
        "values": values,
    }


def make_scene(t):
    x, y = obstacle_xy(t)
    return {
        "version": int(t),
        "world_objects": [
            {
                "name": "box_obstacle",
                "type": "box",
                "position": [round(x, 3), round(y, 3), 0.15],
                "dimensions": [0.10, 0.10, 0.30],
            }
        ],
        "attached_objects": [],
        "robot_state": {"joint_positions": [], "gripper_state": 0},
    }


def target_xy(t):
    """Orbits the grid; the orbit crosses the obstacle blob, so the
    planner periodically reports a blocked goal."""
    return (
        0.45 + 0.35 * math.cos(t / 30.0),
        0.15 + 0.35 * math.sin(t / 30.0),
    )


def main():
    node = Node()
    t0 = time.time()
    while True:
        event = node.try_recv()
        if event is not None and event.get("type") == "STOP":
            break
        t = time.time() - t0

        node.send_output(
            "costmap",
            pa.array(list(json.dumps(make_costmap(t)).encode()), type=pa.uint8()),
        )
        node.send_output(
            "scene_update",
            pa.array(list(json.dumps(make_scene(t)).encode()), type=pa.uint8()),
        )
        x, y = target_xy(t)
        node.send_output("target_point", pa.array([x, y, 0.30]))

        time.sleep(CADENCE_S)


if __name__ == "__main__":
    main()
