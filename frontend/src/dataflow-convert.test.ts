import { definitionToGraph, graphToPayload, runtimeForPath } from './dataflow-convert'
import type { DataflowDefinitionResponse } from './api'
import type { DataflowGraph } from './components/DataflowCanvas.vue'

const def: DataflowDefinitionResponse = {
  id: 'abc', name: 'demo', relativePath: 'p/dataflow.yml', source: '',
  nodeCount: 2, edgeCount: 1, project: 'p',
  nodes: [
    {
      id: 'cam', path: 'cam.py',
      inputs: [],
      outputs: ['image'],
      inputTypes: {},
      outputTypes: { image: 'std/media/v1/Image' },
    },
    {
      id: 'sink', path: 'sink.py',
      inputs: ['image: cam/image'],
      outputs: [],
      inputTypes: { image: 'std/media/v1/Image' },
      outputTypes: {},
    },
  ],
}

function assertDefined<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('expected defined')
  return value
}

const graph = definitionToGraph(def)
const cam = assertDefined(graph.nodes.find(n => n.id === 'cam'))
const sink = assertDefined(graph.nodes.find(n => n.id === 'sink'))
if (cam.outputs.image?.type !== 'std/media/v1/Image') throw new Error('output URN lost')
if (sink.inputs.image?.type !== 'std/media/v1/Image') throw new Error('input URN lost')
if (cam.path !== 'cam.py') throw new Error('path lost')
if (graph.edges.length !== 1) throw new Error('edge not derived from source')
if (graph.edges[0].sourceNode !== 'cam' || graph.edges[0].targetPort !== 'image') throw new Error('edge endpoints wrong')

const payload = graphToPayload(graph)
if (payload.nodes.length !== 2 || payload.edges.length !== 1) throw new Error('payload roundtrip failed')
if (payload.nodes.find(n => n.id === 'cam')?.output_types?.image !== 'std/media/v1/Image') throw new Error('output_types not emitted')
if (payload.type_rules.length !== 0) throw new Error('type_rules default empty')

if (runtimeForPath('a.cpp') !== 'cpp') throw new Error('cpp runtime must serialize as cpp')
if (runtimeForPath('a.py') !== 'python' || runtimeForPath('a.rs') !== 'rust') throw new Error('runtime mapping wrong')

const typed = { ...def, nodes: def.nodes.map((n, i) => ({ ...n, id: i === 0 ? 'cam' : 'sink' })) }
const g2 = definitionToGraph(typed)
if (g2.nodes.length !== 2) throw new Error('reconvert failed')

console.log('dataflow-convert tests passed')
