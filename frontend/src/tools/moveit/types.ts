// MoveIt wire types (M13) — shapes verified against dora-moveit2 source
// (2026-08-14, see plans2.0/M13-moveit-bridge.md Revision R1).

export interface PlanStatus {
  success: boolean;
  plan_id?: number;
  planning_time?: number;
  path_length?: number;
  num_waypoints?: number;
  num_nodes?: number;
  message?: string;
}

export interface ExecutionStatus {
  is_executing: boolean;
  current_waypoint: number;
  progress: number;
  execution_count?: number;
  total_waypoints?: number;
}

export type SceneObjectType = 'sphere' | 'box' | 'cylinder';

/** Collision object in a planning scene. Attached objects carry no color on
 * the wire — the parser fills the rviz-convention default gray. */
export interface SceneObject {
  name: string;
  type: SceneObjectType;
  position: [number, number, number];
  /** [r] | [sx,sy,sz] | [r,h] */
  dimensions: number[];
  color: [number, number, number, number];
}

export interface AttachedObject extends SceneObject {
  attached_link: string;
}

export interface RobotState {
  joint_positions: number[];
  gripper_state: number;
}

export interface PlanningScene {
  version: number;
  timestamp?: number;
  world_objects: SceneObject[];
  attached_objects: AttachedObject[];
  robot_state: RobotState;
}
