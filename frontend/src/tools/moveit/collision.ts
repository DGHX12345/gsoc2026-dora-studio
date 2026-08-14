// Collision scene helpers (M13 D5) — yellow wireframe overlays (rviz
// convention) for planning-scene primitives and a bounding-sphere
// collision check (the plan's "simple" scope: pairwise object spheres,
// no full mesh collision).

import {
  BoxGeometry,
  CylinderGeometry,
  LineBasicMaterial,
  LineSegments,
  SphereGeometry,
  WireframeGeometry,
} from 'three';
import type { SceneObject } from './types';

/** RViz-convention collision wireframe color. */
export const COLLISION_WIREFRAME_COLOR = 0xfacc15;

export interface BoundingSphere {
  center: [number, number, number];
  radius: number;
}

/** Bounding sphere of a scene primitive:
 * sphere → r; box → half-diagonal; cylinder → sqrt(r² + (h/2)²). */
export function boundingSphereOf(obj: SceneObject): BoundingSphere {
  if (obj.type === 'sphere') {
    return { center: obj.position, radius: obj.dimensions[0] ?? 0 };
  }
  if (obj.type === 'box') {
    const [sx = 0, sy = 0, sz = 0] = obj.dimensions;
    return { center: obj.position, radius: Math.hypot(sx, sy, sz) / 2 };
  }
  const [r = 0, h = 0] = obj.dimensions;
  return { center: obj.position, radius: Math.hypot(r, h / 2) };
}

export interface CollisionPair {
  a: string;
  b: string;
  distance: number;
}

/** Pairwise bounding-sphere overlaps among world objects. */
export function findCollisions(objects: SceneObject[]): CollisionPair[] {
  const spheres = objects.map((obj) => ({ name: obj.name, ...boundingSphereOf(obj) }));
  const pairs: CollisionPair[] = [];
  for (let i = 0; i < spheres.length; i++) {
    for (let j = i + 1; j < spheres.length; j++) {
      const a = spheres[i];
      const b = spheres[j];
      const distance = Math.hypot(
        a.center[0] - b.center[0],
        a.center[1] - b.center[1],
        a.center[2] - b.center[2],
      );
      if (distance < a.radius + b.radius) {
        pairs.push({ a: a.name, b: b.name, distance });
      }
    }
  }
  return pairs;
}

/** Yellow wireframe LineSegments for a scene primitive, positioned at the
 * object's location. */
export function buildWireframeMesh(obj: SceneObject): LineSegments {
  let solid: BoxGeometry | SphereGeometry | CylinderGeometry;
  if (obj.type === 'sphere') {
    solid = new SphereGeometry(obj.dimensions[0] ?? 0.1, 16, 12);
  } else if (obj.type === 'box') {
    solid = new BoxGeometry(obj.dimensions[0] ?? 0.1, obj.dimensions[1] ?? 0.1, obj.dimensions[2] ?? 0.1);
  } else {
    const radius = obj.dimensions[0] ?? 0.05;
    solid = new CylinderGeometry(radius, radius, obj.dimensions[1] ?? 0.1, 24);
  }
  const wireframe = new WireframeGeometry(solid);
  solid.dispose(); // the wireframe copies the edges; the solid is scratch
  const material = new LineBasicMaterial({ color: COLLISION_WIREFRAME_COLOR });
  const mesh = new LineSegments(wireframe, material);
  mesh.name = `collision-${obj.name}`;
  mesh.position.set(obj.position[0], obj.position[1], obj.position[2]);
  return mesh;
}

/** Dispose a wireframe mesh built by buildWireframeMesh. */
export function disposeWireframeMesh(mesh: LineSegments) {
  mesh.geometry.dispose();
  (mesh.material as LineBasicMaterial).dispose();
}
