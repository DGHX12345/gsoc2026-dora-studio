// MoveItTool tests (M13 D2). Self-executes on import — see tests.ts.
//
// Constructs a real THREE.Scene headlessly (like the M12 tool tests):
// three object construction works without a renderer.

import assert from 'node:assert/strict';
import { BufferAttribute, Frustum, Group, Line, LineSegments, Matrix4, PerspectiveCamera, Scene, Vector3 } from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';

import { matchToolPorts } from '../matching';
import type { ToolBatch, ToolContext, ToolPayload } from '../types';
import { MoveItTool, type ModelLoader } from './MoveItTool';
import { buildRobotModel } from './urdf/robot';
import { parseUrdf } from './urdf/urdf';

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

/** 3-joint revolute chain used as the fake robot model. */
const CHAIN_URDF = `
<robot name="chain">
  <link name="base_link"/>
  <link name="link1"/>
  <link name="link2"/>
  <link name="link3"/>
  <joint name="j1" type="revolute">
    <origin xyz="0 0 0.1" rpy="0 0 0"/>
    <parent link="base_link"/><child link="link1"/>
    <axis xyz="0 0 1"/>
  </joint>
  <joint name="j2" type="revolute">
    <origin xyz="0.2 0 0" rpy="0 0 0"/>
    <parent link="link1"/><child link="link2"/>
    <axis xyz="0 0 1"/>
  </joint>
  <joint name="j3" type="revolute">
    <origin xyz="0.1 0 0" rpy="0 0 0"/>
    <parent link="link2"/><child link="link3"/>
    <axis xyz="0 0 1"/>
  </joint>
</robot>`;

/** Arm + prismatic gripper fingers (B601-like) for previewPose tests. */
const GRIPPER_URDF = `
<robot name="gripper_arm">
  <link name="base_link"/>
  <link name="link1"/>
  <link name="finger_left"/>
  <link name="finger_right"/>
  <joint name="j1" type="revolute">
    <parent link="base_link"/><child link="link1"/>
    <axis xyz="0 0 1"/>
  </joint>
  <joint name="gripper_joint1" type="prismatic">
    <parent link="link1"/><child link="finger_left"/>
    <axis xyz="1 0 0"/>
    <limit lower="0" upper="0.0715" effort="100" velocity="15"/>
  </joint>
  <joint name="gripper_joint2" type="prismatic">
    <parent link="link1"/><child link="finger_right"/>
    <axis xyz="1 0 0"/>
    <limit lower="0" upper="0.0715" effort="100" velocity="15"/>
  </joint>
</robot>`;

const chainLoader: ModelLoader = async () => buildRobotModel(parseUrdf(CHAIN_URDF));
// Stub catalog so the auto-load never touches the network in tests.
const stubCatalog = async () => [{ id: 'b601', urdfPath: '/models/b601/x.urdf', meshBasePath: '/models/b601/' }];

