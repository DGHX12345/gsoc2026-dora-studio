// Robot kinematic chain built from a parsed URDF (M13 D4) — the tool-owned
// model with FK for free: forward kinematics is the transform-tree update
// itself. Joint pivots carry the <origin> transform plus the joint motion
// (rotation around <axis> for revolute/continuous, translation along
// <axis> for prismatic), values clamped to <limit> when present.

import { Group, Material, Matrix4, Quaternion, Vector3 } from 'three';
import type { UrdfRobot } from './urdf';

export interface RobotJointEntry {
  name: string;
  type: 'fixed' | 'revolute' | 'continuous' | 'prismatic';
  limit: { lower: number; upper: number } | null;
  /** The transform node that carries origin + joint motion. */
  pivot: Group;
  axis: Vector3;
  originPosition: Vector3;
  originQuaternion: Quaternion;
}

export interface RobotModel {
  /** Scene root holding the whole robot. */
  root: Group;
  links: Map<string, Group>;
  joints: Map<string, RobotJointEntry>;
  /** Joint names in URDF document order. */
  jointOrder: string[];
  /** Name of the end-effector link (last joint's child). */
  endEffectorLink: string;
  setJointValue(name: string, value: number): void;
  setJointValues(values: Record<string, number>): void;
  updateWorld(): void;
  getLinkWorldPosition(linkName: string): Vector3;
  getEndEffectorPosition(): Vector3;
  /** Detached copy of the robot with semi-transparent materials (ghost
   * poses). Geometries are shared; materials are cloned per ghost. */
  clonePose(opacity: number): Group;
}

const scratchPosition = new Vector3();

/** URDF rpy is fixed-axis roll-pitch-yaw: R = Rz(yaw)·Ry(pitch)·Rx(roll). */
export function poseToTransform(xyz: [number, number, number], rpy: [number, number, number]) {
  const [x, y, z] = xyz;
  const [roll, pitch, yaw] = rpy;
  const position = new Vector3(x, y, z);
  const quaternion = new Quaternion()
    .setFromAxisAngle(new Vector3(0, 0, 1), yaw)
    .multiply(new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), pitch))
    .multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), roll));
  return { position, quaternion };
}

export function buildRobotModel(urdf: UrdfRobot): RobotModel {
  const root = new Group();
  root.name = urdf.name;
  const links = new Map<string, Group>();
  const joints = new Map<string, RobotJointEntry>();

  for (const link of urdf.links.values()) {
    const group = new Group();
    group.name = link.name;
    links.set(link.name, group);
  }
  root.add(links.get(urdf.rootLink)!);

  for (const joint of urdf.joints) {
    const parentLink = links.get(joint.parent);
    const childLink = links.get(joint.child);
    if (!parentLink || !childLink) {
      throw new Error(`URDF error: joint ${joint.name} references unknown link`);
    }
    const { position, quaternion } = poseToTransform(joint.origin.xyz, joint.origin.rpy);
    const pivot = new Group();
    pivot.name = `joint:${joint.name}`;
    pivot.position.copy(position);
    pivot.quaternion.copy(quaternion);
    parentLink.add(pivot);
    pivot.add(childLink);

    joints.set(joint.name, {
      name: joint.name,
      type: joint.type,
      limit: joint.limit,
      pivot,
      axis: new Vector3(...joint.axis).normalize(),
      originPosition: position.clone(),
      originQuaternion: quaternion.clone(),
    });
  }

  const endEffectorLink = urdf.joints.length > 0 ? urdf.joints[urdf.joints.length - 1].child : urdf.rootLink;

  const model: RobotModel = {
    root,
    links,
    joints,
    jointOrder: urdf.joints.map((joint) => joint.name),
    endEffectorLink,
    setJointValue(name, value) {
      const entry = joints.get(name);
      if (!entry) return;
      const clamped = entry.limit
        ? Math.min(entry.limit.upper, Math.max(entry.limit.lower, value))
        : value;
      if (entry.type === 'fixed') return;
      if (entry.type === 'prismatic') {
        scratchPosition.copy(entry.axis).multiplyScalar(clamped).applyQuaternion(entry.originQuaternion);
        entry.pivot.position.copy(entry.originPosition).add(scratchPosition);
        return;
      }
      const motion = new Quaternion().setFromAxisAngle(entry.axis, clamped);
      entry.pivot.quaternion.copy(entry.originQuaternion).multiply(motion);
    },
    setJointValues(values) {
      for (const [name, value] of Object.entries(values)) {
        model.setJointValue(name, value);
      }
    },
    updateWorld() {
      root.updateMatrixWorld(true);
    },
    getLinkWorldPosition(linkName) {
      const link = links.get(linkName);
      if (!link) throw new Error(`URDF error: unknown link ${linkName}`);
      return link.getWorldPosition(new Vector3());
    },
    getEndEffectorPosition() {
      return model.getLinkWorldPosition(endEffectorLink);
    },
    clonePose(opacity) {
      // Object3D.clone shares material REFERENCES (geometries too), so the
      // ghost needs its own cloned materials — otherwise making the ghost
      // transparent would turn the live model transparent as well.
      const clone = root.clone(true);
      clone.traverse((obj) => {
        const mesh = obj as { material?: Material | Material[] };
        const material = mesh.material;
        if (material && !Array.isArray(material)) {
          const cloned = material.clone();
          cloned.transparent = true;
          cloned.opacity = opacity;
          cloned.depthWrite = false;
          mesh.material = cloned;
        }
      });
      return clone;
    },
  };
  return model;
}

/** Debug helper: the root transform of a model (tests only). */
export function rootMatrix(model: RobotModel): Matrix4 {
  model.updateWorld();
  return model.root.matrixWorld.clone();
}
