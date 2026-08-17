// LiveFeedEngine tests — self-executing node:test module (async-safe:
// node:test awaits async test bodies, unlike the legacy sync runner).

import { strict as assert } from 'node:assert'
import { test } from 'node:test'

import type { LiveFrame } from './api'
import { defaultSinceTs, frameToToolBatch, LiveFeedEngine } from './live-feed'
import type { ToolBatch } from './tools/types'

const baseFrame: LiveFrame = {
  node_id: 'planner',
  output_id: 'trajectory',
  timestamp: 1000,
  payload: {},
}

test('frameToToolBatch converts a values array to f32 plus full-precision json', () => {
  const batch = frameToToolBatch({
    ...baseFrame,
    payload: { values: [1.5, 2.5, 3.5] },
  })
  assert.ok(batch)
  assert.equal(batch.nodeId, 'planner')
  assert.equal(batch.outputId, 'trajectory')
  assert.equal(batch.timestampNs, 1000)
  assert.deepEqual(Array.from(batch.payload.f32!), [1.5, 2.5, 3.5])
  assert.deepEqual(batch.payload.json, [1.5, 2.5, 3.5])
})

test('frameToToolBatch passes a json object payload through', () => {
  const batch = frameToToolBatch({
    ...baseFrame,
    payload: { json: { success: true, message: 'ok' } },
  })
  assert.ok(batch)
  assert.deepEqual(batch.payload.json, { success: true, message: 'ok' })
  assert.equal(batch.payload.f32, undefined)
})

test('frameToToolBatch decodes bytes_base64 into raw bytes', () => {
  const batch = frameToToolBatch({
    ...baseFrame,
    payload: { bytes_base64: Buffer.from([0, 1, 255]).toString('base64') },
  })
  assert.ok(batch)
  assert.deepEqual(Array.from(batch.payload.bytes!), [0, 1, 255])
})

test('frameToToolBatch attaches sender metadata when present', () => {
  const batch = frameToToolBatch({
    ...baseFrame,
    payload: { values: [1, 2, 3, 4], metadata: { num_waypoints: 2, num_joints: 2 } },
  })
  assert.ok(batch)
  assert.deepEqual(batch.payload.metadata, { num_waypoints: 2, num_joints: 2 })
})

test('frameToToolBatch returns null when no payload channel is set', () => {
  assert.equal(frameToToolBatch({ ...baseFrame, payload: {} }), null)
  assert.equal(
    frameToToolBatch({ ...baseFrame, payload: { values: ['not', 'numbers'] as unknown as number[] } }),
    null,
  )
})

test('defaultSinceTs is two seconds behind the given wall clock', () => {
  assert.equal(defaultSinceTs(10_000_000_000), 8_000_000_000)
})

test('engine polls with since_ts, broadcasts converted frames, and advances since_ts', async () => {
  const responses: LiveFrame[][] = [
    [
      { ...baseFrame, timestamp: 1000, payload: { values: [1, 2] } },
      { ...baseFrame, output_id: 'plan_status', timestamp: 1200, payload: { json: { success: true } } },
    ],
    [
      { ...baseFrame, timestamp: 1300, payload: { values: [3, 4] } },
    ],
  ]
  const requestedSince: number[] = []
  const broadcastBatches: ToolBatch[] = []
  const engine = new LiveFeedEngine(
    async (sinceTs) => {
      requestedSince.push(sinceTs)
      return responses.shift() ?? []
    },
    (batch) => { broadcastBatches.push(batch) },
    50,
    900,
  )

  engine.start()
  await engine.poll()
  await engine.poll()
  engine.stop()

  assert.deepEqual(requestedSince, [900, 1200])
  assert.equal(broadcastBatches.length, 3)
  assert.equal(broadcastBatches[2].timestampNs, 1300)
})

test('engine skips a poll while another is in flight', async () => {
  let resolveFirst: (frames: LiveFrame[]) => void = () => {}
  const firstFetch = new Promise<LiveFrame[]>((resolve) => { resolveFirst = resolve })
  let fetchCalls = 0
  const engine = new LiveFeedEngine(
    async () => {
      fetchCalls += 1
      if (fetchCalls === 1) return firstFetch
      return []
    },
    () => {},
    50,
    0,
  )

  const first = engine.poll()
  const second = engine.poll()
  resolveFirst([{ ...baseFrame, timestamp: 100, payload: { values: [1] } }])
  await Promise.all([first, second])

  assert.equal(fetchCalls, 1)
})

test('engine reports error status when the fetch fails and recovers on success', async () => {
  let fail = true
  const engine = new LiveFeedEngine(
    async () => {
      if (fail) throw new Error('backend down')
      return [{ ...baseFrame, timestamp: 100, payload: { values: [1] } }]
    },
    () => {},
    50,
    0,
  )

  await engine.poll()
  assert.equal(engine.status, 'error')

  fail = false
  await engine.poll()
  assert.equal(engine.status, 'running')
})

test('engine stop cancels the polling interval', async () => {
  let fetchCalls = 0
  const engine = new LiveFeedEngine(
    async () => {
      fetchCalls += 1
      return []
    },
    () => {},
    10,
    0,
  )

  engine.start()
  await new Promise((resolve) => setTimeout(resolve, 45))
  engine.stop()
  const callsAfterStop = fetchCalls
  await new Promise((resolve) => setTimeout(resolve, 30))
  assert.equal(fetchCalls, callsAfterStop)
  assert.equal(engine.status, 'stopped')
})

test('engine applies tf payloads to a tree passed to broadcast', async () => {
  const tfFrame: LiveFrame = {
    node_id: 'tf_broadcaster',
    output_id: 'tf',
    timestamp: 100,
    payload: {
      json: {
        transforms: [
          {
            parent_frame: 'map',
            child_frame: 'odom',
            translation: [1, 0, 0],
            rotation: [0, 0, 0, 1],
          },
        ],
      },
    },
  }
  const broadcastTfs: unknown[] = []
  const engine = new LiveFeedEngine(
    async () => [tfFrame, { ...baseFrame, timestamp: 200, payload: { values: [1] } }],
    (_batch, tf) => { broadcastTfs.push(tf) },
    50,
    0,
  )

  await engine.poll()

  assert.equal(broadcastTfs.length, 2)
  assert.ok(broadcastTfs[0] !== undefined)
  assert.ok(broadcastTfs[1] !== undefined)
})

test('default poll interval is 50ms for the 20Hz physics mirror (M15 C4)', () => {
  const original = globalThis.setInterval
  let captured = 0
  globalThis.setInterval = ((_fn: () => void, ms?: number) => {
    captured = ms ?? 0
    return 1 as unknown as ReturnType<typeof setInterval>
  }) as typeof setInterval
  try {
    const engine = new LiveFeedEngine(async () => [], () => {})
    engine.start()
  } finally {
    globalThis.setInterval = original
  }
  assert.equal(captured, 50)
})
