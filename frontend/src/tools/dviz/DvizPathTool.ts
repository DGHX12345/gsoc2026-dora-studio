// DvizPathTool — M12 D2 core renderer for dviz planner path data.
//
// Renders waypoints/path/trajectory batches as 3D wide lines (Line2) with
// start/end markers and direction arrows on each node's primary path, plus a
// target marker. Costmap/esdf batches (M12 D3) render as a semi-transparent
// textured plane under the paths, color-ramped blue→yellow→red. Path data
// arrives in the world frame (dviz world topics), so the tf argument is
// ignored entirely (R11) — no TF transforms.
//
// The tool is the single source of truth for the D4 control panel:
// subscribe()/getSnapshot()/setPathVisible()/setCostmapVisible()/
// setCostmapOpacity() expose path/target/costmap state.

import {
  ConeGeometry,
  DataTexture,
  DynamicDrawUsage,
  Group,
  InstancedMesh,
  Material,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  NearestFilter,
  PlaneGeometry,
  Quaternion,
  SphereGeometry,
  SRGBColorSpace,
  Vector3,
} from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import type { Component } from 'vue';

import type { TfTree } from '../tf';
import type { ToolBatch, ToolContext, ViewportTool } from '../types';
import {
  computePathLength,
  parseCostmap,
  parseTarget,
  parseTrajectory,
  parseWaypoints,
} from './parse';

const NODE_COLORS = [0x22d3ee, 0xe879f9, 0xfb923c]; // cyan, magenta, orange
const TARGET_COLOR = 0xff6ad5;
const MARKER_RADIUS = 0.02;
const START_COLOR = 0x22c55e;
const END_COLOR = 0xef4444;
const ALTERNATIVE_COLOR = 0xffffff;
const ALTERNATIVE_OPACITY = 0.3;
const LINE_WIDTH = 2;
const ARROW_EVERY = 10; // direction arrow on every 10th waypoint
const CONE_RADIUS = 0.015;
const CONE_HEIGHT = 0.05;
const CONE_SEGMENTS = 8;
const COSTMAP_DEFAULT_OPACITY = 0.6;
/** Costmap plane height: below the path lines (z = 0.05), above the ground. */
const COSTMAP_Z = 0.02;

/** 256-entry RGB lookup: blue (0,0,255) at 0 → yellow (255,255,0) at 0.5 →
 * red (255,0,0) at 1, piecewise linear. Returns Uint8Array of length 768. */
export function buildCostmapLUT(): Uint8Array {
  const lut = new Uint8Array(256 * 3);
  for (let i = 0; i < 256; i++) {
    const t = i / 255;
    let r: number;
    let g: number;
    let b: number;
    if (t <= 0.5) {
      const k = t / 0.5; // blue → yellow: 0..1
      r = 255 * k;
      g = 255 * k;
      b = 255 * (1 - k);
    } else {
      const k = (t - 0.5) / 0.5; // yellow → red: 0..1
      r = 255;
      g = 255 * (1 - k);
      b = 0;
    }
    lut[3 * i] = Math.round(r);
    lut[3 * i + 1] = Math.round(g);
    lut[3 * i + 2] = Math.round(b);
  }
  return lut;
}

/** The 256-entry RGB LUT, computed once at module load (768 bytes). */
const COSTMAP_LUT = buildCostmapLUT();

/** Pure helper: bounding box of flat xyz points → { center, radius }.
 * radius = half-diagonal of the box (covers all points). */
export function computePathBounds(points: number[]): {
  center: { x: number; y: number; z: number };
  radius: number;
} {
  if (points.length % 3 !== 0 || points.length === 0)
    return { center: { x: 0, y: 0, z: 0 }, radius: 0 };
  let minX = Infinity;
  let minY = Infinity;
  let minZ = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  let maxZ = -Infinity;
  for (let i = 0; i < points.length; i += 3) {
    const x = points[i];
    const y = points[i + 1];
    const z = points[i + 2];
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (z < minZ) minZ = z;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
    if (z > maxZ) maxZ = z;
  }
  return {
    center: {
      x: (minX + maxX) / 2,
      y: (minY + maxY) / 2,
      z: (minZ + maxZ) / 2,
    },
    radius: Math.hypot(maxX - minX, maxY - minY, maxZ - minZ) / 2,
  };
}

