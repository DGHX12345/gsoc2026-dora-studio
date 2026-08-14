// URDF parsing (M13 D4) — link/joint tree extraction for the tool-owned
// robot model. Only <visual> geometry matters for rendering; collision and
// inertial elements are skipped.

import { parseXml, type XmlElement } from './xml';

export type JointType = 'fixed' | 'revolute' | 'continuous' | 'prismatic';

export interface UrdfPose {
  xyz: [number, number, number];
  rpy: [number, number, number];
}

export type UrdfGeometry =
  | { kind: 'mesh'; filename: string }
  | { kind: 'box'; size: [number, number, number] }
  | { kind: 'cylinder'; radius: number; length: number }
  | { kind: 'sphere'; radius: number };

export interface UrdfVisual {
  origin: UrdfPose;
  geometry: UrdfGeometry;
  /** RGBA from <material><color>; null when absent (mesh builder defaults). */
  color: [number, number, number, number] | null;
}

export interface UrdfLink {
  name: string;
  visuals: UrdfVisual[];
}

export interface UrdfJoint {
  name: string;
  type: JointType;
  parent: string;
  child: string;
  origin: UrdfPose;
  axis: [number, number, number];
  limit: { lower: number; upper: number } | null;
}

export interface UrdfRobot {
  name: string;
  links: Map<string, UrdfLink>;
  /** Document order. */
  joints: UrdfJoint[];
  /** The link no joint points to as a child. */
  rootLink: string;
}

const JOINT_TYPES: JointType[] = ['fixed', 'revolute', 'continuous', 'prismatic'];
const IDENTITY_POSE: UrdfPose = { xyz: [0, 0, 0], rpy: [0, 0, 0] };

export function parseUrdf(xmlText: string): UrdfRobot {
  const root = parseXml(xmlText);
  if (root.name !== 'robot') throw new Error(`URDF parse error: root element is <${root.name}>, expected <robot>`);

  const links = new Map<string, UrdfLink>();
  const joints: UrdfJoint[] = [];

  for (const element of root.children) {
    if (element.name === 'link') {
      const link = parseLink(element);
      links.set(link.name, link);
    } else if (element.name === 'joint') {
      joints.push(parseJoint(element));
    }
  }

  const childLinks = new Set(joints.map((joint) => joint.child));
  const rootLink = [...links.keys()].find((name) => !childLinks.has(name));
  if (!rootLink) throw new Error('URDF parse error: no root link found (every link is a joint child)');

  return { name: root.attributes.name ?? '', links, joints, rootLink };
}

function parseLink(element: XmlElement): UrdfLink {
  const name = element.attributes.name;
  if (!name) throw new Error('URDF parse error: <link> without a name');

  const visuals: UrdfVisual[] = [];
  for (const child of element.children) {
    if (child.name !== 'visual') continue;
    visuals.push(parseVisual(child));
  }
  return { name, visuals };
}

function parseVisual(element: XmlElement): UrdfVisual {
  let origin = IDENTITY_POSE;
  let geometry: UrdfGeometry | null = null;
  let color: [number, number, number, number] | null = null;

  for (const child of element.children) {
    if (child.name === 'origin') {
      origin = parsePose(child);
    } else if (child.name === 'geometry') {
      geometry = parseGeometry(child);
    } else if (child.name === 'material') {
      for (const grandchild of child.children) {
        if (grandchild.name === 'color') {
          color = parseNumberTuple(grandchild.attributes.rgba ?? '', 4) as [number, number, number, number];
        }
      }
    }
  }
  if (!geometry) throw new Error('URDF parse error: <visual> without <geometry>');
  return { origin, geometry, color };
}

function parseGeometry(element: XmlElement): UrdfGeometry {
  const child = element.children[0];
  if (!child) throw new Error('URDF parse error: empty <geometry>');
  const attrs = child.attributes;
  switch (child.name) {
    case 'mesh': {
      const filename = attrs.filename;
      if (!filename) throw new Error('URDF parse error: <mesh> without filename');
      return { kind: 'mesh', filename };
    }
    case 'box':
      return { kind: 'box', size: parseNumberTuple(attrs.size ?? '', 3) as [number, number, number] };
    case 'cylinder':
      return { kind: 'cylinder', radius: parseNumber(attrs.radius, 'cylinder radius'), length: parseNumber(attrs.length, 'cylinder length') };
    case 'sphere':
      return { kind: 'sphere', radius: parseNumber(attrs.radius, 'sphere radius') };
    default:
      throw new Error(`URDF parse error: unsupported geometry <${child.name}>`);
  }
}

function parseJoint(element: XmlElement): UrdfJoint {
  const name = element.attributes.name;
  const type = element.attributes.type as JointType | undefined;
  if (!name) throw new Error('URDF parse error: <joint> without a name');
  if (!type || !JOINT_TYPES.includes(type)) {
    throw new Error(`URDF parse error: unknown joint type "${type ?? ''}" on ${name}`);
  }

  let origin = IDENTITY_POSE;
  let parent: string | null = null;
  let child: string | null = null;
  let axis: [number, number, number] = [1, 0, 0];
  let limit: { lower: number; upper: number } | null = null;

  for (const sub of element.children) {
    if (sub.name === 'origin') {
      origin = parsePose(sub);
    } else if (sub.name === 'parent') {
      parent = sub.attributes.link ?? null;
    } else if (sub.name === 'child') {
      child = sub.attributes.link ?? null;
    } else if (sub.name === 'axis') {
      axis = parseNumberTuple(sub.attributes.xyz ?? '1 0 0', 3) as [number, number, number];
    } else if (sub.name === 'limit') {
      limit = {
        lower: parseNumber(sub.attributes.lower, `${name} lower limit`),
        upper: parseNumber(sub.attributes.upper, `${name} upper limit`),
      };
    }
  }
  if (!parent) throw new Error(`URDF parse error: joint ${name} has no <parent>`);
  if (!child) throw new Error(`URDF parse error: joint ${name} has no <child>`);
  return { name, type, parent, child, origin, axis, limit };
}

function parsePose(element: XmlElement): UrdfPose {
  return {
    xyz: parseNumberTuple(element.attributes.xyz ?? '0 0 0', 3) as [number, number, number],
    rpy: parseNumberTuple(element.attributes.rpy ?? '0 0 0', 3) as [number, number, number],
  };
}

function parseNumber(value: string | undefined, what: string): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) throw new Error(`URDF parse error: invalid ${what} "${value}"`);
  return parsed;
}

/** Space-separated number tuple with a required length; throws when any
 * entry is not finite. */
export function parseNumberTuple(value: string, length: number): number[] {
  const parts = value.trim().split(/\s+/).filter(Boolean).map(Number);
  if (parts.length !== length || parts.some((n) => !Number.isFinite(n))) {
    throw new Error(`URDF parse error: expected ${length} numbers in "${value}"`);
  }
  return parts;
}
