// DvizPathTool tests (M12 D2). Self-executes on import — see tests.ts.
//
// Constructs a real THREE.Scene headlessly: three object construction
// (including Line2/LineMaterial) works without a renderer.

import assert from 'node:assert/strict';
import { Group, Mesh, Scene } from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';

import { matchToolPorts } from '../matching';
import type { ToolBatch, ToolContext, ToolPayload } from '../types';
import { computePathLength } from './parse';
import { computeStaleness, DvizPathTool } from './DvizPathTool';

type TestCase = {
  name: string;
  run: () => void;
};

const makeContext = (): ToolContext => {
  const scene = new Scene();
  return { scene, camera: {} as never, requestRender: () => {} };
};

const batch = (
  nodeId: string,
  outputId: string,
  timestampNs: number,
  payload: ToolPayload,
): ToolBatch => ({ nodeId, outputId, timestampNs, payload });

const f32 = (values: number[]): ToolPayload => ({ f32: Float32Array.from(values) });

const rootGroup = (context: ToolContext): Group =>
  context.scene.children[0] as Group;

const pathGroup = (context: ToolContext, name: string): Group =>
  rootGroup(context).children.find((c) => c.name === name) as Group;

const lineMaterialOf = (group: Group): LineMaterial =>
  (group.children[0] as Line2).material as LineMaterial;

const lineGeometryOf = (group: Group): Line2['geometry'] =>
  (group.children[0] as Line2).geometry;

const firstPointOf = (geometry: Line2['geometry']): number[] => {
  const start = geometry.attributes.instanceStart.array as Float32Array;
  return [start[0], start[1], start[2]];
};

