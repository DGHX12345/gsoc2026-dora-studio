// MoveItTool — M13 core tool: consumes dora-moveit2 planner/executor/scene
// streams and renders the joint-space trajectory in the viewport.
//
// D2 scope: parallel-coordinates joint-space chart — the plan's FK fallback
// (robot geometry rendering arrives in D4 with the URDF loader and replaces
// the chart with FK poses). Status streams (plan_status, execution_status,
// joint_commands, joint_positions, scene_update) parse and expose through
// getSnapshot() for the D6 panel; scene rendering lands in D5.
//
// Trajectory batches arrive as the object envelope { waypoints: [[q...]] }
// (replay demo form) or flat arrays (M15-B live) reshaped with the joint
// count from the robot config (D3). Real .drec Arrow IPC bytes stay
// unsupported: the parser returns null and the last known data survives.

import { BufferGeometry, Color, Group, Line, LineBasicMaterial, Vector3 } from 'three';
import type { Component } from 'vue';

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

export interface MoveItSnapshot {
  robotId: string | null;
  jointLabels: string[];
  numJoints: number | null;
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

  private context: ToolContext | null = null;
  private group: Group | null = null;

  private trajectory: { nodeId: string; waypoints: number[][]; lastBatchTs: number } | null = null;
  private planStatus: { status: PlanStatus; lastBatchTs: number } | null = null;
  private execution: { status: ExecutionStatus; lastBatchTs: number } | null = null;
  private jointCommands: { values: number[]; lastBatchTs: number } | null = null;
  private jointPositions: { values: number[]; lastBatchTs: number } | null = null;
  private scene: { data: PlanningScene; lastBatchTs: number } | null = null;
  private robotId: string | null = null;
  private numJoints: number | null = null;
  private lastSeekTs: number | null = null;
  private readonly listeners = new Set<() => void>();

  // Parallel-coordinates chart resources (D2 fallback visualization)
  private chartGroup: Group | null = null;
  private chartPolylines: Line[] = [];
  private chartAxes: Line[] = [];
  private axisMaterial: LineBasicMaterial | null = null;
  private polylineMaterials: LineBasicMaterial[] = [];

  onAttach(context: ToolContext) {
    if (this.context) return; // already attached: no-op

    this.group = new Group();
    this.group.name = 'moveit-bridge';
    context.scene.add(this.group);
    this.context = context; // only after the scene add succeeds
    context.requestRender();
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
        this.notify();
      }
    } else if (outputId === 'joint_positions') {
      const values = parseJointPositions(batch.payload);
      if (values) {
        this.jointPositions = { values, lastBatchTs: batch.timestampNs };
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
    this.context.scene.remove(this.group);
    this.context.requestRender();
    this.disposeChart();

    this.trajectory = null;
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
    return {
      robotId: this.robotId,
      jointLabels: jointLabelsFor(this.robotId, this.numJoints ?? 0),
      numJoints: this.numJoints,
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
   * for flat trajectory reshaping (D3 config). */
  setRobot(robotId: string | null) {
    this.robotId = robotId;
    this.notify();
  }

  // -------------------------------------------------------------------------
  // Internals

  private resolveNumJoints(): number | null {
    if (this.robotId) {
      const config = getRobotConfig(this.robotId);
      if (config) return config.jointNames.length;
    }
    return this.numJoints;
  }

  private handleTrajectory(batch: ToolBatch) {
    const waypoints = parseTrajectory(batch.payload, this.resolveNumJoints());
    if (waypoints === null) return; // invalid/unsupported: keep last known plan
    this.trajectory = { nodeId: batch.nodeId, waypoints, lastBatchTs: batch.timestampNs };
    this.numJoints = waypoints[0]?.length ?? this.numJoints;
    this.renderChart(waypoints);
    this.context?.requestRender();
    this.notify();
  }

  /** Parallel-coordinates joint-space chart — the plan's FK fallback until
   * D4 robot geometry lands. One vertical axis per joint (per-joint
   * min/max scaled); one blue→red polyline per waypoint across the axes. */
  private renderChart(waypoints: number[][]) {
    if (!this.group) return;
    const jointCount = waypoints[0]?.length ?? 0;
    if (jointCount === 0) return;

    this.disposeChart();
    const chart = new Group();
    chart.name = 'moveit-joint-chart';

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

  private notify() {
    for (const listener of this.listeners) listener();
  }
}