type TestCase = {
  name: string;
  run: () => void | Promise<void>;
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

const json = (value: unknown): ToolPayload => ({ json: value });
const f32 = (values: number[]): ToolPayload => ({ f32: Float32Array.from(values) });

const rootGroup = (context: ToolContext): Group | undefined =>
  context.scene.children.find((c) => c.name === 'moveit-bridge') as Group | undefined;

const chartGroup = (context: ToolContext): Group | undefined =>
  rootGroup(context)?.children.find((c) => c.name === 'moveit-joint-chart') as Group | undefined;

const polylinesOf = (chart: Group): Line[] =>
  chart.children.filter((c): c is Line => c instanceof Line && c.name === 'chart-polyline');

const axesOf = (chart: Group): Line[] =>
  chart.children.filter((c): c is Line => c instanceof Line && c.name === 'chart-axis');

const TRAJECTORY_ENVELOPE = {
  waypoints: [
    [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
    [0.15, 0.25, 0.35, 0.45, 0.55, 0.65],
    [0.2, 0.3, 0.4, 0.5, 0.6, 0.7],
  ],
};

const tests: TestCase[] = [
  {
    name: 'subscribePorts match all six moveit ports and reject unrelated ones',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      for (const output of ['trajectory', 'joint_positions', 'joint_commands', 'scene_update', 'execution_status', 'plan_status']) {
        assert.ok(matchToolPorts(tool.subscribePorts, 'planner', output), output);
        assert.ok(matchToolPorts(tool.subscribePorts, 'mujoco_sim', output), output);
      }
      assert.ok(!matchToolPorts(tool.subscribePorts, 'planner', 'waypoints'));
      assert.ok(!matchToolPorts(tool.subscribePorts, 'camera', 'image'));
    },
  },
  {
    name: 'a trajectory batch renders a parallel-coordinates chart in the scene',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));

      const snapshot = tool.getSnapshot();
      assert.ok(snapshot.trajectory);
      assert.equal(snapshot.trajectory!.waypointCount, 3);
      assert.equal(snapshot.numJoints, 6);
      assert.equal(snapshot.trajectory!.nodeId, 'planner');

      const chart = chartGroup(context);
      assert.ok(chart, 'chart group present');
      assert.equal(polylinesOf(chart!).length, 3);
      assert.equal(axesOf(chart!).length, 6);
    },
  },
  {
    name: 'flat trajectory batches are ignored until a joint count is known',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'trajectory', 1_000, f32([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])));
      assert.equal(tool.getSnapshot().trajectory, null);
      assert.equal(chartGroup(context), undefined);

      // Robot config supplies the count: 12 values → 2 waypoints of 6 joints
      tool.setRobot('ur5e');
      tool.onBatch(batch('planner', 'trajectory', 2_000, f32([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])));
      const snapshot = tool.getSnapshot();
      assert.ok(snapshot.trajectory);
      assert.equal(snapshot.trajectory!.waypointCount, 2);
      assert.equal(snapshot.numJoints, 6);
    },
  },
  {
    name: 'plan/execution status and joint streams update the snapshot; invalid payloads keep the last value',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      tool.onBatch(batch('planner', 'plan_status', 1_000, json({ success: true, message: 'ok', num_waypoints: 3 })));
      tool.onBatch(
        batch('trajectory_executor', 'execution_status', 1_000, json({ is_executing: true, current_waypoint: 1, progress: 0.5 })),
      );
      tool.onBatch(batch('trajectory_executor', 'joint_commands', 1_000, json([0.5, -0.25, 0.125])));
      tool.onBatch(batch('mujoco_sim', 'joint_positions', 1_000, json([0.4, -0.2, 0.1])));

      let snapshot = tool.getSnapshot();
      assert.ok(snapshot.planStatus);
      assert.equal(snapshot.planStatus!.status.success, true);
      assert.equal(snapshot.planStatus!.status.num_waypoints, 3);
      assert.equal(snapshot.execution!.status.is_executing, true);
      assert.deepEqual(snapshot.jointCommands!.values, [0.5, -0.25, 0.125]);
      assert.deepEqual(snapshot.jointPositions!.values, [0.4, -0.2, 0.1]);

      // Invalid payloads: last known values survive
      tool.onBatch(batch('planner', 'plan_status', 2_000, json({ message: 'no success field' })));
      tool.onBatch(batch('trajectory_executor', 'joint_commands', 2_000, json({ q: [1] })));
      snapshot = tool.getSnapshot();
      assert.equal(snapshot.planStatus!.status.message, 'ok');
      assert.deepEqual(snapshot.jointCommands!.values, [0.5, -0.25, 0.125]);
      assert.equal(snapshot.planStatus!.lastBatchTs, 1_000);
    },
  },
  {
    name: 'stale flags follow the timeline seek position',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      tool.onBatch(batch('trajectory_executor', 'execution_status', 1_000, json({ is_executing: false, current_waypoint: 0, progress: 0 })));

      tool.onTimelineSeek!(500_000_000);
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.trajectory!.stale, true);
      assert.equal(snapshot.execution!.stale, true);

      tool.onTimelineSeek!(1_500);
      assert.equal(tool.getSnapshot().trajectory!.stale, false);
    },
  },
  {
    name: 'setRobot updates the joint labels in the snapshot',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      assert.deepEqual(tool.getSnapshot().jointLabels, []);
      tool.setRobot('b601');
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.robotId, 'b601');
      assert.deepEqual(snapshot.jointLabels.slice(0, 2), ['joint1', 'joint2']);
      assert.equal(snapshot.jointLabels.length, 6);
    },
  },
  {
    name: 'subscribe notifies on batch updates and unsubscribes cleanly',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      let notified = 0;
      const unsubscribe = tool.subscribe(() => {
        notified += 1;
      });
      tool.onBatch(batch('planner', 'plan_status', 1_000, json({ success: true })));
      assert.equal(notified, 1);
      unsubscribe();
      tool.onBatch(batch('planner', 'plan_status', 2_000, json({ success: false })));
      assert.equal(notified, 1);
    },
  },
  {
    name: 'the chart stays inside the framed camera frustum',
    run: () => {
      // Regression: the initial placement sat on the camera's blind side
      // (positive x) and was invisible in the viewport. NanoRobotViewer's
      // frameCameraToModel frames a small model: camera at
      // (r*1.75, -r*2.15, r*1.1) looking at the origin, 35° FOV.
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const chart = chartGroup(context);
      assert.ok(chart);

      const radius = 0.3;
      const camera = new PerspectiveCamera(35, 16 / 9, 0.01, 50);
      camera.position.set(radius * 1.75, -radius * 2.15, radius * 1.1);
      camera.lookAt(0, 0, 0);
      camera.updateMatrixWorld(true);
      const frustum = new Frustum().setFromProjectionMatrix(
        new Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse),
      );

      const point = new Vector3();
      for (const child of chart.children) {
        const geometry = (child as Line).geometry;
        const position = geometry.getAttribute('position') as BufferAttribute;
        for (let i = 0; i < position.count; i++) {
          point.fromBufferAttribute(position, i);
          assert.ok(frustum.containsPoint(point), `chart vertex ${i} outside the framed frustum`);
        }
      }
    },
  },
  {
    name: 'auto-loads the robot model on attach and hides the chart',
    run: async () => {
      let loadedRobot: string | null = null;
      const tool = new MoveItTool(async (robotId) => {
        loadedRobot = robotId;
        return buildRobotModel(parseUrdf(CHAIN_URDF));
      }, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      assert.equal(loadedRobot, 'b601');
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.robotState, 'loaded');
      assert.equal(snapshot.modelName, 'chain');

      const root = rootGroup(context)!;
      assert.ok(root.children.find((c) => c.name === 'moveit-robot'));

      // A trajectory batch now renders FK artifacts; with a loaded model
      // the fallback chart is never created.
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      assert.equal(chartGroup(context), undefined);
      const eePath = root.children.find((c) => c.name === 'moveit-ee-path');
      assert.ok(eePath, 'EE path present');
      const ghosts = root.children.find((c) => c.name === 'moveit-ghosts') as Group;
      assert.ok(ghosts);
      assert.equal(ghosts.children.length, 5);
      // LineGeometry instanceStart holds one vertex per SEGMENT
      // (waypoints - 1) — 3 waypoints → 2 segments
      const line = (eePath as Group).children[0] as Line2;
      const count = line.geometry.getAttribute('instanceStart').count;
      assert.equal(count, TRAJECTORY_ENVELOPE.waypoints.length - 1);
    },
  },
  {
    name: 'robot load failure falls back to the chart with an honest state',
    run: async () => {
      const tool = new MoveItTool(async () => {
        throw new Error('model not found');
      }, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      assert.equal(tool.getSnapshot().robotState, 'unavailable');
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const chart = chartGroup(context);
      assert.ok(chart);
      assert.equal(chart.visible, true);
    },
  },
  {
    name: 'ghost rebuilds do not dispose the shared model geometry',
    run: async () => {
      // Ghosts share the model's BufferGeometry (only materials are
      // cloned). Disposing a ghost must never dispose the shared geometry —
      // that would force a GPU re-upload of the whole model every frame.
      const { loadUrdfRobot } = await import('./urdf/meshes');
      const urdf = '<robot name="m"><link name="a"><visual><geometry><box size="0.1 0.1 0.1"/></geometry></visual></link></robot>';
      const model = await loadUrdfRobot(urdf, async () => {
        throw new Error('no meshes');
      });
      let geometryDisposed = 0;
      model.root.traverse((obj) => {
        const mesh = obj as { geometry?: { addEventListener: (t: string, cb: () => void) => void } };
        mesh.geometry?.addEventListener('dispose', () => {
          geometryDisposed += 1;
        });
      });

      const tool = new MoveItTool(async () => model, stubCatalog);
      tool.onAttach(makeContext());
      await flush();
      // Two trajectory batches: the second rebuilds ghosts
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      tool.onBatch(batch('planner', 'trajectory', 2_000, json(TRAJECTORY_ENVELOPE)));
      assert.equal(geometryDisposed, 0, 'shared geometry was disposed during ghost rebuild');
    },
  },
  {
    name: 'ghost count zero hides all ghosts and the count round-trips',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const ghosts = rootGroup(context)!.children.find(
        (c) => c.name === 'moveit-ghosts',
      ) as Group;
      assert.equal(ghosts.children.length, 5);

      tool.setGhostCount(0);
      assert.equal(ghosts.children.length, 0);
      assert.equal(tool.getSnapshot().ghostCount, 0);

      tool.setGhostCount(3);
      assert.equal(ghosts.children.length, 3);
      assert.equal(tool.getSnapshot().ghostCount, 3);
    },
  },
  {
    name: 'identical trajectory batches skip the FK rebuild',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const root = rootGroup(context)!;
      const ghosts = root.children.find((c) => c.name === 'moveit-ghosts') as Group;
      const firstGhosts = [...ghosts.children];
      const firstEePath = root.children.find((c) => c.name === 'moveit-ee-path');

      // Same content, later timestamp: no rebuild — same objects survive
      tool.onBatch(batch('planner', 'trajectory', 2_000, json(TRAJECTORY_ENVELOPE)));
      assert.deepEqual([...ghosts.children], firstGhosts);
      assert.equal(root.children.find((c) => c.name === 'moveit-ee-path'), firstEePath);

      // Changed content: rebuild happens
      const changed = {
        waypoints: TRAJECTORY_ENVELOPE.waypoints.slice(0, 2),
      };
      tool.onBatch(batch('planner', 'trajectory', 3_000, json(changed)));
      assert.notEqual(root.children.find((c) => c.name === 'moveit-ee-path'), firstEePath);
    },
  },
  {
    name: 'scene_update batches render yellow wireframes and report collisions',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(
        batch(
          'planning_scene',
          'scene_update',
          1_000,
          json({
            version: 1,
            world_objects: [
              { name: 'a', type: 'sphere', position: [0, 0, 0], dimensions: [0.3], color: [1, 1, 0, 1] },
              { name: 'b', type: 'sphere', position: [0.5, 0, 0], dimensions: [0.3], color: [1, 1, 0, 1] },
              { name: 'table', type: 'box', position: [2, 2, 2], dimensions: [0.8, 0.6, 0.4], color: [1, 1, 0, 1] },
            ],
            attached_objects: [],
            robot_state: { joint_positions: [], gripper_state: 0 },
          }),
        ),
      );
      const root = rootGroup(context)!;
      const collision = root.children.find((c) => c.name === 'moveit-collision') as Group;
      assert.ok(collision, 'collision overlay present');
      assert.equal(collision.children.length, 3);
      assert.equal((collision.children[0] as LineSegments).name, 'collision-a');
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.sceneCollisions.length, 1);
      assert.equal(snapshot.sceneCollisions[0].a, 'a');
      assert.equal(snapshot.sceneCollisions[0].b, 'b');
    },
  },
  {
    name: 'attached scene objects parent under their robot link',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(
        batch(
          'planning_scene',
          'scene_update',
          1_000,
          json({
            version: 1,
            world_objects: [],
            attached_objects: [
              { name: 'tool', type: 'cylinder', position: [0, 0, 0.1], dimensions: [0.05, 0.2], attached_link: 'link1' },
            ],
            robot_state: { joint_positions: [], gripper_state: 0 },
          }),
        ),
      );
      const model = tool.getRobotModel()!;
      const link1 = model.links.get('link1')!;
      const wire = link1.children.find((c) => c.name === 'collision-tool') as LineSegments;
      assert.ok(wire, 'attached wireframe parented under link1');
      // The link-local position is applied directly
      assert.equal(wire.position.z, 0.1);
    },
  },
  {
    name: 'identical scene versions skip the overlay rebuild',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      const payload = json({
        version: 2,
        world_objects: [
          { name: 'a', type: 'sphere', position: [0, 0, 0], dimensions: [0.3], color: [1, 1, 0, 1] },
        ],
        attached_objects: [],
        robot_state: { joint_positions: [], gripper_state: 0 },
      });
      tool.onBatch(batch('planning_scene', 'scene_update', 1_000, payload));
      const root = rootGroup(context)!;
      const first = (root.children.find((c) => c.name === 'moveit-collision') as Group).children[0];
      tool.onBatch(batch('planning_scene', 'scene_update', 2_000, payload));
      const second = (root.children.find((c) => c.name === 'moveit-collision') as Group).children[0];
      assert.equal(first, second, 'same version: same wireframe object survives');
    },
  },
  {
    name: 'joint_positions batches drive the current pose of the loaded robot',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(batch('mujoco_sim', 'joint_positions', 1_000, json([0.5, 0, 0])));
      const snapshot = tool.getSnapshot();
      assert.deepEqual(snapshot.jointPositions!.values, [0.5, 0, 0]);
      // The model's j1 pivot quaternion reflects 0.5 rad around z
      const model = tool.getRobotModel()!;
      const quat = model.joints.get('j1')!.pivot.quaternion;
      const angle = 2 * Math.acos(Math.min(1, Math.abs(quat.w)));
      assert.ok(Math.abs(angle - 0.5) < 1e-6, `expected ~0.5 rad, got ${angle}`);
    },
  },
  {
    name: 'previewPose maps arm radians and converts the gripper degrees-linear to meters',
    run: async () => {
      const tool = new MoveItTool(async () => buildRobotModel(parseUrdf(GRIPPER_URDF)), stubCatalog);
      tool.onAttach(makeContext());
      await flush();
      // 56.8° full range (student decision) → 0.991 rad; half → 0.03575 m.
      // Arm at 0 keeps the fingers' x offset unrotated for the exact check.
      tool.previewPose([0, 28.4 * (Math.PI / 180)]);
      const model = tool.getRobotModel()!;
      model.updateWorld();
      const finger1 = model.getLinkWorldPosition('finger_left');
      const finger2 = model.getLinkWorldPosition('finger_right');
      assert.ok(Math.abs(finger1.x - 0.03575) < 1e-6, `finger1 x = ${finger1.x}`);
      assert.ok(Math.abs(finger2.x - 0.03575) < 1e-6, `finger2 x = ${finger2.x}`);
      // Arm joint takes the raw radian value
      tool.previewPose([0.3, 28.4 * (Math.PI / 180)]);
      model.updateWorld();
      const angle = 2 * Math.acos(Math.min(1, Math.abs(model.joints.get('j1')!.pivot.quaternion.w)));
      assert.ok(Math.abs(angle - 0.3) < 1e-6);
      // Above-range gripper values clamp to the URDF limit
      tool.previewPose([0, 10]);
      model.updateWorld();
      assert.ok(Math.abs(model.getLinkWorldPosition('finger_left').x - 0.0715) < 1e-6);
    },
  },
  {
    name: 'trajectory player advances waypoints, drives the pose, and stops at the end',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      await flush();
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));

      tool.setTrajectoryPlayback({ playing: true, speed: 1 });
      let snapshot = tool.getSnapshot();
      assert.equal(snapshot.player.playing, true);
      assert.equal(snapshot.player.syncToTimeline, false);
      assert.equal(snapshot.player.waypointIndex, 0);

      tool.advancePlayer();
      snapshot = tool.getSnapshot();
      assert.equal(snapshot.player.waypointIndex, 1);
      assert.deepEqual(snapshot.currentJointValues, TRAJECTORY_ENVELOPE.waypoints[1]);

      tool.advancePlayer();
      tool.advancePlayer();
      snapshot = tool.getSnapshot();
      assert.equal(snapshot.player.waypointIndex, 2);
      assert.equal(snapshot.player.playing, false, 'stops at the last waypoint');
    },
  },
  {
    name: 'stepTrajectory moves one waypoint without starting playback',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      await flush();
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      tool.stepTrajectory(1);
      assert.equal(tool.getSnapshot().player.waypointIndex, 1);
      assert.equal(tool.getSnapshot().player.playing, false);
      tool.stepTrajectory(1);
      tool.stepTrajectory(1);
      assert.equal(tool.getSnapshot().player.waypointIndex, 2, 'clamped at the last waypoint');
      tool.stepTrajectory(-1);
      assert.equal(tool.getSnapshot().player.waypointIndex, 1);
      tool.stepTrajectory(-1);
      tool.stepTrajectory(-1);
      assert.equal(tool.getSnapshot().player.waypointIndex, 0, 'clamped at zero');
    },
  },
  {
    name: 'sync-to-timeline mode pauses the player and restores stream-driven poses',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      tool.onAttach(makeContext());
      await flush();
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      tool.setTrajectoryPlayback({ playing: true });
      tool.advancePlayer(); // pose = waypoint 1
      // Joint streams must not override the player pose while playing
      tool.onBatch(batch('mujoco_sim', 'joint_positions', 1_000, json([0.9, 0, 0])));
      assert.deepEqual(tool.getSnapshot().currentJointValues, TRAJECTORY_ENVELOPE.waypoints[1]);

      tool.setSyncToTimeline(true);
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.player.syncToTimeline, true);
      assert.equal(snapshot.player.playing, false);
      // Streams own the pose again
      tool.onBatch(batch('mujoco_sim', 'joint_positions', 2_000, json([0.7, 0, 0])));
      assert.deepEqual(tool.getSnapshot().currentJointValues, [0.7, 0, 0]);
    },
  },
  {
    name: 'setGhostCount rebuilds ghosts within 0..20',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const ghosts = () =>
        (rootGroup(context)!.children.find((c) => c.name === 'moveit-ghosts') as Group).children.length;
      assert.equal(ghosts(), 5);
      tool.setGhostCount(3);
      assert.equal(ghosts(), 3);
      tool.setGhostCount(25);
      assert.equal(ghosts(), 20, 'clamped to 20');
      tool.setGhostCount(0);
      assert.equal(ghosts(), 0, 'clamped to 0');
    },
  },
  {
    name: 'setCollisionVisible toggles the scene overlay',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      tool.onBatch(
        batch(
          'planning_scene',
          'scene_update',
          1_000,
          json({
            version: 1,
            world_objects: [
              { name: 'a', type: 'sphere', position: [0, 0, 0], dimensions: [0.3], color: [1, 1, 0, 1] },
            ],
            attached_objects: [],
            robot_state: { joint_positions: [], gripper_state: 0 },
          }),
        ),
      );
      const collision = rootGroup(context)!.children.find((c) => c.name === 'moveit-collision') as Group;
      assert.equal(collision.visible, true);
      tool.setCollisionVisible(false);
      assert.equal(collision.visible, false);
      assert.equal(tool.getSnapshot().collisionVisible, false);
    },
  },
  {
    name: 'setRobot reloads the model when a different robot is selected',
    run: async () => {
      const loads: string[] = [];
      const loader: ModelLoader = async (robotId) => {
        loads.push(robotId);
        return buildRobotModel(parseUrdf(CHAIN_URDF));
      };
      const catalog = async () => [
        { id: 'b601', urdfPath: '/models/b601/x.urdf', meshBasePath: '/models/b601/' },
        { id: 'ur5e', urdfPath: '/models/ur5e/x.urdf', meshBasePath: '/models/ur5e/' },
      ];
      const tool = new MoveItTool(loader, catalog);
      tool.onAttach(makeContext());
      await flush();
      assert.deepEqual(loads, ['b601']); // auto-load on attach
      tool.setRobot('ur5e');
      await flush();
      assert.deepEqual(loads, ['b601', 'ur5e']); // the switch must reload
      assert.equal(tool.getSnapshot().robotState, 'loaded');
      assert.equal(tool.getSnapshot().robotId, 'ur5e');
    },
  },
  {
    name: 'unloadRobot falls back to the chart and restores the nano display contract',
    run: async () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      await flush();
      assert.equal(tool.getSnapshot().robotState, 'loaded');
      tool.unloadRobot();
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.robotState, null);
      assert.equal(rootGroup(context)!.children.find((c) => c.name === 'moveit-robot'), undefined);
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      const chart = chartGroup(context);
      assert.ok(chart);
      assert.equal(chart.visible, true);
    },
  },
  {
    name: 'detach removes the group and clears all state',
    run: () => {
      const tool = new MoveItTool(chainLoader, stubCatalog);
      const context = makeContext();
      tool.onAttach(context);
      tool.setRobot('ur5e');
      tool.onBatch(batch('planner', 'trajectory', 1_000, json(TRAJECTORY_ENVELOPE)));
      assert.equal(context.scene.children.length, 1);

      tool.onDetach();
      assert.equal(context.scene.children.length, 0);
      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.trajectory, null);
      assert.equal(snapshot.robotId, null);
      assert.equal(snapshot.numJoints, null);
    },
  },
];

let failures = 0;

for (const test of tests) {
  try {
    // Async tests must be awaited — a sync runner would print "ok" before
    // the assertions ran and swallow async failures.
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
