// Tool slot protocol — the common interface every viewport tool implements.
//
// Revision R1 (2026-08-13): payloads are flat, not structured Arrow. dviz /
// dora-moveit2 data is flat Float32Array or JSON bytes (M12/M13 source
// audit); tools parse their own payloads.
//
// Revision R3: `tf` is optional — no TF data source exists yet; tools render
// in the world frame when it is absent.

import type * as THREE from 'three';
import type { Component } from 'vue';
import type { TfTree } from './tf';

export type ToolCategory = 'visualization' | 'diagnostics' | 'planning';

export interface PortPattern {
  nodeIdPattern: string | RegExp; // e.g. "planner*" or /moveit.*/i
  outputIdPattern: string | RegExp; // e.g. "trajectory" or /^(waypoints|path)$/i
}

export interface ToolPayload {
  f32?: Float32Array;
  json?: unknown;
  bytes?: Uint8Array;
  /** M15 B4: live frames carry the sender's per-send dora metadata
   * (e.g. num_waypoints/num_joints). Absent in .drec replay. */
  metadata?: Record<string, unknown>;
}

export interface ToolBatch {
  nodeId: string;
  outputId: string;
  timestampNs: number;
  payload: ToolPayload;
}

export type ToolStatus = 'detached' | 'attached' | 'error';

/** Everything a tool needs from the viewport it mounts into.
 *
 * Revision R9 (2026-08-13): NanoRobotViewer renders on demand (GPU drops to
 * zero when idle), so tools must be able to invalidate the render — hence
 * `requestRender` in the context instead of the bare scene/camera pair. */
export interface ToolContext {
  scene: THREE.Scene;
  camera: THREE.Camera;
  requestRender: () => void;
  /** Revision R5 (M12): optional camera-focus helper with OrbitControls
   * target sync, provided by the viewer when available. */
  focusOn?: (center: { x: number; y: number; z: number }, radius: number) => void;
}

export interface ViewportTool {
  readonly id: string;
  readonly displayName: string;
  readonly category: ToolCategory;
  readonly description?: string;
  readonly subscribePorts: PortPattern[];

  onAttach(context: ToolContext): void;
  onBatch(batch: ToolBatch, tf?: TfTree): void;
  onTimelineSeek?(timestampNs: number): void;
  onDetach(): void;

  /** Optional control panel rendered inside the ToolPanel. */
  panelComponent?: Component;
}