/** Replay staleness: no fresh data at the current timeline position.
 * 100 ms = 2× the replay frame window (±50 ms) — tuned for the 30 Hz demo;
 * a real planner publishing below ~10 Hz would need a larger threshold. */
export function computeStaleness(
  seekTs: number | null,
  batchTs: number,
  thresholdNs = 100_000_000,
): boolean {
  if (seekTs === null) return false;
  return seekTs - batchTs > thresholdNs;
}

export interface PathInfo {
  key: string;
  nodeId: string;
  outputId: string;
  pointCount: number;
  length: number;
  colorHex: number;
  kind: 'primary' | 'alternative';
  visible: boolean;
  lastBatchTs: number;
  stale: boolean;
}

export interface CostmapSnapshot {
  visible: boolean;
  opacity: number;
  width: number;
  height: number;
  resolution: number;
  lastBatchTs: number;
}

export interface ToolSnapshot {
  paths: PathInfo[];
  target: { x: number; y: number; z: number } | null;
  lastSeekTs: number | null;
  /** null until the first valid costmap/esdf batch. */
  costmap: CostmapSnapshot | null;
}

interface PathState {
  key: string;
  nodeId: string;
  outputId: string;
  kind: 'primary' | 'alternative';
  colorHex: number;
  /** Last parsed flat xyz triplets, kept for camera framing (M12 D4). */
  points: number[];
  group: Group;
  line: Line2;
  lineGeometry: LineGeometry;
  lineMaterial: LineMaterial;
  startMarker: Mesh | null;
  endMarker: Mesh | null;
  arrowGeometry: ConeGeometry | null;
  arrowMaterial: MeshBasicMaterial | null;
  arrows: InstancedMesh | null;
  arrowCapacity: number;
  pointCount: number;
  length: number;
  lastBatchTs: number;
}

// Scratch objects for the per-arrow matrix math (allocated once).
const _pos = new Vector3();
const _dir = new Vector3();
const _quat = new Quaternion();
const _matrix = new Matrix4();
const _scale = new Vector3(1, 1, 1);
const _up = new Vector3(0, 1, 0);

export class DvizPathTool implements ViewportTool {
  readonly id = 'dviz-path';
  readonly displayName = 'dviz Path Visualization';
  readonly category = 'planning' as const;
  readonly description =
    'Renders planner waypoints/trajectory/target data as 3D paths and costmap/esdf grids as a ground plane.';
  readonly subscribePorts = [
    { nodeIdPattern: /.*/, outputIdPattern: /^(waypoints|path)$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^(target_point|target|goal)$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^(costmap|esdf)$/i },
  ];
  panelComponent?: Component;

  private context: ToolContext | null = null;
  private group: Group | null = null;
  private targetMarker: Mesh | null = null;
  private targetMarkerGeometry: SphereGeometry | null = null;
  private targetMarkerMaterial: MeshBasicMaterial | null = null;

  private costmapMesh: Mesh | null = null;
  private costmapTexture: DataTexture | null = null;
  private costmapMaterial: MeshBasicMaterial | null = null;
  private costmapGeometry: PlaneGeometry | null = null;
  private costmapWidth = 0;
  private costmapHeight = 0;
  private costmapResolution = 0;
  private costmapVisible = true;
  private costmapOpacity = COSTMAP_DEFAULT_OPACITY;
  private costmapLastBatchTs = 0;

  /** Path identity keyed by `${nodeId}/${outputId}`, insertion order = arrival. */
  private readonly paths = new Map<string, PathState>();
  private readonly pathKeysByNode = new Map<string, string[]>();
  private readonly nodeColors = new Map<string, number>();
  private target: { x: number; y: number; z: number } | null = null;
  private lastSeekTs: number | null = null;
  private readonly listeners = new Set<() => void>();

