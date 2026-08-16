"""arm_joints — arm-joint extractor for the ur5e mujoco mirror (M15 C1).

The dora-mujoco ur5e model exposes qpos as 21 values
(ball freejoint 7 + arm 6 + gripper 8); the arm joints live at
qpos[7:13]. Every arm consumer (planning_scene, console, executor,
Studio bridge) subscribes this node's 6-value joint_positions instead of
the raw stream, so the moveit tool in the viewport applies the right
values to the 6-joint URDF model. ARM_JOINT_START (default 7) keeps the
offset explicit.
"""

import os

import numpy as np
import pyarrow as pa
from dora import Node

DEFAULT_ARM_JOINT_START = 7
NUM_ARM_JOINTS = 6


def extract_arm_joints(qpos, arm_start=DEFAULT_ARM_JOINT_START, num_joints=NUM_ARM_JOINTS):
    """qpos array -> arm joint slice, or None if too short."""
    arr = np.asarray(qpos, dtype=np.float64)
    if len(arr) < arm_start + num_joints:
        return None
    return arr[arm_start:arm_start + num_joints]


def main():
    node = Node()
    env = node.node_config().get("env", {})
    arm_start = int(env.get("ARM_JOINT_START", DEFAULT_ARM_JOINT_START))

    for event in node:
        if event["type"] == "INPUT" and event["id"] == "joint_positions_raw":
            try:
                arm = extract_arm_joints(event["value"].to_numpy(), arm_start)
            except Exception as e:
                print(f"[ArmJoints] Error: {e}")
                continue
            if arm is None:
                continue
            node.send_output(
                "joint_positions",
                pa.array(arm, type=pa.float64()),
                {"encoding": "jointstate"},
            )
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
