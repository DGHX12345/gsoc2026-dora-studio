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
  LineSegments,
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
import {
  buildWireframeMesh,
  disposeWireframeMesh,
  findCollisions,
  type CollisionPair,
} from './collision';
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

export interface ModelCatalogEntry {
  id: string;
  urdfPath: string;
  meshBasePath: string;
}

export type ModelLoader = (robotId: string) => Promise<RobotModel>;

/** Robot models available under backend `models/` (GET /api/models, M13
 * D6) — drop a URDF directory in and it shows up, no code change. */
export function fetchModelCatalog(): Promise<ModelCatalogEntry[]> {
  return fetch(`${BACKEND_BASE_URL}/api/models`)
    .then((response) => {
      if (!response.ok) throw new Error(`model catalog fetch failed: ${response.status}`);
      return response.json() as Promise<{ models: ModelCatalogEntry[] }>;
    })
    .then((catalog) => catalog.models);
}

function defaultModelLoader(robotId: string): Promise<RobotModel> {
  return fetchModelCatalog().then((catalog) => {
    const entry = catalog.find((model) => model.id === robotId);
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
  });
}

export type RobotState = 'loading' | 'loaded' | 'unavailable';

export interface MoveItSnapshot {
  robotId: string | null;
  robotState: RobotState | null;
  modelName: string | null;
  jointLabels: string[];
  numJoints: number | null;
  endEffector: { x: number; y: number; z: number } | null;
  /** The pose currently applied to the model (player or stream driven). */
  currentJointValues: number[] | null;
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
  sceneCollisions: { a: string; b: string; distance: number }[];
  collisionVisible: boolean;
  ghostCount: number;
  player: {
    playing: boolean;
    speed: number;
    syncToTimeline: boolean;
    waypointIndex: number;
    waypointCount: number;
  };
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

  // D5 collision scene overlay
  private collisionGroup: Group | null = null;
  private attachedWires: LineSegments[] = [];
  private sceneCollisions: CollisionPair[] = [];
  private lastSceneVersion: number | null = null;
  private collisionVisible = true;
  private ghostCount = DEFAULT_GHOST_COUNT;

  // D6 trajectory player (independent of the replay timeline)
  private playerPlaying = false;
  private playerSpeed = 1;
  private playerIndex = 0;
  private syncToTimeline = true;
  private playerTimer: ReturnType<typeof setInterval> | null = null;
  private lastPoseValues: number[] | null = null;

  // Parallel-coordinates chart resources (D2 fallback visualization)
  private chartGroup: Group | null = null;
  private chartPolylines: Line[] = [];
  private chartAxes: Line[] = [];
  private axisMaterial: LineBasicMaterial | null = null;
  private polylineMaterials: LineBasicMaterial[] = [];

  constructor(
    modelLoader: ModelLoader = defaultModelLoader,
    private readonly catalogFetcher: () => Promise<ModelCatalogEntry[]> = fetchModelCatalog,
  ) {
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

    // Auto-load the first locally discovered robot; the D6 selector can
    // switch models or unload.
    void this.loadFirstAvailableModel();
  }

  private async loadFirstAvailableModel() {
    const epoch = this.attachEpoch;
    try {
      const catalog = await this.catalogFetcher();
      if (epoch !== this.attachEpoch) return;
      if (catalog.length > 0) {
        await this.loadRobot(catalog[0].id);
      } else {
        this.robotState = 'unavailable'; // honest: no local models
        this.notify();
      }
    } catch {
      if (epoch !== this.attachEpoch) return;
      this.robotState = 'unavailable';
      this.notify();
    }
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
        // The demo re-sends scenes; only a version bump rebuilds the overlay.
        if (scene.version !== this.lastSceneVersion) {
          this.lastSceneVersion = scene.version;
          this.renderSceneOverlay();
          this.context?.requestRender();
        }
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
    this.sceneCollisions = [];
    this.lastSceneVersion = null;
    this.robotId = null;
    this.numJoints = null;
    this.lastSeekTs = null;
    this.stopPlayerTimer();
    this.playerPlaying = false;
    this.playerSpeed = 1;
    this.playerIndex = 0;
    this.syncToTimeline = true;
    this.ghostCount = DEFAULT_GHOST_COUNT;
    this.collisionVisible = true;
    this.lastPoseValues = null;
    this.group = null;
    this.context = null;
    this.notify();
    // Listeners stay registered: subscribers (panels, the viewport) manage
    // their own lifecycle, and clearing here broke the detach→reattach
    // flow (the viewport's nano-visibility subscription went silent after
    // the first detach). The registry tool instance is a singleton.
  }

