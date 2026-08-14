// MoveIt payload parsers (M13 D1) — flat float32 / JSON-as-uint8 shapes
// verified against dora-moveit2 source (2026-08-14).
//
// Replay note: .drec entries carry no Arrow metadata, so num_waypoints /
// num_joints never arrive on the wire — trajectory reshaping uses the joint
// count from the D3 joint config. The demo generator writes the object
// envelope form instead (unambiguous against dviz xyz trajectories on the
// shared `trajectory` port). Raw Arrow IPC bytes (real .drec) are honestly
// unsupported: every parser returns null.

import type { ToolPayload } from '../types';
import type {
  AttachedObject,
  ExecutionStatus,
  PlanningScene,
  PlanStatus,
  RobotState,
  SceneObject,
  SceneObjectType,
} from './types';

const SCENE_OBJECT_TYPES: SceneObjectType[] = ['sphere', 'box', 'cylinder'];

/** Gray wireframe default — attached objects have no color on the wire. */
const DEFAULT_COLOR: [number, number, number, number] = [0.5, 0.5, 0.5, 1];

/** Joint-space trajectory → rows of waypoints. The object envelope
 * { waypoints: [[q...], ...] } wins when present; otherwise a flat
 * f32/json number array is reshaped into numJoints-wide rows. A flat
 * payload without a joint count (no metadata on .drec replay) is
 * unparseable — honest null. */
export function parseTrajectory(payload: ToolPayload, numJoints: number | null): number[][] | null {
  const envelope = envelopeWaypoints(payload.json);
  if (envelope !== undefined) return envelope;

  if (numJoints === null || !Number.isInteger(numJoints) || numJoints <= 0) return null;
  const flat = flatNumbers(payload);
  if (flat === null || flat.length === 0 || flat.length % numJoints !== 0) return null;

  const rows: number[][] = [];
  for (let i = 0; i < flat.length; i += numJoints) {
    rows.push(flat.slice(i, i + numJoints));
  }
  return rows;
}

/** planner → plan_status. Real payload carries 7 keys; the error branch
 * sends only {plan_id, success, message}. `success` is the one required
 * field — everything else passes through when present and finite. */
export function parsePlanStatus(payload: ToolPayload): PlanStatus | null {
  const obj = asObject(payload.json);
  if (obj === null || typeof obj.success !== 'boolean') return null;

  const status: PlanStatus = { success: obj.success };
  if (typeof obj.message === 'string') status.message = obj.message;
  if (isFiniteNumber(obj.plan_id)) status.plan_id = obj.plan_id;
  if (isFiniteNumber(obj.planning_time)) status.planning_time = obj.planning_time;
  if (isFiniteNumber(obj.path_length)) status.path_length = obj.path_length;
  if (isFiniteNumber(obj.num_waypoints)) status.num_waypoints = obj.num_waypoints;
  if (isFiniteNumber(obj.num_nodes)) status.num_nodes = obj.num_nodes;
  return status;
}

/** trajectory_executor → execution_status. The 3 core keys are required;
 * execution_count / total_waypoints are optional extras. */
export function parseExecutionStatus(payload: ToolPayload): ExecutionStatus | null {
  const obj = asObject(payload.json);
  if (obj === null) return null;
  if (typeof obj.is_executing !== 'boolean') return null;
  if (!isFiniteNumber(obj.current_waypoint)) return null;
  if (!isFiniteNumber(obj.progress)) return null;

  const status: ExecutionStatus = {
    is_executing: obj.is_executing,
    current_waypoint: obj.current_waypoint,
    progress: obj.progress,
  };
  if (isFiniteNumber(obj.execution_count)) status.execution_count = obj.execution_count;
  if (isFiniteNumber(obj.total_waypoints)) status.total_waypoints = obj.total_waypoints;
  return status;
}

/** planning_scene → scene_update. Malformed object entries are dropped;
 * valid ones survive. `version` must be a finite number. */
export function parseSceneUpdate(payload: ToolPayload): PlanningScene | null {
  const obj = asObject(payload.json);
  if (obj === null) return null;
  if (!isFiniteNumber(obj.version)) return null;
  if (!Array.isArray(obj.world_objects) || !Array.isArray(obj.attached_objects)) return null;

  const scene: PlanningScene = {
    version: obj.version,
    world_objects: obj.world_objects.map(parseSceneObject).filter((o): o is SceneObject => o !== null),
    attached_objects: obj.attached_objects
      .map(parseAttachedObject)
      .filter((o): o is AttachedObject => o !== null),
    robot_state: parseRobotState(obj.robot_state),
  };
  if (isFiniteNumber(obj.timestamp)) scene.timestamp = obj.timestamp;
  return scene;
}

