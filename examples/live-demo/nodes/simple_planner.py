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


def obstacle_cells(values, width, height):
    """The set of impassable cells — the replan trigger for costmap
    changes (a drifting blob that keeps its obstacle cells does not
    force replans)."""
    return frozenset(
        (i, j)
        for i in range(height)
        for j in range(width)
        if values[i * width + j] >= OBSTACLE_THRESHOLD
    )


def segment_cells(p1, p2, resolution):
    """All cells a straight world-space segment passes through."""
    i1, j1 = round(p1[1] / resolution), round(p1[0] / resolution)
    i2, j2 = round(p2[1] / resolution), round(p2[0] / resolution)
    steps = max(abs(i2 - i1), abs(j2 - j1))
    if steps == 0:
        return [(i1, j1)]
    cells = []
    for k in range(steps + 1):
        cells.append(
            (round(i1 + (i2 - i1) * k / steps), round(j1 + (j2 - j1) * k / steps))
        )
    return cells


def line_of_sight(values, width, height, p1, p2, resolution):
    """True when the straight segment between two world points crosses
    no obstacle cells."""
    return all(
        not is_obstacle(values, width, height, i, j)
        for i, j in segment_cells(p1, p2, resolution)
    )


def smooth_path(world_path, values, width, height, resolution):
    """Greedy line-of-sight simplification: collapse grid staircases
    into straight segments while never cutting through obstacles."""
    if len(world_path) <= 2:
        return world_path
    cells = [
        (round(p[1] / resolution), round(p[0] / resolution)) for p in world_path
    ]
    smoothed = [cells[0]]
    idx = 1
    while idx < len(cells):
        end = idx
        while end + 1 < len(cells) and line_of_sight(
            values, width, height, world_path[len(smoothed) - 1], world_path[end + 1], resolution
        ):
            end += 1
        smoothed.append(cells[end])
        idx = end + 1
    return [
        [round(j * resolution, 4), round(i * resolution, 4)] for i, j in smoothed
    ]


def replan_key(target, cells):
    """B6: dedupe key — replan only when the target or the obstacle
    layout actually changed (no more per-tick replan spam)."""
    tx, ty = round(target[0], 2), round(target[1], 2)
    return (tx, ty, cells)


def choose_target(goal, orbit):
    """B6: the console goal overrides the orbiting demo target until the
    console sends `auto` (goal cleared back to None)."""
    return goal if goal is not None else orbit


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
    orbit_target = None
    goal = None
    plan_id = 0
    last_key = None
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
                    orbit_target = (float(arr[0]), float(arr[1]))
        elif event["id"] == "goal":
            value = event.get("value")
            if value is not None:
                arr = value.to_pylist()
                if len(arr) >= 2:
                    goal = (float(arr[0]), float(arr[1]))
        elif event["id"] == "resume":
            goal = None

        target = choose_target(goal, orbit_target)
        if costmap is None or target is None:
            continue

        cells = obstacle_cells(costmap["values"], costmap["width"], costmap["height"])
        key = replan_key(target, cells)
        if key == last_key:
            continue
        last_key = key

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

        path = smooth_path(
            path, costmap["values"], costmap["width"], costmap["height"], float(costmap["resolution"])
        )
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
