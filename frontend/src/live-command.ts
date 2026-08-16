// Live console command helpers (M15 B6) — build backend command payloads
// for the Motion Planner console and extract live statuses from the feed.

import type { LiveCommandRequest, LiveFrame } from './api'

export interface ConsoleStatus {
  planStatus: Record<string, unknown> | null
  execution: Record<string, unknown> | null
  joints: number[] | null
}

export function buildPlanCommand(target: number[], planner?: string): LiveCommandRequest {
  const command: LiveCommandRequest = { kind: 'plan', target }
  if (planner) command.planner = planner
  return command
}

export function buildExecuteCommand(kind: 'execute' | 'stop' | 'auto'): LiveCommandRequest {
  return { kind }
}

export function buildSceneAddCommand(
  name: string,
  type: 'box' | 'sphere' | 'cylinder',
  position: number[],
  dimensions: number[],
): LiveCommandRequest {
  return {
    kind: 'scene',
    action: 'add',
    object: { name, type, position, dimensions },
  }
}

export function buildSceneRemoveCommand(name: string): LiveCommandRequest {
  return { kind: 'scene', action: 'remove', object: { name } }
}

/** Strict finite-number parsing for the target inputs; null on any
 * invalid field (empty, NaN, non-numeric). */
export function parseTargetInputs(x: string, y: string, z: string): number[] | null {
  if (!x.trim() || !y.trim() || !z.trim()) return null
  const parsed = [Number(x), Number(y), Number(z)]
  if (parsed.some((n) => !Number.isFinite(n))) return null
  return parsed
}

/** Latest plan_status / execution_status across the live frame feed;
 * everything else is ignored. */
export function extractConsoleStatus(frames: LiveFrame[]): ConsoleStatus {
  let planStatus: Record<string, unknown> | null = null
  let execution: Record<string, unknown> | null = null
  let joints: number[] | null = null
  for (const frame of frames) {
    if (frame.output_id === 'joint_positions' && Array.isArray(frame.payload.values)) {
      joints = frame.payload.values as number[]
      continue
    }
    const json = frame.payload.json
    if (json === null || typeof json !== 'object' || Array.isArray(json)) continue
    if (frame.output_id === 'plan_status') planStatus = json as Record<string, unknown>
    if (frame.output_id === 'execution_status') {
      execution = json as Record<string, unknown>
    }
  }
  return { planStatus, execution, joints }
}
