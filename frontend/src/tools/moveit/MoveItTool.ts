// MoveItTool — M13 core tool: consumes dora-moveit2 planner/executor/scene
// streams and renders them in the viewport.
//
// D4: a tool-owned URDF robot model (Option B) supplies FK for free — the
// end-effector path (gradient Line2), ghost poses (semi-transparent
// clones) and the current pose (driven by joint_positions, then
// joint_commands). Without a model, the D2 parallel-coordinates
// joint-space chart remains the fallback. D4.1: previewPose() drives the
// model from the LeRobot attribution preview (B601 gripper degrees-linear
// mapping). Scene rendering lands in D5; the control panel in D6.
//
// Trajectory batches arrive as the object envelope { waypoints: [[q...]] }
// (replay demo form) or flat arrays (M15-B live) reshaped with the joint
// count from the robot config (D3). Real .drec Arrow IPC bytes stay
// unsupported: the parser returns null and the last known data survives.

import {
  BufferGeometry,
  Color,
  Group,
  Line,
  LineBasicMaterial,
  Material,
  Vector3,
} from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import type { Component } from 'vue';

import { BACKEND_BASE_URL } from '../../api';
import { computeStaleness } from '../dviz/DvizPathTool';
import type { TfTree } from '../tf';
import type { ToolBatch, ToolContext, ViewportTool } from '../types';
import { getRobotConfig, jointLabelsFor } from './joint-config';
import {
  parseExecutionStatus,
  parseJointCommands,
  parseJointPositions,
  parsePlanStatus,
  parseSceneUpdate,
  parseTrajectory,
} from './parse';
import type { ExecutionStatus, PlanningScene, PlanStatus } from './types';
import { loadUrdfRobot } from './urdf/meshes';
import type { RobotModel } from './urdf/robot';

/** Parallel-coordinates chart placement. The viewport camera frames the
 * robot from the right-front (position x > 0, y < 0), so only the scene's
 * LEFT side (negative x) is reliably in view — the initial right-side
 * placement was invisible (regression-tested below). Verified against the
 * default and frameCameraToModel (r = 0.3 / 0.5) frusta: all vertices
 * inside. */
const CHART_CENTER = new Vector3(-0.5, 0.15, 0.2);
const AXIS_SPACING = 0.07;
const AXIS_HEIGHT = 0.4;
const AXIS_COLOR = 0x9ca3af;
const GRADIENT_START = new Color(0x3b82f6); // blue — trajectory start
const GRADIENT_END = new Color(0xef4444); // red — trajectory end
/** Hard cap on chart polylines: a huge live trajectory must not flood the
 * scene; only the first N waypoints render. */
const MAX_CHART_POLYLINES = 64;

const EE_LINE_WIDTH = 2;
/** Ghost pose count until the D6 slider lands. */
const DEFAULT_GHOST_COUNT = 5;
const GHOST_OPACITY = 0.35;

/** B601 gripper full travel in radians (56.8°, the b601_pilot_v1 dataset
 * range — student decision 2026-08-14). previewPose converts the gripper
 * value linearly from this range to the URDF prismatic limit. */
const GRIPPER_FULL_RANGE_RAD = (56.8 * Math.PI) / 180;

/** Robots with a locally available URDF, served by the backend /models
 * static route (models/ is gitignored, Nano precedent). Paths resolve
 * against the backend origin at load time — the Vite dev server has no
 * /models proxy (Nano precedent). */
export const AVAILABLE_MODELS = [
  {
    robotId: 'b601',
    label: 'reBot B601 (Seeed)',
    urdfPath: '/models/b601/reBot_B601_DM_with_gripper.urdf',
    meshBasePath: '/models/b601/',
  },
] as const;

export type ModelLoader = (robotId: string) => Promise<RobotModel>;

function defaultModelLoader(robotId: string): Promise<RobotModel> {
  const entry = AVAILABLE_MODELS.find((model) => model.robotId === robotId);
  if (!entry) return Promise.reject(new Error(`no local URDF for robot "${robotId}"`));
  const urdfUrl = `${BACKEND_BASE_URL}${entry.urdfPath}`;
  const meshBaseUrl = `${BACKEND_BASE_URL}${entry.meshBasePath}`;
  return fetch(urdfUrl)
    .then((response) => {
      if (!response.ok) throw new Error(`URDF fetch failed: ${response.status}`);
      return response.text();
    })
    .then((urdfText) =>
      loadUrdfRobot(urdfText, async (relativePath) => {
        const response = await fetch(`${meshBaseUrl}${relativePath}`);
        if (!response.ok) throw new Error(`mesh fetch failed: ${response.status}`);
        return response.arrayBuffer();
      }),
    );
}

