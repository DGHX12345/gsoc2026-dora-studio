"""Tests for the arm_joints extractor (M15 C1)."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from arm_joints import extract_arm_joints


class TestExtractArmJoints(unittest.TestCase):
    def test_ur5e_layout_slices_arm_joints(self):
        import numpy as np

        qpos = np.arange(21, dtype=np.float64)
        arm = extract_arm_joints(qpos)
        self.assertEqual(len(arm), 6)
        self.assertEqual(arm.tolist(), [7.0, 8.0, 9.0, 10.0, 11.0, 12.0])

    def test_short_input_returns_none(self):
        import numpy as np

        self.assertIsNone(extract_arm_joints(np.arange(5, dtype=np.float64)))

    def test_offset_respected(self):
        import numpy as np

        qpos = np.arange(21, dtype=np.float64)
        arm = extract_arm_joints(qpos, arm_start=13, num_joints=6)
        self.assertEqual(arm.tolist(), [13.0, 14.0, 15.0, 16.0, 17.0, 18.0])

    def test_plain_list_input(self):
        arm = extract_arm_joints(list(range(21)))
        self.assertEqual(arm.tolist(), [7, 8, 9, 10, 11, 12])


if __name__ == "__main__":
    unittest.main()