  onAttach(context: ToolContext) {
    if (this.context) return; // already attached: no-op

    this.group = new Group();
    this.group.name = 'dviz-path';

    this.targetMarkerGeometry = new SphereGeometry(MARKER_RADIUS, 8, 8);
    this.targetMarkerMaterial = new MeshBasicMaterial({ color: TARGET_COLOR });
    this.targetMarker = new Mesh(this.targetMarkerGeometry, this.targetMarkerMaterial);
    this.targetMarker.name = 'dviz-path-target';
    this.targetMarker.visible = false;
    this.group.add(this.targetMarker);

    context.scene.add(this.group);
    this.context = context; // only after the scene add succeeds
    context.requestRender();
  }

  onBatch(batch: ToolBatch, _tf?: TfTree) {
    if (!this.context || !this.group) return; // batches before attach: no state

    const outputId = batch.outputId.toLowerCase();
    if (outputId === 'waypoints' || outputId === 'path') {
      this.handlePath(batch, outputId, parseWaypoints(batch.payload));
    } else if (outputId === 'trajectory') {
      this.handlePath(batch, outputId, parseTrajectory(batch.payload));
    } else if (outputId === 'target_point' || outputId === 'target' || outputId === 'goal') {
      this.handleTarget(batch);
    } else if (outputId === 'costmap' || outputId === 'esdf') {
      this.handleCostmap(batch);
    }
  }

  onTimelineSeek(timestampNs: number) {
    this.lastSeekTs = timestampNs;
    // Paths stay at last-known data: no scene changes, no requestRender.
    this.notify();
  }

