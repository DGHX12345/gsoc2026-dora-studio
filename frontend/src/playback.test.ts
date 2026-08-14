// PlaybackEngine tests — self-executing node:test module.
// play() schedules via requestAnimationFrame, which node lacks — stub it so
// the synchronous first tick can run without throwing.

import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { PlaybackEngine } from './playback'

let rafCounter = 0
;(globalThis as unknown as { requestAnimationFrame: (cb: () => void) => number }).requestAnimationFrame = () => ++rafCounter
;(globalThis as unknown as { cancelAnimationFrame: (id: number) => void }).cancelAnimationFrame = () => {}

test('seek and stop notify all onTick listeners', () => {
  const engine = new PlaybackEngine()
  engine.duration = 10_000_000_000
  const seenA: number[] = []
  const seenB: number[] = []
  engine.onTick((t) => seenA.push(t))
  engine.onTick((t) => seenB.push(t))

  engine.seek(500_000_000, true)
  engine.stop()

  assert.deepEqual(seenA, [500_000_000, 0])
  assert.deepEqual(seenB, [500_000_000, 0])
})

test('play delivers its first tick to all onTick listeners', () => {
  const engine = new PlaybackEngine()
  engine.duration = 10_000_000_000
  let count = 0
  engine.onTick(() => { count += 1 })
  engine.onTick(() => { count += 1 })

  engine.play()
  engine.pause()

  assert.equal(count, 2)
})