  private stopPlayerTimer() {
    if (this.playerTimer !== null) {
      clearInterval(this.playerTimer);
      this.playerTimer = null;
    }
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
      sceneCollisions: [...this.sceneCollisions],
      collisionVisible: this.collisionVisible,
      ghostCount: this.ghostCount,
      currentJointValues: this.lastPoseValues ? [...this.lastPoseValues] : null,
      player: {
        playing: this.playerPlaying,
        speed: this.playerSpeed,
        syncToTimeline: this.syncToTimeline,
        waypointIndex: this.playerIndex,
        waypointCount: this.trajectory?.waypoints.length ?? 0,
      },
      lastSeekTs: this.lastSeekTs,
    };
  }

  /** D6 panel: pick the robot — supplies joint labels and the joint count
   * for flat trajectory reshaping (D3 config), and loads the matching
   * URDF when a local model exists. */
  setRobot(robotId: string | null) {
    // Capture the loaded id BEFORE overwriting robotId — the guard must
    // compare against the previously loaded model, not the new selection.
    const previouslyLoaded = this.loadedModelRobotId();
    this.robotId = robotId;
    this.notify();
    if (robotId && previouslyLoaded !== robotId) {
      void this.loadRobot(robotId);
    }
  }

  getRobotModel(): RobotModel | null {
    return this.robotModel;
  }

  /** D6 panel: unload the robot model — the joint-space chart becomes the
   * visualization again (and the viewport restores the Nano display). */
  unloadRobot() {
    this.stopPlayerTimer();
    this.disposeRobot();
    this.robotId = null;
    this.numJoints = null;
    this.trajectorySignature = null;
    this.notify();
  }

  /** D6 panel: play/pause the trajectory player (independent of the
   * replay timeline). Starting playback switches off timeline sync. */
  setTrajectoryPlayback(opts: { playing: boolean; speed?: number }) {
    if (opts.speed !== undefined && opts.speed > 0) this.playerSpeed = opts.speed;
    this.playerPlaying = opts.playing;
    if (opts.playing) this.syncToTimeline = false;
    this.stopPlayerTimer();
    if (this.playerPlaying) {
      // One waypoint per tick; the speed scales the tick rate (50ms base).
      this.playerTimer = setInterval(() => this.advancePlayer(), 50 / this.playerSpeed);
    }
    this.notify();
  }

  /** Advance the player by one waypoint; stops at the last one. */
  advancePlayer() {
    if (!this.playerPlaying || !this.trajectory) return;
    const count = this.trajectory.waypoints.length;
    if (count === 0) return;
    if (this.playerIndex + 1 >= count) {
      this.playerPlaying = false;
      this.stopPlayerTimer();
    } else {
      this.playerIndex += 1;
      this.applyWaypoint(this.trajectory.waypoints[this.playerIndex]);
      this.context?.requestRender();
    }
    this.notify();
  }

  /** Step the player by one waypoint without starting playback. */
  stepTrajectory(delta: 1 | -1) {
    if (!this.trajectory) return;
    const count = this.trajectory.waypoints.length;
    if (count === 0) return;
    const next = Math.min(count - 1, Math.max(0, this.playerIndex + delta));
    this.playerIndex = next;
    this.applyWaypoint(this.trajectory.waypoints[next]);
    this.context?.requestRender();
    this.notify();
  }

  /** D6 panel: timeline sync on = the replay's joint streams own the
   * pose (player paused); off = the player owns it. */
  setSyncToTimeline(sync: boolean) {
    this.syncToTimeline = sync;
    if (sync) {
      this.playerPlaying = false;
      this.stopPlayerTimer();
      // Return the pose to the last stream values.
      this.applyCurrentPose();
    }
    this.notify();
  }

  /** D6 panel: ghost pose count, clamped to 0..20 (0 hides all ghosts);
   * rebuilds the ghosts. */
  setGhostCount(count: number) {
    this.ghostCount = Math.min(20, Math.max(0, Math.round(count)));
    if (this.robotState === 'loaded' && this.trajectory) {
      this.rebuildGhosts(this.trajectory.waypoints);
      this.context?.requestRender();
    }
    this.notify();
  }

  /** D6 panel: collision wireframe overlay visibility. */
  setCollisionVisible(visible: boolean) {
    this.collisionVisible = visible;
    if (this.collisionGroup) this.collisionGroup.visible = visible;
    this.context?.requestRender();
    this.notify();
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

    // A scene that arrived before the model now re-parents its attached
    // objects under the fresh link groups.
    if (this.scene) this.renderSceneOverlay();
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
    this.lastPoseValues = [...waypoint];
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
    const count = this.ghostCount;
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

  /** Yellow wireframe overlays for the planning scene (D5): world objects
   * under the tool group at their scene positions; attached objects parent
   * under their robot link so they follow the model. Also refreshes the
   * bounding-sphere collision report. */
  private renderSceneOverlay() {
    if (!this.group) return;
    this.clearSceneOverlay();
    this.collisionGroup = new Group();
    this.collisionGroup.name = 'moveit-collision';
    this.collisionGroup.visible = this.collisionVisible;
    this.group.add(this.collisionGroup);
    if (!this.scene) return;

    for (const obj of this.scene.data.world_objects) {
      this.collisionGroup.add(buildWireframeMesh(obj));
    }
    for (const obj of this.scene.data.attached_objects) {
      const wire = buildWireframeMesh(obj);
      const link = this.robotModel?.links.get(obj.attached_link);
      if (link) {
        link.add(wire);
      } else {
        this.collisionGroup.add(wire);
      }
      this.attachedWires.push(wire);
    }
    this.sceneCollisions = findCollisions(this.scene.data.world_objects);
  }

  private clearSceneOverlay() {
    if (this.collisionGroup && this.group) this.group.remove(this.collisionGroup);
    if (this.collisionGroup) {
      for (const child of [...this.collisionGroup.children]) {
        disposeWireframeMesh(child as LineSegments);
        this.collisionGroup.remove(child);
      }
    }
    for (const wire of this.attachedWires) {
      wire.removeFromParent();
      disposeWireframeMesh(wire);
    }
    this.attachedWires = [];
    this.collisionGroup = null;
  }

  private applyCurrentPose() {
    if (!this.robotModel) return;
    if (this.playerPlaying) return; // the player owns the pose
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
    this.clearSceneOverlay(); // attached wires live inside the model tree
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
