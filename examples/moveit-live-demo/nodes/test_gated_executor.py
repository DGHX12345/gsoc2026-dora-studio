"""Tests for the gated trajectory executor (M15 C1).

Pure logic only: the ExecutionGate state machine and the flat-trajectory
parsing helper. Runs without a dora runtime.

Gate semantics: on_trajectory/on_execute/on_resume return the trajectory
to execute (consuming the pending slot) or None. Manual mode holds the
trajectory until an execute command; auto mode starts every new one.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gated_trajectory_executor import ExecutionGate, reshape_trajectory

TRAJ_A = [[0.0] * 6, [1.0] * 6]
TRAJ_B = [[2.0] * 6, [3.0] * 6]


class TestExecutionGate(unittest.TestCase):
    def test_trajectory_waits_without_auto_or_execute(self):
        gate = ExecutionGate()
        self.assertIsNone(gate.on_trajectory(TRAJ_A))
        self.assertFalse(gate.running)

    def test_execute_starts_pending_trajectory(self):
        gate = ExecutionGate()
        gate.on_trajectory(TRAJ_A)
        started = gate.on_execute()
        self.assertEqual(started, TRAJ_A)
        self.assertTrue(gate.running)

    def test_execute_without_trajectory_does_nothing(self):
        gate = ExecutionGate()
        self.assertIsNone(gate.on_execute())

    def test_second_execute_is_idempotent_while_running(self):
        gate = ExecutionGate()
        gate.on_trajectory(TRAJ_A)
        gate.on_execute()
        self.assertIsNone(gate.on_execute())

    def test_stop_halts_and_complete_releases(self):
        gate = ExecutionGate()
        gate.on_trajectory(TRAJ_A)
        gate.on_execute()
        self.assertTrue(gate.on_stop())
        self.assertFalse(gate.running)
        self.assertIsNone(gate.on_complete())

    def test_resume_enables_auto_and_starts_pending(self):
        gate = ExecutionGate()
        gate.on_trajectory(TRAJ_A)
        started = gate.on_resume()
        self.assertEqual(started, TRAJ_A)
        self.assertTrue(gate.running)
        self.assertTrue(gate.auto)

    def test_auto_starts_every_new_trajectory(self):
        gate = ExecutionGate()
        gate.on_resume()
        started = gate.on_trajectory(TRAJ_A)
        self.assertEqual(started, TRAJ_A)
        self.assertTrue(gate.running)

    def test_auto_complete_does_not_restart_consumed_trajectory(self):
        gate = ExecutionGate()
        gate.on_resume()
        self.assertIsNone(gate.on_complete())
        self.assertFalse(gate.running)
        gate.on_trajectory(TRAJ_A)
        self.assertTrue(gate.running)
        self.assertIsNone(gate.on_complete())
        self.assertFalse(gate.running)

    def test_new_trajectory_replaces_pending_in_manual_mode(self):
        gate = ExecutionGate()
        gate.on_trajectory(TRAJ_A)
        self.assertIsNone(gate.on_trajectory(TRAJ_B))
        started = gate.on_execute()
        self.assertEqual(started, TRAJ_B)


class TestReshapeTrajectory(unittest.TestCase):
    def test_reshape_flat_trajectory_with_metadata(self):
        import numpy as np

        flat = np.array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], dtype=np.float64)
        waypoints = reshape_trajectory(flat, num_waypoints=2, num_joints=6)
        self.assertEqual(len(waypoints), 2)
        self.assertEqual(waypoints[0].tolist(), [0, 1, 2, 3, 4, 5])
        self.assertEqual(waypoints[1].tolist(), [6, 7, 8, 9, 10, 11])

    def test_reshape_defaults_to_joint_count(self):
        import numpy as np

        flat = np.array([0, 1, 2, 3, 4, 5], dtype=np.float64)
        waypoints = reshape_trajectory(flat, num_waypoints=None, num_joints=6)
        self.assertEqual(len(waypoints), 1)
        self.assertEqual(waypoints[0].tolist(), [0, 1, 2, 3, 4, 5])

    def test_reshape_rejects_indivisible_length(self):
        import numpy as np

        flat = np.array([0, 1, 2, 3, 4], dtype=np.float64)
        self.assertIsNone(reshape_trajectory(flat, num_waypoints=None, num_joints=6))


if __name__ == "__main__":
    unittest.main()