export type RobotState = 'loading' | 'loaded' | 'unavailable';

export interface MoveItSnapshot {
  robotId: string | null;
  robotState: RobotState | null;
  modelName: string | null;
  jointLabels: string[];
  numJoints: number | null;
  endEffector: { x: number; y: number; z: number } | null;
  trajectory: {
    nodeId: string;
    waypointCount: number;
    lastBatchTs: number;
    stale: boolean;
  } | null;
  planStatus: { status: PlanStatus; lastBatchTs: number } | null;
  execution: { status: ExecutionStatus; lastBatchTs: number; stale: boolean } | null;
  jointCommands: { values: number[]; lastBatchTs: number } | null;
  jointPositions: { values: number[]; lastBatchTs: number } | null;
  scene: { data: PlanningScene; lastBatchTs: number } | null;
  lastSeekTs: number | null;
}

export class MoveItTool implements ViewportTool {
  readonly id = 'moveit-bridge';
  readonly displayName = 'MoveIt Bridge';
  readonly category = 'planning' as const;
  readonly description =
    'Renders dora-moveit2 trajectories, planning scene objects and execution status in the viewport.';
  readonly subscribePorts = [
    { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^joint_positions$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^joint_commands$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^scene_update$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^execution_status$/i },
    { nodeIdPattern: /.*/, outputIdPattern: /^plan_status$/i },
  ];
  panelComponent?: Component;

  private readonly modelLoader: ModelLoader;

  private context: ToolContext | null = null;
  private group: Group | null = null;
  /** Monotonic attach epoch: discards robot loads that resolve after a
   * detach/reattach cycle. */
  private attachEpoch = 0;

  private trajectory: { nodeId: string; waypoints: number[][]; lastBatchTs: number } | null = null;
  /** Content signature of the rendered trajectory — identical re-publishes
   * skip the FK rebuild (see handleTrajectory). */
  private trajectorySignature: string | null = null;
  private planStatus: { status: PlanStatus; lastBatchTs: number } | null = null;
  private execution: { status: ExecutionStatus; lastBatchTs: number } | null = null;
  private jointCommands: { values: number[]; lastBatchTs: number } | null = null;
  private jointPositions: { values: number[]; lastBatchTs: number } | null = null;
  private scene: { data: PlanningScene; lastBatchTs: number } | null = null;
  private robotId: string | null = null;
  private numJoints: number | null = null;
  private lastSeekTs: number | null = null;
  private readonly listeners = new Set<() => void>();

  private robotState: RobotState | null = null;
  private robotModel: RobotModel | null = null;
  private robotGroup: Group | null = null;
  private ghostsGroup: Group | null = null;
  private eePathGroup: Group | null = null;
  private eePathGeometry: LineGeometry | null = null;
  private eePathMaterial: LineMaterial | null = null;

  // Parallel-coordinates chart resources (D2 fallback visualization)
  private chartGroup: Group | null = null;
  private chartPolylines: Line[] = [];
  private chartAxes: Line[] = [];
  private axisMaterial: LineBasicMaterial | null = null;
  private polylineMaterials: LineBasicMaterial[] = [];

  constructor(modelLoader: ModelLoader = defaultModelLoader) {
    this.modelLoader = modelLoader;
  }

  onAttach(context: ToolContext) {
    if (this.context) return; // already attached: no-op

    this.group = new Group();
    this.group.name = 'moveit-bridge';
    context.scene.add(this.group);
    this.context = context; // only after the scene add succeeds
    this.attachEpoch += 1;
    context.requestRender();

    // Auto-load the single locally available robot; D6 adds the selector.
    const first = AVAILABLE_MODELS[0];
    if (first) void this.loadRobot(first.robotId);
  }

