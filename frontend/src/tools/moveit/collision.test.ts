// Collision scene helpers (M13 D5): bounding-sphere math + wireframe
// building. Self-executes on import — see tests.ts.

import assert from 'node:assert/strict';
import { LineBasicMaterial, LineSegments } from 'three';

import { boundingSphereOf, buildWireframeMesh, findCollisions } from './collision';
import type { SceneObject } from './types';

type TestCase = {
  name: string;
  run: () => void;
};

const sphere = (name: string, x: number, y: number, z: number, r: number): SceneObject => ({
  name,
  type: 'sphere',
  position: [x, y, z],
  dimensions: [r],
  color: [1, 1, 0, 1],
});

const box = (name: string, x: number, y: number, z: number, sx: number, sy: number, sz: number): SceneObject => ({
  name,
  type: 'box',
  position: [x, y, z],
  dimensions: [sx, sy, sz],
  color: [1, 1, 0, 1],
});

const cylinder = (name: string, x: number, y: number, z: number, r: number, h: number): SceneObject => ({
  name,
  type: 'cylinder',
  position: [x, y, z],
  dimensions: [r, h],
  color: [1, 1, 0, 1],
});

const tests: TestCase[] = [
  {
    name: 'boundingSphereOf derives radii per primitive type',
    run: () => {
      assert.deepEqual(boundingSphereOf(sphere('s', 1, 2, 3, 0.5)), {
        center: [1, 2, 3],
        radius: 0.5,
      });
      // box: half-diagonal of (0.8, 0.6, 0.4) = sqrt(0.16+0.09+0.04) = sqrt(0.29)
      const b = boundingSphereOf(box('b', 0, 0, 0, 0.8, 0.6, 0.4));
      assert.ok(Math.abs(b.radius - Math.sqrt(0.29)) < 1e-12);
      // cylinder: sqrt(r² + (h/2)²) = sqrt(0.04 + 0.09) = sqrt(0.13)
      const c = boundingSphereOf(cylinder('c', 0, 0, 0, 0.2, 0.6));
      assert.ok(Math.abs(c.radius - Math.sqrt(0.13)) < 1e-12);
    },
  },
  {
    name: 'findCollisions reports overlapping bounding spheres and skips separated ones',
    run: () => {
      const objects = [
        sphere('a', 0, 0, 0, 0.3),
        sphere('b', 0.5, 0, 0, 0.3), // distance 0.5 < 0.6 → overlap
        sphere('c', 2, 2, 2, 0.1), // far away
      ];
      const collisions = findCollisions(objects);
      assert.equal(collisions.length, 1);
      assert.equal(collisions[0].a, 'a');
      assert.equal(collisions[0].b, 'b');
      assert.ok(Math.abs(collisions[0].distance - 0.5) < 1e-12);
    },
  },
  {
    name: 'findCollisions returns an empty list for clear scenes',
    run: () => {
      assert.deepEqual(findCollisions([]), []);
      assert.deepEqual(findCollisions([sphere('a', 0, 0, 0, 0.1), box('b', 1, 1, 1, 0.1, 0.1, 0.1)]), []);
    },
  },
  {
    name: 'buildWireframeMesh produces yellow LineSegments per primitive',
    run: () => {
      for (const obj of [sphere('s', 0, 0, 0, 0.1), box('b', 0, 0, 0, 0.1, 0.2, 0.3), cylinder('c', 0, 0, 0, 0.05, 0.2)]) {
        const mesh = buildWireframeMesh(obj);
        assert.ok(mesh instanceof LineSegments);
        assert.equal(mesh.name, `collision-${obj.name}`);
        assert.equal((mesh.material as LineBasicMaterial).color.getHex(), 0xfacc15); // rviz yellow
        assert.equal(mesh.position.x, obj.position[0]);
        assert.ok(mesh.geometry.getAttribute('position').count > 0);
        mesh.geometry.dispose();
        (mesh.material as LineBasicMaterial).dispose();
      }
    },
  },
];

let failures = 0;

for (const test of tests) {
  try {
    test.run();
    console.log(`ok - ${test.name}`);
  } catch (error) {
    failures += 1;
    console.error(`not ok - ${test.name}`);
    console.error(error);
  }
}

if (failures > 0) {
  process.exitCode = 1;
}
