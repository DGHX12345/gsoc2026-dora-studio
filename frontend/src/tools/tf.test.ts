import assert from 'node:assert/strict';

import { SimpleTfTree, parseTfPayload, type TfStamped } from './tf';

const EPSILON = 1e-5;

type TestCase = {
  name: string;
  run: () => void;
};

const assertClose = (actual: number, expected: number) => {
  assert.ok(
    Math.abs(actual - expected) < EPSILON,
    `expected ${actual} to be within ${EPSILON} of ${expected}`,
  );
};

const assertTranslation = (
  matrix: { elements: number[] },
  expected: [number, number, number],
) => {
  assertClose(matrix.elements[12], expected[0]);
  assertClose(matrix.elements[13], expected[1]);
  assertClose(matrix.elements[14], expected[2]);
};

const tf = (partial: Partial<TfStamped> & { parent: string; child: string }): TfStamped => ({
  translation: [0, 0, 0],
  rotation: [0, 0, 0, 1],
  ...partial,
});

const tests: TestCase[] = [
  {
    name: 'apply composes translation into a parent-to-child matrix',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: 'map', child: 'base', translation: [1, 2, 3] })]);

      const m = tree.getTransform('map', 'base');
      assert.ok(m, 'expected transform for known frames');
      assertTranslation(m, [1, 2, 3]);
    },
  },
  {
    name: 'apply composes a z-axis rotation into the matrix',
    run: () => {
      const tree = new SimpleTfTree();
      // 90 degrees about z: quaternion (x, y, z, w)
      tree.apply([tf({ parent: 'map', child: 'base', rotation: [0, 0, Math.SQRT1_2, Math.SQRT1_2] })]);

      const m = tree.getTransform('map', 'base');
      assert.ok(m);
      // x axis rotated to +y
      assertClose(m.elements[0], 0);
      assertClose(m.elements[1], 1);
      assertClose(m.elements[2], 0);
      // y axis rotated to -x
      assertClose(m.elements[4], -1);
      assertClose(m.elements[5], 0);
    },
  },
  {
    name: 'getTransform walks a multi-level parent chain',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([
        tf({ parent: 'map', child: 'odom', translation: [1, 0, 0] }),
        tf({ parent: 'odom', child: 'base', translation: [0, 2, 0] }),
      ]);

      const m = tree.getTransform('map', 'base');
      assert.ok(m);
      assertTranslation(m, [1, 2, 0]);
    },
  },
  {
    name: 'getTransform returns the inverse for the reverse direction',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: 'map', child: 'base', translation: [1, 2, 3] })]);

      const m = tree.getTransform('base', 'map');
      assert.ok(m);
      assertTranslation(m, [-1, -2, -3]);
    },
  },
  {
    name: 'getTransform returns identity for the same frame',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: 'map', child: 'base', translation: [5, 5, 5] })]);

      const m = tree.getTransform('base', 'base');
      assert.ok(m);
      assertTranslation(m, [0, 0, 0]);
      assertClose(m.elements[0], 1);
      assertClose(m.elements[5], 1);
      assertClose(m.elements[10], 1);
    },
  },
  {
    name: 'getTransform returns null for unknown frames',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: 'map', child: 'base' })]);

      assert.equal(tree.getTransform('map', 'nope'), null);
      assert.equal(tree.getTransform('nope', 'base'), null);
    },
  },
  {
    name: 'a frame only seen as a parent is a valid root',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: 'world', child: 'base', translation: [3, 0, 0] })]);

      const m = tree.getTransform('world', 'base');
      assert.ok(m);
      assertTranslation(m, [3, 0, 0]);
    },
  },
  {
    name: 'leading slashes in frame names are normalized',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: '/map', child: '/base', translation: [1, 0, 0] })]);

      const m = tree.getTransform('map', 'base');
      assert.ok(m);
      assertTranslation(m, [1, 0, 0]);
    },
  },
  {
    name: 'a cycle in the frame graph terminates and returns null',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([
        tf({ parent: 'b', child: 'a' }),
        tf({ parent: 'a', child: 'b' }),
      ]);

      assert.equal(tree.getTransform('a', 'b'), null);
      assert.equal(tree.getTransform('a', 'a'), null);
    },
  },
  {
    name: 'apply updates an existing frame transform',
    run: () => {
      const tree = new SimpleTfTree();
      tree.apply([tf({ parent: 'map', child: 'base', translation: [1, 0, 0] })]);
      tree.apply([tf({ parent: 'map', child: 'base', translation: [0, 4, 0] })]);

      const m = tree.getTransform('map', 'base');
      assert.ok(m);
      assertTranslation(m, [0, 4, 0]);
    },
  },
  {
    name: 'parseTfPayload accepts a ROS-style transforms array',
    run: () => {
      const entries = parseTfPayload({
        transforms: [
          { parent: 'map', child: 'base', translation: [1, 2, 3], rotation: [0, 0, 0, 1] },
        ],
      });

      assert.equal(entries.length, 1);
      assert.equal(entries[0].parent, 'map');
      assert.equal(entries[0].child, 'base');
      assert.deepEqual(entries[0].translation, [1, 2, 3]);
      assert.deepEqual(entries[0].rotation, [0, 0, 0, 1]);
    },
  },
  {
    name: 'parseTfPayload skips malformed entries',
    run: () => {
      const entries = parseTfPayload({
        transforms: [
          { parent: 'map', child: 'base', translation: [1, 2, 3], rotation: [0, 0, 0, 1] },
          { parent: '', child: 'x', translation: [0, 0, 0], rotation: [0, 0, 0, 1] },
          { parent: 'map', child: 'y', translation: 'not-an-array', rotation: [0, 0, 0, 1] },
          { parent: 'map', child: 'z', translation: [0, 0, 0], rotation: [0, 0, 1] },
          'not an object',
        ],
      });

      assert.equal(entries.length, 1);
      assert.equal(entries[0].child, 'base');
    },
  },
  {
    name: 'parseTfPayload returns an empty list for non-transform payloads',
    run: () => {
      assert.deepEqual(parseTfPayload(null), []);
      assert.deepEqual(parseTfPayload('hello'), []);
      assert.deepEqual(parseTfPayload({ joints: {} }), []);
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
