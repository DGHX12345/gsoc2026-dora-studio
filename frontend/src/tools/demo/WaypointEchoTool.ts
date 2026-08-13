// WaypointEchoTool — M11 reference implementation proving the tool slot
// protocol end to end. Renders the latest waypoints/trajectory batch as a
// polyline with start/end markers. M12 replaces this with the full dviz
// path tool.

import { BufferGeometry, Float32BufferAttribute, Group, Line, LineBasicMaterial, Mesh, MeshBasicMaterial, SphereGeometry } from 'three';
import type { Component } from 'vue';

import type { TfTree } from '../tf';
import type { ToolBatch, ToolContext, ToolPayload, ViewportTool } from '../types';

export interface WaypointStats {
  pointCount: number;
  lastSource: string;
  lastTimestampNs: number;
}

export function parseWaypointPoints(payload: ToolPayload): number[] {
  const { f32, json } = payload;

  let flat: number[] | null = null;
  if (f32) {
    flat = [...f32];
  } else if (Array.isArray(json) && json.every((n) => typeof n === 'number')) {
    flat = json as number[];
  } else if (json && typeof json === 'object') {
    const waypoints = (json as { waypoints?: unknown }).waypoints;
    if (Array.isArray(waypoints)) {
      const out: number[] = [];
      for (const row of waypoints) {
        if (
          Array.isArray(row)
          && row.length >= 2
          && typeof row[0] === 'number'
          && typeof row[1] === 'number'
        ) {
          out.push(row[0], row[1], 0.05);
        }
      }
      return out;
    }
  }

  if (!flat) return [];
  if (flat.length % 3 === 0) return flat;
  if (flat.length % 2 === 0) {
    const out: number[] = [];
    for (let i = 0; i < flat.length; i += 2) out.push(flat[i], flat[i + 1], 0.05);
    return out;
  }
  return [];
}

export class WaypointEchoTool implements ViewportTool {
  readonly id = 'waypoint-echo';
  readonly displayName = 'Waypoint Echo';
  readonly category = 'visualization' as const;
  readonly description = 'M11 demo tool: renders waypoints/trajectory batches as a 3D path.';
  readonly subscribePorts = [
    { nodeIdPattern: '*', outputIdPattern: /^(waypoints|path)$/i },
    { nodeIdPattern: '*', outputIdPattern: /^trajectory$/i },
  ];
  panelComponent?: Component;

  private context: ToolContext | null = null;
  private group: Group | null = null;
  private line: Line | null = null;
  private startMarker: Mesh | null = null;
  private endMarker: Mesh | null = null;
  private geometry: BufferGeometry | null = null;
  private lastStats: WaypointStats | null = null;

  onAttach(context: ToolContext) {
    this.context = context;

    this.group = new Group();
    this.group.name = 'waypoint-echo';

    this.geometry = new BufferGeometry();
    this.line = new Line(
      this.geometry,
      new LineBasicMaterial({ color: 0x22d3ee, linewidth: 2 }),
    );
    this.startMarker = new Mesh(
      new SphereGeometry(0.02, 8, 8),
      new MeshBasicMaterial({ color: 0x22c55e }),
    );
    this.endMarker = new Mesh(
      new SphereGeometry(0.02, 8, 8),
      new MeshBasicMaterial({ color: 0xef4444 }),
    );

    this.group.add(this.line, this.startMarker, this.endMarker);
    this.group.visible = false;
    context.scene.add(this.group);
    context.requestRender();
  }

  onBatch(batch: ToolBatch, _tf?: TfTree) {
    if (!this.context || !this.group || !this.geometry) return;
    const points = parseWaypointPoints(batch.payload);
    if (points.length === 0) return;

    this.geometry.setAttribute('position', new Float32BufferAttribute(points, 3));
    this.geometry.computeBoundingSphere();

    const first = points;
    const last = points.slice(-3);
    this.startMarker?.position.set(first[0], first[1], first[2]);
    this.endMarker?.position.set(last[0], last[1], last[2]);

    this.group.visible = true;
    this.lastStats = {
      pointCount: points.length / 3,
      lastSource: `${batch.nodeId}/${batch.outputId}`,
      lastTimestampNs: batch.timestampNs,
    };
    this.context.requestRender();
  }

  onDetach() {
    if (this.group && this.context) {
      this.context.scene.remove(this.group);
    }
    this.geometry?.dispose();
    (this.line?.material as { dispose?: () => void } | undefined)?.dispose?.();
    (this.startMarker?.material as { dispose?: () => void } | undefined)?.dispose?.();
    (this.endMarker?.material as { dispose?: () => void } | undefined)?.dispose?.();
    this.startMarker?.geometry.dispose();
    this.endMarker?.geometry.dispose();

    this.context = null;
    this.group = null;
    this.line = null;
    this.startMarker = null;
    this.endMarker = null;
    this.geometry = null;
    this.lastStats = null;
  }

  stats(): WaypointStats | null {
    return this.lastStats;
  }
}