  onDetach() {
    if (!this.context || !this.group) return;
    this.context.scene.remove(this.group);
    this.context.requestRender();

    for (const path of this.paths.values()) {
      path.lineGeometry.dispose();
      path.lineMaterial.dispose();
      path.startMarker?.geometry.dispose();
      (path.startMarker?.material as Material | undefined)?.dispose();
      path.endMarker?.geometry.dispose();
      (path.endMarker?.material as Material | undefined)?.dispose();
      path.arrowGeometry?.dispose();
      path.arrowMaterial?.dispose();
    }
    this.targetMarkerGeometry?.dispose();
    this.targetMarkerMaterial?.dispose();

    this.disposeCostmapResources();
    this.costmapWidth = 0;
    this.costmapHeight = 0;
    this.costmapResolution = 0;
    this.costmapVisible = true;
    this.costmapOpacity = COSTMAP_DEFAULT_OPACITY;
    this.costmapLastBatchTs = 0;

    this.paths.clear();
    this.pathKeysByNode.clear();
    this.nodeColors.clear();
    this.target = null;
    this.lastSeekTs = null;
    this.group = null;
    this.targetMarker = null;
    this.targetMarkerGeometry = null;
    this.targetMarkerMaterial = null;
    this.context = null;
    this.notify();
    this.listeners.clear(); // no stale subscribers across attach cycles
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  getSnapshot(): ToolSnapshot {
    return {
      paths: [...this.paths.values()].map((path) => ({
        key: path.key,
        nodeId: path.nodeId,
        outputId: path.outputId,
        pointCount: path.pointCount,
        length: path.length,
        colorHex: path.colorHex,
        kind: path.kind,
        visible: path.group.visible,
        lastBatchTs: path.lastBatchTs,
        stale: computeStaleness(this.lastSeekTs, path.lastBatchTs),
      })),
      target: this.target ? { ...this.target } : null,
      lastSeekTs: this.lastSeekTs,
      costmap: this.costmapMesh
        ? {
            visible: this.costmapVisible,
            opacity: this.costmapOpacity,
            width: this.costmapWidth,
            height: this.costmapHeight,
            resolution: this.costmapResolution,
            lastBatchTs: this.costmapLastBatchTs,
          }
        : null,
    };
  }

  /** D4 panel: show/hide the costmap plane (sticky before the first batch). */
  setCostmapVisible(visible: boolean) {
    this.costmapVisible = visible;
    if (this.costmapMesh) this.costmapMesh.visible = visible;
    this.context?.requestRender();
    this.notify();
  }

  /** D4 panel: plane opacity, clamped to [0, 1]. */
  setCostmapOpacity(opacity: number) {
    this.costmapOpacity = Math.min(1, Math.max(0, opacity));
    if (this.costmapMaterial) this.costmapMaterial.opacity = this.costmapOpacity;
    this.context?.requestRender();
    this.notify();
  }

  setPathVisible(key: string, visible: boolean) {
    const path = this.paths.get(key);
    if (!path) return;
    path.group.visible = visible;
    this.context?.requestRender();
    this.notify();
  }

  /** M12 D4: frame the camera on the path's current points. Uses
   * context.focusOn (OrbitControls-target synced) when the viewer provides
   * it; otherwise falls back to a bare camera position + lookAt. No-op when
   * the path is unknown or has no points. */
  snapCameraToPath(key: string) {
    const path = this.paths.get(key);
    if (!path || path.points.length === 0 || !this.context) return;
    const { center, radius } = computePathBounds(path.points);
    if (this.context.focusOn) {
      this.context.focusOn(center, radius);
      return;
    }
    // A single-point path yields radius 0; a 0 offset would leave the camera
    // at the point and lookAt would build a NaN matrix. Frame at unit scale.
    const safeRadius = radius > 0 ? radius : 1;
    this.context.camera.position.set(
      center.x + safeRadius * 1.75,
      center.y - safeRadius * 2.15,
      center.z + safeRadius * 1.1,
    );
    this.context.camera.lookAt(center.x, center.y, center.z);
    this.context.requestRender();
  }

  // -------------------------------------------------------------------------
  // Internals

  private handleTarget(batch: ToolBatch) {
    if (!this.targetMarker) return;
    const target = parseTarget(batch.payload);
    if (target === null) return; // invalid: keep the last known target
    this.target = target;
    this.targetMarker.position.set(target.x, target.y, target.z);
    this.targetMarker.visible = true;
    this.context?.requestRender();
    this.notify();
  }

  /** Costmap/esdf batch: render the grid as a textured ground plane. Invalid
   * payloads keep the last known costmap (no throw, no scene change). */
  private handleCostmap(batch: ToolBatch) {
    if (!this.group) return;
    const costmap = parseCostmap(batch.payload);
    if (costmap === null) return;
    const { width, height, resolution, values } = costmap;
    this.costmapWidth = width;
    this.costmapHeight = height;
    this.costmapResolution = resolution;
    this.costmapLastBatchTs = batch.timestampNs;

    // Cell color = LUT[clamp(value, 0, 1)]; RGBA interleaved per cell. The
    // texture defaults to RGBAFormat (4 bytes/texel), so every texel carries
    // an alpha byte of 255 — an RGB-only buffer would stride each texel by 4
    // bytes and garble the image (1-byte shift per texel + OOB reads).
    const rgba = new Uint8Array(width * height * 4);
    for (let i = 0; i < values.length; i++) {
      const t = Math.min(1, Math.max(0, values[i]));
      const k = Math.round(t * 255) * 3;
      const o = i * 4;
      rgba[o] = COSTMAP_LUT[k];
      rgba[o + 1] = COSTMAP_LUT[k + 1];
      rgba[o + 2] = COSTMAP_LUT[k + 2];
      rgba[o + 3] = 255; // opaque
    }

    if (
      this.costmapTexture &&
      this.costmapTexture.image.width === width &&
      this.costmapTexture.image.height === height
    ) {
      // Same dimensions: reuse texture and geometry, refresh the data only.
      // (DataTexture always carries the array we constructed it with.)
      this.costmapTexture.image.data!.set(rgba);
      this.costmapTexture.needsUpdate = true;
    } else {
      // Dimensions changed: drop the old plane and rebuild from scratch.
      this.disposeCostmapResources();
      this.createCostmapMesh(width, height, resolution, rgba);
    }
    this.context?.requestRender();
    this.notify();
  }

  private createCostmapMesh(width: number, height: number, resolution: number, rgba: Uint8Array) {
    // Default RGBAFormat matches the 4-bytes-per-texel buffer built per batch.
    const texture = new DataTexture(rgba, width, height);
    texture.magFilter = NearestFilter; // cell look: no blending between cells
    texture.minFilter = NearestFilter;
    texture.colorSpace = SRGBColorSpace;
    texture.needsUpdate = true;

    // PlaneGeometry already lies flat in the XY plane (normal +Z): the
    // costmap sits on the XY ground plane, below the z = 0.05 path lines.
    const geometry = new PlaneGeometry(width * resolution, height * resolution);

    const material = new MeshBasicMaterial({
      map: texture,
      transparent: true,
      opacity: this.costmapOpacity,
      depthWrite: false,
    });

    const mesh = new Mesh(geometry, material);
    mesh.name = 'costmap';
    mesh.position.z = COSTMAP_Z; // below the path lines (0.05), above ground (0)
    mesh.visible = this.costmapVisible;

    this.costmapTexture = texture;
    this.costmapGeometry = geometry;
    this.costmapMaterial = material;
    this.costmapMesh = mesh;
    this.group!.add(mesh);
  }

  /** Drop the costmap plane from the group and dispose its GPU resources. */
  private disposeCostmapResources() {
    if (this.costmapMesh && this.group) this.group.remove(this.costmapMesh);
    this.costmapTexture?.dispose();
    this.costmapMaterial?.dispose();
    this.costmapGeometry?.dispose();
    this.costmapMesh = null;
    this.costmapTexture = null;
    this.costmapMaterial = null;
    this.costmapGeometry = null;
  }

  private handlePath(batch: ToolBatch, outputId: string, points: number[]) {
    // Empty or non-triplet parses keep the last known path (parsers today
    // always emit multiples of 3; the guard defends against future ones).
    if (points.length === 0 || points.length % 3 !== 0 || !this.group) return;
    const key = `${batch.nodeId}/${outputId}`;
    let path = this.paths.get(key);
    if (!path) {
      path = this.createPath(batch.nodeId, outputId, key);
      this.paths.set(key, path);
      this.group.add(path.group);
    }
    this.updatePath(path, points, batch.timestampNs);
    this.context?.requestRender();
    this.notify();
  }

  private createPath(nodeId: string, outputId: string, key: string): PathState {
    const nodePaths = this.pathKeysByNode.get(nodeId) ?? [];
    const kind: 'primary' | 'alternative' = nodePaths.length === 0 ? 'primary' : 'alternative';

    if (nodePaths.length === 0) {
      this.pathKeysByNode.set(nodeId, []);
      // First appearance order across nodes; cycle when the palette runs out.
      this.nodeColors.set(nodeId, NODE_COLORS[this.nodeColors.size % NODE_COLORS.length]);
    }
    this.pathKeysByNode.get(nodeId)!.push(key);
    const nodeColor = this.nodeColors.get(nodeId)!;
    // Snapshot/panel color = the rendered line color.
    const colorHex = kind === 'primary' ? nodeColor : ALTERNATIVE_COLOR;

    const group = new Group();
    group.name = `path:${key}`;

    const lineGeometry = new LineGeometry();
    const lineMaterial = new LineMaterial(
      kind === 'primary'
        ? { color: colorHex, linewidth: LINE_WIDTH }
        : {
            color: ALTERNATIVE_COLOR,
            linewidth: LINE_WIDTH,
            dashed: true,
            transparent: true,
            opacity: ALTERNATIVE_OPACITY,
          },
    );
    const line = new Line2(lineGeometry, lineMaterial);
    group.add(line);

    let startMarker: Mesh | null = null;
    let endMarker: Mesh | null = null;
    let arrowGeometry: ConeGeometry | null = null;
    let arrowMaterial: MeshBasicMaterial | null = null;
    let arrows: InstancedMesh | null = null;

    if (kind === 'primary') {
      startMarker = new Mesh(
        new SphereGeometry(MARKER_RADIUS, 8, 8),
        new MeshBasicMaterial({ color: START_COLOR }),
      );
      endMarker = new Mesh(
        new SphereGeometry(MARKER_RADIUS, 8, 8),
        new MeshBasicMaterial({ color: END_COLOR }),
      );
      arrowGeometry = new ConeGeometry(CONE_RADIUS, CONE_HEIGHT, CONE_SEGMENTS);
      arrowMaterial = new MeshBasicMaterial({ color: colorHex });
      arrows = new InstancedMesh(arrowGeometry, arrowMaterial, 0);
      group.add(startMarker, endMarker, arrows);
    }

    return {
      key,
      nodeId,
      outputId,
      kind,
      colorHex,
      points: [],
      group,
      line,
      lineGeometry,
      lineMaterial,
      startMarker,
      endMarker,
      arrowGeometry,
      arrowMaterial,
      arrows,
      arrowCapacity: 0,
      pointCount: 0,
      length: 0,
      lastBatchTs: 0,
    };
  }

  private updatePath(path: PathState, points: number[], timestampNs: number) {
    path.points = points;
    path.lineGeometry.setPositions(points);
    if (path.lineMaterial.dashed) {
      // Line distances drive the dash rendering (only dashes need them;
      // solid paths skip the per-batch distance attribute allocation).
      path.line.computeLineDistances();
    }

    if (path.startMarker && path.endMarker) {
      const last = points.length - 3;
      path.startMarker.position.set(points[0], points[1], points[2]);
      path.endMarker.position.set(points[last], points[last + 1], points[last + 2]);
    }
    if (path.arrows) this.syncArrows(path, points);

    path.pointCount = Math.floor(points.length / 3);
    path.length = computePathLength(points);
    path.lastBatchTs = timestampNs;
  }

  /** Rebuild/sync the direction-arrow instance matrices; count 0 when idle. */
  private syncArrows(path: PathState, points: number[]) {
    const pointCount = Math.floor(points.length / 3);
    let mesh = path.arrows;
    if (!mesh) return;

    let count = 0;
    for (let i = ARROW_EVERY; i < pointCount - 1; i += ARROW_EVERY) count += 1;

    if (count > path.arrowCapacity) {
      // Amortized growth: double the capacity instead of sizing per batch.
      const capacity = Math.max(count, path.arrowCapacity * 2, 1);
      const next = new InstancedMesh(path.arrowGeometry!, path.arrowMaterial!, capacity);
      // Matrices are rewritten every batch: keep the buffer on the dynamic path.
      next.instanceMatrix.setUsage(DynamicDrawUsage);
      path.group.remove(mesh);
      path.arrows = next;
      path.arrowCapacity = capacity;
      mesh = next;
      path.group.add(mesh);
    }
    mesh.count = count;
    if (count === 0) {
      mesh.instanceMatrix.needsUpdate = true;
      return;
    }

    let k = 0;
    for (let i = ARROW_EVERY; i < pointCount - 1; i += ARROW_EVERY) {
      _pos.set(points[3 * i], points[3 * i + 1], points[3 * i + 2]);
      _dir.set(
        points[3 * (i + 1)] - points[3 * i],
        points[3 * (i + 1) + 1] - points[3 * i + 1],
        points[3 * (i + 1) + 2] - points[3 * i + 2],
      );
      if (_dir.lengthSq() === 0) _dir.copy(_up); // degenerate segment: keep cone up
      _dir.normalize();
      _quat.setFromUnitVectors(_up, _dir);
      _matrix.compose(_pos, _quat, _scale);
      mesh.setMatrixAt(k, _matrix);
      k += 1;
    }
    mesh.instanceMatrix.needsUpdate = true;
  }

  private notify() {
    for (const listener of this.listeners) listener();
  }
}