  onBatch(batch: ToolBatch, _tf?: TfTree) {
    if (!this.context || !this.group) return; // batches before attach: no state

    const outputId = batch.outputId.toLowerCase();
    if (outputId === 'trajectory') {
      this.handleTrajectory(batch);
    } else if (outputId === 'plan_status') {
      const status = parsePlanStatus(batch.payload);
      if (status) {
        this.planStatus = { status, lastBatchTs: batch.timestampNs };
        this.notify();
      }
    } else if (outputId === 'execution_status') {
      const status = parseExecutionStatus(batch.payload);
      if (status) {
        this.execution = { status, lastBatchTs: batch.timestampNs };
        this.notify();
      }
    } else if (outputId === 'joint_commands') {
      const values = parseJointCommands(batch.payload);
      if (values) {
        this.jointCommands = { values, lastBatchTs: batch.timestampNs };
        this.applyCurrentPose();
        this.notify();
      }
    } else if (outputId === 'joint_positions') {
      const values = parseJointPositions(batch.payload);
      if (values) {
        this.jointPositions = { values, lastBatchTs: batch.timestampNs };
        this.applyCurrentPose();
        this.notify();
      }
    } else if (outputId === 'scene_update') {
      const scene = parseSceneUpdate(batch.payload);
      if (scene) {
        this.scene = { data: scene, lastBatchTs: batch.timestampNs };
        this.notify();
      }
    }
  }

  onTimelineSeek(timestampNs: number) {
    this.lastSeekTs = timestampNs;
    // Data stays at last-known values: no scene changes, no requestRender.
    this.notify();
  }

