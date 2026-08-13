// ToolRegistry — the single mount point for viewport tools (D2).
//
// Tools register themselves, attach to the 3D scene, and receive batches
// broadcast at playback time. The registry keeps tools isolated: one tool
// throwing never breaks another tool's delivery or detach.

import { matchToolPorts } from './matching';
import type { TfTree } from './tf';
import type { ToolBatch, ToolContext, ToolStatus, ViewportTool } from './types';

interface ToolState {
  status: ToolStatus;
}

export class ToolRegistry {
  private readonly tools = new Map<string, ViewportTool>();
  private readonly states = new Map<string, ToolState>();
  private readonly listeners = new Set<() => void>();

  register(tool: ViewportTool) {
    if (this.tools.has(tool.id)) {
      throw new Error(`tool already registered: ${tool.id}`);
    }
    this.tools.set(tool.id, tool);
    this.states.set(tool.id, { status: 'detached' });
    this.notify();
  }

  unregister(id: string) {
    if (!this.tools.has(id)) return;
    this.tools.delete(id);
    this.states.delete(id);
    this.notify();
  }

  get(id: string): ViewportTool | undefined {
    return this.tools.get(id);
  }

  list(): ViewportTool[] {
    return [...this.tools.values()];
  }

  statusOf(id: string): ToolStatus {
    return this.states.get(id)?.status ?? 'detached';
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  attachToScene(id: string, context: ToolContext) {
    const tool = this.tools.get(id);
    if (!tool) throw new Error(`unknown tool: ${id}`);
    const state = this.states.get(id);
    if (!state || state.status === 'attached') return;

    try {
      tool.onAttach(context);
      state.status = 'attached';
    } catch (error) {
      state.status = 'error';
      console.error(`tool ${id} failed to attach:`, error);
    }
    this.notify();
  }

  detachFromScene(id: string) {
    const tool = this.tools.get(id);
    const state = this.states.get(id);
    if (!tool || !state || state.status !== 'attached') return;

    try {
      tool.onDetach();
    } catch (error) {
      console.error(`tool ${id} failed to detach:`, error);
    }
    state.status = 'detached';
    this.notify();
  }

  broadcastBatch(batch: ToolBatch, tf?: TfTree) {
    for (const tool of this.tools.values()) {
      if (this.statusOf(tool.id) !== 'attached') continue;
      if (!matchToolPorts(tool.subscribePorts, batch.nodeId, batch.outputId)) continue;
      try {
        tool.onBatch(batch, tf);
      } catch (error) {
        this.markError(tool.id);
        console.error(`tool ${tool.id} failed to process batch:`, error);
      }
    }
  }

  broadcastSeek(timestampNs: number) {
    for (const tool of this.tools.values()) {
      if (this.statusOf(tool.id) !== 'attached') continue;
      if (!tool.onTimelineSeek) continue;
      try {
        tool.onTimelineSeek(timestampNs);
      } catch (error) {
        this.markError(tool.id);
        console.error(`tool ${tool.id} failed to seek:`, error);
      }
    }
  }

  private markError(id: string) {
    const state = this.states.get(id);
    if (state) {
      state.status = 'error';
      this.notify();
    }
  }

  private notify() {
    for (const listener of this.listeners) listener();
  }
}

export const toolRegistry = new ToolRegistry();
