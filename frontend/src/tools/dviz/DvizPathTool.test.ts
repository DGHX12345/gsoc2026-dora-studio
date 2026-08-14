// DvizPathTool tests (M12 D2). Self-executes on import — see tests.ts.
//
// Constructs a real THREE.Scene headlessly: three object construction
// (including Line2/LineMaterial) works without a renderer.

import assert from 'node:assert/strict';
import {
  DataTexture,
  Group,
  InstancedMesh,
  Mesh,
  MeshBasicMaterial,
  NearestFilter,
  PerspectiveCamera,
  PlaneGeometry,
  RGBAFormat,
  Scene,
  SphereGeometry,
  SRGBColorSpace,
  Vector3,
} from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';

import { matchToolPorts } from '../matching';
import type { ToolBatch, ToolContext, ToolPayload } from '../types';
import { computePathLength } from './parse';
import { buildCostmapLUT, computePathBounds, computeStaleness, DvizPathTool } from './DvizPathTool';

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

const arrowsOf = (group: Group): InstancedMesh =>
  group.children.find((c): c is InstancedMesh => c instanceof InstancedMesh)!;

/** Start/end markers are the SphereGeometry meshes of a primary path group.
 * (Line2 extends Mesh in this three version, so geometry type disambiguates.) */
const markerOf = (group: Group, index: number): Mesh =>
  group.children.filter(
    (c): c is Mesh => c instanceof Mesh && c.geometry instanceof SphereGeometry,
  )[index];

/** Assert a 3x3-identity-rotated arrow matrix landed at (x, y, z=0.05).
 * instanceMatrix is an InstancedBufferAttribute (column-major, 16 floats per
 * matrix); translation z rides a Float32Array: 0.05 is not exactly representable. */
const assertArrowAt = (array: ArrayLike<number>, k: number, x: number, y: number) => {
  const off = k * 16 + 12; // translation column
  assert.deepEqual([array[off], array[off + 1]], [x, y]);
  assert.ok(Math.abs(array[off + 2] - 0.05) < 1e-6, `arrow ${k} z is 0.05`);
};

/** Flat xy pairs for n waypoints at (i, 0). */
const waypointXY = (n: number): number[] => {
  const out: number[] = [];
  for (let i = 0; i < n; i++) out.push(i, 0);
  return out;
};

/** The costmap ground-plane mesh inside the dviz-path group, if any. */
const costmapMeshOf = (context: ToolContext): Mesh | undefined =>
  rootGroup(context).children.find((c) => c.name === 'costmap') as Mesh | undefined;

const costmapTextureOf = (mesh: Mesh): DataTexture =>
  (mesh.material as MeshBasicMaterial).map as DataTexture;

