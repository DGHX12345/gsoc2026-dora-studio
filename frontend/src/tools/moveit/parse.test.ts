// MoveIt payload parser tests (M13 D1). Self-executes on import — see tests.ts.
//
// Verified against dora-moveit2 source (2026-08-14):
//   - trajectory: flat float32, row-major [q1_1..q1_n, q2_1..q2_n, ...],
//     metadata num_waypoints/num_joints NOT present in .drec replay
//   - plan_status: JSON {plan_id, success, planning_time, path_length,
//     num_waypoints, num_nodes, message}; error branch sends only
//     {plan_id, success: false, message}
//   - execution_status: JSON {is_executing, execution_count,
//     current_waypoint, total_waypoints, progress}
//   - scene_update: JSON {version, timestamp, world_objects,
//     attached_objects, robot_state}; attached objects have no color
//   - joint_positions: float64 qpos (json channel keeps full precision)

import assert from 'node:assert/strict';

import type { ToolPayload } from '../types';
import {
  parseExecutionStatus,
  parseJointCommands,
  parseJointPositions,
  parsePlanStatus,
  parseSceneUpdate,
  parseTrajectory,
} from './parse';

type TestCase = {
  name: string;
  run: () => void;
};

const f32Payload = (values: number[]): ToolPayload => ({ f32: Float32Array.from(values) });
const jsonPayload = (json: unknown): ToolPayload => ({ json });
const bytesPayload = (bytes: number[]): ToolPayload => ({ bytes: Uint8Array.from(bytes) });