const tests: TestCase[] = [
  {
    name: 'subscribePorts match dviz path output ids and reject unrelated ports',
    run: () => {
      const tool = new DvizPathTool();

      for (const outputId of [
        'waypoints',
        'path',
        'trajectory',
        'target_point',
        'target',
        'goal',
        'costmap',
        'esdf',
        'WAYPOINTS',
        'Trajectory',
      ]) {
        assert.ok(
          matchToolPorts(tool.subscribePorts, 'planner', outputId),
          `expected match on ${outputId}`,
        );
      }
      for (const outputId of ['image', 'joint_state', 'point_cloud', 'odom']) {
        assert.ok(
          !matchToolPorts(tool.subscribePorts, 'planner', outputId),
          `expected no match on ${outputId}`,
        );
      }
    },
  },
  {
    name: 'onAttach adds a group named dviz-path; second attach is a no-op',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();

      tool.onAttach(context);
      assert.equal(context.scene.children.length, 1);
      const group = rootGroup(context);
      assert.ok(group instanceof Group);
      assert.equal(group.name, 'dviz-path');

      tool.onAttach(context);
      assert.equal(context.scene.children.length, 1);
      assert.equal(tool.getSnapshot().paths.length, 0);
      tool.onDetach();
    },
  },
  {
    name: 'a waypoints batch creates one primary cyan path with z=0.05 line points',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      const points = [0, 0, 1, 1, 2, 0];
      tool.onBatch(batch('planner', 'waypoints', 100, f32(points)));

      const group = pathGroup(context, 'path:planner/waypoints');
      assert.ok(group instanceof Group);
      assert.equal(group.name, 'path:planner/waypoints');
      assert.ok(group.children[0] instanceof Line2);

      assert.equal(lineMaterialOf(group).color.getHex(), 0x22d3ee);
      // z lands in a Float32 attribute: 0.05 is not exactly representable.
      assert.ok(Math.abs(firstPointOf(lineGeometryOf(group))[2] - 0.05) < 1e-6);

      const info = tool.getSnapshot().paths[0];
      assert.equal(info.key, 'planner/waypoints');
      assert.equal(info.nodeId, 'planner');
      assert.equal(info.outputId, 'waypoints');
      assert.equal(info.pointCount, 3);
      assert.equal(info.colorHex, 0x22d3ee);
      assert.equal(info.kind, 'primary');
      assert.equal(info.visible, true);
      assert.equal(info.lastBatchTs, 100);
      assert.equal(info.length, computePathLength([0, 0, 0.05, 1, 1, 0.05, 2, 0, 0.05]));
      assert.equal(info.stale, false);
      tool.onDetach();
    },
  },
  {
    name: 'a second distinct output from the same node becomes a dashed white alternative',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1, 2, 0])));
      tool.onBatch(
        batch('planner', 'trajectory', 200, f32([0, 0, 0.05, 1, 1, 0.05, 2, 0, 0.05])),
      );

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 2);
      assert.equal(snapshot.paths[0].kind, 'primary');
      const alt = snapshot.paths[1];
      assert.equal(alt.key, 'planner/trajectory');
      assert.equal(alt.kind, 'alternative');
      assert.equal(alt.colorHex, 0xffffff);

      const material = lineMaterialOf(pathGroup(context, 'path:planner/trajectory'));
      assert.equal(material.dashed, true);
      assert.equal(material.transparent, true);
      assert.equal(material.opacity, 0.3);
      assert.equal(material.color.getHex(), 0xffffff);
      tool.onDetach();
    },
  },
  {
    name: 'a second node gets magenta and a third node gets orange',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1])));
      tool.onBatch(batch('planner2', 'waypoints', 200, f32([1, 1, 2, 2])));
      tool.onBatch(batch('planner3', 'waypoints', 300, f32([2, 2, 3, 3])));

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 3);
      assert.deepEqual(
        snapshot.paths.map((p) => p.colorHex),
        [0x22d3ee, 0xe879f9, 0xfb923c],
      );
      assert.ok(snapshot.paths.every((p) => p.kind === 'primary'));
      assert.equal(lineMaterialOf(pathGroup(context, 'path:planner2/waypoints')).color.getHex(), 0xe879f9);
      assert.equal(lineMaterialOf(pathGroup(context, 'path:planner3/waypoints')).color.getHex(), 0xfb923c);
      tool.onDetach();
    },
  },
  {
    name: 'a trajectory batch updates the existing trajectory path positions',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'trajectory', 100, f32([0, 0, 0, 1, 0, 0, 2, 0, 0])));
      tool.onBatch(batch('planner', 'trajectory', 200, f32([5, 5, 5, 6, 5, 5, 7, 5, 5])));

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 1);
      const info = snapshot.paths[0];
      assert.equal(info.pointCount, 3);
      assert.equal(info.lastBatchTs, 200);
      assert.equal(info.length, computePathLength([5, 5, 5, 6, 5, 5, 7, 5, 5]));

      assert.deepEqual(firstPointOf(lineGeometryOf(pathGroup(context, 'path:planner/trajectory'))), [
        5, 5, 5,
      ]);
      tool.onDetach();
    },
  },
  {
    name: 'a target batch positions the target marker; invalid targets keep the last known',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'target', 100, f32([1, 2])));
      const marker = rootGroup(context).children.find((c) => c.name === 'dviz-path-target') as Mesh;
      assert.ok(marker);
      assert.equal(marker.visible, true);
      assert.deepEqual([marker.position.x, marker.position.y, marker.position.z], [1, 2, 0.05]);
      assert.deepEqual(tool.getSnapshot().target, { x: 1, y: 2, z: 0.05 });

      tool.onBatch(batch('planner', 'target', 200, f32([1])));
      assert.deepEqual(tool.getSnapshot().target, { x: 1, y: 2, z: 0.05 });
      assert.deepEqual([marker.position.x, marker.position.y, marker.position.z], [1, 2, 0.05]);
      tool.onDetach();
    },
  },
  {
    name: 'a costmap batch is ignored: no new path and no throw',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(
        batch('planner', 'costmap', 100, {
          json: { width: 2, height: 2, resolution: 1, values: [0, 0, 0, 0] },
        }),
      );
      tool.onBatch(
        batch('planner', 'esdf', 200, {
          json: { width: 2, height: 2, resolution: 1, values: [0, 0, 0, 0] },
        }),
      );

      assert.equal(tool.getSnapshot().paths.length, 0);
      assert.equal(tool.getSnapshot().target, null);
      tool.onDetach();
    },
  },
  {
    name: 'batches before onAttach do not throw and create no state',
    run: () => {
      const tool = new DvizPathTool();

      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1])));
      tool.onBatch(batch('planner', 'target', 100, f32([1, 1])));
      tool.onBatch(batch('planner', 'costmap', 100, { json: {} }));

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 0);
      assert.equal(snapshot.target, null);
      assert.equal(snapshot.lastSeekTs, null);
    },
  },
  {
    name: 'setPathVisible hides the path group and notifies subscribers',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      let notified = 0;
      const unsubscribe = tool.subscribe(() => {
        notified += 1;
      });

      tool.onAttach(context);
      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1, 2, 0])));
      assert.equal(notified, 1);

      tool.setPathVisible('planner/waypoints', false);
      assert.equal(notified, 2);
      assert.equal(pathGroup(context, 'path:planner/waypoints').visible, false);
      assert.equal(tool.getSnapshot().paths[0].visible, false);

      tool.setPathVisible('planner/waypoints', true);
      assert.equal(tool.getSnapshot().paths[0].visible, true);
      assert.equal(notified, 3);

      tool.setPathVisible('missing/key', true); // unknown key: no throw, no notify
      assert.equal(notified, 3);

      unsubscribe();
      tool.onDetach();
    },
  },
  {
    name: 'computeStaleness flags paths without fresh data at the seek position',
    run: () => {
      assert.equal(computeStaleness(null, 0), false);
      assert.equal(computeStaleness(150_000_000, 100_000_000), false); // 50ms: fresh
      assert.equal(computeStaleness(150_000_000, 100_000_000, 25_000_000), true); // custom threshold
      assert.equal(computeStaleness(250_000_000, 100_000_000), true); // 150ms: stale
      assert.equal(computeStaleness(100_000_000, 250_000_000), false); // batch after seek
    },
  },
  {
    name: 'onTimelineSeek stores lastSeekTs and marks old paths stale',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'waypoints', 100_000_000, f32([0, 0, 1, 1])));
      tool.onTimelineSeek(500_000_000);

      assert.equal(tool.getSnapshot().lastSeekTs, 500_000_000);
      assert.equal(tool.getSnapshot().paths[0].stale, true);

      tool.onBatch(batch('planner', 'waypoints', 510_000_000, f32([1, 1, 2, 2])));
      assert.equal(tool.getSnapshot().paths[0].stale, false);
      tool.onDetach();
    },
  },
  {
    name: 'onDetach removes the group, clears the snapshot, and double detach is safe',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1, 2, 0])));
      tool.onBatch(batch('planner', 'trajectory', 200, f32([0, 0, 0.05, 1, 1, 0.05])));
      tool.onBatch(batch('planner', 'target', 300, f32([1, 2])));
      tool.onTimelineSeek(500);

      assert.equal(context.scene.children.length, 1);
      tool.onDetach();

      assert.equal(context.scene.children.length, 0);
      assert.deepEqual(tool.getSnapshot(), { paths: [], target: null, lastSeekTs: null });
      tool.onDetach(); // must not throw
      assert.deepEqual(tool.getSnapshot(), { paths: [], target: null, lastSeekTs: null });
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
