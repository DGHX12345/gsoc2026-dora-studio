"""Unit tests for the live-demo command node (studio_console.py).

Run with the M15 venv:
    /home/dora/.venvs/dora-studio/bin/python -m unittest \
        examples/live-demo/nodes/test_studio_console.py
"""

import unittest

import studio_console as sc


class TestCommandToOutputs(unittest.TestCase):
    def test_plan_command_maps_to_target_point_and_mode_plan(self):
        outputs = sc.command_to_outputs(
            {"seq": 1, "kind": "plan", "target": [0.5, 0.2], "planner": None}
        )
        self.assertEqual(len(outputs), 2)
        output_id, payload = outputs[0]
        self.assertEqual(output_id, "target_point")
        self.assertEqual(payload, [0.5, 0.2, 0.30])
        self.assertEqual(outputs[1], ("mode_command", {"command": "plan"}))

    def test_plan_command_keeps_explicit_z(self):
        outputs = sc.command_to_outputs(
            {"seq": 2, "kind": "plan", "target": [0.5, 0.2, 0.45], "planner": None}
        )
        self.assertEqual(outputs[0][1], [0.5, 0.2, 0.45])

    def test_execute_and_stop_map_to_execute_command(self):
        for kind, command in (("execute", "execute"), ("stop", "stop")):
            outputs = sc.command_to_outputs({"seq": 3, "kind": kind})
            self.assertEqual(len(outputs), 1)
            self.assertEqual(outputs[0][0], "execute_command")
            self.assertEqual(outputs[0][1], {"command": command})

    def test_auto_command_maps_to_resume(self):
        outputs = sc.command_to_outputs({"seq": 4, "kind": "auto"})
        self.assertEqual(outputs, [("resume_command", {"command": "auto"})])

    def test_scene_command_forwards_action_and_object(self):
        obj = {"name": "box1", "type": "box", "position": [0.5, 0.1, 0.15]}
        outputs = sc.command_to_outputs(
            {"seq": 5, "kind": "scene", "action": "add", "object": obj}
        )
        self.assertEqual(len(outputs), 1)
        self.assertEqual(outputs[0][0], "scene_command")
        self.assertEqual(outputs[0][1], {"action": "add", "object": obj})

    def test_plan_command_without_target_is_skipped(self):
        self.assertEqual(sc.command_to_outputs({"seq": 6, "kind": "plan"}), [])

    def test_unknown_kind_is_skipped(self):
        self.assertEqual(sc.command_to_outputs({"seq": 7, "kind": "explode"}), [])


if __name__ == "__main__":
    unittest.main()


class TestAdvanceWatermark(unittest.TestCase):
    def test_watermark_resets_after_backend_restart(self):
        # backend restarted: next_seq below the watermark -> reset to 0
        self.assertEqual(sc.advance_watermark(5, 3), 0)

    def test_watermark_unchanged_in_normal_operation(self):
        self.assertEqual(sc.advance_watermark(1, 5), 1)
        self.assertEqual(sc.advance_watermark(2, 3), 2)
