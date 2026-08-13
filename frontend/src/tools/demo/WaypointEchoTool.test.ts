import assert from 'node:assert/strict';
import { Group, Scene } from 'three';

import { WaypointEchoTool, parseWaypointPoints, type WaypointStats } from './WaypointEchoTool';
import type { ToolContext, ToolPayload } from '../types';

type TestCase = {
  name: string;
  run: () => void;
};

const makeContext = (): ToolContext => {
  const scene = new Scene();
  return { scene, camera: {} as never, requestRender: () => {} };
};

const tests: TestCase[] = [
  {
    name: 'parseWaypointPoints passes a flat stride-3 f32 array through',
    run: () => {
      const payload: ToolPayload = { f32: new Float32Array([0, 1, 2, 3, 4, 5]) };

      assert.deepEqual(parseWaypointPoints(payload), [0, 1, 2, 3, 4, 5]);
    },
  },
  {
    name: 'parseWaypointPoints lifts xy pairs to z=0.05',
    run: () => {
      const payload: ToolPayload = { f32: new Float32Array([0, 1, 2, 3]) };

      assert.deepEqual(parseWaypointPoints(payload), [0, 1, 0.05, 2, 3, 0.05]);
    },
  },
  {
    name: 'parseWaypointPoints reads a waypoints JSON object',
    run: () => {
      const payload: ToolPayload = { json: { waypoints: [[1, 2], [3, 4]] } };

      assert.deepEqual(parseWaypointPoints(payload), [1, 2, 0.05, 3, 4, 0.05]);
    },
  },
  {
    name: 'parseWaypointPoints reads a flat JSON number array',
    run: () => {
      const payload: ToolPayload = { json: [0, 1, 2, 3, 4, 5] };

      assert.deepEqual(parseWaypointPoints(payload), [0, 1, 2, 3, 4, 5]);
    },
  },
  {
    name: 'parseWaypointPoints skips malformed waypoint rows',
    run: () => {
      const payload: ToolPayload = { json: { waypoints: [[1, 2], ['x', 'y'], [3]] } };

      assert.deepEqual(parseWaypointPoints(payload), [1, 2, 0.05]);
    },
  },
  {
    name: 'parseWaypointPoints returns an empty list for unusable payloads',
    run: () => {
      assert.deepEqual(parseWaypointPoints({}), []);
      assert.deepEqual(parseWaypointPoints({ json: { joints: {} } }), []);
      assert.deepEqual(parseWaypointPoints({ f32: new Float32Array([1, 2, 3, 4, 5]) }), []);
      assert.deepEqual(parseWaypointPoints({ json: [1, 'two', 3] }), []);
    },
  },
  {
    name: 'the tool subscribes to the dviz waypoint and trajectory ports',
    run: () => {
      const tool = new WaypointEchoTool();

      assert.equal(tool.id, 'waypoint-echo');
      assert.equal(tool.category, 'visualization');
      assert.equal(tool.subscribePorts.length, 2);
      assert.ok(
        tool.subscribePorts.some(
          (spec) =>
            spec.nodeIdPattern === '*' &&
            spec.outputIdPattern instanceof RegExp &&
            (spec.outputIdPattern as RegExp).test('waypoints'),
        ),
      );
      assert.ok(
        tool.subscribePorts.some(
          (spec) =>
            spec.nodeIdPattern === '*' &&
            spec.outputIdPattern instanceof RegExp &&
            (spec.outputIdPattern as RegExp).test('trajectory'),
        ),
      );
    },
  },
  {
    name: 'onAttach adds its group to the scene and a batch builds the path',
    run: () => {
      const context = makeContext();
      const tool = new WaypointEchoTool();

      tool.onAttach(context);
      assert.equal(context.scene.children.length, 1);
      assert.ok(context.scene.children[0] instanceof Group);

      tool.onBatch(
        {
          nodeId: 'planner',
          outputId: 'waypoints',
          timestampNs: 100,
          payload: { json: { waypoints: [[0, 0], [1, 1], [2, 0]] } },
        },
        undefined,
      );
      assert.equal(tool.stats()!.pointCount, 3);
      assert.equal(tool.stats()!.lastSource, 'planner/waypoints');

      tool.onDetach();
      assert.equal(context.scene.children.length, 0);
      assert.equal(tool.stats(), null as unknown as WaypointStats | null);
    },
  },
  {
    name: 'a batch without waypoint data leaves the path empty',
    run: () => {
      const context = makeContext();
      const tool = new WaypointEchoTool();
      tool.onAttach(context);

      tool.onBatch(
        { nodeId: 'camera', outputId: 'image', timestampNs: 100, payload: { json: {} } },
        undefined,
      );

      assert.equal(tool.stats(), null as unknown as WaypointStats | null);
      tool.onDetach();
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
