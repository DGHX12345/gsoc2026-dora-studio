"""Unit tests for the live-demo trajectory executor (trajectory_executor.py).

Run with the M15 venv:
    /home/dora/.venvs/dora-studio/bin/python -m unittest \
        examples/live-demo/nodes/test_trajectory_executor.py
"""

import math
import unittest

import trajectory_executor as te


class TestQuinticStep(unittest.TestCase):
    def test_endpoints_match_targets(self):
        q0 = [0.1, -0.5, 1.2]
        q1 = [0.9, 0.7, -0.3]
        start = te.quintic_step(0.0, q0, q1)
        end = te.quintic_step(1.0, q0, q1)
        for i in range(3):
            self.assertAlmostEqual(start[i], q0[i])
            self.assertAlmostEqual(end[i], q1[i])

    def test_midpoint_is_halfway(self):
        mid = te.quintic_step(0.5, [0.0, 0.0], [1.0, -2.0])
        self.assertAlmostEqual(mid[0], 0.5)
        self.assertAlmostEqual(mid[1], -1.0)

    def test_zero_velocity_at_ends(self):
        q0 = [0.0]
        q1 = [1.0]
        eps = 1e-6
        # quintic: q(s) = q0 + (q1-q0)(10s^3 - 15s^4 + 6s^5)
        # near s=0 the leading term is 10s^3 — displacement ~ eps^3
        near_start = te.quintic_step(eps, q0, q1)[0]
        self.assertLess(abs(near_start - q0[0]), 1e-10)
        near_end = te.quintic_step(1.0 - eps, q0, q1)[0]
        self.assertLess(abs(q1[0] - near_end), 1e-10)


class TestPoseTowardTarget(unittest.TestCase):
    def test_azimuth_tracks_four_quadrants(self):
        # q1 = 0.3 * atan2(y, x) — gentle nano-range tracking
        right = te.pose_toward_target((1.0, 0.0))
        left = te.pose_toward_target((-1.0, 0.0))
        front = te.pose_toward_target((0.0, 1.0))
        back = te.pose_toward_target((0.0, -1.0))
        self.assertAlmostEqual(right[0], 0.0)
        self.assertAlmostEqual(left[0], 0.3 * math.pi)
        self.assertAlmostEqual(front[0], 0.15 * math.pi)
        self.assertAlmostEqual(back[0], -0.15 * math.pi)

    def test_returns_seven_joints(self):
        pose = te.pose_toward_target((0.5, 0.3))
        self.assertEqual(len(pose), 7)
        for value in pose:
            self.assertTrue(math.isfinite(value))

    def test_pose_stays_within_nano_friendly_bounds(self):
        # the mirror is the NANO model: poses must stay plausible for it
        # (|arm joints| <= 1.0 rad, gripper within 0..0.0715 m)
        for target in ((1, 0), (-1, 0), (0, 1), (0, -1), (0.3, 0.4)):
            pose = te.pose_toward_target(target)
            for value in pose[:6]:
                self.assertLessEqual(abs(value), 1.0)
            self.assertGreaterEqual(pose[6], 0.0)
            self.assertLessEqual(pose[6], 0.0715)

    def test_home_return_ends_at_home(self):
        samples = te.home_return([0.5, 0.2, -0.4, 0.3, 0.1, 0.05, 0.02])
        self.assertEqual(samples[0][0], 0.5)
        self.assertEqual(samples[-1], te.HOME)
        self.assertGreater(len(samples), 1)


class TestPlanSamples(unittest.TestCase):
    def test_sample_count_matches_duration(self):
        # 1.5s move at 0.05s ticks -> 30 samples plus the final pose
        samples = te.plan_samples([0.0] * 7, [1.0] * 7, 1.5, 0.05)
        self.assertEqual(len(samples), 31)
        self.assertEqual(samples[0], [0.0] * 7)
        self.assertEqual(samples[-1], [1.0] * 7)

    def test_progress_fraction(self):
        self.assertAlmostEqual(te.progress_fraction(15, 30), 0.5)
        self.assertAlmostEqual(te.progress_fraction(30, 30), 1.0)

    def test_trajectory_envelope_shape(self):
        samples = [[0.0] * 7, [0.5] * 7, [1.0] * 7]
        envelope = te.trajectory_envelope(samples)
        self.assertEqual(envelope["waypoints"], samples)
        self.assertEqual(len(envelope["waypoints"][0]), 7)


class TestParseExecuteCommand(unittest.TestCase):
    def test_parses_execute_and_stop(self):
        self.assertEqual(te.parse_execute_command({"command": "execute"}), "execute")
        self.assertEqual(te.parse_execute_command({"command": "stop"}), "stop")

    def test_rejects_unknown_commands(self):
        self.assertIsNone(te.parse_execute_command({"command": "fly"}))
        self.assertIsNone(te.parse_execute_command("stop"))
if __name__ == "__main__":
    unittest.main()

