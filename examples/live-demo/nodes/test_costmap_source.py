"""Unit tests for the live-demo costmap source (costmap_source.py).

Run with the M15 venv:
    /home/dora/.venvs/dora-studio/bin/python -m unittest \
        examples/live-demo/nodes/test_costmap_source.py
"""

import math
import unittest

import costmap_source as cs


class TestCostmapSource(unittest.TestCase):
    def test_costmap_shape_and_bounds(self):
        cm = cs.make_costmap(0.0)
        self.assertEqual(cm["width"], 24)
        self.assertEqual(cm["height"], 24)
        self.assertEqual(cm["resolution"], 0.05)
        self.assertEqual(len(cm["values"]), 24 * 24)
        self.assertEqual(max(cm["values"]), 100.0)
        self.assertGreaterEqual(min(cm["values"]), 0.0)

    def test_obstacle_blob_is_small_and_centered(self):
        cm = cs.make_costmap(0.0)
        values = cm["values"]
        blob = [
            values[i * 24 + j]
            for i in range(24)
            for j in range(24)
            if values[i * 24 + j] >= cs.OBSTACLE_THRESHOLD
        ]
        self.assertGreaterEqual(len(blob), 1)
        self.assertLessEqual(len(blob), 16, "obstacle blob too large")

    def test_scene_box_matches_obstacle_position(self):
        t = 12.3
        x, y = cs.obstacle_xy(t)
        scene = cs.make_scene(t)
        obj = scene["world_objects"][0]
        self.assertEqual(obj["name"], "box_obstacle")
        self.assertAlmostEqual(obj["position"][0], x, places=2)
        self.assertAlmostEqual(obj["position"][1], y, places=2)
        self.assertEqual(obj["type"], "box")

    def test_target_stays_inside_grid(self):
        for t in (0.0, 10.0, 40.0, 99.0):
            x, y = cs.target_xy(t)
            self.assertTrue(math.isfinite(x) and math.isfinite(y))
            self.assertGreaterEqual(x, 0.0)
            self.assertLessEqual(x, 1.2)
            self.assertGreaterEqual(y, 0.0)
            self.assertLessEqual(y, 1.2)


if __name__ == "__main__":
    unittest.main()
