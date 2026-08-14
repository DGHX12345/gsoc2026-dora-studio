// Joint-name config tests (M13 D3). Self-executes on import — see tests.ts.

import assert from 'node:assert/strict';

import { getRobotConfig, jointLabelsFor, KNOWN_ROBOTS } from './joint-config';

type TestCase = {
  name: string;
  run: () => void;
};

const tests: TestCase[] = [
  {
    name: 'known robots carry their joint names',
    run: () => {
      const ur5e = getRobotConfig('ur5e');
      assert.ok(ur5e);
      assert.deepEqual(ur5e.jointNames, [
        'shoulder_pan_joint',
        'shoulder_lift_joint',
        'elbow_joint',
        'wrist_1_joint',
        'wrist_2_joint',
        'wrist_3_joint',
      ]);
      assert.equal(ur5e.jointNames.length, 6);

      const gen72 = getRobotConfig('gen72');
      assert.ok(gen72);
      assert.equal(gen72.jointNames.length, 8);
      assert.equal(gen72.jointNames[6], 'gripper_left');

      // B601 names verified against reBot_B601_DM_with_gripper.urdf
      const b601 = getRobotConfig('b601');
      assert.ok(b601);
      assert.deepEqual(b601.jointNames, [
        'joint1',
        'joint2',
        'joint3',
        'joint4',
        'joint5',
        'joint6',
        'gripper',
      ]);
    },
  },
  {
    name: 'getRobotConfig returns undefined for unknown ids',
    run: () => {
      assert.equal(getRobotConfig('nonexistent'), undefined);
      assert.equal(getRobotConfig(''), undefined);
    },
  },
  {
    name: 'jointLabelsFor returns the configured names for a matching count',
    run: () => {
      assert.deepEqual(jointLabelsFor('ur5e', 6), [
        'shoulder_pan_joint',
        'shoulder_lift_joint',
        'elbow_joint',
        'wrist_1_joint',
        'wrist_2_joint',
        'wrist_3_joint',
      ]);
    },
  },
  {
    name: 'jointLabelsFor pads with generic labels beyond the config',
    run: () => {
      assert.deepEqual(jointLabelsFor('ur5e', 8)[6], 'J6');
      assert.equal(jointLabelsFor('ur5e', 8).length, 8);
    },
  },
  {
    name: 'jointLabelsFor falls back to generic labels without a robot',
    run: () => {
      assert.deepEqual(jointLabelsFor(null, 3), ['J0', 'J1', 'J2']);
      assert.deepEqual(jointLabelsFor('unknown-robot', 2), ['J0', 'J1']);
      assert.deepEqual(jointLabelsFor(undefined, 0), []);
    },
  },
  {
    name: 'KNOWN_ROBOTS entries have unique ids and non-empty names',
    run: () => {
      const ids = new Set(KNOWN_ROBOTS.map((r) => r.id));
      assert.equal(ids.size, KNOWN_ROBOTS.length);
      for (const robot of KNOWN_ROBOTS) {
        assert.ok(robot.jointNames.length > 0);
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
