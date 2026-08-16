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


class TestUserObjects(unittest.TestCase):
    def test_user_box_adds_obstacle_blob_at_its_position(self):
        cm = cs.make_costmap(0.0, [USER_BOX])
        values = cm["values"]
        bx, by = round(0.6 / 0.05), round(0.4 / 0.05)
        self.assertGreaterEqual(values[by * 24 + bx], cs.OBSTACLE_THRESHOLD)

    def test_scene_includes_user_objects(self):
        scene = cs.make_scene(0.0, [USER_BOX])
        names = [o["name"] for o in scene["world_objects"]]
        self.assertIn("user_box", names)
        self.assertIn("box_obstacle", names)

    def test_apply_scene_command_adds_and_removes(self):
        objects = cs.apply_scene_command([], {"action": "add", "object": USER_BOX})
        self.assertEqual(len(objects), 1)
        objects = cs.apply_scene_command(
            objects, {"action": "remove", "object": {"name": "user_box"}}
        )
        self.assertEqual(objects, [])

class TestModeGating(unittest.TestCase):
    def test_plan_command_disables_auto_orbit(self):
        self.assertFalse(cs.apply_mode_command(True, {"command": "plan"}))
        self.assertFalse(cs.apply_mode_command(False, {"command": "plan"}))

    def test_auto_command_enables_auto_orbit(self):
        self.assertTrue(cs.apply_mode_command(False, {"command": "auto"}))
        self.assertTrue(cs.apply_mode_command(True, {"command": "auto"}))

    def test_unknown_command_keeps_state(self):
        self.assertTrue(cs.apply_mode_command(True, {"command": "fly"}))
        self.assertFalse(cs.apply_mode_command(False, "plan"))

if __name__ == "__main__":
    unittest.main()

USER_BOX = {
    "name": "user_box",
    "type": "box",
    "position": [0.6, 0.4, 0.15],
    "dimensions": [0.1, 0.1, 0.3],
}


