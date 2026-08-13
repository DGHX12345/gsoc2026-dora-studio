// Flat dviz path parser tests (M12 D1). Self-executes on import — see tests.ts.

import assert from 'node:assert/strict';

import type { ToolPayload } from '../types';
import {
  computePathLength,
  parseCostmap,
  parseTarget,
  parseTrajectory,
  parseWaypoints,
} from './parse';

type TestCase = {
  name: string;
  run: () => void;
};

const Z = 0.05;

const f32Payload = (values: number[]): ToolPayload => ({ f32: Float32Array.from(values) });
const jsonPayload = (json: unknown): ToolPayload => ({ json });

const tests: TestCase[] = [
  {
    name: 'parseWaypoints converts a flat even-length f32 array to xyz triplets',
    run: () => {
      const points = parseWaypoints(f32Payload([1, 2, 3, 4, 5, 6]));
      assert.deepEqual(points, [1, 2, Z, 3, 4, Z, 5, 6, Z]);
    },
  },
  {
    name: 'parseWaypoints drops the trailing orphan value on odd-length input',
    run: () => {
      assert.deepEqual(parseWaypoints(f32Payload([1, 2, 3])), [1, 2, Z]);
      assert.deepEqual(parseWaypoints(f32Payload([1, 2, 3, 4, 5])), [1, 2, Z, 3, 4, Z]);
      assert.deepEqual(parseWaypoints(f32Payload([1])), []);
    },
  },
  {
    name: 'parseWaypoints accepts the nested { waypoints: [[x, y], ...] } json form',
    run: () => {
      const points = parseWaypoints(jsonPayload({ waypoints: [[1, 2], [3, 4]] }));
      assert.deepEqual(points, [1, 2, Z, 3, 4, Z]);
    },
  },
  {
    name: 'parseWaypoints falls back to the json number-array form without f32',
    run: () => {
      assert.deepEqual(parseWaypoints(jsonPayload([1, 2, 3, 4])), [1, 2, Z, 3, 4, Z]);
      assert.deepEqual(parseWaypoints(jsonPayload([1, 2, 3])), [1, 2, Z]);
    },
  },
  {
    name: 'parseWaypoints returns [] for invalid payloads',
    run: () => {
      assert.deepEqual(parseWaypoints(jsonPayload(['a', 'b'])), []);
      assert.deepEqual(parseWaypoints(jsonPayload({ waypoints: 'nope' })), []);
      assert.deepEqual(parseWaypoints(jsonPayload({ waypoints: [[1]] })), []);
      assert.deepEqual(parseWaypoints(jsonPayload({ waypoints: [['a', 'b']] })), []);
      assert.deepEqual(parseWaypoints(jsonPayload({ waypoints: [[1, 'a']] })), []);
      assert.deepEqual(parseWaypoints(jsonPayload(null)), []);
      assert.deepEqual(parseWaypoints({}), []);
      assert.deepEqual(parseWaypoints(f32Payload([NaN, 2, 3, 4])), []);
    },
  },
  {
    name: 'parseTrajectory passes stride-3 data through unchanged',
    run: () => {
      const flat = [1, 2, 3, 4, 5, 6];
      assert.deepEqual(parseTrajectory(f32Payload(flat)), flat);
    },
  },
  {
    name: 'parseTrajectory keeps only xyz for stride-7 data (quaternion dropped)',
    run: () => {
      // two pose samples: xyz + quaternion (x, y, z, w)
      const flat = [1, 2, 3, 0, 0, 0, 1, 4, 5, 6, 0, 0, 0, 1];
      assert.deepEqual(parseTrajectory(f32Payload(flat)), [1, 2, 3, 4, 5, 6]);
    },
  },
  {
    name: 'parseTrajectory returns [] when the length matches neither stride',
    run: () => {
      assert.deepEqual(parseTrajectory(f32Payload([1, 2, 3, 4, 5])), []);
      assert.deepEqual(parseTrajectory(f32Payload([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])), []);
      assert.deepEqual(parseTrajectory(f32Payload([])), []);
    },
  },
  {
    name: 'parseTrajectory accepts the json number-array form',
    run: () => {
      assert.deepEqual(parseTrajectory(jsonPayload([1, 2, 3, 4, 5, 6])), [1, 2, 3, 4, 5, 6]);
      assert.deepEqual(parseTrajectory(jsonPayload([1, 2, 3, 0, 0, 0, 1, 4, 5, 6, 0, 0, 0, 1])), [1, 2, 3, 4, 5, 6]);
      assert.deepEqual(parseTrajectory(jsonPayload(['a', 'b'])), []);
    },
  },
  {
    name: 'parseTarget lifts a flat [x, y] pair to z = 0.05',
    run: () => {
      assert.deepEqual(parseTarget(f32Payload([1.5, -2.5])), { x: 1.5, y: -2.5, z: Z });
    },
  },
  {
    name: 'parseTarget passes [x, y, z] through unchanged',
    run: () => {
      assert.deepEqual(parseTarget(f32Payload([1, 2, 3])), { x: 1, y: 2, z: 3 });
    },
  },
  {
    name: 'parseTarget accepts the json number-array form',
    run: () => {
      assert.deepEqual(parseTarget(jsonPayload([4, 5])), { x: 4, y: 5, z: Z });
      assert.deepEqual(parseTarget(jsonPayload([4, 5, 6])), { x: 4, y: 5, z: 6 });
    },
  },
  {
    name: 'parseTarget returns null for invalid payloads',
    run: () => {
      assert.equal(parseTarget(f32Payload([])), null);
      assert.equal(parseTarget(f32Payload([1])), null);
      assert.equal(parseTarget(f32Payload([1, 2, 3, 4])), null);
      assert.equal(parseTarget(jsonPayload(['a', 'b'])), null);
      assert.equal(parseTarget(jsonPayload({ x: 1, y: 2 })), null);
      assert.equal(parseTarget({}), null);
    },
  },
  {
    name: 'parseCostmap normalizes a valid costmap object',
    run: () => {
      const map = parseCostmap(
        jsonPayload({ width: 2, height: 3, resolution: 0.1, values: [0, 1, 2, 3, 4, 5] }),
      );
      assert.ok(map);
      assert.equal(map.width, 2);
      assert.equal(map.height, 3);
      assert.equal(map.resolution, 0.1);
      assert.ok(map.values instanceof Float32Array);
      assert.deepEqual([...map.values], [0, 1, 2, 3, 4, 5]);
    },
  },
  {
    name: 'parseCostmap rejects a mismatched values length',
    run: () => {
      const map = parseCostmap(
        jsonPayload({ width: 2, height: 2, resolution: 0.1, values: [0, 1, 2] }),
      );
      assert.equal(map, null);
    },
  },
  {
    name: 'parseCostmap rejects missing fields',
    run: () => {
      assert.equal(parseCostmap(jsonPayload({ width: 2, height: 2, resolution: 0.1 })), null);
      assert.equal(parseCostmap(jsonPayload({ width: 2, resolution: 0.1, values: [0, 1, 2, 3] })), null);
      assert.equal(parseCostmap(jsonPayload({ width: 2, height: 2, values: [0, 1, 2, 3] })), null);
      assert.equal(parseCostmap(jsonPayload({})), null);
      assert.equal(parseCostmap({}), null);
      assert.equal(parseCostmap(jsonPayload([0, 1, 2, 3])), null);
    },
  },
  {
    name: 'parseCostmap rejects non-integer or non-positive dimensions and bad resolution',
    run: () => {
      assert.equal(
        parseCostmap(jsonPayload({ width: 2.5, height: 2, resolution: 0.1, values: [0, 1, 2, 3, 4] })),
        null,
      );
      assert.equal(parseCostmap(jsonPayload({ width: 0, height: 2, resolution: 0.1, values: [] })), null);
      assert.equal(parseCostmap(jsonPayload({ width: 2, height: 0, resolution: 0.1, values: [] })), null);
      assert.equal(parseCostmap(jsonPayload({ width: 2, height: 2, resolution: 0, values: [0, 1, 2, 3] })), null);
      assert.equal(parseCostmap(jsonPayload({ width: 2, height: 2, resolution: -0.5, values: [0, 1, 2, 3] })), null);
      assert.equal(parseCostmap(jsonPayload({ width: 2, height: 2, resolution: 0.1, values: ['a', 'b', 'c', 'd'] })), null);
    },
  },
  {
    name: 'computePathLength returns 0 for empty or single-point paths',
    run: () => {
      assert.equal(computePathLength([]), 0);
      assert.equal(computePathLength([1, 2, 0.05]), 0);
    },
  },
  {
    name: 'computePathLength sums segment lengths over xyz triplets',
    run: () => {
      // (0, 0) → (3, 4): length 5
      assert.equal(computePathLength([0, 0, 0, 3, 4, 0]), 5);
    },
  },
  {
    name: 'computePathLength totals a closed 3-4-5 triangle to its perimeter 12',
    run: () => {
      // (0,0) → (3,0) → (0,4) → (0,0): 3 + 5 + 4 = 12
      assert.equal(computePathLength([0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 0]), 12);
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