/** mujoco → joint_positions (float64 qpos). The json channel keeps full
 * precision (feed.ts exposes both); f32 is the fallback. */
export function parseJointPositions(payload: ToolPayload): number[] | null {
  return flatNumbers(payload);
}

/** trajectory_executor → joint_commands (flat float32 per tick). */
export function parseJointCommands(payload: ToolPayload): number[] | null {
  return flatNumbers(payload);
}

// ---------------------------------------------------------------------------
// Helpers

/** The { waypoints: [[q...], ...] } envelope: parsed rows, null when the
 * envelope is present but invalid, undefined when it is absent. */
function envelopeWaypoints(json: unknown): number[][] | null | undefined {
  if (json === null || typeof json !== 'object' || Array.isArray(json)) return undefined;
  const waypoints = (json as Record<string, unknown>).waypoints;
  if (waypoints === undefined) return undefined;
  if (!Array.isArray(waypoints) || waypoints.length === 0) return null;

  const rows: number[][] = [];
  const width = (waypoints[0] as unknown[] | undefined)?.length;
  if (!Number.isInteger(width) || width! <= 0) return null;
  for (const entry of waypoints) {
    if (!Array.isArray(entry) || entry.length !== width || !isFiniteNumberArray(entry)) return null;
    rows.push([...entry]);
  }
  return rows;
}

function parseSceneObject(entry: unknown): SceneObject | null {
  const base = sceneObjectBase(entry);
  if (base === null) return null;
  return { ...base, color: parseColor(entry) };
}

function parseAttachedObject(entry: unknown): AttachedObject | null {
  const base = sceneObjectBase(entry);
  if (base === null) return null;
  const obj = entry as Record<string, unknown>;
  return {
    ...base,
    color: DEFAULT_COLOR,
    attached_link: typeof obj.attached_link === 'string' ? obj.attached_link : '',
  };
}

function sceneObjectBase(entry: unknown): Omit<SceneObject, 'color'> | null {
  if (entry === null || typeof entry !== 'object' || Array.isArray(entry)) return null;
  const obj = entry as Record<string, unknown>;
  if (typeof obj.name !== 'string' || obj.name.length === 0) return null;
  if (typeof obj.type !== 'string' || !SCENE_OBJECT_TYPES.includes(obj.type as SceneObjectType)) {
    return null;
  }
  if (!Array.isArray(obj.position) || obj.position.length !== 3 || !isFiniteNumberArray(obj.position)) {
    return null;
  }
  if (!Array.isArray(obj.dimensions) || obj.dimensions.length === 0 || !isFiniteNumberArray(obj.dimensions)) {
    return null;
  }
  return {
    name: obj.name,
    type: obj.type as SceneObjectType,
    position: obj.position as [number, number, number],
    dimensions: [...obj.dimensions],
  };
}

function parseColor(entry: unknown): [number, number, number, number] {
  const obj = entry as Record<string, unknown>;
  if (Array.isArray(obj.color) && obj.color.length === 4 && isFiniteNumberArray(obj.color)) {
    return obj.color as [number, number, number, number];
  }
  return DEFAULT_COLOR;
}

function parseRobotState(value: unknown): RobotState {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return { joint_positions: [], gripper_state: 0 };
  }
  const obj = value as Record<string, unknown>;
  const joint_positions = Array.isArray(obj.joint_positions) && isFiniteNumberArray(obj.joint_positions)
    ? [...obj.joint_positions]
    : [];
  const gripper_state = isFiniteNumber(obj.gripper_state) ? obj.gripper_state : 0;
  return { joint_positions, gripper_state };
}

function asObject(json: unknown): Record<string, unknown> | null {
  if (json === null || typeof json !== 'object' || Array.isArray(json)) return null;
  return json as Record<string, unknown>;
}

/** The flat number array from a payload, if any: json first (full float64
 * precision), then f32. */
function flatNumbers(payload: ToolPayload): number[] | null {
  const json = payload.json;
  if (Array.isArray(json) && isFiniteNumberArray(json)) return json;
  if (payload.f32 instanceof Float32Array && isFiniteNumberArray(payload.f32)) {
    return Array.from(payload.f32);
  }
  return null;
}

function isFiniteNumberArray(values: ArrayLike<unknown>): values is ArrayLike<number> {
  for (let i = 0; i < values.length; i++) {
    if (typeof values[i] !== 'number' || !Number.isFinite(values[i])) return false;
  }
  return true;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}
