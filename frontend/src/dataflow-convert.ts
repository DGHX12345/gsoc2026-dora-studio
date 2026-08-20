import type { DataflowDefinitionResponse, TypeRule } from './api'
import type { DataflowGraph, NodeSpec, EdgeSpec, PortSpec } from './components/DataflowCanvas.vue'

function portSpec(urn?: string) {
  return urn ? { type: urn } : {}
}

/** Convert a backend dataflow definition into the canvas graph model. */
export function definitionToGraph(def: DataflowDefinitionResponse): DataflowGraph {
  const nodes: NodeSpec[] = def.nodes.map((node, index) => {
    const inputs: Record<string, PortSpec> = {}
    for (const entry of node.inputs) {
      const [name, source] = entry.split(': ')
      // Sources that are not graph nodes (e.g. `dora/timer/millis/500` or a
      // bare external source with no "/") have no edge to carry them, so the
      // raw source is stored on the input port. Node-to-node sources keep the
      // port clean — the edge carries the connection.
      const from = source?.split('/')[0]
      const external = source !== undefined && !def.nodes.some(n => n.id === from)
      inputs[name] = external
        ? { ...portSpec(node.inputTypes?.[name]), source }
        : portSpec(node.inputTypes?.[name])
    }
    const outputs: Record<string, { type?: string }> = {}
    for (const name of node.outputs) {
      outputs[name] = portSpec(node.outputTypes?.[name])
    }
    return {
      id: node.id,
      operatorId: node.path ?? node.id,
      runtime: runtimeForPath(node.path ?? undefined),
      path: node.path ?? undefined,
      inputs,
      outputs,
      position: { x: 80 + index * 240, y: 80 + (index % 3) * 180 },
    }
  })
  const edges: EdgeSpec[] = []
  let edgeIndex = 0
  for (const node of def.nodes) {
    for (const entry of node.inputs) {
      if (!entry.includes(': ')) continue
      const [name, source] = entry.split(': ')
      const [from, output] = source.split('/')
      if (nodes.some(n => n.id === from)) {
        edgeIndex += 1
        edges.push({
          id: `e${edgeIndex}`,
          sourceNode: from,
          sourcePort: output ?? name,
          targetNode: node.id,
          targetPort: name,
        })
      }
    }
  }
  return { nodes, edges }
}

export function runtimeForPath(path?: string): string {
  const ext = path?.split('.').pop()
  return { py: 'python', rs: 'rust', cpp: 'cpp', cc: 'cpp', cxx: 'cpp', c: 'c' }[ext ?? ''] ?? 'python'
}

export type BuilderGraphPayload = {
  nodes: Array<{
    id: string; operator_id: string; runtime: string; path?: string
    inputs: Record<string, PortSpec>; outputs: Record<string, PortSpec>
    input_types: Record<string, string>; output_types: Record<string, string>
    position: { x: number; y: number }
  }>
  edges: Array<{ id: string; source_node: string; source_port: string; target_node: string; target_port: string }>
  type_rules: TypeRule[]
}

/** Convert the canvas graph into the backend save/build payload. */
export function graphToPayload(graph: DataflowGraph, typeRules: TypeRule[] = []): BuilderGraphPayload {
  return {
    nodes: graph.nodes.map(node => ({
      id: node.id,
      operator_id: node.operatorId,
      runtime: node.runtime,
      path: node.path,
      inputs: node.inputs,
      outputs: node.outputs,
      input_types: Object.fromEntries(
        Object.entries(node.inputs).filter(([, port]) => port.type).map(([name, port]) => [name, port.type as string])
      ),
      output_types: Object.fromEntries(
        Object.entries(node.outputs).filter(([, port]) => port.type).map(([name, port]) => [name, port.type as string])
      ),
      position: node.position,
    })),
    edges: graph.edges.map(edge => ({
      id: edge.id,
      source_node: edge.sourceNode,
      source_port: edge.sourcePort,
      target_node: edge.targetNode,
      target_port: edge.targetPort,
    })),
    type_rules: typeRules,
  }
}
