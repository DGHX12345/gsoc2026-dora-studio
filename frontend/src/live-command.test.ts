// Live command helpers (M15 B6) — self-executing node:test module.

import { strict as assert } from 'node:assert'
import { test } from 'node:test'

import type { LiveFrame } from './api'
import {
  buildExecuteCommand,
  buildPlanCommand,
  buildSceneAddCommand,
  buildSceneRemoveCommand,
  extractConsoleStatus,
  parseTargetInputs,
} from './live-command'

test('buildPlanCommand carries kind and target', () => {
  assert.deepEqual(buildPlanCommand([0.5, 0.2, 0.3], 'simple_planner'), {
    kind: 'plan',
    planner: 'simple_planner',
    target: [0.5, 0.2, 0.3],
  })
})

test('buildExecuteCommand covers execute, stop and auto', () => {
  assert.deepEqual(buildExecuteCommand('execute'), { kind: 'execute' })
  assert.deepEqual(buildExecuteCommand('stop'), { kind: 'stop' })
  assert.deepEqual(buildExecuteCommand('auto'), { kind: 'auto' })
})

test('parseTargetInputs parses valid triples and rejects garbage', () => {
  assert.deepEqual(parseTargetInputs('0.5', '-0.2', '0.3'), [0.5, -0.2, 0.3])
  assert.equal(parseTargetInputs('0.5', 'abc', '0.3'), null)
  assert.equal(parseTargetInputs('', '0.2', '0.3'), null)
  assert.equal(parseTargetInputs('0.5', '0.2', ''), null)
})

test('buildSceneAddCommand builds the scene object payload', () => {
  assert.deepEqual(
    buildSceneAddCommand('my_box', 'box', [0.6, 0.4, 0.15], [0.1, 0.1, 0.3]),
    {
      kind: 'scene',
      action: 'add',
      object: {
        name: 'my_box',
        type: 'box',
        position: [0.6, 0.4, 0.15],
        dimensions: [0.1, 0.1, 0.3],
      },
    },
  )
})

test('buildSceneRemoveCommand carries the object name', () => {
  assert.deepEqual(buildSceneRemoveCommand('my_box'), {
    kind: 'scene',
    action: 'remove',
    object: { name: 'my_box' },
  })
})

test('extractConsoleStatus picks the latest plan and execution statuses', () => {
  const frames: LiveFrame[] = [
    {
      node_id: 'simple_planner',
      output_id: 'plan_status',
      timestamp: 100,
      payload: { json: { success: true, plan_id: 1 } },
    },
    {
      node_id: 'trajectory_executor',
      output_id: 'execution_status',
      timestamp: 200,
      payload: { json: { is_executing: true, progress: 0.5 } },
    },
    {
      node_id: 'simple_planner',
      output_id: 'plan_status',
      timestamp: 300,
      payload: { json: { success: false, plan_id: 2 } },
    },
    {
      node_id: 'other',
      output_id: 'noise',
      timestamp: 400,
      payload: { values: [1, 2, 3] },
    },
  ]
  const status = extractConsoleStatus(frames)
  assert.deepEqual(status.planStatus, { success: false, plan_id: 2 })
  assert.deepEqual(status.execution, { is_executing: true, progress: 0.5 })
})

test('extractConsoleStatus returns nulls for an empty feed', () => {
  const status = extractConsoleStatus([])
  assert.equal(status.planStatus, null)
  assert.equal(status.execution, null)
})

test('extractConsoleStatus picks the latest joint_positions', () => {
  const frames: LiveFrame[] = [
    {
      node_id: 'trajectory_executor',
      output_id: 'joint_positions',
      timestamp: 100,
      payload: { values: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.03] },
    },
    {
      node_id: 'trajectory_executor',
      output_id: 'joint_positions',
      timestamp: 200,
      payload: { values: [1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 0.02] },
    },
  ]
  const status = extractConsoleStatus(frames)
  assert.deepEqual(status.joints, [1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 0.02])
})

