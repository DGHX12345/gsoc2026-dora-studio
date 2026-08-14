// Robot kinematic chain + FK tests (M13 D4). Self-executes on import —
// see tests.ts. Builds real THREE.Group hierarchies headlessly.

import assert from 'node:assert/strict';
import { Group, Vector3 } from 'three';

import { buildRobotModel } from './robot';
import { parseUrdf } from './urdf';

type TestCase = {
  name: string;
  run: () => void | Promise<void>;
};

/** 3-link chain: base → revolute(z) → link1 → revolute(z) → link2 →
 * revolute(z) → link3 (the end effector). Joint origins stack along z and
 * x so rotations are discriminating. */
const CHAIN = `
<robot name="chain">
  <link name="base_link"><visual><geometry><box size="0.1 0.1 0.1"/></geometry></visual></link>
  <link name="link1"><visual><geometry><box size="0.1 0.1 0.1"/></geometry></visual></link>
  <link name="link2"><visual><geometry><box size="0.1 0.1 0.1"/></geometry></visual></link>
  <link name="link3"><visual><geometry><box size="0.1 0.1 0.1"/></geometry></visual></link>
  <joint name="j1" type="revolute">
    <origin xyz="0 0 0.1" rpy="0 0 0"/>
    <parent link="base_link"/><child link="link1"/>
    <axis xyz="0 0 1"/>
    <limit lower="-3.14" upper="3.14" effort="1" velocity="1"/>
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

const near = (a: Vector3, b: [number, number, number]) => {
  assert.ok(
    Math.abs(a.x - b[0]) < 1e-9 && Math.abs(a.y - b[1]) < 1e-9 && Math.abs(a.z - b[2]) < 1e-9,
    `expected (${b[0]}, ${b[1]}, ${b[2]}), got (${a.x}, ${a.y}, ${a.z})`,
  );
};

const tests: TestCase[] = [
  {
    name: 'builds the joint hierarchy with pivots parented under their parent links',
    run: () => {
      const robot = parseUrdf(CHAIN);
      const model = buildRobotModel(robot);
      assert.equal(model.root.name, 'chain');
      assert.equal(model.links.size, 4);
      assert.deepEqual(model.jointOrder, ['j1', 'j2', 'j3']);
      const link1 = model.links.get('link1')!;
      // link1's parent in the scene is the j1 pivot, which is a child of base
      assert.equal(link1.parent, model.joints.get('j1')!.pivot);
    },
  },
  {
    name: 'FK: zero pose stacks joint origins along z and x',
    run: () => {
      const model = buildRobotModel(parseUrdf(CHAIN));
      model.updateWorld();
      near(model.getLinkWorldPosition('link1'), [0, 0, 0.1]);
      near(model.getLinkWorldPosition('link2'), [0.2, 0, 0.1]);
      near(model.getLinkWorldPosition('link3'), [0.3, 0, 0.1]);
      near(model.getEndEffectorPosition(), [0.3, 0, 0.1]);
    },
  },
  {
    name: 'FK: revolute joints rotate descendants around their axes',
    run: () => {
      const model = buildRobotModel(parseUrdf(CHAIN));
      model.setJointValue('j1', Math.PI / 2);
      model.updateWorld();
      // link1 origin unchanged; link2's (0.2, 0, 0.1) offset rotates with j1
      near(model.getLinkWorldPosition('link2'), [0, 0.2, 0.1]);
      near(model.getLinkWorldPosition('link3'), [0, 0.3, 0.1]);

      model.setJointValue('j2', Math.PI / 2);
      model.updateWorld();
      // link3 offset (0.1,0,0) now points along world +y after j1+j2 = π
      near(model.getLinkWorldPosition('link3'), [-0.1, 0.2, 0.1]);
    },
  },
  {
    name: 'prismatic joints translate along the axis with limit clamping',
    run: () => {
      const robot = parseUrdf(
        '<robot name="p"><link name="a"/><link name="b"/>' +
          '<joint name="slide" type="prismatic">' +
          '<origin xyz="0 0 0" rpy="0 0 0"/>' +
          '<parent link="a"/><child link="b"/>' +
          '<axis xyz="1 0 0"/>' +
          '<limit lower="0" upper="0.0715" effort="1" velocity="1"/>' +
          '</joint></robot>',
      );
      const model = buildRobotModel(robot);
      model.setJointValue('slide', 0.05);
      model.updateWorld();
      near(model.getLinkWorldPosition('b'), [0.05, 0, 0]);

      // Clamped to the upper limit
      model.setJointValue('slide', 0.5);
      model.updateWorld();
      near(model.getLinkWorldPosition('b'), [0.0715, 0, 0]);

      // Revolute joints also clamp when limits exist: 10 rad → 3.14 ≈ π,
      // so link2's (0.2, 0, 0.1) offset flips to (-0.2, ~0, 0.1).
      const chain = buildRobotModel(parseUrdf(CHAIN));
      chain.setJointValue('j1', 10);
      chain.updateWorld();
      const p = chain.getLinkWorldPosition('link2');
      assert.ok(Math.abs(p.x + 0.2) < 0.01 && Math.abs(p.y) < 0.01 && Math.abs(p.z - 0.1) < 1e-9);
    },
  },
  {
    name: 'setJointValues applies a map of values at once',
    run: () => {
      const model = buildRobotModel(parseUrdf(CHAIN));
      model.setJointValues({ j1: Math.PI / 2, j2: Math.PI / 2 });
      model.updateWorld();
      near(model.getLinkWorldPosition('link3'), [-0.1, 0.2, 0.1]);
    },
  },
  {
    name: 'clonePose produces a detached copy with transparent materials',
    run: async () => {
      // Meshes only exist after loadUrdfRobot assembles the visuals —
      // buildRobotModel alone is geometry-free.
      const { loadUrdfRobot } = await import('./meshes');
      const model = await loadUrdfRobot(CHAIN, async () => {
        throw new Error('no mesh visuals in CHAIN');
      });
      const ghost = model.clonePose(0.35);
      assert.notEqual(ghost, model.root);
      assert.equal(ghost.parent, null);
      let ghostMeshCount = 0;
      ghost.traverse((obj) => {
        const mesh = obj as { material?: { transparent?: boolean; opacity?: number } };
        if (mesh.material) {
          ghostMeshCount += 1;
          assert.equal(mesh.material.transparent, true);
          assert.equal(mesh.material.opacity, 0.35);
        }
      });
      assert.ok(ghostMeshCount > 0);
      // The original model materials stay opaque
      let checked = 0;
      model.root.traverse((obj) => {
        const mesh = obj as { material?: { transparent?: boolean; opacity?: number } };
        if (mesh.material) {
          assert.notEqual(mesh.material.transparent, true);
          checked += 1;
        }
      });
      assert.equal(checked, ghostMeshCount);
    },
  },
];

let failures = 0;

for (const test of tests) {
  try {
    // Async tests must be awaited — a sync runner swallows async failures.
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
