// Minimal TF tree — mirrors dviz-core's StampedTransform model
// (parent_frame / child_frame / translation / rotation xyzw), confirmed
// against dviz-core/src/types/transform.rs and dviz-rosbag/src/tf.rs.
// dora has no official TF message; TF arrives as JSON payloads carrying
// `{ transforms: [TfStamped...] }`.

import { Matrix4, Quaternion, Vector3 } from 'three';

export interface TfStamped {
  parent: string;
  child: string;
  translation: [number, number, number];
  rotation: [number, number, number, number]; // xyzw, ROS / glam order
  timestampNs?: number;
}

export interface TfTree {
  readonly frames: ReadonlyMap<string, { parent: string; transform: Matrix4 }>;
  getTransform(from: string, to: string): Matrix4 | null;
  apply(entries: TfStamped[]): void;
}

const normalizeFrame = (id: string) => id.replace(/^\/+/, '');

export class SimpleTfTree implements TfTree {
  private _frames = new Map<string, { parent: string; transform: Matrix4 }>();
  private _known = new Set<string>();

  get frames(): ReadonlyMap<string, { parent: string; transform: Matrix4 }> {
    return this._frames;
  }

  apply(entries: TfStamped[]) {
    for (const e of entries) {
      if (!e.parent || !e.child) continue;
      if (!Array.isArray(e.translation) || e.translation.length !== 3) continue;
      if (!Array.isArray(e.rotation) || e.rotation.length !== 4) continue;
      if ([...e.translation, ...e.rotation].some((n) => typeof n !== 'number')) continue;

      const parent = normalizeFrame(e.parent);
      const child = normalizeFrame(e.child);
      const matrix = new Matrix4().compose(
        new Vector3(e.translation[0], e.translation[1], e.translation[2]),
        new Quaternion(e.rotation[0], e.rotation[1], e.rotation[2], e.rotation[3]),
        new Vector3(1, 1, 1),
      );

      this._frames.set(child, { parent, transform: matrix });
      this._known.add(parent);
      this._known.add(child);
    }
  }

  getTransform(from: string, to: string): Matrix4 | null {
    const fromKey = normalizeFrame(from);
    const toKey = normalizeFrame(to);
    if (!this._known.has(fromKey) || !this._known.has(toKey)) return null;

    const worldFrom = this.worldTransform(fromKey, new Set());
    const worldTo = this.worldTransform(toKey, new Set());
    if (worldFrom === null || worldTo === null) return null;

    return new Matrix4().copy(worldFrom).invert().multiply(worldTo);
  }

  private worldTransform(frame: string, seen: Set<string>): Matrix4 | null {
    if (seen.has(frame)) return null; // cycle detection
    seen.add(frame);

    const entry = this._frames.get(frame);
    if (!entry) return new Matrix4(); // root frame: identity in world

    const parent = this.worldTransform(entry.parent, seen);
    if (parent === null) return null;
    return new Matrix4().copy(parent).multiply(entry.transform);
  }
}

export function parseTfPayload(json: unknown): TfStamped[] {
  if (typeof json !== 'object' || json === null) return [];
  const transforms = (json as { transforms?: unknown }).transforms;
  if (!Array.isArray(transforms)) return [];

  const out: TfStamped[] = [];
  for (const item of transforms) {
    if (typeof item !== 'object' || item === null) continue;
    const t = item as Record<string, unknown>;
    const parent = typeof t.parent === 'string' ? t.parent : '';
    const child = typeof t.child === 'string' ? t.child : '';
    const { translation, rotation } = t;
    if (!parent || !child) continue;
    if (!Array.isArray(translation) || translation.length !== 3) continue;
    if (!Array.isArray(rotation) || rotation.length !== 4) continue;
    if ([...translation, ...rotation].some((n) => typeof n !== 'number')) continue;

    out.push({
      parent,
      child,
      translation: [translation[0], translation[1], translation[2]],
      rotation: [rotation[0], rotation[1], rotation[2], rotation[3]],
    });
  }
  return out;
}
