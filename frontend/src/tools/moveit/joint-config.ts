// Joint-name configuration (M13 D3) — joint names never arrive on the wire
// (dora-moveit2 reads them from config files). Studio maps known robots and
// falls back to generic J0..Jn labels; the robot is user-selectable in the
// D6 panel.

export interface RobotJointConfig {
  id: string;
  label: string;
  /** Ordered joint names; the trajectory width equals this list's length. */
  jointNames: string[];
}

export const KNOWN_ROBOTS: RobotJointConfig[] = [
  {
    id: 'ur5e',
    label: 'UR5e (Universal Robots)',
    jointNames: [
      'shoulder_pan_joint',
      'shoulder_lift_joint',
      'elbow_joint',
      'wrist_1_joint',
      'wrist_2_joint',
      'wrist_3_joint',
    ],
  },
  {
    id: 'gen72',
    label: 'GEN72 (AgileX)',
    jointNames: [
      'joint_1',
      'joint_2',
      'joint_3',
      'joint_4',
      'joint_5',
      'joint_6',
      'gripper_left',
      'gripper_right',
    ],
  },
  {
    id: 'b601',
    label: 'reBot B601 (Seeed)',
    // Names verified against reBot_B601_DM_with_gripper.urdf (2026-08-14).
    // The 7th trajectory value drives the gripper finger joints
    // (gripper_joint1/gripper_joint2 in the URDF; D4.1 maps the value).
    jointNames: ['joint1', 'joint2', 'joint3', 'joint4', 'joint5', 'joint6', 'gripper'],
  },
];

export function getRobotConfig(id: string): RobotJointConfig | undefined {
  return KNOWN_ROBOTS.find((robot) => robot.id === id);
}

/** Joint display labels for a robot id and a count: configured names first,
 * generic Jk labels beyond the config. */
export function jointLabelsFor(robotId: string | null | undefined, count: number): string[] {
  const names = robotId ? getRobotConfig(robotId)?.jointNames : undefined;
  const labels: string[] = [];
  for (let i = 0; i < count; i++) {
    labels.push(names?.[i] ?? `J${i}`);
  }
  return labels;
}
