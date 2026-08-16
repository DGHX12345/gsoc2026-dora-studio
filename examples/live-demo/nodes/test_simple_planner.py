"""Unit tests for the live-demo A* planner (simple_planner.py).

Run with the M15 venv:
    /home/dora/.venvs/dora-studio/bin/python -m unittest \
        examples/live-demo/nodes/test_simple_planner.py
"""

import unittest

import simple_planner as sp

W = 20
H = 20
RES = 0.05


def empty_grid():
    return [0.0] * (W * H)


def grid_with_wall(wall_j=10, gap_i=5):
    values = [0.0] * (W * H)
    for i in range(H):
        if i != gap_i:
            values[i * W + wall_j] = 100.0
    return values


def cells_of(path, res):
    # (row i, column j) — matches is_obstacle(values, w, h, i, j)
    return [(round(p[1] / res), round(p[0] / res)) for p in path]


class TestPlanPath(unittest.TestCase):
    def test_straight_path_on_empty_grid(self):
        path = sp.plan_path(W, H, RES, empty_grid(), (0.0, 0.0), (0.5, 0.0))
        self.assertIsNotNone(path)
        self.assertGreaterEqual(len(path), 2)
        self.assertEqual(path[0], [0.0, 0.0])
        self.assertEqual(path[-1], [0.5, 0.0])
        # all cells non-obstacle
        for cell in cells_of(path, RES):
            self.assertFalse(sp.is_obstacle(empty_grid(), W, H, *cell))

    def test_path_detours_around_obstacle_wall(self):
        values = grid_with_wall()
        # start left of the wall, goal right of it — only gap at row 5
        path = sp.plan_path(W, H, RES, values, (0.05, 0.05), (0.9, 0.05))
        self.assertIsNotNone(path)
        cells = cells_of(path, RES)
        for i, j in cells:
            self.assertFalse(
                sp.is_obstacle(values, W, H, i, j),
                f"path cell ({i},{j}) is an obstacle",
            )
        # the path must cross the wall column through the gap row
        crossings = [j for i, j in cells if j == 10]
        self.assertGreater(len(crossings), 0)
        self.assertTrue(all(i == 5 for i, j in cells if j == 10))

    def test_returns_none_when_goal_is_blocked(self):
        values = [0.0] * (W * H)
        # surround the goal cell (10,10) with obstacles
        for di in (-1, 0, 1):
            for dj in (-1, 0, 1):
                if di == 0 and dj == 0:
                    continue
                values[(10 + di) * W + (10 + dj)] = 100.0
        values[10 * W + 10] = 0.0
        path = sp.plan_path(W, H, RES, values, (0.05, 0.05), (0.5, 0.5))
        self.assertIsNone(path)

    def test_start_equals_goal_returns_single_point(self):
        path = sp.plan_path(W, H, RES, empty_grid(), (0.25, 0.25), (0.25, 0.25))
        self.assertEqual(path, [[0.25, 0.25]])

    def test_path_steps_are_grid_adjacent(self):
        values = grid_with_wall()
        path = sp.plan_path(W, H, RES, values, (0.05, 0.05), (0.9, 0.9))
        self.assertIsNotNone(path)
        cells = cells_of(path, RES)
        for (i1, j1), (i2, j2) in zip(cells, cells[1:]):
            self.assertLessEqual(abs(i1 - i2), 1)
            self.assertLessEqual(abs(j1 - j2), 1)

    def test_costmap_penalty_steers_away_from_high_cost(self):
        values = [0.0] * (W * H)
        # expensive (but passable) band at j=8..12, except a cheap gap at i=2
        for i in range(H):
            for j in range(8, 13):
                values[i * W + j] = 70.0
        for j in range(8, 13):
            values[2 * W + j] = 0.0
        path = sp.plan_path(W, H, RES, values, (0.05, 0.05), (0.9, 0.05))
        self.assertIsNotNone(path)
        cells = cells_of(path, RES)
        crossings = [(i, j) for i, j in cells if 8 <= j <= 12]
        # the path should prefer the cheap gap (i=2) over the costly band
        self.assertTrue(all(i == 2 for i, j in crossings))


class TestChooseTarget(unittest.TestCase):
    def test_console_goal_wins_over_orbit_target(self):
        self.assertEqual(
            sp.choose_target((0.4, 0.1), (0.8, 0.9)), (0.4, 0.1)
        )

    def test_orbit_target_used_when_no_goal(self):
        self.assertEqual(sp.choose_target(None, (0.8, 0.9)), (0.8, 0.9))

class TestReplanKey(unittest.TestCase):
    def test_key_changes_with_target(self):
        cells = frozenset([(1, 2)])
        k1 = sp.replan_key((0.5, 0.2), cells)
        k2 = sp.replan_key((0.9, 0.1), cells)
        self.assertNotEqual(k1, k2)

    def test_key_changes_with_obstacle_cells(self):
        k1 = sp.replan_key((0.5, 0.2), frozenset([(1, 2)]))
        k2 = sp.replan_key((0.5, 0.2), frozenset([(1, 2), (3, 4)]))
        self.assertNotEqual(k1, k2)

    def test_key_stable_for_identical_inputs(self):
        cells = frozenset([(1, 2), (3, 4)])
        self.assertEqual(sp.replan_key((0.5, 0.2), cells), sp.replan_key((0.5, 0.2), cells))

    def test_obstacle_cells_only_above_threshold(self):
        values = [0.0] * (10 * 10)
        values[0] = 100.0
        values[5 * 10 + 5] = 79.0
        cells = sp.obstacle_cells(values, 10, 10)
        self.assertIn((0, 0), cells)
        self.assertNotIn((5, 5), cells)

class TestSmoothPath(unittest.TestCase):
    def _world(self, cells, res=0.05):
        return [[round(j * res, 4), round(i * res, 4)] for i, j in cells]

    def test_straight_staircase_collapses_to_two_points(self):
        values = [0.0] * (W * H)
        raw = self._world([(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)])
        smoothed = sp.smooth_path(raw, values, W, H, RES)
        self.assertEqual(smoothed, [raw[0], raw[-1]])

    def test_wall_detour_survives_smoothing(self):
        values = grid_with_wall()
        path = sp.plan_path(W, H, RES, values, (0.05, 0.05), (0.9, 0.05))
        self.assertIsNotNone(path)
        smoothed = sp.smooth_path(path, values, W, H, RES)
        self.assertEqual(smoothed[0], path[0])
        self.assertEqual(smoothed[-1], path[-1])
        # every smoothed segment must stay clear of obstacles
        for p1, p2 in zip(smoothed, smoothed[1:]):
            cells = sp.segment_cells(p1, p2, RES)
            for i, j in cells:
                self.assertFalse(sp.is_obstacle(values, W, H, i, j))
        # and it must still cross the wall column through the gap
        crossed = [p for p in smoothed if abs(p[0] - 10 * RES) < 1e-9]
        self.assertGreater(len(crossed), 0)

    def test_two_point_path_unchanged(self):
        values = [0.0] * (W * H)
        raw = self._world([(1, 1), (5, 1)])
        self.assertEqual(sp.smooth_path(raw, values, W, H, RES), raw)
if __name__ == "__main__":
    unittest.main()



