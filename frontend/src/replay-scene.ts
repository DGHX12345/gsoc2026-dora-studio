// ReplayScene — bridges the playback engine (M05) with the 3D viewport.
// Fetches recording entries at the current timestamp, parses known data
// layers (joint state, base pose), and updates reactive objects that the
// NanoRobotViewer consumes via its existing props.

import type { PlaybackEngine } from './playback'
import { getRecordingEntriesWithData, type SeekEntryResponse } from './api'
import { entryToToolBatch } from './tools/feed'
import { toolRegistry } from './tools/registry'
import { parseTfPayload, SimpleTfTree } from './tools/tf'

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

export interface RobotJointState {
  [jointName: string]: number
}

export interface RobotBasePose {
  x: number
  y: number
  yaw: number
}

export interface ReplayFrame {
  timestampNanos: number
  joints: RobotJointState
  basePose: RobotBasePose
  activeStreams: string[] // nodeId/outputId pairs that have data at this frame
}

const emptyJoints: RobotJointState = {
  joint_1: 0, joint_2: 0, joint_3: 0,
  joint_4: 0, joint_5: 0, joint_6: 0,
}

const emptyBasePose: RobotBasePose = { x: 0, y: 0, yaw: 0 }

// ---------------------------------------------------------------------------
// ReplayScene
// ---------------------------------------------------------------------------

export class ReplayScene {
  private _recordingId: string
  private _tfTree = new SimpleTfTree()
  private _currentFrame: ReplayFrame = {
    timestampNanos: 0,
    joints: { ...emptyJoints },
    basePose: { ...emptyBasePose },
    activeStreams: [],
  }

  // Callbacks for the viewport to observe
  private _onFrameChange: ((frame: ReplayFrame) => void) | null = null

  constructor(recordingId: string) {
    this._recordingId = recordingId
  }

  get currentFrame(): Readonly<ReplayFrame> { return this._currentFrame }

  onFrameChange(cb: (frame: ReplayFrame) => void) { this._onFrameChange = cb }

  /** Wire up to a PlaybackEngine. Call this once after both are created. */
  attach(engine: PlaybackEngine) {
    const fetchAndUpdate = async (timestampNs: number) => {
      try {
        await this.updateFromTimestamp(timestampNs)
      } catch {
        // Ignore fetch errors during rapid scrubbing
      }
    }

    // On each timeline tick, fetch entries and parse
    engine.onTick(async (t) => {
      // Debounce: skip intermediate updates during rapid scrubbing
      fetchAndUpdate(t)
    })
  }

  async updateFromTimestamp(timestampNs: number): Promise<void> {
    const frameWindow = 50_000_000 // ±50ms window
    const entries = await this.fetchEntriesAt(timestampNs, frameWindow)

    // Parse entries into data layers
    const joints: RobotJointState = { ...emptyJoints }
    const basePose: RobotBasePose = { ...emptyBasePose }
    const activeStreams: string[] = []

    for (const e of entries) {
      const streamKey = `${e.nodeId}/${e.outputId}`
      activeStreams.push(streamKey)

      // Parse event_bytes as UTF-8 JSON
      if (!e.eventBytes) continue
      try {
        const text = utf8Decode(e.eventBytes)
        const data = JSON.parse(text)

        if (data.joints && typeof data.joints === 'object') {
          Object.assign(joints, data.joints)
        }
        if (data.basePose && typeof data.basePose === 'object') {
          Object.assign(basePose, data.basePose)
        }
      } catch {
        // Skip entries with non-JSON payloads
      }

      // M11: feed tool slots. TF payloads update the frame tree first so the
      // transforms of this frame are current for the tools receiving them.
      const batch = entryToToolBatch(e)
      if (!batch) continue
      const tfEntries = parseTfPayload(batch.payload.json)
      if (tfEntries.length > 0) this._tfTree.apply(tfEntries)
      toolRegistry.broadcastBatch(batch, this._tfTree)
    }
    toolRegistry.broadcastSeek(timestampNs)

    this._currentFrame = {
      timestampNanos: timestampNs,
      joints,
      basePose,
      activeStreams,
    }

    this._onFrameChange?.(this._currentFrame)
  }

  private async fetchEntriesAt(
    timestampNs: number,
    windowNs: number,
  ): Promise<SeekEntryResponse[]> {
    const result = await getRecordingEntriesWithData(this._recordingId, { limit: 200 })
    return result.entries.filter(
      (e) => Math.abs(e.timestampNanos - timestampNs) < windowNs,
    )
  }

  dispose() {
    this._onFrameChange = null
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function utf8Decode(bytes: number[] | Uint8Array): string {
  const decoder = new TextDecoder()
  if (bytes instanceof Uint8Array) return decoder.decode(bytes)
  return decoder.decode(new Uint8Array(bytes))
}

// ---------------------------------------------------------------------------
// Joint interpolation helpers (for smooth playback)
// ---------------------------------------------------------------------------

export function lerpAngle(a: number, b: number, t: number): number {
  // Handle angle wrapping for revolute joints
  let diff = b - a
  while (diff > Math.PI) diff -= 2 * Math.PI
  while (diff < -Math.PI) diff += 2 * Math.PI
  return a + diff * t
}

export function interpolateJoints(
  from: RobotJointState,
  to: RobotJointState,
  t: number,
): RobotJointState {
  const result: RobotJointState = {}
  for (const key of Object.keys(from)) {
    result[key] = lerpAngle(from[key] || 0, to[key] || 0, t)
  }
  return result
}