  onDetach() {
    if (!this.context || !this.group) return;
    this.attachEpoch += 1; // discard in-flight robot loads
    this.context.scene.remove(this.group);
    this.context.requestRender();
    this.disposeChart();
    this.disposeRobot();

    this.trajectory = null;
    this.trajectorySignature = null;
    this.planStatus = null;
    this.execution = null;
    this.jointCommands = null;
    this.jointPositions = null;
    this.scene = null;
    this.robotId = null;
    this.numJoints = null;
    this.lastSeekTs = null;
    this.group = null;
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

  getSnapshot(): MoveItSnapshot {
    let endEffector: { x: number; y: number; z: number } | null = null;
    if (this.robotModel) {
      this.robotModel.updateWorld();
      const position = this.robotModel.getEndEffectorPosition();
      endEffector = { x: position.x, y: position.y, z: position.z };
    }
    return {
      robotId: this.robotId,
      robotState: this.robotState,
      modelName: this.robotModel ? this.robotModel.root.name : null,
      jointLabels: jointLabelsFor(this.robotId, this.numJoints ?? 0),
      numJoints: this.numJoints,
      endEffector,
      trajectory: this.trajectory
        ? {
            nodeId: this.trajectory.nodeId,
            waypointCount: this.trajectory.waypoints.length,
            lastBatchTs: this.trajectory.lastBatchTs,
            stale: computeStaleness(this.lastSeekTs, this.trajectory.lastBatchTs),
          }
        : null,
      planStatus: this.planStatus ? { ...this.planStatus } : null,
      execution: this.execution
        ? {
            ...this.execution,
            stale: computeStaleness(this.lastSeekTs, this.execution.lastBatchTs),
          }
        : null,
      jointCommands: this.jointCommands ? { ...this.jointCommands } : null,
      jointPositions: this.jointPositions ? { ...this.jointPositions } : null,
      scene: this.scene ? { ...this.scene } : null,
      lastSeekTs: this.lastSeekTs,
    };
  }

  /** D6 panel: pick the robot — supplies joint labels and the joint count
   * for flat trajectory reshaping (D3 config), and loads the matching
   * URDF when a local model exists. */
  setRobot(robotId: string | null) {
    this.robotId = robotId;
    this.notify();
    if (robotId && this.loadedModelRobotId() !== robotId) {
      void this.loadRobot(robotId);
    }
  }

  getRobotModel(): RobotModel | null {
    return this.robotModel;
  }

  /** D4.1: drive the loaded robot from the LeRobot attribution preview.
   * Arm values are radians; the trailing gripper value (when the vector
   * is one shorter than the motion joints, i.e. one value for two
   * fingers) converts linearly from the 56.8° dataset range to the URDF
   * prismatic limit, clamped. No-op without a loaded model. */
  previewPose(values: number[]) {
    if (!this.robotModel) return;
    const motion = this.motionJoints();
    const count = Math.min(values.length, motion.length);
    for (let i = 0; i < count; i++) {
      const joint = this.robotModel.joints.get(motion[i]);
      if (!joint || joint.type === 'fixed') continue;
      if (joint.type === 'prismatic') {
        const clamped = Math.min(1, Math.max(0, values[i] / GRIPPER_FULL_RANGE_RAD));
        const span = joint.limit ? joint.limit.upper - joint.limit.lower : GRIPPER_FULL_RANGE_RAD;
        const lower = joint.limit ? joint.limit.lower : 0;
        this.robotModel.setJointValue(motion[i], lower + clamped * span);
      } else {
        this.robotModel.setJointValue(motion[i], values[i]);
      }
    }
    // One value for two finger joints (B601): the last value also drives
    // the second finger.
    if (values.length === motion.length - 1 && motion.length >= 2) {
      const last = this.robotModel.joints.get(motion[motion.length - 1]);
      if (last && last.type === 'prismatic' && last.limit) {
        const clamped = Math.min(1, Math.max(0, values[values.length - 1] / GRIPPER_FULL_RANGE_RAD));
        this.robotModel.setJointValue(
          motion[motion.length - 1],
          last.limit.lower + clamped * (last.limit.upper - last.limit.lower),
        );
      }
    }
    this.robotModel.updateWorld();
    this.context?.requestRender();
    this.notify();
  }

  // -------------------------------------------------------------------------
  // Internals

  private loadedModelRobotId(): string | null {
    return this.robotState === 'loaded' && this.robotId ? this.robotId : null;
  }

  private async loadRobot(robotId: string) {
    const epoch = this.attachEpoch;
    this.robotId = robotId;
    this.robotState = 'loading';
    this.notify();
    try {
      const model = await this.modelLoader(robotId);
      if (epoch !== this.attachEpoch || !this.group) return; // detached meanwhile
      this.disposeRobot(); // drop any previous model before mounting
      this.robotModel = model;
      this.robotState = 'loaded';
      this.mountRobot();
      this.renderFkArtifacts();
      this.context?.requestRender();
      this.notify();
    } catch (error) {
      if (epoch !== this.attachEpoch) return;
      console.error(`moveit robot "${robotId}" load failed:`, error);
      this.robotState = 'unavailable';
      this.notify();
    }
  }

  /** Add the model root under the tool group; the chart hides when a
   * model is available. loadRobot() disposes any previous model first. */
  private mountRobot() {
    if (!this.group || !this.robotModel) return;
    this.robotGroup = new Group();
    this.robotGroup.name = 'moveit-robot';
    this.robotGroup.add(this.robotModel.root);
    this.group.add(this.robotGroup);

    this.ghostsGroup = new Group();
    this.ghostsGroup.name = 'moveit-ghosts';
    this.group.add(this.ghostsGroup);

    if (this.chartGroup) this.chartGroup.visible = false;
  }

  private resolveNumJoints(): number | null {
    if (this.robotId) {
      const config = getRobotConfig(this.robotId);
      if (config) return config.jointNames.length;
    }
    return this.numJoints;
  }

  /** Motion joints in document order (fixed joints excluded). */
  private motionJoints(): string[] {
    if (!this.robotModel) return [];
    return this.robotModel.jointOrder.filter(
      (name) => this.robotModel!.joints.get(name)!.type !== 'fixed',
    );
  }

  /** Map a trajectory waypoint (index → value) onto the model's motion
   * joints by index. */
  private applyWaypoint(waypoint: number[]) {
    if (!this.robotModel) return;
    const motion = this.motionJoints();
    for (let i = 0; i < Math.min(waypoint.length, motion.length); i++) {
      this.robotModel.setJointValue(motion[i], waypoint[i]);
    }
    this.robotModel.updateWorld();
  }

  private handleTrajectory(batch: ToolBatch) {
    const waypoints = parseTrajectory(batch.payload, this.resolveNumJoints());
    if (waypoints === null) return; // invalid/unsupported: keep last known plan
    this.trajectory = { nodeId: batch.nodeId, waypoints, lastBatchTs: batch.timestampNs };
    this.numJoints = waypoints[0]?.length ?? this.numJoints;
    const signature = waypoints.map((row) => row.join(',')).join(';');
    if (this.robotState === 'loaded') {
      // The demo re-publishes the same plan every frame — rebuilding ghost
      // clones and the EE path per frame would churn the scene for nothing.
      if (signature !== this.trajectorySignature) {
        this.trajectorySignature = signature;
        this.renderFkArtifacts();
      }
    } else {
      this.renderChart(waypoints);
    }
    this.context?.requestRender();
    this.notify();
  }

  /** FK rendering: gradient end-effector path + ghost poses, then the
   * current joint state is restored on the live model. */
  private renderFkArtifacts() {
    if (!this.robotModel || !this.trajectory) return;
    const waypoints = this.trajectory.waypoints;
    if (waypoints.length === 0) return;

    const positions: number[] = [];
    const colors: number[] = [];
    const gradient = new Color();
    for (let k = 0; k < waypoints.length; k++) {
      this.applyWaypoint(waypoints[k]);
      const p = this.robotModel.getEndEffectorPosition();
      positions.push(p.x, p.y, p.z);
      const t = waypoints.length <= 1 ? 0 : k / (waypoints.length - 1);
      gradient.lerpColors(GRADIENT_START, GRADIENT_END, t);
      colors.push(gradient.r, gradient.g, gradient.b);
    }

    this.rebuildEePath(positions, colors);
    this.rebuildGhosts(waypoints);
    this.applyCurrentPose(); // the live model returns to the current state
  }

  private rebuildEePath(positions: number[], colors: number[]) {
    if (!this.group) return;
    this.disposeEePath();
    const geometry = new LineGeometry();
    geometry.setPositions(positions);
    geometry.setColors(colors);
    const material = new LineMaterial({ linewidth: EE_LINE_WIDTH, vertexColors: true });
    const line = new Line2(geometry, material);
    line.name = 'moveit-ee-line';
    const group = new Group();
    group.name = 'moveit-ee-path';
    group.add(line);
    this.eePathGeometry = geometry;
    this.eePathMaterial = material;
    this.eePathGroup = group;
    this.group.add(group);
  }

  private rebuildGhosts(waypoints: number[][]) {
    if (!this.robotModel || !this.ghostsGroup) return;
    this.clearGhosts();
    const count = DEFAULT_GHOST_COUNT;
    for (let k = 0; k < count; k++) {
      const index = count <= 1 ? 0 : Math.round((k / (count - 1)) * (waypoints.length - 1));
      const ghost = this.robotModel.clonePose(GHOST_OPACITY);
      const motion = this.motionJoints();
      const waypoint = waypoints[index];
      for (let i = 0; i < Math.min(waypoint.length, motion.length); i++) {
        ghost.traverse(() => {}); // no-op; joints set below via clone traversal
        const joint = this.robotModel!.joints.get(motion[i])!;
        const ghostPivot = ghost.getObjectByName(`joint:${motion[i]}`)!;
        applyPoseToPivot(ghostPivot, joint.pivot.position, joint.pivot.quaternion);
      }
      ghost.updateMatrixWorld(true);
      ghost.name = `ghost-${index}`;
      this.ghostsGroup!.add(ghost);
    }
  }

  private clearGhosts() {
    if (!this.ghostsGroup) return;
    for (const child of [...this.ghostsGroup.children]) {
      // Materials only: ghosts SHARE the model's BufferGeometry, and
      // disposing it would force a GPU re-upload of the whole model.
      disposeMaterialsOnly(child);
      this.ghostsGroup.remove(child);
    }
  }

  private applyCurrentPose() {
    if (!this.robotModel) return;
    const values = this.jointPositions?.values ?? this.jointCommands?.values;
    if (!values) return;
    this.applyWaypoint(values);
    this.context?.requestRender();
  }

  /** Parallel-coordinates joint-space chart — the FK fallback. One
   * vertical axis per joint (per-joint min/max scaled); one blue→red
   * polyline per waypoint across the axes. */
  private renderChart(waypoints: number[][]) {
    if (!this.group) return;
    const jointCount = waypoints[0]?.length ?? 0;
    if (jointCount === 0) return;

    this.disposeChart();
    const chart = new Group();
    chart.name = 'moveit-joint-chart';
    chart.visible = this.robotState !== 'loaded';

    const mins = new Array<number>(jointCount).fill(Infinity);
    const maxs = new Array<number>(jointCount).fill(-Infinity);
    for (const row of waypoints) {
      for (let j = 0; j < jointCount; j++) {
        const v = row[j] ?? 0;
        if (v < mins[j]) mins[j] = v;
        if (v > maxs[j]) maxs[j] = v;
      }
    }
    // Constant joints get a fixed span — a zero range would divide by zero.
    const span = (j: number) => (maxs[j] - mins[j] > 0 ? maxs[j] - mins[j] : 0.2);
    const valuePoint = (j: number, v: number) =>
      new Vector3(
        CHART_CENTER.x + (j - (jointCount - 1) / 2) * AXIS_SPACING,
        CHART_CENTER.y,
        CHART_CENTER.z - AXIS_HEIGHT / 2 + ((v - mins[j]) / span(j)) * AXIS_HEIGHT,
      );

    this.axisMaterial = new LineBasicMaterial({ color: AXIS_COLOR });
    for (let j = 0; j < jointCount; j++) {
      const x = CHART_CENTER.x + (j - (jointCount - 1) / 2) * AXIS_SPACING;
      const geometry = new BufferGeometry().setFromPoints([
        new Vector3(x, CHART_CENTER.y, CHART_CENTER.z - AXIS_HEIGHT / 2),
        new Vector3(x, CHART_CENTER.y, CHART_CENTER.z + AXIS_HEIGHT / 2),
      ]);
      const axis = new Line(geometry, this.axisMaterial);
      axis.name = 'chart-axis';
      chart.add(axis);
      this.chartAxes.push(axis);
    }

    const gradient = new Color();
    const rowCount = Math.min(waypoints.length, MAX_CHART_POLYLINES);
    for (let k = 0; k < rowCount; k++) {
      const t = rowCount <= 1 ? 0 : k / (rowCount - 1);
      gradient.lerpColors(GRADIENT_START, GRADIENT_END, t);
      const material = new LineBasicMaterial({ color: gradient.getHex() });
      const geometry = new BufferGeometry().setFromPoints(waypoints[k].map((v, j) => valuePoint(j, v)));
      const polyline = new Line(geometry, material);
      polyline.name = 'chart-polyline';
      chart.add(polyline);
      this.polylineMaterials.push(material);
      this.chartPolylines.push(polyline);
    }

    this.chartGroup = chart;
    this.group.add(chart);
  }

  private disposeChart() {
    if (this.chartGroup && this.group) this.group.remove(this.chartGroup);
    for (const line of this.chartPolylines) line.geometry.dispose();
    for (const axis of this.chartAxes) axis.geometry.dispose();
    for (const material of this.polylineMaterials) material.dispose();
    this.axisMaterial?.dispose();
    this.chartPolylines = [];
    this.chartAxes = [];
    this.polylineMaterials = [];
    this.axisMaterial = null;
    this.chartGroup = null;
  }

  private disposeEePath() {
    if (this.eePathGroup && this.group) this.group.remove(this.eePathGroup);
    this.eePathGeometry?.dispose();
    this.eePathMaterial?.dispose();
    this.eePathGeometry = null;
    this.eePathMaterial = null;
    this.eePathGroup = null;
  }

  private disposeRobot() {
    this.clearGhosts();
    if (this.robotGroup && this.group) this.group.remove(this.robotGroup);
    if (this.robotModel) disposeObject(this.robotModel.root);
    this.disposeEePath();
    this.robotGroup = null;
    this.ghostsGroup = null;
    this.robotModel = null;
    this.robotState = null;
  }

  private notify() {
    for (const listener of this.listeners) listener();
  }
}

/** Copy a pivot's current pose onto a cloned ghost pivot (the clone was
 * captured at identity; joints are re-applied per waypoint). */
function applyPoseToPivot(
  pivot: import('three').Object3D,
  position: Vector3,
  quaternion: import('three').Quaternion,
) {
  pivot.position.copy(position);
  pivot.quaternion.copy(quaternion);
}

function disposeObject(root: import('three').Object3D) {
  root.traverse((obj) => {
    const mesh = obj as { geometry?: { dispose?: () => void }; material?: Material | Material[] };
    mesh.geometry?.dispose?.();
    if (mesh.material) {
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      for (const material of materials) material.dispose();
    }
  });
}

function disposeMaterialsOnly(root: import('three').Object3D) {
  root.traverse((obj) => {
    const mesh = obj as { material?: Material | Material[] };
    if (mesh.material) {
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      for (const material of materials) material.dispose();
    }
  });
}
