// Mesh loading tests (M13 D4): package:// remapping, binary STL parsing,
// async robot assembly. Self-executes on import — see tests.ts.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { Mesh } from 'three';

import { buildRobotModel } from './robot';
import { loadUrdfRobot, parseStlGeometry, remapPackagePath } from './meshes';
import { parseUrdf } from './urdf';

type TestCase = {
  name: string;
  run: () => void;
};

/** One minimal binary STL triangle (134 bytes). */
function buildBinaryStl(vertices: number[][]): ArrayBuffer {
  const buffer = new ArrayBuffer(84 + 50 * vertices.length);
  const view = new DataView(buffer);
  view.setUint32(80, vertices.length, true); // triangle count
  let offset = 84;
  for (const [x, y, z] of vertices) {
    view.setFloat32(offset, 0, true); // normal
    view.setFloat32(offset + 4, 0, true);
    view.setFloat32(offset + 8, 1, true);
    view.setFloat32(offset + 12, x, true);
    view.setFloat32(offset + 16, y, true);
    view.setFloat32(offset + 20, z, true);
    view.setFloat32(offset + 24, x + 1, true);
    view.setFloat32(offset + 28, y + 1, true);
    view.setFloat32(offset + 32, z + 1, true);
    view.setFloat32(offset + 36, x + 2, true);
    view.setFloat32(offset + 40, y + 2, true);
    view.setFloat32(offset + 44, z + 2, true);
    view.setUint16(offset + 48, 0, true); // attribute byte count
    offset += 50;
  }
  return buffer;
}

const PRIMITIVES_ONLY = `
<robot name="prims">
  <link name="base_link">
    <visual>
      <geometry><box size="0.1 0.2 0.3"/></geometry>
      <material name=""><color rgba="1 0 0 1"/></material>
    </visual>
  </link>
  <link name="tip">
    <visual>
      <origin xyz="0 0 0.5" rpy="0 0 0"/>
      <geometry><sphere radius="0.05"/></geometry>
    </visual>
  </link>
  <joint name="j" type="fixed">
    <parent link="base_link"/><child link="tip"/>
  </joint>
</robot>`;

const tests: TestCase[] = [
  {
    name: 'remapPackagePath strips the package:// prefix through description/',
    run: () => {
      assert.equal(
        remapPackagePath('package://rebotarm_bringup/description/meshes_b601_gripper/base_link.STL'),
        'meshes_b601_gripper/base_link.STL',
      );
      // Without a description/ segment, the package name alone is stripped
      assert.equal(remapPackagePath('package://some_pkg/meshes/a.STL'), 'meshes/a.STL');
      // Package names ending in _description (robot_descriptions style)
      // strip through the description segment too
      assert.equal(remapPackagePath('package://ur_description/meshes/ur5e/base.stl'), 'meshes/ur5e/base.stl');
      // Plain relative paths pass through untouched
      assert.equal(remapPackagePath('meshes_b601_gripper/link1.STL'), 'meshes_b601_gripper/link1.STL');
    },
  },
  {
    name: 'parseStlGeometry parses a binary STL into a triangle geometry',
    run: () => {
      const geometry = parseStlGeometry(buildBinaryStl([[0, 0, 0], [1, 0, 0]]));
      const position = geometry.getAttribute('position');
      assert.equal(position.count, 6); // 2 triangles × 3 vertices, non-indexed
      geometry.dispose();
    },
  },
  {
    name: 'loadUrdfRobot builds primitive geometry synchronously without the mesh resolver',
    run: async () => {
      let resolverCalled = false;
      const model = await loadUrdfRobot(PRIMITIVES_ONLY, async () => {
        resolverCalled = true;
        throw new Error('must not be called');
      });
      const meshes: Mesh[] = [];
      model.root.traverse((obj) => {
        if ((obj as Mesh).isMesh) meshes.push(obj as Mesh);
      });
      assert.equal(meshes.length, 2);
      assert.equal(resolverCalled, false);
      // The sphere visual carries its origin offset
      const tip = model.links.get('tip')!;
      assert.equal(tip.children.length, 1);
    },
  },
  {
    name: 'loadUrdfRobot resolves package:// meshes through the injected resolver',
    run: async () => {
      const urdf = `
      <robot name="m">
        <link name="base_link">
          <visual>
            <geometry><mesh filename="package://pkg/description/meshes/part.STL"/></geometry>
            <material name=""><color rgba="0.5 0.5 0.5 1"/></material>
          </visual>
        </link>
      </robot>`;
      const stl = buildBinaryStl([[0, 0, 0]]);
      const seen: string[] = [];
      const model = await loadUrdfRobot(urdf, async (relativePath) => {
        seen.push(relativePath);
        return stl;
      });
      assert.deepEqual(seen, ['meshes/part.STL']);
      const meshes: Mesh[] = [];
      model.root.traverse((obj) => {
        if ((obj as Mesh).isMesh) meshes.push(obj as Mesh);
      });
      assert.equal(meshes.length, 1);
      assert.equal(meshes[0].geometry.getAttribute('position').count, 3);
    },
  },
  {
    name: 'loadUrdfRobot assembles the real B601 model with STL meshes',
    run: async () => {
      const dir = fileURLToPath(new URL('../../../../../models/b601', import.meta.url));
      let urdfText: string;
      try {
        urdfText = readFileSync(`${dir}/reBot_B601_DM_with_gripper.urdf`, 'utf8');
      } catch {
        console.log('skip - real B601 URDF not present (models/b601 is local-only)');
        return;
      }
      const robot = parseUrdf(urdfText);
      const model = await loadUrdfRobot(urdfText, async (relativePath) => {
        const data = readFileSync(`${dir}/${relativePath}`);
        return data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
      });
      const meshCount = robot.links.size;
      const meshes: Mesh[] = [];
      model.root.traverse((obj) => {
        if ((obj as Mesh).isMesh) meshes.push(obj as Mesh);
      });
      // Every link carries at least one visual mesh
      assert.ok(meshes.length >= meshCount);
      // FK works on the loaded model: joint6 rotates the wrist
      model.setJointValue('joint1', 0.5);
      model.updateWorld();
      const before = model.getEndEffectorPosition().clone();
      model.setJointValue('joint1', 0);
      model.updateWorld();
      assert.ok(before.distanceTo(model.getEndEffectorPosition()) > 0.01);
      assert.equal(buildRobotModel(robot).jointOrder.length, 9);
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
