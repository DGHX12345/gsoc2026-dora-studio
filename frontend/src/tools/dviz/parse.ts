// Flat Float32Array / JSON parsers for dviz-style path data (M12 D1).
//
// dviz publishes flat number arrays on these ports:
//   - waypoints|path:       flat xy pairs
//   - trajectory:           flat stride 3 (xyz) or stride 7 (xyz + quaternion)
//   - target_point|target|goal: flat [x, y]
// Costmaps do not exist in dviz; we define the JSON object form ourselves.
//
// feed.ts exposes JSON number arrays as BOTH f32 (Float32Array) and json,
// so every parser accepts both channels: f32 first, then the json array
// fallback. JSON objects pass through as `json` only.

import type { ToolPayload } from '../types';

/** Height the Studio renders 2D waypoints/targets at (z = 0.05). */
const WAYPOINT_Z = 0.05;

/** Waypoints: flat [x1,y1,x2,y2,...] → xyz triplets with z = WAYPOINT_Z.
 *
 * Also accepts the backend demo generator's json form
 * { waypoints: [[x, y], ...] }. Odd-length flat input drops the trailing
 * orphan value. Invalid payloads return []. */
export function parseWaypoints(payload: ToolPayload): number[] {
  if (payload.f32 instanceof Float32Array) {
    return flatToTriplets(payload.f32);
  }
  const json = payload.json;
  if (Array.isArray(json) && isFiniteNumberArray(json)) {
    return flatToTriplets(json);
  }
  const pairs = nestedWaypointPairs(json);
  if (pairs !== null) return flattenPairs(pairs);
  return [];
}

/** Trajectory: flat array; stride 3 (xyz) passes through, stride 7 keeps the
 * first 3 of each 7 (drops the quaternion). At lengths divisible by both 3
 * and 7, stride 3 wins (dimensions are implicit on the wire). Anything else
 * → []. */
export function parseTrajectory(payload: ToolPayload): number[] {
  const flat = flatNumbers(payload);
  if (flat === null) return [];
  const len = flat.length;
  if (len % 3 === 0) return flat;
  if (len % 7 === 0) {
    const out: number[] = [];
    for (let i = 0; i < len; i += 7) {
      out.push(flat[i], flat[i + 1], flat[i + 2]);
    }
    return out;
  }
  return [];
}

/** Target: flat [x, y] or [x, y, z] → { x, y, z } with z defaulting to
 * WAYPOINT_Z. Accepted from f32 or json number array. Invalid → null. */
export function parseTarget(payload: ToolPayload): { x: number; y: number; z: number } | null {
  const flat = flatNumbers(payload);
  if (flat === null) return null;
  if (flat.length === 2) return { x: flat[0], y: flat[1], z: WAYPOINT_Z };
  if (flat.length === 3) return { x: flat[0], y: flat[1], z: flat[2] };
  return null;
}

export interface CostmapData {
  width: number;
  height: number;
  resolution: number;
  values: Float32Array;
}

/** Costmap: single JSON object { width, height, resolution, values }.
 * width/height must be positive integers, resolution > 0, and
 * values.length === width * height. Invalid → null. */
export function parseCostmap(payload: ToolPayload): CostmapData | null {
  const json = payload.json;
  if (json === null || typeof json !== 'object' || Array.isArray(json)) return null;
  const obj = json as Record<string, unknown>;
  const { width, height, resolution, values } = obj;
  if (!isPositiveInteger(width)) return null;
  if (!isPositiveInteger(height)) return null;
  if (typeof resolution !== 'number' || !Number.isFinite(resolution) || resolution <= 0) return null;
  if (!Array.isArray(values) || values.length !== width * height) return null;
  if (!isFiniteNumberArray(values)) return null;
  return { width, height, resolution, values: Float32Array.from(values) };
}

/** Path length: sum of segment lengths over flat xyz triplets. */
export function computePathLength(points: number[]): number {
  const n = Math.floor(points.length / 3);
  let total = 0;
  for (let i = 1; i < n; i++) {
    const dx = points[3 * i] - points[3 * (i - 1)];
    const dy = points[3 * i + 1] - points[3 * (i - 1) + 1];
    const dz = points[3 * i + 2] - points[3 * (i - 1) + 2];
    total += Math.hypot(dx, dy, dz);
  }
  return total;
}

// ---------------------------------------------------------------------------
// Helpers

/** The flat number array from a payload, if any: f32 first, then json. */
function flatNumbers(payload: ToolPayload): number[] | null {
  if (payload.f32 instanceof Float32Array) {
    if (!isFiniteNumberArray(payload.f32)) return null;
    return Array.from(payload.f32);
  }
  const json = payload.json;
  if (Array.isArray(json) && isFiniteNumberArray(json)) return json;
  return null;
}

function isFiniteNumberArray(values: ArrayLike<unknown>): values is ArrayLike<number> {
  for (let i = 0; i < values.length; i++) {
    if (typeof values[i] !== 'number' || !Number.isFinite(values[i])) return false;
  }
  return true;
}

function isPositiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value > 0;
}

/** Flat [x1,y1,x2,y2,...] → xyz triplets; drops a trailing odd value.
 * Non-finite coordinates invalidate the whole path. */
function flatToTriplets(values: ArrayLike<number>): number[] {
  const pairs = Math.floor(values.length / 2);
  const out: number[] = [];
  for (let i = 0; i < pairs; i++) {
    const x = values[2 * i];
    const y = values[2 * i + 1];
    if (!Number.isFinite(x) || !Number.isFinite(y)) return [];
    out.push(x, y, WAYPOINT_Z);
  }
  return out;
}

/** The nested { waypoints: [[x, y], ...] } form, or null when invalid. */
function nestedWaypointPairs(json: unknown): number[][] | null {
  if (json === null || typeof json !== 'object' || Array.isArray(json)) return null;
  const waypoints = (json as Record<string, unknown>).waypoints;
  if (!Array.isArray(waypoints)) return null;
  const pairs: number[][] = [];
  for (const entry of waypoints) {
    if (!Array.isArray(entry) || entry.length !== 2 || !isFiniteNumberArray(entry)) return null;
    pairs.push([entry[0], entry[1]]);
  }
  return pairs;
}

function flattenPairs(pairs: number[][]): number[] {
  const out: number[] = [];
  for (const [x, y] of pairs) {
    out.push(x, y, WAYPOINT_Z);
  }
  return out;
}
