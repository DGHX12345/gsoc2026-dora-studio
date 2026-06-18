<template>
  <section class="explorer-layout">
    <aside class="panel sidebar-panel">
      <div class="panel-header">
        <h2>Dataflows</h2>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>
      <button
        v-for="flow in dataflows"
        :key="flow.id"
        :class="['flow-file', { active: selectedDataflowId === flow.id }]"
        @click="selectDataflow(flow.id)"
      >
        <strong>{{ flow.name }}</strong>
        <small>{{ flow.nodeCount }} nodes · {{ flow.edgeCount }} edges</small>
      </button>

      <div v-if="definition" class="diagnostics-box">
        <h3>Dataflow 文件</h3>
        <div class="diagnostic info">{{ definition.relativePath }}</div>
        <div class="diagnostic info">{{ definition.nodeCount }} 个节点 · {{ definition.edgeCount }} 条边</div>
      </div>

      <div class="diagnostics-box">
        <h3>诊断信息</h3>
        <div v-if="apiError" class="diagnostic warning">{{ apiError }}</div>
        <div v-for="item in diagnostics" :key="item.message" :class="['diagnostic', item.severity]">
          {{ item.message }}
        </div>
      </div>
    </aside>

    <article class="panel graph-panel">
      <div class="panel-header">
        <div>
          <h2>Dataflow 结构图</h2>
          <p>由 descriptor graph API 生成的图，用于确认交互和信息架构。</p>
        </div>
        <span class="pill running">{{ nodes.length }} 个节点 · {{ graphEdges.length }} 条边</span>
      </div>

      <div class="graph-canvas">
        <svg class="edge-layer" viewBox="0 0 1000 480" preserveAspectRatio="xMinYMin meet">
          <defs>
            <marker id="arrow" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto" markerUnits="strokeWidth">
              <path d="M0,0 L0,6 L9,3 z" fill="#7c8aa5" />
            </marker>
          </defs>
          <path v-for="edge in graphEdges" :key="edge.id" :d="edgePath(edge.from, edge.to)" marker-end="url(#arrow)" />
        </svg>

        <button
          v-for="node in nodes"
          :key="node.id"
          :class="['graph-node', node.status, { selected: selectedNode?.id === node.id }]"
          :style="{ left: `${node.x}px`, top: `${node.y}px` }"
          @click="selectedId = node.id"
        >
          <span>{{ node.kind }}</span>
          <strong>{{ node.label }}</strong>
          <small>{{ node.outputs.length }} 个输出 · {{ node.pending }} 条 pending</small>
        </button>
      </div>
    </article>

    <aside class="panel inspector-panel">
      <div class="panel-header">
        <h2>节点详情</h2>
        <span v-if="selectedNode" :class="['pill', selectedNode.status]">{{ selectedNode.status }}</span>
      </div>
      <template v-if="selectedNode">
        <div class="inspector-title">
          <strong>{{ selectedNode.label }}</strong>
          <span>{{ selectedNode.kind }}</span>
        </div>
        <p class="muted">{{ selectedNode.note }}</p>

        <div class="inspector-section">
          <h3>输入</h3>
          <code v-for="input in selectedNode.inputs" :key="input">{{ input }}</code>
          <p v-if="selectedNode.inputs.length === 0" class="muted">没有输入。</p>
        </div>

        <div class="inspector-section">
          <h3>输出</h3>
          <code v-for="output in selectedNode.outputs" :key="output">{{ output }}</code>
          <p v-if="selectedNode.outputs.length === 0" class="muted">没有输出。</p>
        </div>

        <div class="mini-metrics">
          <div><span>CPU</span><b>{{ selectedNode.cpu }}%</b></div>
          <div><span>内存</span><b>{{ selectedNode.memory }} MB</b></div>
          <div><span>重启</span><b>{{ selectedNode.restarts }}</b></div>
          <div><span>Pending</span><b>{{ selectedNode.pending }}</b></div>
        </div>
      </template>
      <p v-else class="muted">请选择一个节点。</p>
    </aside>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  getDataflowDefinition,
  getDataflowGraph,
  getDataflows,
  type ApiSource,
  type DataflowDefinitionResponse,
  type DataflowGraphResponse,
  type DataflowSummaryResponse,
} from '../api'
import {
  dataflowEdges,
  dataflowNodes,
  diagnostics as fallbackDiagnostics,
  type StudioEdge,
  type StudioNode,
} from '../data/mockStudio'

