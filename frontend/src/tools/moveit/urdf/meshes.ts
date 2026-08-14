// Mesh loading for the URDF robot model (M13 D4) — binary/ASCII STL
// parsing via three's STLLoader (synchronous parse, no loader dependency)
// and async assembly of visual geometry onto a RobotModel. The byte
// resolver is injectable: fetch in the browser, fs in node tests.

import {
  BoxGeometry,
  BufferGeometry,
  CylinderGeometry,
  Mesh,
  MeshStandardMaterial,
  SphereGeometry,
  Vector3,
} from 'three';
import { STLLoader } from 'three/examples/jsm/loaders/STLLoader.js';

import { buildRobotModel, poseToTransform, type RobotModel } from './robot';
import { parseUrdf } from './urdf';

/** Default gray for visuals without a <material><color> (rviz convention). */
const DEFAULT_COLOR: [number, number, number, number] = [0.5, 0.5, 0.5, 1];

/** package://remap: strip everything through `description/` (the model
 * files live under the model dir with the remainder of the path). Plain
 * relative paths pass through untouched. */
export function remapPackagePath(filename: string): string {
  if (!filename.startsWith('package://')) return filename;
  const rest = filename.slice('package://'.length);
  const description = rest.indexOf('/description/');
  if (description !== -1) return rest.slice(description + '/description/'.length);
  const firstSlash = rest.indexOf('/');
  return firstSlash === -1 ? rest : rest.slice(firstSlash + 1);
}

/** Parse a binary or ASCII STL buffer into a BufferGeometry. */
export function parseStlGeometry(data: ArrayBuffer | string): BufferGeometry {
  const loader = new STLLoader();
  return loader.parse(data) as BufferGeometry;
}

/** Assemble a URDF robot with its visual geometry. `meshResolver` maps a
 * remapped relative path to STL bytes; it is only invoked for mesh
 * visuals (box/cylinder/sphere build synchronously). */
export async function loadUrdfRobot(
  urdfText: string,
  meshResolver: (relativePath: string) => Promise<ArrayBuffer>,
): Promise<RobotModel> {
  const urdf = parseUrdf(urdfText);
  const model = buildRobotModel(urdf);
  const stlLoader = new STLLoader();
  const geometryCache = new Map<string, BufferGeometry>();

  for (const link of urdf.links.values()) {
    const linkGroup = model.links.get(link.name);
    if (!linkGroup) continue;
    for (const visual of link.visuals) {
      const mesh = await buildVisualMesh(visual.geometry, visual.color, async (relativePath) => {
        const cached = geometryCache.get(relativePath);
        if (cached) return cached;
        const bytes = await meshResolver(relativePath);
        const geometry = parseStlGeometry(bytes);
        geometryCache.set(relativePath, geometry);
        return geometry;
      });
      const { position, quaternion } = poseToTransform(visual.origin.xyz, visual.origin.rpy);
      mesh.position.copy(position);
      mesh.quaternion.copy(quaternion);
      linkGroup.add(mesh);
    }
  }
  return model;
}

async function buildVisualMesh(
  geometry: { kind: 'mesh'; filename: string } | { kind: 'box'; size: [number, number, number] } | { kind: 'cylinder'; radius: number; length: number } | { kind: 'sphere'; radius: number },
  color: [number, number, number, number] | null,
  resolveGeometry: (relativePath: string) => Promise<BufferGeometry>,
): Promise<Mesh> {
  let geometryData: BufferGeometry;
  if (geometry.kind === 'mesh') {
    geometryData = await resolveGeometry(remapPackagePath(geometry.filename));
  } else if (geometry.kind === 'box') {
    geometryData = new BoxGeometry(...geometry.size);
  } else if (geometry.kind === 'cylinder') {
    geometryData = new CylinderGeometry(geometry.radius, geometry.radius, geometry.length, 24);
  } else {
    geometryData = new SphereGeometry(geometry.radius, 24, 16);
  }

  const rgba = color ?? DEFAULT_COLOR;
  const material = new MeshStandardMaterial({
    color: rgba[0] * 0x100 * 0x100 + rgba[1] * 0x100 + rgba[2],
    opacity: rgba[3],
    transparent: rgba[3] < 1,
  });
  return new Mesh(geometryData, material);
}

/** The end-effector link of a parsed URDF — last joint's child. */
export function endEffectorLinkOf(model: RobotModel): string {
  return model.endEffectorLink;
}

export { Vector3 };
