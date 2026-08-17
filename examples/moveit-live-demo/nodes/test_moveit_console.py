"""Pure-function tests for moveit_console (M15 C1).

Runs without a dora runtime: only the command-mapping helpers are tested.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from moveit_console import (
    build_gate_command,
    build_ik_request,
    build_plan_request,
    build_scene_command,
    initial_watermark,
    parse_ik_solution,
    parse_target,
    plan_ready,
)


class TestParseTarget(unittest.TestCase):
    def test_xyz_triple_pads_rpy_zeros(self):
        self.assertEqual(parse_target("0.3 0.2 0.5"), [0.3, 0.2, 0.5, 0.0, 0.0, 0.0])

    def test_comma_separated_triple(self):
        self.assertEqual(parse_target("0.3,0.2,0.5"), [0.3, 0.2, 0.5, 0.0, 0.0, 0.0])

    def test_full_pose_six_values(self):
        self.assertEqual(
            parse_target("0.3 0.2 0.5 1.0 -1.0 0.5"),
            [0.3, 0.2, 0.5, 1.0, -1.0, 0.5],
        )

    def test_rejects_wrong_counts(self):
        self.assertIsNone(parse_target("0.3 0.2"))
        self.assertIsNone(parse_target("0.3 0.2 0.5 1.0"))

    def test_rejects_garbage(self):
        self.assertIsNone(parse_target("abc def ghi"))
        self.assertIsNone(parse_target(""))

    def test_rejects_non_string(self):
        self.assertIsNone(parse_target(None))
        self.assertIsNone(parse_target(42))


class TestRequestBuilders(unittest.TestCase):
    def test_build_ik_request_is_float_list(self):
        self.assertEqual(
            build_ik_request([0.3, 0.2, 0.5, 0, 0, 0]),
            [0.3, 0.2, 0.5, 0.0, 0.0, 0.0],
        )

    def test_build_plan_request_shape(self):
        req = build_plan_request(
            [0.0] * 6, [1.0] * 6, planner="rrt_connect", max_time=5.0
        )
        self.assertEqual(req["start"], [0.0] * 6)
        self.assertEqual(req["goal"], [1.0] * 6)
        self.assertEqual(req["planner"], "rrt_connect")
        self.assertEqual(req["max_time"], 5.0)

    def test_build_scene_command_passthrough(self):
        obj = {"name": "box1", "type": "box", "position": [0.3, 0, 0.4], "dimensions": [0.1, 0.1, 0.1]}
        self.assertEqual(build_scene_command("add", obj), {"action": "add", "object": obj})

    def test_build_scene_remove_moves_name_to_top_level(self):
        # The moveit planning scene reads remove targets from the
        # top-level `name`; the B6 console nests it under `object`.
        self.assertEqual(
            build_scene_command("remove", {"name": "box_1"}),
            {"action": "remove", "name": "box_1"},
        )

    def test_build_scene_remove_without_name_returns_none(self):
        self.assertIsNone(build_scene_command("remove", {}))
        self.assertIsNone(build_scene_command("remove", None))

    def test_initial_watermark_is_next_seq_minus_one(self):
        # Watermark semantics: "last consumed seq" — exactly-next_seq
        # would trip the restart detection (since >= next -> reset 0).
        self.assertEqual(initial_watermark({"next_seq": 17}), 16)

    def test_initial_watermark_defaults_to_zero(self):
        self.assertEqual(initial_watermark({}), 0)

    def test_initial_watermark_never_negative(self):
        self.assertEqual(initial_watermark({"next_seq": 0}), 0)

    def test_build_gate_command_envelope(self):
        self.assertEqual(build_gate_command("execute"), {"command": "execute"})
        self.assertEqual(build_gate_command("stop"), {"command": "stop"})
        self.assertEqual(build_gate_command("auto"), {"command": "auto"})


class TestPlanReady(unittest.TestCase):
    def test_waits_for_both_inputs(self):
        self.assertIsNone(plan_ready(None, [1.0] * 6))
        self.assertIsNone(plan_ready([0.0] * 6, None))
        self.assertIsNone(plan_ready(None, None))

    def test_builds_request_once_both_present(self):
        req = plan_ready([0.1] * 6, [1.0] * 6)
        self.assertIsNotNone(req)
        self.assertEqual(req["start"], [0.1] * 6)
        self.assertEqual(req["goal"], [1.0] * 6)

    def test_truncates_long_solutions_to_six_joints(self):
        req = plan_ready([0.0] * 6, [1.0] * 7)
        self.assertEqual(req["goal"], [1.0] * 6)


class TestParseIkSolution(unittest.TestCase):
    def test_pyarrow_array_payload(self):
        import pyarrow as pa

        arr = pa.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
        self.assertEqual(parse_ik_solution(arr), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0])

    def test_short_payload_rejected(self):
        import pyarrow as pa

        self.assertIsNone(parse_ik_solution(pa.array([1.0, 2.0])))

    def test_non_array_rejected(self):
        self.assertIsNone(parse_ik_solution("not an array"))


if __name__ == "__main__":
    unittest.main()
