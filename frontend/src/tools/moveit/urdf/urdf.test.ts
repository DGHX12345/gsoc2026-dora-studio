// URDF parser tests (M13 D4). Self-executes on import — see tests.ts.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { parseUrdf } from './urdf';

type TestCase = {
  name: string;
  run: () => void;
};

const SAMPLE = `
<robot name="test_arm">
  <link name="base_link">
    <visual>
      <origin xyz="0 0 0" rpy="0 0 0"/>
      <geometry><mesh filename="package://pkg/description/meshes/base.STL"/></geometry>
      <material name="gray"><color rgba="0.5 0.5 0.5 1"/></material>
    </visual>
    <collision><geometry><box size="1 2 3"/></geometry></collision>
    <inertial><mass value="0.8"/></inertial>
  </link>
  <link name="link1">
    <visual>
      <geometry><box size="0.1 0.2 0.3"/></geometry>
      <material name="red"><color rgba="1 0 0 1"/></material>
    </visual>
    <visual>
      <origin xyz="0.01 0 0" rpy="0 0 0"/>
      <geometry><cylinder radius="0.05" length="0.2"/></geometry>
    </visual>
  </link>
  <link name="link2">
    <visual><geometry><sphere radius="0.1"/></geometry></visual>
  </link>
  <joint name="joint1" type="revolute">
    <origin xyz="0 0 0.1" rpy="0 0 1.5708"/>
    <parent link="base_link"/>
    <child link="link1"/>
    <axis xyz="0 0 1"/>
    <limit lower="-3.14" upper="3.14" effort="10" velocity="1"/>
  </joint>
  <joint name="joint2" type="prismatic">
    <parent link="link1"/>
    <child link="link2"/>
    <axis xyz="1 0 0"/>
    <limit lower="0" upper="0.0285" effort="5" velocity="0.05"/>
  </joint>
</robot>`;

const tests: TestCase[] = [
  {
    name: 'parses links, joints, visuals and geometry kinds',
    run: () => {
      const robot = parseUrdf(SAMPLE);
      assert.equal(robot.name, 'test_arm');
      assert.equal(robot.links.size, 3);
      assert.equal(robot.joints.length, 2);
      assert.equal(robot.rootLink, 'base_link');

      const base = robot.links.get('base_link')!;
      assert.equal(base.visuals.length, 1);
      assert.deepEqual(base.visuals[0].geometry, { kind: 'mesh', filename: 'package://pkg/description/meshes/base.STL' });
      assert.deepEqual(base.visuals[0].color, [0.5, 0.5, 0.5, 1]);

      const link1 = robot.links.get('link1')!;
      assert.equal(link1.visuals.length, 2);
      assert.deepEqual(link1.visuals[0].geometry, { kind: 'box', size: [0.1, 0.2, 0.3] });
      assert.deepEqual(link1.visuals[0].color, [1, 0, 0, 1]);
      assert.deepEqual(link1.visuals[1].geometry, { kind: 'cylinder', radius: 0.05, length: 0.2 });
      // No material element → null color (default gray at mesh build time)
      assert.equal(link1.visuals[1].color, null);

      const link2 = robot.links.get('link2')!;
      assert.deepEqual(link2.visuals[0].geometry, { kind: 'sphere', radius: 0.1 });
    },
  },
  {
    name: 'parses joint origins, axes and limits',
    run: () => {
      const robot = parseUrdf(SAMPLE);
      const joint1 = robot.joints[0];
      assert.equal(joint1.name, 'joint1');
      assert.equal(joint1.type, 'revolute');
      assert.equal(joint1.parent, 'base_link');
      assert.equal(joint1.child, 'link1');
      assert.deepEqual(joint1.origin, { xyz: [0, 0, 0.1], rpy: [0, 0, 1.5708] });
      assert.deepEqual(joint1.axis, [0, 0, 1]);
      assert.deepEqual(joint1.limit, { lower: -3.14, upper: 3.14 });

      const joint2 = robot.joints[1];
      assert.equal(joint2.type, 'prismatic');
      // Missing origin/axis default to identity and (1, 0, 0)
      assert.deepEqual(joint2.origin, { xyz: [0, 0, 0], rpy: [0, 0, 0] });
      assert.deepEqual(joint2.axis, [1, 0, 0]);
      assert.deepEqual(joint2.limit, { lower: 0, upper: 0.0285 });
    },
  },
  {
    name: 'ignores collision/inertial children and picks the non-child link as root',
    run: () => {
      const robot = parseUrdf(SAMPLE);
      assert.equal(robot.rootLink, 'base_link');
      // collision/inertial of base_link were skipped — no extra visuals
      assert.equal(robot.links.get('base_link')!.visuals.length, 1);
    },
  },
  {
    name: 'joints without limits parse with limit null',
    run: () => {
      const robot = parseUrdf(
        '<robot name="r"><link name="a"/><link name="b"/>' +
          '<joint name="j" type="continuous"><parent link="a"/><child link="b"/></joint></robot>',
      );
      assert.equal(robot.joints[0].limit, null);
      assert.equal(robot.joints[0].type, 'continuous');
    },
  },
  {
    name: 'throws on unknown joint types',
    run: () => {
      assert.throws(
        () =>
          parseUrdf(
            '<robot name="r"><link name="a"/><link name="b"/>' +
              '<joint name="j" type="planar"><parent link="a"/><child link="b"/></joint></robot>',
          ),
        /unknown joint type/,
      );
    },
  },
  {
    name: 'throws on joints without parent or child links',
    run: () => {
      assert.throws(
        () =>
          parseUrdf(
            '<robot name="r"><link name="a"/><joint name="j" type="fixed">' +
              '<child link="a"/></joint></robot>',
          ),
        /parent/,
      );
    },
  },
  {
    name: 'throws on malformed number tuples',
    run: () => {
      assert.throws(
        () =>
          parseUrdf(
            '<robot name="r"><link name="a"/><joint name="j" type="fixed">' +
              '<origin xyz="0 0 notanumber"/><parent link="a"/><child link="a"/></joint></robot>',
          ),
        /expected 3 numbers/,
      );
    },
  },
  {
    name: 'parses the real B601 URDF from models/b601',
    run: () => {
      const path = fileURLToPath(new URL('../../../../../models/b601/reBot_B601_DM_with_gripper.urdf', import.meta.url));
      let text: string;
      try {
        text = readFileSync(path, 'utf8');
      } catch {
        console.log('skip - real B601 URDF not present (models/b601 is local-only)');
        return;
      }
      const robot = parseUrdf(text);
      assert.equal(robot.name, 'reBot_B601_DM_with_gripper');
      assert.equal(robot.rootLink, 'base_link');
      // joint1-6 + gripper_joint + gripper_joint1 + gripper_joint2
      assert.deepEqual(
        robot.joints.map((j) => j.name),
        ['joint1', 'joint2', 'joint3', 'joint4', 'joint5', 'joint6', 'gripper_joint', 'gripper_joint1', 'gripper_joint2'],
      );
      const gripper = robot.joints.find((j) => j.name === 'gripper_joint1')!;
      assert.equal(gripper.type, 'prismatic');
      assert.ok(gripper.limit);
      // Verified from the file: prismatic finger travel 0..0.0715
      // (the handoff's 0.0285 was from a different URDF variant)
      assert.equal(gripper.limit!.upper, 0.0715);
      // Mesh visuals carry package:// paths for remapping
      const base = robot.links.get('base_link')!;
      assert.ok(base.visuals.some((v) => v.geometry.kind === 'mesh'));
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