/** A valid costmap JSON payload: width×height values normalized to [0, 1]. */
const costmapJson = (width: number, height: number, resolution: number, values: number[]) => ({
  json: { width, height, resolution, values },
});

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
    name: 'costmap and esdf batches create no path and no target but no throw',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(2, 2, 1, [0, 0, 0, 0])));
      tool.onBatch(batch('planner', 'esdf', 200, costmapJson(2, 2, 1, [0, 0, 0, 0])));

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 0);
      assert.equal(snapshot.target, null);
      // The esdf batch is the last known costmap source.
      assert.equal(snapshot.costmap?.width, 2);
      assert.equal(snapshot.costmap?.lastBatchTs, 200);
      assert.ok(costmapMeshOf(context) instanceof Mesh);
      tool.onDetach();
    },
  },
  {
    name: 'buildCostmapLUT: blue→yellow→red piecewise ramp, 768 bytes',
    run: () => {
      const lut = buildCostmapLUT();
      assert.equal(lut.length, 768);
      // Endpoints: blue at 0, red at 1.
      assert.deepEqual([...lut.slice(0, 3)], [0, 0, 255]);
      assert.deepEqual([...lut.slice(255 * 3, 255 * 3 + 3)], [255, 0, 0]);
      // Entries 127/128 sit at the blue→yellow/yellow→red seam: ≈ yellow.
      for (const i of [127, 128]) {
        assert.ok(Math.abs(lut[3 * i] - 255) <= 1, `entry ${i} red channel near 255`);
        assert.ok(Math.abs(lut[3 * i + 1] - 255) <= 1, `entry ${i} green channel near 255`);
        assert.ok(lut[3 * i + 2] <= 1, `entry ${i} blue channel near 0`);
      }
      // Blue→yellow half (0..127): red and green climb, blue falls.
      for (let i = 1; i <= 127; i++) {
        assert.ok(lut[3 * i] >= lut[3 * (i - 1)], `red non-decreasing at ${i}`);
        assert.ok(lut[3 * i + 1] >= lut[3 * (i - 1) + 1], `green non-decreasing at ${i}`);
        assert.ok(lut[3 * i + 2] <= lut[3 * (i - 1) + 2], `blue non-increasing at ${i}`);
      }
      // Yellow→red half (128..255): red pinned, green falls, blue stays 0.
      for (let i = 129; i < 256; i++) {
        assert.equal(lut[3 * i], 255, `red pinned at ${i}`);
        assert.ok(lut[3 * i + 1] <= lut[3 * (i - 1) + 1], `green non-increasing at ${i}`);
        assert.equal(lut[3 * i + 2], 0, `blue zero at ${i}`);
      }
    },
  },
  {
    name: 'a valid costmap batch creates a semi-transparent textured plane and snapshot',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      const values = Array.from({ length: 16 }, (_, i) => i / 15);
      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(4, 4, 0.5, values)));

      const mesh = costmapMeshOf(context)!;
      assert.ok(mesh instanceof Mesh);
      assert.ok(mesh.geometry instanceof PlaneGeometry);
      // World extent = cell count × resolution, flat in the XY plane at z=0.02.
      assert.equal(mesh.geometry.parameters.width, 4 * 0.5);
      assert.equal(mesh.geometry.parameters.height, 4 * 0.5);
      assert.ok(Math.abs(mesh.position.z - 0.02) < 1e-6);
      assert.equal(mesh.visible, true);

      // Plane lies flat on the XY ground plane: no rotation, normal stays +Z.
      assert.deepEqual([mesh.quaternion.x, mesh.quaternion.y, mesh.quaternion.z, mesh.quaternion.w], [
        0, 0, 0, 1,
      ]);
      assert.deepEqual(
        new Vector3(0, 0, 1).applyQuaternion(mesh.quaternion).toArray(),
        [0, 0, 1],
      );

      const material = mesh.material as MeshBasicMaterial;
      assert.equal(material.transparent, true);
      assert.equal(material.opacity, 0.6);
      assert.equal(material.depthWrite, false);
      const texture = costmapTextureOf(mesh);
      assert.ok(texture instanceof DataTexture);
      assert.equal(texture.image.width, 4);
      assert.equal(texture.image.height, 4);
      // 4 bytes/texel (RGBA): the default RGBAFormat matches the buffer stride.
      assert.equal(texture.format, RGBAFormat);
      assert.equal((texture.image.data as Uint8Array).length, 4 * 4 * 4);
      assert.equal(texture.magFilter, NearestFilter);
      assert.equal(texture.minFilter, NearestFilter);
      assert.equal(texture.colorSpace, SRGBColorSpace);

      const snapshot = tool.getSnapshot();
      assert.ok(snapshot.costmap);
      assert.equal(snapshot.costmap.visible, true);
      assert.equal(snapshot.costmap.opacity, 0.6);
      assert.equal(snapshot.costmap.width, 4);
      assert.equal(snapshot.costmap.height, 4);
      assert.equal(snapshot.costmap.resolution, 0.5);
      assert.equal(snapshot.costmap.lastBatchTs, 100);
      assert.equal(snapshot.paths.length, 0);
      tool.onDetach();
    },
  },
  {
    name: 'a same-dimension costmap batch reuses the texture and geometry (data refreshed)',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(
        batch('planner', 'costmap', 100, costmapJson(4, 4, 0.5, Array.from({ length: 16 }, (_, i) => i / 15))),
      );
      const mesh = costmapMeshOf(context)!;
      const texture = costmapTextureOf(mesh);
      const geometry = mesh.geometry;
      // The tool always stores the Uint8Array it constructed the texture with.
      const data = texture.image.data as Uint8Array;

      // All-free (0) map → pure blue; same texture object, data rewritten.
      tool.onBatch(batch('planner', 'costmap', 200, costmapJson(4, 4, 0.5, Array(16).fill(0))));
      assert.deepEqual([data[0], data[1], data[2]], [0, 0, 255]);
      assert.equal(costmapTextureOf(costmapMeshOf(context)!), texture);
      assert.equal(costmapMeshOf(context)!.geometry, geometry);
      // needsUpdate is a write-only setter in three 0.185: every refresh bump
      // increments version (1 = creation, 2 = this refresh).
      assert.ok(texture.version >= 2, 'texture refresh bumped version');

      // All-obstacle (1) map → pure red.
      tool.onBatch(batch('planner', 'costmap', 300, costmapJson(4, 4, 0.5, Array(16).fill(1))));
      assert.deepEqual([data[0], data[1], data[2]], [255, 0, 0]);

      // Still exactly one plane in the group; snapshot dims/timestamp follow.
      assert.equal(rootGroup(context).children.filter((c) => c.name === 'costmap').length, 1);
      assert.equal(tool.getSnapshot().costmap?.lastBatchTs, 300);
      tool.onDetach();
    },
  },
  {
    name: 'costmap values outside [0,1] clamp to the LUT endpoints (blue for <=0, red for >=1)',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      // Cell 0 = -0.5 → clamped to 0 → LUT[0] blue; cell 1 = 1.7 → clamped to
      // 1 → LUT[255] red; the other cells stay mid-ramp.
      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(2, 2, 1, [-0.5, 1.7, 0.5, 0.5])));
      const data = costmapTextureOf(costmapMeshOf(context)!).image.data as Uint8Array;

      // Texel 0 (value -0.5): pure blue, alpha 255.
      assert.deepEqual([data[0], data[1], data[2], data[3]], [0, 0, 255, 255]);
      // Texel 1 (value 1.7): pure red, alpha 255.
      assert.deepEqual([data[4], data[5], data[6], data[7]], [255, 0, 0, 255]);
      tool.onDetach();
    },
  },
  {
    name: 'a different-dimension costmap batch rebuilds the plane and disposes the old texture',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(4, 4, 0.5, Array(16).fill(0))));
      const oldMesh = costmapMeshOf(context)!;
      const oldTexture = costmapTextureOf(oldMesh);

      tool.onBatch(batch('planner', 'costmap', 200, costmapJson(8, 8, 0.25, Array(64).fill(0.5))));

      const meshes = rootGroup(context).children.filter((c) => c.name === 'costmap');
      assert.equal(meshes.length, 1);
      const rebuilt = meshes[0] as Mesh;
      assert.notEqual(rebuilt, oldMesh);
      const newTexture = costmapTextureOf(rebuilt);
      assert.notEqual(newTexture, oldTexture); // old texture no longer referenced
      assert.ok(newTexture.image.width === 8 && newTexture.image.height === 8);
      assert.ok(rebuilt.geometry instanceof PlaneGeometry);
      assert.equal(rebuilt.geometry.parameters.width, 8 * 0.25);
      assert.equal(rebuilt.geometry.parameters.height, 8 * 0.25);
      assert.equal(tool.getSnapshot().costmap?.width, 8);
      assert.equal(tool.getSnapshot().costmap?.resolution, 0.25);
      tool.onDetach();
    },
  },
  {
    name: 'setCostmapOpacity clamps to [0,1] and notifies; snapshot follows',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      let notified = 0;
      tool.subscribe(() => {
        notified += 1;
      });
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(2, 2, 1, [0, 0.5, 0.25, 1])));
      assert.equal(notified, 1);

      const material = costmapMeshOf(context)!.material as MeshBasicMaterial;
      tool.setCostmapOpacity(0.3);
      assert.equal(material.opacity, 0.3);
      assert.equal(tool.getSnapshot().costmap?.opacity, 0.3);
      assert.equal(notified, 2);

      tool.setCostmapOpacity(1.7); // clamped down to 1
      assert.equal(material.opacity, 1);
      assert.equal(tool.getSnapshot().costmap?.opacity, 1);

      tool.setCostmapOpacity(-0.5); // clamped up to 0
      assert.equal(material.opacity, 0);
      assert.equal(tool.getSnapshot().costmap?.opacity, 0);
      assert.equal(notified, 4);
      tool.onDetach();
    },
  },
  {
    name: 'setCostmapVisible toggles the plane and notifies; snapshot follows',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      let notified = 0;
      tool.subscribe(() => {
        notified += 1;
      });
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(2, 2, 1, [0, 0, 0, 0])));

      const mesh = costmapMeshOf(context)!;
      tool.setCostmapVisible(false);
      assert.equal(mesh.visible, false);
      assert.equal(tool.getSnapshot().costmap?.visible, false);
      assert.equal(notified, 2);

      tool.setCostmapVisible(true);
      assert.equal(mesh.visible, true);
      assert.equal(tool.getSnapshot().costmap?.visible, true);
      assert.equal(notified, 3);
      tool.onDetach();
    },
  },
  {
    name: 'an invalid costmap payload keeps the last known costmap without throwing',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(2, 2, 1, [0, 0.5, 0.25, 1])));
      const mesh = costmapMeshOf(context)!;
      const texture = costmapTextureOf(mesh);

      // Wrong values length: parseCostmap rejects it.
      assert.doesNotThrow(() => {
        tool.onBatch(batch('planner', 'costmap', 200, costmapJson(2, 2, 1, [0, 1])));
      });
      // Plane and texture untouched; snapshot keeps the last known dims/ts.
      assert.equal(costmapMeshOf(context), mesh);
      assert.equal(costmapTextureOf(costmapMeshOf(context)!), texture);
      assert.equal(tool.getSnapshot().costmap?.width, 2);
      assert.equal(tool.getSnapshot().costmap?.lastBatchTs, 100);
      tool.onDetach();
    },
  },
  {
    name: 'a costmap batch before any path batch works: plane present, paths empty',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'esdf', 100, costmapJson(2, 2, 1, [0.5, 0.5, 0.5, 0.5])));
      const mesh = costmapMeshOf(context)!;
      assert.ok(mesh instanceof Mesh);
      assert.equal(mesh.visible, true);
      const snapshot = tool.getSnapshot();
      assert.ok(snapshot.costmap);
      assert.equal(snapshot.costmap.width, 2);
      assert.equal(snapshot.costmap.lastBatchTs, 100);
      assert.equal(snapshot.paths.length, 0);
      assert.equal(snapshot.target, null);
      tool.onDetach();
    },
  },
  {
    name: 'onDetach after a costmap leaves the scene empty and the snapshot costmap null',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'costmap', 100, costmapJson(2, 2, 1, [0, 0, 0, 0])));
      assert.equal(context.scene.children.length, 1);

      tool.onDetach();
      assert.equal(context.scene.children.length, 0);
      assert.equal(tool.getSnapshot().costmap, null);
      tool.onDetach(); // double detach stays safe
      assert.equal(tool.getSnapshot().costmap, null);
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
      assert.equal(snapshot.costmap, null);
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
      assert.deepEqual(tool.getSnapshot(), { paths: [], target: null, lastSeekTs: null, costmap: null });
      tool.onDetach(); // must not throw
      assert.deepEqual(tool.getSnapshot(), { paths: [], target: null, lastSeekTs: null, costmap: null });
    },
  },
  {
    name: 'arrows: 25 waypoints yield 2 cones at waypoint indices 10 and 20',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'waypoints', 100, f32(waypointXY(25))));

      const arrows = arrowsOf(pathGroup(context, 'path:planner/waypoints'));
      assert.equal(arrows.count, 2);
      assert.equal(arrows.instanceMatrix.array.length, 2 * 16); // capacity 2
      const e = arrows.instanceMatrix.array;
      // k=0 → waypoint 10, k=1 → waypoint 20; waypoint 24 stays bare (i < n-1).
      assertArrowAt(e, 0, 10, 0);
      assertArrowAt(e, 1, 20, 0);
      tool.onDetach();
    },
  },
  {
    name: 'arrows: capacity grows amortized (doubling) across increasing waypoint counts',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);
      const group = () => pathGroup(context, 'path:planner/waypoints');

      tool.onBatch(batch('planner', 'waypoints', 100, f32(waypointXY(25)))); // 2 arrows
      assert.equal(arrowsOf(group()).count, 2);
      assert.equal(arrowsOf(group()).instanceMatrix.array.length, 2 * 16);

      tool.onBatch(batch('planner', 'waypoints', 200, f32(waypointXY(55)))); // 5 arrows
      assert.equal(arrowsOf(group()).count, 5);
      assert.equal(arrowsOf(group()).instanceMatrix.array.length, 5 * 16);

      tool.onBatch(batch('planner', 'waypoints', 300, f32(waypointXY(85)))); // 8 arrows
      assert.equal(arrowsOf(group()).count, 8);
      // Amortized growth doubles the previous capacity (5 -> 10), not to 8.
      assert.equal(arrowsOf(group()).instanceMatrix.array.length, 10 * 16);
      tool.onDetach();
    },
  },
  {
    name: 'arrows: shrinking waypoint count keeps capacity and the correct count',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'waypoints', 100, f32(waypointXY(25))));
      const group = pathGroup(context, 'path:planner/waypoints');
      // capacity 2 → 8: doubling gives 4 but 8 are needed (85 waypoints → 8 cones).
      tool.onBatch(batch('planner', 'waypoints', 200, f32(waypointXY(85))));
      const grown = arrowsOf(group);
      assert.equal(grown.count, 8);
      assert.equal(grown.instanceMatrix.array.length, 8 * 16);

      tool.onBatch(batch('planner', 'waypoints', 300, f32(waypointXY(25)))); // shrink to 2
      assert.equal(arrowsOf(group), grown); // no reallocation on shrink
      assert.equal(arrowsOf(group).count, 2);
      assert.equal(arrowsOf(group).instanceMatrix.array.length, 8 * 16); // capacity kept
      assertArrowAt(arrowsOf(group).instanceMatrix.array, 0, 10, 0); // repositioned
      tool.onDetach();
    },
  },
  {
    name: 'arrows: duplicate consecutive waypoints give a degenerate segment with cone +Y and no NaN',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      // Waypoints 10 and 11 are both (7, 0): the segment at arrow 0 is degenerate.
      const xy: number[] = [];
      for (let i = 0; i < 25; i++) xy.push(i === 10 || i === 11 ? 7 : i, 0);
      tool.onBatch(batch('planner', 'waypoints', 100, f32(xy)));

      const arrows = arrowsOf(pathGroup(context, 'path:planner/waypoints'));
      assert.equal(arrows.count, 2);
      const e = arrows.instanceMatrix.array;
      // k=0 (waypoint 10): identity rotation keeps the cone pointing +Y.
      assert.deepEqual([e[0], e[5], e[10]], [1, 1, 1]);
      assert.equal(e[1], 0); // no off-diagonal rotation
      assert.deepEqual([e[12], e[13]], [7, 0]);
      assert.ok(!e.some(Number.isNaN), 'arrow matrices contain no NaN');
      tool.onDetach();
    },
  },
  {
    name: 'attach → detach → re-attach: one fresh dviz-path group and a populated snapshot',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1, 2, 0])));

      assert.equal(context.scene.children.length, 1);
      tool.onDetach();
      assert.equal(context.scene.children.length, 0);
      assert.deepEqual(tool.getSnapshot(), { paths: [], target: null, lastSeekTs: null, costmap: null });

      tool.onAttach(context);
      tool.onBatch(batch('planner', 'waypoints', 200, f32([5, 5, 6, 5, 7, 5])));

      assert.equal(context.scene.children.length, 1);
      assert.equal(rootGroup(context).name, 'dviz-path');
      assert.equal(context.scene.children.filter((c) => c.name === 'dviz-path').length, 1);

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 1);
      assert.equal(snapshot.paths[0].key, 'planner/waypoints');
      assert.equal(snapshot.paths[0].pointCount, 3);
      assert.equal(snapshot.paths[0].lastBatchTs, 200);
      assert.equal(snapshot.paths[0].colorHex, 0x22d3ee);
      assert.equal(
        lineMaterialOf(pathGroup(context, 'path:planner/waypoints')).color.getHex(),
        0x22d3ee,
      );
      tool.onDetach();
    },
  },
  {
    name: 'a hidden path stays hidden when a new batch arrives',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'waypoints', 100, f32([0, 0, 1, 1, 2, 0])));
      tool.setPathVisible('planner/waypoints', false);

      tool.onBatch(batch('planner', 'waypoints', 200, f32([1, 1, 2, 1, 3, 1])));
      assert.equal(pathGroup(context, 'path:planner/waypoints').visible, false);
      assert.equal(tool.getSnapshot().paths[0].visible, false);
      assert.equal(tool.getSnapshot().paths[0].lastBatchTs, 200); // data still updates
      tool.onDetach();
    },
  },
  {
    name: 'a single-point path does not throw: empty line, overlapping markers, no arrows',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      assert.doesNotThrow(() => {
        tool.onBatch(batch('planner', 'waypoints', 100, f32([1, 2, 3])));
      });

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 1);
      assert.equal(snapshot.paths[0].pointCount, 1);

      const group = pathGroup(context, 'path:planner/waypoints');
      const start = markerOf(group, 0);
      const end = markerOf(group, 1);
      assert.deepEqual([start.position.x, start.position.y, start.position.z], [1, 2, 0.05]);
      assert.deepEqual([end.position.x, end.position.y, end.position.z], [1, 2, 0.05]);
      assert.equal(arrowsOf(group).count, 0);
      // A single position yields no segments: the instance buffer stays empty.
      assert.equal(lineGeometryOf(group).attributes.instanceStart.array.length, 0);
      tool.onDetach();
    },
  },
  {
    name: 'output ids differing only in case collapse into one path key',
    run: () => {
      const context = makeContext();
      const tool = new DvizPathTool();
      tool.onAttach(context);

      tool.onBatch(batch('planner', 'Waypoints', 100, f32([0, 0, 1, 1, 2, 0])));
      tool.onBatch(batch('planner', 'waypoints', 200, f32([5, 5, 6, 5, 7, 5])));

      const snapshot = tool.getSnapshot();
      assert.equal(snapshot.paths.length, 1);
      assert.equal(snapshot.paths[0].key, 'planner/waypoints');
      assert.equal(snapshot.paths[0].pointCount, 3);
      assert.equal(snapshot.paths[0].lastBatchTs, 200);

      const named = rootGroup(context).children.filter((c) => c.name === 'path:planner/waypoints');
      assert.equal(named.length, 1);
      const first = firstPointOf(lineGeometryOf(pathGroup(context, 'path:planner/waypoints')));
      assert.deepEqual([first[0], first[1]], [5, 5]);
      assert.ok(Math.abs(first[2] - 0.05) < 1e-6);
      tool.onDetach();
    },
  },
  {
    name: 'computePathBounds: empty, single point, and the corners of a 1×2×3 box',
    run: () => {
      assert.deepEqual(computePathBounds([]), { center: { x: 0, y: 0, z: 0 }, radius: 0 });

      assert.deepEqual(computePathBounds([4, -2, 7]), {
        center: { x: 4, y: -2, z: 7 },
        radius: 0,
      });

      // All 8 corners of the box [0,1]×[0,2]×[0,3].
      const box = [
        0, 0, 0, 1, 0, 0, 0, 2, 0, 1, 2, 0,
        0, 0, 3, 1, 0, 3, 0, 2, 3, 1, 2, 3,
      ];
      const { center, radius } = computePathBounds(box);
      assert.deepEqual(center, { x: 0.5, y: 1, z: 1.5 });
      assert.equal(radius, Math.hypot(1, 2, 3) / 2); // half-diagonal

      // Non-triplet input: guarded, no NaN min/max.
      assert.deepEqual(computePathBounds([0, 0, 0, 1]), {
        center: { x: 0, y: 0, z: 0 },
        radius: 0,
      });

      // Corners of the box [-1,-2,-3]×[1,2,3] spanning negative coordinates.
      const negativeBox = [
        -1, -2, -3, 1, -2, -3, -1, 2, -3, 1, 2, -3,
        -1, -2, 3, 1, -2, 3, -1, 2, 3, 1, 2, 3,
      ];
      const neg = computePathBounds(negativeBox);
      assert.deepEqual(neg.center, { x: 0, y: 0, z: 0 });
      assert.equal(neg.radius, Math.hypot(1, 2, 3)); // half-diagonal = sqrt(14)
    },
  },
  {
    name: 'snapCameraToPath calls focusOn with the path bounds when the context provides it',
    run: () => {
      const calls: Array<{ center: { x: number; y: number; z: number }; radius: number }> = [];
      const context: ToolContext = {
        scene: new Scene(),
        camera: new PerspectiveCamera(),
        requestRender: () => {},
        focusOn: (center, radius) => {
          calls.push({ center, radius });
        },
      };
      const tool = new DvizPathTool();
      tool.onAttach(context);

      // trajectory = stride-3 xyz, passes through the parser unchanged.
      tool.onBatch(batch('planner', 'trajectory', 100, f32([0, 0, 0, 1, 2, 3])));
      tool.snapCameraToPath('planner/trajectory');
      assert.equal(calls.length, 1);
      assert.deepEqual(calls[0].center, { x: 0.5, y: 1, z: 1.5 });
      assert.equal(calls[0].radius, Math.hypot(1, 2, 3) / 2);

      // A later batch updates the stored points: framing follows.
      tool.onBatch(batch('planner', 'trajectory', 200, f32([5, 5, 5, 7, 7, 7])));
      tool.snapCameraToPath('planner/trajectory');
      assert.equal(calls.length, 2);
      assert.deepEqual(calls[1].center, { x: 6, y: 6, z: 6 });
      assert.equal(calls[1].radius, Math.hypot(2, 2, 2) / 2);

      tool.snapCameraToPath('missing/path'); // unknown key: no call
      assert.equal(calls.length, 2);
      tool.onDetach();
    },
  },
  {
    name: 'snapCameraToPath without focusOn falls back to camera position + lookAt',
    run: () => {
      const camera = new PerspectiveCamera();
      camera.position.set(0, 0, 0);
      const context: ToolContext = { scene: new Scene(), camera, requestRender: () => {} };
      const tool = new DvizPathTool();
      tool.onAttach(context);
      tool.onBatch(batch('planner', 'trajectory', 100, f32([0, 0, 0, 1, 2, 3])));

      assert.doesNotThrow(() => tool.snapCameraToPath('planner/trajectory'));
      const radius = Math.hypot(1, 2, 3) / 2;
      assert.deepEqual(
        [camera.position.x, camera.position.y, camera.position.z],
        [0.5 + radius * 1.75, 1 - radius * 2.15, 1.5 + radius * 1.1],
      );
      assert.ok(camera.position.lengthSq() > 0); // moved off the origin
      tool.onDetach();
    },
  },
  {
    name: 'snapCameraToPath without focusOn: a single-point path frames off the point with no NaN',
    run: () => {
      const camera = new PerspectiveCamera();
      camera.position.set(0, 0, 0);
      const context: ToolContext = { scene: new Scene(), camera, requestRender: () => {} };
      const tool = new DvizPathTool();
      tool.onAttach(context);
      // Single point → radius 0: the fallback must use a safe unit radius.
      tool.onBatch(batch('planner', 'trajectory', 100, f32([1, 2, 3])));

      assert.doesNotThrow(() => tool.snapCameraToPath('planner/trajectory'));
      assert.ok(
        [camera.position.x, camera.position.y, camera.position.z].every(Number.isFinite),
        'camera position contains no NaN/Infinity',
      );
      assert.deepEqual(
        [camera.position.x, camera.position.y, camera.position.z],
        [1 + 1.75, 2 - 2.15, 3 + 1.1],
      );
      // Moved off the path point: never parked exactly on it.
      assert.notDeepEqual(
        [camera.position.x, camera.position.y, camera.position.z],
        [1, 2, 3],
      );
      assert.ok(camera.position.lengthSq() > 0);
      tool.onDetach();
    },
  },
  {
    name: 'snapCameraToPath before attach is a no-op and does not throw',
    run: () => {
      const tool = new DvizPathTool();
      assert.doesNotThrow(() => tool.snapCameraToPath('planner/waypoints'));
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
