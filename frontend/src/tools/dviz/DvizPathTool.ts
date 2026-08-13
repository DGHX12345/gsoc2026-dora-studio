// DvizPathTool — M12 D2 core renderer for dviz planner path data.
//
// Renders waypoints/path/trajectory batches as 3D wide lines (Line2) with
// start/end markers and direction arrows on each node's primary path, plus a
// target marker. Path data arrives in the world frame (dviz world topics),
// so the tf argument is ignored entirely (R11) — no TF transforms.
//
// The tool is the single source of truth for the D4 control panel:
// subscribe()/getSnapshot()/setPathVisible() expose path/target state.

import {
  ConeGeometry,
  DynamicDrawUsage,
  Group,
  InstancedMesh,
  Material,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  Quaternion,
  SphereGeometry,
  Vector3,
} from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import type { Component } from 'vue';

import type { TfTree } from '../tf';
import type { ToolBatch, ToolContext, ViewportTool } from '../types';
import { computePathLength, parseTarget, parseTrajectory, parseWaypoints } from './parse';

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

/** Replay staleness: no fresh data at the current timeline position. */
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

export interface ToolSnapshot {
  paths: PathInfo[];
  target: { x: number; y: number; z: number } | null;
  lastSeekTs: number | null;
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
  readonly description = 'Renders planner waypoints/trajectory/target data as 3D paths.';
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
    }
    // costmap | esdf: ignored for now — costmap rendering is a later task.
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
    };
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
