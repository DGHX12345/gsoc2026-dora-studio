// D7 co-visualization tests: dviz + moveit tools attached to the SAME
// scene, fed the moveit demo streams. Both tools subscribe to the shared
// `trajectory` port — the moveit envelope must be invisible to the dviz
// parsers and dviz waypoints invisible to the moveit tool. Self-executes
// on import — see tests.ts.

import assert from 'node:assert/strict';
import { Group, Scene } from 'three';

import { DvizPathTool } from '../dviz/DvizPathTool';
import type { ToolBatch, ToolContext, ToolPayload } from '../types';
import { MoveItTool } from './MoveItTool';
import { buildRobotModel } from './urdf/robot';
import { parseUrdf } from './urdf/urdf';

type TestCase = {
  name: string;
  run: () => void | Promise<void>;
};

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const CHAIN_URDF = `
<robot name="chain">
  <link name="base_link"/>
  <link name="link1"/>
  <joint name="j1" type="revolute">
    <origin xyz="0 0 0.1" rpy="0 0 0"/>
    <parent link="base_link"/><child link="link1"/>
    <axis xyz="0 0 1"/>
  </joint>
</robot>`;

const stubCatalog = async () => [{ id: 'b601', urdfPath: '/models/b601/x.urdf', meshBasePath: '/models/b601/' }];

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

const json = (value: unknown): ToolPayload => ({ json: value });

/** Streams from the M13 moveit demo file (real node names). */
const MOVEIT_DEMO_TRAJECTORY = {
  waypoints: [
    [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
    [0.15, 0.25, 0.35, 0.45, 0.55, 0.65],
  ],
};

const tests: TestCase[] = [
  {
    name: 'both tools render their own layers from the shared demo replay',
    run: async () => {
      const dviz = new DvizPathTool();
      const moveit = new MoveItTool(
        async () => buildRobotModel(parseUrdf(CHAIN_URDF)),
        stubCatalog,
      );
      const context = makeContext();
      dviz.onAttach(context);
      moveit.onAttach(context);
      await flush();

      // dviz streams
      dviz.onBatch(batch('simple_planner', 'waypoints', 1_000, json({ waypoints: [[0, 0], [0.1, 0.1]] })));
      dviz.onBatch(batch('costmap_node', 'costmap', 1_000, json({
        width: 8, height: 8, resolution: 0.1,
        values: new Array(64).fill(0),
      })));
      // moveit streams (envelope trajectory — the shared port)
      dviz.onBatch(batch('planner', 'trajectory', 1_000, json(MOVEIT_DEMO_TRAJECTORY)));
      moveit.onBatch(batch('planner', 'trajectory', 1_000, json(MOVEIT_DEMO_TRAJECTORY)));
      moveit.onBatch(batch('mujoco_sim', 'joint_positions', 1_000, json([0.1, 0])));
      // registry-style broadcast would send every batch to both tools; the
      // tool handlers below mirror that routing explicitly.
      moveit.onBatch(batch('simple_planner', 'waypoints', 1_000, json({ waypoints: [[0, 0]] })));
      moveit.onBatch(batch('costmap_node', 'costmap', 1_000, json({
        width: 8, height: 8, resolution: 0.1,
        values: new Array(64).fill(0),
      })));

      const dvizRoot = context.scene.children.find((c) => c.name === 'dviz-path') as Group;
      const moveitRoot = context.scene.children.find((c) => c.name === 'moveit-bridge') as Group;
      assert.ok(dvizRoot, 'dviz root present');
      assert.ok(moveitRoot, 'moveit root present');

      // dviz renders the waypoints path + costmap, and did NOT turn the
      // moveit envelope into a path (its trajectory parser rejects objects)
      const dvizPathGroups = dvizRoot.children.filter((c) => c.name.startsWith('path:'));
      assert.equal(dvizPathGroups.length, 1, 'only the waypoints path (envelope ignored)');
      assert.ok(dvizRoot.children.find((c) => c.name === 'costmap'));

      // moveit loaded the robot and rendered FK artifacts; dviz waypoints/
      // costmap batches left no trace in its scene
      assert.equal(moveit.getSnapshot().robotState, 'loaded');
      assert.ok(moveitRoot.children.find((c) => c.name === 'moveit-robot'));
      assert.ok(moveitRoot.children.find((c) => c.name === 'moveit-ee-path'));
      assert.ok(moveitRoot.children.find((c) => c.name === 'moveit-ghosts'));
    },
  },
  {
    name: 'a dviz xyz trajectory batch is not mis-parsed as joint waypoints',
    run: async () => {
      // 120 flat xyz values (the M12 tool_demo stream). With the b601
      // config (7 joints) the reshape is rejected — 120 % 7 ≠ 0.
      const moveit = new MoveItTool(
        async () => buildRobotModel(parseUrdf(CHAIN_URDF)),
        stubCatalog,
      );
      const context = makeContext();
      moveit.onAttach(context);
      await flush();
      const xyz = Array.from({ length: 120 }, (_, i) => (i % 3 === 2 ? 0.05 : (i % 10) * 0.01));
      moveit.onBatch(batch('planner', 'trajectory', 1_000, json(xyz)));
      assert.equal(moveit.getSnapshot().trajectory, null, 'xyz stream must not become a joint trajectory');
    },
  },
];

let failures = 0;

for (const test of tests) {
  try {
    await test.run();
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
