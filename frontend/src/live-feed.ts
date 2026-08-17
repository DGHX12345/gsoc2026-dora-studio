// LiveFeed — bridges the backend live frame buffer (M15 B3) into the tool
// slot system (M15 B4). Polls /api/live/recent and broadcasts every new
// frame as a ToolBatch so the M12/M13 tools render live data unchanged.

import type { LiveFrame } from './api'
import { parseTfPayload, SimpleTfTree } from './tools/tf'
import type { TfTree } from './tools/tf'
import type { ToolBatch, ToolPayload } from './tools/types'

export type LiveFeedStatus = 'stopped' | 'running' | 'error'

/** First poll window: only frames from the last 2s are fetched (the backend
 * ring buffer can hold much more; catch-up playback is the .drec path). */
export const INITIAL_WINDOW_NS = 2_000_000_000

export function defaultSinceTs(nowNs: number): number {
  return nowNs - INITIAL_WINDOW_NS
}

export function frameToToolBatch(frame: LiveFrame): ToolBatch | null {
  if (!frame.node_id || !frame.output_id || !frame.payload) return null
  const p = frame.payload

  let payload: ToolPayload
  if (Array.isArray(p.values) && p.values.every((n) => typeof n === 'number')) {
    payload = { f32: Float32Array.from(p.values), json: p.values }
  } else if (p.json !== undefined) {
    payload = { json: p.json }
  } else if (typeof p.bytes_base64 === 'string') {
    const binary = atob(p.bytes_base64)
    payload = { bytes: Uint8Array.from(binary, (c) => c.charCodeAt(0)) }
  } else {
    return null
  }

  if (p.metadata && typeof p.metadata === 'object') {
    payload.metadata = p.metadata
  }
  return {
    nodeId: frame.node_id,
    outputId: frame.output_id,
    timestampNs: frame.timestamp,
    payload,
  }
}

export class LiveFeedEngine {
  private timer: ReturnType<typeof setInterval> | null = null
  private inFlight = false
  private sinceTs: number
  private _status: LiveFeedStatus = 'stopped'
  private readonly listeners = new Set<() => void>()
  private readonly tfTree: TfTree = new SimpleTfTree()

  constructor(
    private readonly fetchRecent: (sinceTs: number) => Promise<LiveFrame[]>,
    private readonly broadcast: (batch: ToolBatch, tf?: TfTree) => void,
    private readonly intervalMs = 50,
    initialSinceTs = 0,
  ) {
    this.sinceTs = initialSinceTs
  }

  get status(): LiveFeedStatus {
    return this._status
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  start() {
    if (this.timer !== null) return
    this._status = 'running'
    this.timer = setInterval(() => { void this.poll() }, this.intervalMs)
    this.notify()
  }

  stop() {
    if (this.timer !== null) {
      clearInterval(this.timer)
      this.timer = null
    }
    this._status = 'stopped'
    this.notify()
  }

  async poll(): Promise<void> {
    if (this.inFlight) return
    this.inFlight = true
    try {
      const frames = await this.fetchRecent(this.sinceTs)
      let maxTs = this.sinceTs
      for (const frame of frames) {
        if (frame.timestamp > maxTs) maxTs = frame.timestamp
        const batch = frameToToolBatch(frame)
        if (!batch) continue
        // TF payloads update the frame tree first so this frame's
        // transforms are current for the tools receiving them (M11 R3).
        const tfEntries = parseTfPayload(batch.payload.json)
        if (tfEntries.length > 0) this.tfTree.apply(tfEntries)
        this.broadcast(batch, this.tfTree)
      }
      this.sinceTs = maxTs
      if (this._status === 'error') {
        this._status = 'running'
        this.notify()
      }
    } catch {
      if (this._status !== 'error') {
        this._status = 'error'
        this.notify()
      }
    } finally {
      this.inFlight = false
    }
  }

  private notify() {
    for (const listener of this.listeners) listener()
  }
}