const fallbackDataflows: DataflowSummaryResponse[] = [
  {
    id: 'robot-perception-demo',
    name: 'robot-perception-demo.yml',
    status: 'running',
    nodeCount: dataflowNodes.length,
    edgeCount: dataflowEdges.length,
  },
]

const fallbackDefinition: DataflowDefinitionResponse = {
  id: 'robot-perception-demo',
  name: 'robot-perception-demo.yml',
  relativePath: 'examples/robot-perception-test/dataflow.yml',
  source: '',
  nodeCount: dataflowNodes.length,
  edgeCount: dataflowEdges.length,
  nodes: dataflowNodes.map((node) => ({
    id: node.id,
    path: null,
    inputs: node.inputs,
    outputs: node.outputs,
  })),
}

const fallbackGraph: DataflowGraphResponse = {
  nodes: dataflowNodes,
  edges: dataflowEdges,
  diagnostics: fallbackDiagnostics,
}

const dataflows = ref<DataflowSummaryResponse[]>(fallbackDataflows)
const definition = ref<DataflowDefinitionResponse | null>(fallbackDefinition)
const nodes = ref<StudioNode[]>(fallbackGraph.nodes)
const graphEdges = ref<StudioEdge[]>(fallbackGraph.edges)
const diagnostics = ref(fallbackGraph.diagnostics)
const selectedDataflowId = ref(fallbackDataflows[0].id)
const selectedId = ref('detector')
const apiSource = ref<ApiSource>('fallback')
const apiError = ref('')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))
const selectedNode = computed(() => nodes.value.find((node) => node.id === selectedId.value) ?? nodes.value[0] ?? null)

function edgePath(from: string, to: string) {
  const source = nodes.value.find((node) => node.id === from)
  const target = nodes.value.find((node) => node.id === to)

  if (!source || !target) return ''

  const nodeWidth = 220
  const nodeHeight = 104
  const startX = source.x + nodeWidth
  const startY = source.y + nodeHeight / 2
  const endX = target.x
  const endY = target.y + nodeHeight / 2
  const gap = Math.max(80, Math.abs(endX - startX) / 2)

  if (startX <= endX) {
    return `M ${startX} ${startY} C ${startX + gap} ${startY}, ${endX - gap} ${endY}, ${endX} ${endY}`
  }

  const loopX = Math.max(startX, endX) + 80
  return `M ${startX} ${startY} C ${loopX} ${startY}, ${loopX} ${endY}, ${endX} ${endY}`
}

async function loadDataflow(id: string) {
  selectedDataflowId.value = id

  const [definitionResult, graphResult] = await Promise.all([
    getDataflowDefinition(id, fallbackDefinition),
    getDataflowGraph(id, fallbackGraph),
  ])

  definition.value = definitionResult.data
  nodes.value = graphResult.data.nodes
  graphEdges.value = graphResult.data.edges
  diagnostics.value = graphResult.data.diagnostics
  selectedId.value = nodes.value[0]?.id ?? ''
  apiSource.value = definitionResult.source === 'connected' || graphResult.source === 'connected' ? 'connected' : 'fallback'
  apiError.value = definitionResult.error ?? graphResult.error ?? ''
}

async function selectDataflow(id: string) {
  await loadDataflow(id)
}

onMounted(async () => {
  const result = await getDataflows(fallbackDataflows)
  dataflows.value = result.data
  apiSource.value = result.source
  apiError.value = result.error ?? ''

  if (dataflows.value.length > 0) {
    await loadDataflow(dataflows.value[0].id)
  }
})
</script>
