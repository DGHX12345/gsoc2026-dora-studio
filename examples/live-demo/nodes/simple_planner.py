"""simple_planner — real A* grid planner node for the live demo (M15 B5).

The implanted planning algorithm: consumes a costmap and a moving
target, plans a 2D workspace path (fixed z) that avoids obstacle cells
and prefers low-cost cells, and emits `waypoints` (dviz path tool) plus
`plan_status` (MoveIt panel). Replans on every new costmap or target.
The B6 Motion Planner console will send plan requests here.

Costmap payload: JSON {width, height, resolution, values} — same shape
as the M12 costmap renderer. Target payload: flat [x, y, z] float
array. Costs are penalties: step cost = 1 + value/50; cells with
value >= OBSTACLE_THRESHOLD are impassable.
"""

import heapq
import json
import math
import time

import pyarrow as pa
from dora import Node

OBSTACLE_THRESHOLD = 80.0
PATH_Z = 0.30
START_XY = (0.05, 0.0)
REPLAN_EPSILON = 0.02


def is_obstacle(values, width, height, i, j):
    if not (0 <= i < height and 0 <= j < width):
        return True
    return values[i * width + j] >= OBSTACLE_THRESHOLD


def cost_at(values, width, i, j):
    return 1.0 + values[i * width + j] / 50.0


def plan_path(width, height, resolution, values, start_xy, goal_xy):
    """A* over the cost grid. World-coordinate in, world waypoints out;
    None when no path exists."""
    return plan_path_with_stats(width, height, resolution, values, start_xy, goal_xy)[0]


def plan_path_with_stats(width, height, resolution, values, start_xy, goal_xy):
    """Returns (path | None, explored_cells) so plan_status can report the
    search footprint honestly."""
    start = (round(start_xy[1] / resolution), round(start_xy[0] / resolution))
    goal = (round(goal_xy[1] / resolution), round(goal_xy[0] / resolution))
    if start == goal:
        return [[start_xy[0], start_xy[1]]], 1
    if is_obstacle(values, width, height, *start):
        return None, 0
    if is_obstacle(values, width, height, *goal):
        return None, 0

    def heuristic(i, j):
        return math.hypot(goal[0] - i, goal[1] - j)

    open_heap = [(heuristic(*start), 0, start[0], start[1])]
    g_score = {start: 0.0}
    came_from = {}
    closed = set()
    tie = 1
    explored = 0

    while open_heap:
        _, _, i, j = heapq.heappop(open_heap)
        if (i, j) in closed:
            continue
        closed.add((i, j))
        explored += 1
        if (i, j) == goal:
            cells = [(i, j)]
            while cells[-1] != start:
                cells.append(came_from[cells[-1]])
            cells.reverse()
            return (
                [
                    [round(j2 * resolution, 4), round(i2 * resolution, 4)]
                    for i2, j2 in cells
                ],
                explored,
            )
        for di in (-1, 0, 1):
            for dj in (-1, 0, 1):
                if di == 0 and dj == 0:
                    continue
                ni, nj = i + di, j + dj
                if is_obstacle(values, width, height, ni, nj):
                    continue
                step = 1.414 if di and dj else 1.0
                ng = g_score[(i, j)] + step * cost_at(values, width, ni, nj)
                if ng < g_score.get((ni, nj), math.inf):
                    g_score[(ni, nj)] = ng
                    came_from[(ni, nj)] = (i, j)
                    heapq.heappush(
                        open_heap, (ng + heuristic(ni, nj), tie, ni, nj)
                    )
                    tie += 1
    return None, explored


def main():
    node = Node()
    costmap = None
    target = None
    plan_id = 0
    while True:
        event = node.try_recv()
        if event is not None and event.get("type") == "STOP":
            break
        if event is None or event.get("type") != "INPUT":
            time.sleep(0.05)
            continue

        if event["id"] == "costmap":
            value = event.get("value")
            if value is None:
                continue
            raw = bytes(value.to_pylist())
            try:
                costmap = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError):
                costmap = None
        elif event["id"] == "target":
            value = event.get("value")
            if value is not None:
                arr = value.to_pylist()
                if len(arr) >= 2:
                    target = (float(arr[0]), float(arr[1]))

        if costmap is None or target is None:
            continue

        t0 = time.perf_counter()
        path, explored = plan_path_with_stats(
            costmap["width"],
            costmap["height"],
            float(costmap["resolution"]),
            costmap["values"],
            START_XY,
            target,
        )
        planning_time = time.perf_counter() - t0
        plan_id += 1
        if path is None:
            status = {
                "plan_id": plan_id,
                "success": False,
                "message": "no path found to target",
            }
            node.send_output(
                "plan_status",
                pa.array(list(json.dumps(status).encode()), type=pa.uint8()),
                {"success": False},
            )
            continue

        waypoints = []
        for x, y in path:
            waypoints.extend([x, y, PATH_Z])
        node.send_output("waypoints", pa.array(waypoints))
        path_length = sum(
            math.hypot(
                path[k + 1][0] - path[k][0], path[k + 1][1] - path[k][1]
            )
            for k in range(len(path) - 1)
        )
        status = {
            "plan_id": plan_id,
            "success": True,
            "message": "ok",
            "planning_time": round(planning_time, 4),
            "path_length": round(path_length, 4),
            "num_waypoints": len(path),
            "num_nodes": explored,
        }
        node.send_output(
            "plan_status",
            pa.array(list(json.dumps(status).encode()), type=pa.uint8()),
            {"success": True},
        )


if __name__ == "__main__":
    main()