const tests: TestCase[] = [
  // -------------------------------------------------------------------------
  // parseTrajectory — object envelope (replay-safe, M13 demo form)
  {
    name: 'parseTrajectory accepts the { waypoints: [[q...], ...] } object envelope',
    run: () => {
      const payload = jsonPayload({
        waypoints: [
          [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
          [0.15, 0.25, 0.35, 0.45, 0.55, 0.65],
        ],
      });
      assert.deepEqual(parseTrajectory(payload, 6), [
        [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
        [0.15, 0.25, 0.35, 0.45, 0.55, 0.65],
      ]);
    },
  },
  {
    name: 'parseTrajectory rejects ragged envelope rows',
    run: () => {
      assert.equal(parseTrajectory(jsonPayload({ waypoints: [[1, 2, 3], [1, 2]] }), 6), null);
    },
  },
  {
    name: 'parseTrajectory rejects envelope with non-finite values',
    run: () => {
      assert.equal(parseTrajectory(jsonPayload({ waypoints: [[0.1, NaN, 0.3]] }), 6), null);
    },
  },
  {
    name: 'parseTrajectory rejects a non-array envelope',
    run: () => {
      assert.equal(parseTrajectory(jsonPayload({ waypoints: 'nope' }), 6), null);
      assert.equal(parseTrajectory(jsonPayload({ waypoints: [[]] }), 6), null);
    },
  },
  {
    name: 'parseTrajectory passes single-joint envelope rows through',
    run: () => {
      assert.deepEqual(parseTrajectory(jsonPayload({ waypoints: [[1], [2], [3]] }), 6), [[1], [2], [3]]);
    },
  },

  // -------------------------------------------------------------------------
  // parseTrajectory — flat reshape (live M15-B path; numJoints from config)
  {
    name: 'parseTrajectory reshapes a flat f32 array into numJoints-wide waypoints',
    run: () => {
      // Values exactly representable in f32 (0.1-style decimals would round).
      const flat = [0.5, -0.25, 0.125, 0.375, 0.0625, 0.75, 0.625, -0.5, 0.25, 0.4375, 0.3125, 0.875];
      assert.deepEqual(parseTrajectory(f32Payload(flat), 6), [
        [0.5, -0.25, 0.125, 0.375, 0.0625, 0.75],
        [0.625, -0.5, 0.25, 0.4375, 0.3125, 0.875],
      ]);
    },
  },
  {
    name: 'parseTrajectory reshapes the json number-array channel too',
    run: () => {
      assert.deepEqual(parseTrajectory(jsonPayload([1, 2, 3, 4]), 2), [
        [1, 2],
        [3, 4],
      ]);
    },
  },
  {
    name: 'parseTrajectory returns null when flat length is not divisible by numJoints',
    run: () => {
      assert.equal(parseTrajectory(f32Payload([1, 2, 3, 4, 5]), 6), null);
      assert.equal(parseTrajectory(f32Payload([]), 6), null);
    },
  },
  {
    name: 'parseTrajectory rejects flat payloads without a joint count',
    run: () => {
      assert.equal(parseTrajectory(f32Payload([1, 2, 3, 4, 5, 6]), null), null);
      // The envelope form carries its own width — no count needed.
      assert.deepEqual(
        parseTrajectory(jsonPayload({ waypoints: [[1, 2], [3, 4]] }), null),
        [
          [1, 2],
          [3, 4],
        ],
      );
    },
  },
  {
    name: 'parseTrajectory returns null for raw bytes (Arrow IPC stays unsupported)',
    run: () => {
      assert.equal(parseTrajectory(bytesPayload([0x41, 0x52, 0x52, 0x4f, 0x57, 0x31]), 6), null);
    },
  },

  // -------------------------------------------------------------------------
  // parsePlanStatus
  {
    name: 'parsePlanStatus parses the full 7-key payload',
    run: () => {
      const status = parsePlanStatus(
        jsonPayload({
          plan_id: 3,
          success: true,
          planning_time: 0.42,
          path_length: 1.234,
          num_waypoints: 12,
          num_nodes: 200,
          message: 'ok',
        }),
      );
      assert.deepEqual(status, {
        success: true,
        plan_id: 3,
        planning_time: 0.42,
        path_length: 1.234,
        num_waypoints: 12,
        num_nodes: 200,
        message: 'ok',
      });
    },
  },
  {
    name: 'parsePlanStatus accepts the minimal error-branch payload',
    run: () => {
      const status = parsePlanStatus(jsonPayload({ plan_id: 0, success: false, message: 'timeout' }));
      assert.ok(status);
      assert.equal(status.success, false);
      assert.equal(status.message, 'timeout');
    },
  },
  {
    name: 'parsePlanStatus rejects payloads without a boolean success',
    run: () => {
      assert.equal(parsePlanStatus(jsonPayload({ message: 'no success' })), null);
      assert.equal(parsePlanStatus(jsonPayload({ success: 'yes' })), null);
      assert.equal(parsePlanStatus(jsonPayload('nope')), null);
    },
  },

  // -------------------------------------------------------------------------
  // parseExecutionStatus
  {
    name: 'parseExecutionStatus parses the full 5-key payload',
    run: () => {
      const status = parseExecutionStatus(
        jsonPayload({
          is_executing: true,
          execution_count: 2,
          current_waypoint: 7,
          total_waypoints: 12,
          progress: 0.58,
        }),
      );
      assert.deepEqual(status, {
        is_executing: true,
        execution_count: 2,
        current_waypoint: 7,
        total_waypoints: 12,
        progress: 0.58,
      });
    },
  },
  {
    name: 'parseExecutionStatus accepts the minimal 3-key payload',
    run: () => {
      const status = parseExecutionStatus(
        jsonPayload({ is_executing: false, current_waypoint: 0, progress: 0 }),
      );
      assert.ok(status);
      assert.equal(status.is_executing, false);
      assert.equal(status.total_waypoints, undefined);
    },
  },
  {
    name: 'parseExecutionStatus rejects payloads without is_executing/progress',
    run: () => {
      assert.equal(parseExecutionStatus(jsonPayload({ current_waypoint: 1 })), null);
      assert.equal(parseExecutionStatus(jsonPayload({ is_executing: true, progress: 'half' })), null);
    },
  },

  // -------------------------------------------------------------------------
  // parseSceneUpdate
  {
    name: 'parseSceneUpdate parses world/attached objects and robot_state',
    run: () => {
      const scene = parseSceneUpdate(
        jsonPayload({
          version: 4,
          timestamp: 1723610000.5,
          world_objects: [
            {
              name: 'table',
              type: 'box',
              position: [0.5, 0, 0.2],
              dimensions: [0.8, 0.6, 0.4],
              color: [0.9, 0.1, 0.1, 1],
            },
          ],
          attached_objects: [
            { name: 'tool', type: 'cylinder', position: [0, 0, 0.1], dimensions: [0.05, 0.2], attached_link: 'wrist_3_link' },
          ],
          robot_state: { joint_positions: [0.1, -0.2, 0.3], gripper_state: 0.5 },
        }),
      );
      assert.ok(scene);
      assert.equal(scene.version, 4);
      assert.equal(scene.world_objects.length, 1);
      assert.equal(scene.attached_objects.length, 1);
      // Attached objects have no color on the wire — default gray applies.
      assert.deepEqual(scene.attached_objects[0].color, [0.5, 0.5, 0.5, 1]);
      assert.deepEqual(scene.robot_state, { joint_positions: [0.1, -0.2, 0.3], gripper_state: 0.5 });
    },
  },
  {
    name: 'parseSceneUpdate drops malformed entries but keeps valid ones',
    run: () => {
      const scene = parseSceneUpdate(
        jsonPayload({
          version: 1,
          world_objects: [
            { name: 'ok', type: 'sphere', position: [1, 2, 3], dimensions: [0.1] },
            { name: 'bad-type', type: 'mesh', position: [0, 0, 0], dimensions: [1] },
            { name: 'bad-pos', type: 'box', position: 'nowhere', dimensions: [1, 1, 1] },
            { name: 'no-color-ok', type: 'box', position: [0, 0, 0], dimensions: [1, 1, 1] },
          ],
          attached_objects: [],
          robot_state: { joint_positions: [], gripper_state: 0 },
        }),
      );
      assert.ok(scene);
      assert.deepEqual(scene.world_objects.map((o) => o.name), ['ok', 'no-color-ok']);
      assert.deepEqual(scene.world_objects[1].color, [0.5, 0.5, 0.5, 1]);
    },
  },
  {
    name: 'parseSceneUpdate rejects payloads without a finite version',
    run: () => {
      assert.equal(parseSceneUpdate(jsonPayload({ world_objects: [], attached_objects: [], robot_state: {} })), null);
      assert.equal(parseSceneUpdate(jsonPayload({ version: 'v2', world_objects: [] })), null);
    },
  },
  {
    name: 'parseSceneUpdate rejects payloads whose object lists are not arrays',
    run: () => {
      assert.equal(parseSceneUpdate(jsonPayload({ version: 1, world_objects: 'none' })), null);
    },
  },

  // -------------------------------------------------------------------------
  // parseJointPositions / parseJointCommands
  {
    name: 'parseJointPositions prefers the json channel (float64 precision)',
    run: () => {
      const precise = [1.23456789012345, -0.00000000012345, 3.0];
      assert.deepEqual(parseJointPositions(jsonPayload(precise)), precise);
    },
  },
  {
    name: 'parseJointPositions falls back to f32 when json is absent',
    run: () => {
      assert.deepEqual(parseJointPositions(f32Payload([1, 2, 3])), [1, 2, 3]);
    },
  },
  {
    name: 'parseJointPositions rejects non-array payloads',
    run: () => {
      assert.equal(parseJointPositions(jsonPayload({ q: [1, 2] })), null);
      assert.equal(parseJointPositions(bytesPayload([1, 2, 3])), null);
    },
  },
  {
    name: 'parseJointCommands parses a flat f32 command array',
    run: () => {
      const values = [0.5, -0.25, 0.125, 0.375, 0.0625, 0.75];
      assert.deepEqual(parseJointCommands(f32Payload(values)), values);
      assert.equal(parseJointCommands(jsonPayload({ q: [1] })), null);
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
