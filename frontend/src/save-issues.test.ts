import { issuesToEdgeStyles, parseSaveError } from './save-issues'
import type { DataflowGraph } from './components/DataflowCanvas.vue'

const graph: DataflowGraph = {
  nodes: [
    { id: 'a', operatorId: 'a', runtime: 'python', inputs: {}, outputs: { frame: {} }, position: { x: 0, y: 0 } },
    { id: 'b', operatorId: 'b', runtime: 'python', inputs: { frame: {}, data: {} }, outputs: {}, position: { x: 1, y: 0 } },
    { id: 'c', operatorId: 'c', runtime: 'python', inputs: { frame: {} }, outputs: {}, position: { x: 2, y: 0 } },
  ],
  edges: [
    { id: 'e1', sourceNode: 'a', sourcePort: 'frame', targetNode: 'b', targetPort: 'frame' },
    { id: 'e2', sourceNode: 'a', sourcePort: 'frame', targetNode: 'c', targetPort: 'frame' },
  ],
}

// node-addressed issue hits ONLY the matching edge (b), not c
const nodeAddressed = issuesToEdgeStyles(
  [{ nodeId: 'b', portId: 'frame', message: 'bad' }],
  graph,
  true
)
if (Object.keys(nodeAddressed).length !== 1 || !nodeAddressed['e1']) throw new Error('node-addressed issue must match only e1')
if (nodeAddressed['e1'].color !== 'var(--accent-red)') throw new Error('blocking color must be red')

const warn = issuesToEdgeStyles([{ nodeId: 'b', portId: 'frame', message: 'w' }], graph, false)
if (warn['e1'].color !== 'var(--accent-yellow)') throw new Error('warning color must be yellow')

const portOnly = issuesToEdgeStyles([{ portId: 'frame', message: 'p' }], graph, false)
if (Object.keys(portOnly).length !== 2) throw new Error('port-only fallback matches both edges')

const parsed = parseSaveError('API request failed: 422 — {"ok":false,"path":"x","warnings":[],"errors":[{"nodeId":"b","message":"m"}]}')
if (!parsed || parsed.errors.length !== 1) throw new Error('envelope parse failed')
if (parseSaveError('plain failure text') !== null) throw new Error('non-envelope must return null')

console.log('save-issues tests passed')
