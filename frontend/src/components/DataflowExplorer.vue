<template>
  <section class="explorer-layout">
    <aside class="panel sidebar-panel">
      <div class="panel-header">
        <h2>Dataflows</h2>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>

      <div v-if="apiError" class="empty-state">{{ apiError }}</div>

      <div v-else-if="dataflows.length === 0" class="empty-state">
        No dataflow YAML files found under examples/.
      </div>

      <button
        v-for="flow in dataflows"
        :key="flow.id"
        :class="['flow-file', { active: selectedDataflowId === flow.id }]"
        @click="selectDataflow(flow.id)"
      >
        <span class="flow-name-row">
          <strong>{{ flow.name }}</strong>
          <span :class="['status-chip', flow.status]">{{ flow.status }}</span>
        </span>
        <small>{{ flow.nodeCount }} nodes &middot; {{ flow.edgeCount }} edges</small>
      </button>

      <div v-if="definition" class="diagnostics-box">
        <h3>Dataflow File</h3>
        <div class="diagnostic info">{{ definition.relativePath }}</div>
        <div class="diagnostic info">{{ definition.nodeCount }} nodes &middot; {{ definition.edgeCount }} edges</div>
      </div>

      <div class="diagnostics-box">
        <h3>Parser Diagnostics</h3>
        <div v-for="item in diagnostics" :key="item.message" :class="['diagnostic', item.severity]">
          {{ item.message }}
        </div>
        <div v-if="diagnostics.length === 0 && definition" class="diagnostic info">No issues detected.</div>
      </div>
    </aside>

    <article class="panel graph-panel">
      <div class="panel-header">
        <div>
          <div class="view-tabs">
            <button :class="['view-tab', { active: viewMode === 'graph' }]" @click="viewMode = 'graph'">Graph</button>
            <button :class="['view-tab', { active: viewMode === 'source' }]" @click="viewMode = 'source'">Source</button>
          </div>
          <p v-if="viewMode === 'graph'">Generated from {{ definition?.relativePath ?? 'dataflow descriptor' }}</p>
          <p v-else>Raw YAML from {{ definition?.relativePath ?? 'dataflow descriptor' }}</p>
        </div>
        <span class="pill" v-if="viewMode === 'graph'">{{ nodes.length }} nodes &middot; {{ graphEdges.length }} edges</span>
        <span class="pill" v-else>{{ definition?.source?.split('\n').length ?? 0 }} lines</span>
      </div>

      <template v-if="viewMode === 'graph'">
        <div v-if="nodes.length === 0 && apiSource === 'connected'" class="empty-state" style="min-height: 400px;">
          Select a dataflow to view its graph.
        </div>

        <div v-else class="graph-canvas">
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
            <small>{{ node.outputs.length }} outputs &middot; {{ node.inputs.length }} inputs</small>
          </button>
        </div>
      </template>

      <div v-else class="source-viewer">
        <pre><code>{{ definition?.source ?? 'No source available.' }}</code></pre>
      </div>
    </article>

    <aside class="panel inspector-panel">
      <div class="panel-header">
        <h2>Node Detail</h2>
        <span v-if="selectedNode" :class="['pill', selectedNode.status]">{{ selectedNode.status }}</span>
      </div>
      <template v-if="selectedNode">
        <div class="inspector-title">
          <strong>{{ selectedNode.label }}</strong>
          <span>{{ selectedNode.kind }}</span>
        </div>
        <p class="muted">{{ selectedNode.note }}</p>

        <div class="inspector-section">
          <h3>Inputs</h3>
          <code v-for="input in selectedNode.inputs" :key="input">{{ input }}</code>
          <p v-if="selectedNode.inputs.length === 0" class="muted">No inputs.</p>
        </div>

        <div class="inspector-section">
          <h3>Outputs</h3>
          <code v-for="output in selectedNode.outputs" :key="output">{{ output }}</code>
          <p v-if="selectedNode.outputs.length === 0" class="muted">No outputs.</p>
        </div>

        <div class="inspector-section">
          <h3>Runtime Metrics</h3>
          <p class="muted">Real-time CPU, memory, and message metrics are not yet available from the dora runtime.</p>
        </div>
      </template>
      <p v-else class="muted">Select a node to inspect.</p>
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
import type { StudioEdge, StudioNode } from '../data/mockStudio'

const emptyDataflows: DataflowSummaryResponse[] = []
const emptyDefinition: DataflowDefinitionResponse = {
  id: '', name: '', relativePath: '', source: '', nodeCount: 0, edgeCount: 0, nodes: [],
}
const emptyGraph: DataflowGraphResponse = { nodes: [], edges: [], diagnostics: [] }

const dataflows = ref<DataflowSummaryResponse[]>([])
const definition = ref<DataflowDefinitionResponse | null>(null)
const nodes = ref<StudioNode[]>([])
const graphEdges = ref<StudioEdge[]>([])
const diagnostics = ref<{ severity: string; message: string }[]>([])
const selectedDataflowId = ref('')
const selectedId = ref('')
const viewMode = ref<'graph' | 'source'>('graph')
const apiSource = ref<ApiSource>('fallback')
const apiError = ref('')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Backend unavailable'))
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
    getDataflowDefinition(id, emptyDefinition),
    getDataflowGraph(id, emptyGraph),
  ])

  definition.value = definitionResult.source === 'connected' ? definitionResult.data : null
  nodes.value = graphResult.data.nodes
  graphEdges.value = graphResult.data.edges
  diagnostics.value = graphResult.data.diagnostics
  selectedId.value = nodes.value[0]?.id ?? ''
  apiSource.value = definitionResult.source === 'connected' || graphResult.source === 'connected' ? 'connected' : 'fallback'
}

async function selectDataflow(id: string) {
  await loadDataflow(id)
}

onMounted(async () => {
  const result = await getDataflows(emptyDataflows)
  dataflows.value = result.data
  apiSource.value = result.source
  apiError.value = result.source === 'fallback' ? (result.error ?? 'Backend API is unavailable.') : ''

  if (dataflows.value.length > 0) {
    await loadDataflow(dataflows.value[0].id)
  }
})
</script>

<style scoped>
.graph-panel {
  min-width: 0;
  overflow: auto;
}

.graph-canvas {
  min-width: 760px;
}

.empty-state {
  color: #94a3b8;
  font-size: 14px;
  line-height: 1.6;
  padding: 20px 0;
  text-align: center;
}

[data-theme="dark"] .empty-state {
  color: #64748b;
}

.flow-name-row {
  align-items: center;
  display: flex;
  gap: 6px;
  min-width: 0;
  width: 100%;
}

.flow-name-row strong {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.flow-name-row .status-chip {
  flex-shrink: 0;
}

.view-tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 6px;
}

.view-tab {
  background: #f3f6fb;
  border-radius: 10px;
  color: #64748b;
  font-size: 14px;
  font-weight: 700;
  padding: 8px 16px;
}

.view-tab.active {
  background: #2457ff;
  color: white;
}

[data-theme="dark"] .view-tab {
  background: #334155;
  color: #94a3b8;
}

[data-theme="dark"] .view-tab.active {
  background: #2563eb;
  color: white;
}

.source-viewer {
  background: #101827;
  border: 1px solid #1f2937;
  border-radius: 14px;
  flex: 1;
  overflow: auto;
  min-height: 400px;
}

.source-viewer pre {
  color: #dbeafe;
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 14px;
  line-height: 1.6;
  margin: 0;
  padding: 20px;
  white-space: pre-wrap;
  word-break: break-all;
}

[data-theme="dark"] .source-viewer {
  background: #0c1525;
  border-color: #1f2937;
}
</style>
