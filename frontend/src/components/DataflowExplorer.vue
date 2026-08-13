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

      <details class="diagnostics-box collapsible" open>
        <summary><h3>File Info</h3></summary>
        <div v-if="definition" class="diagnostic info">{{ definition.relativePath }}</div>
        <div v-if="definition" class="diagnostic info">{{ definition.nodeCount }} nodes &middot; {{ definition.edgeCount }} edges</div>
      </details>
    </aside>

    <article class="panel graph-panel">
      <div class="panel-header">
        <div>
          <div class="view-tabs">
            <button :class="['view-tab', { active: viewMode === 'source' }]" @click="viewMode = 'source'">Source</button>
            <button :class="['view-tab', { active: viewMode === 'build' }]" @click="viewMode = 'build'">Build</button>
          </div>
          <p v-if="viewMode === 'source'">Raw YAML from {{ definition?.relativePath ?? 'dataflow descriptor' }}</p>
          <p v-else>Build dataflows visually — drag nodes, connect ports, generate YAML</p>
        </div>
        <span class="pill">{{ definition?.source?.split('\n').length ?? 0 }} lines</span>
      </div>

<div v-if="viewMode === 'build'" class="build-layout">
        <NodePalette @drag-start="onPaletteDrag" />
        <div class="build-canvas-wrap">
          <div class="build-toolbar">
            <button class="build-tb-btn" @click="buildYaml" title="Generate YAML">Generate YAML</button>
            <span class="build-tb-sep"></span>
            <button class="build-tb-btn secondary" @click="validateBuild" title="Validate graph">Validate</button>
            <span class="build-tb-sep"></span>
            <button class="build-tb-btn secondary" @click="clearBuild">Clear</button>
            <span class="build-tb-sep"></span>
            <button
              class="build-tb-btn run"
              :disabled="!builtYaml || runState === 'running'"
              @click="doRun"
            >{{ runState === 'running' ? 'Running...' : 'Run' }}</button>
            <button
              v-if="runState === 'running'"
              class="build-tb-btn stop"
              @click="doStop"
            >Stop</button>
            <span class="build-tb-spacer"></span>
            <span class="build-tb-status" :class="{ valid: buildValid, invalid: !buildValid && buildChecked, running: runState === 'running' }">{{ runState === 'running' ? 'Dataflow running' : buildStatus }}</span>
          </div>
          <DataflowCanvas
            :graph="buildGraph"
            :selected-node="selectedBuildNode"
            :selected-edge="selectedBuildEdge"
            :edge-styles="edgeStyles"
            :dataflow-id="runState === 'running' ? 'studio-dataflow' : undefined"
            @update:graph="onBuildGraphUpdate"
            @select-node="selectedBuildNode = $event"
            @select-edge="selectedBuildEdge = $event"
          />
          <div class="build-statusbar">
            <span>{{ buildGraph.nodes.length }} nodes</span>
            <span>{{ buildGraph.edges.length }} edges</span>
            <span class="build-zoom">Click + drag to pan &middot; Scroll to zoom</span>
          </div>
        </div>
      </div>

      <div v-else class="source-viewer">
        <pre><code>{{ definition?.source ?? 'No source available.' }}</code></pre>
      </div>
    </article>

  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  getDataflowDefinition,
  getDataflows,
  buildDataflow,
  validateDataflow,
  checkSchema,
  runDataflow,
  stopRuntime,
  getRuntimeStatus,
  type ApiSource,
  type DataflowDefinitionResponse,
  type DataflowSummaryResponse,
  type DataflowGraph,
} from '../api'
import DataflowCanvas from './DataflowCanvas.vue'
import NodePalette from './NodePalette.vue'
import type { DataflowGraph as CanvasGraph } from './DataflowCanvas.vue'

const emptyDataflows: DataflowSummaryResponse[] = []
const emptyDefinition: DataflowDefinitionResponse = {
  id: '', name: '', relativePath: '', source: '', nodeCount: 0, edgeCount: 0, nodes: [],
}

const dataflows = ref<DataflowSummaryResponse[]>([])
const definition = ref<DataflowDefinitionResponse | null>(null)
const selectedDataflowId = ref('')
const viewMode = ref<'source' | 'build'>('build')
const apiSource = ref<ApiSource>('fallback')
const apiError = ref('')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Backend unavailable'))

// --- Build mode state ---
const buildGraph = ref<CanvasGraph>({ nodes: [], edges: [] })
const selectedBuildNode = ref<string | null>(null)
const selectedBuildEdge = ref<string | null>(null)
const buildStatus = ref('New graph')
const buildValid = ref(false)
const buildChecked = ref(false)
const generatedYaml = ref('')

function onPaletteDrag(_entry: unknown) { /* visual feedback handled by browser */ }

function onBuildGraphUpdate(graph: CanvasGraph) {
  buildGraph.value = graph
  buildChecked.value = false
}

const builtYaml = ref('')
const runState = ref<'running' | 'stopped' | 'failed'>('stopped')

async function buildYaml() {
  const g: DataflowGraph = {
    nodes: buildGraph.value.nodes.map(n => ({
      id: n.id, operator_id: n.operatorId, runtime: n.runtime,
      inputs: n.inputs, outputs: n.outputs, position: n.position,
    })),
    edges: buildGraph.value.edges.map(e => ({
      id: e.id, source_node: e.sourceNode, source_port: e.sourcePort,
      target_node: e.targetNode, target_port: e.targetPort,
    })),
  }
  try {
    const result = await buildDataflow(g)
    generatedYaml.value = result.yaml
    builtYaml.value = result.yaml
    buildStatus.value = `${result.node_count} nodes, ${result.edge_count} edges — YAML generated`
    buildValid.value = true; buildChecked.value = true
  } catch {
    buildStatus.value = 'Build failed — check backend'
    buildValid.value = false; buildChecked.value = true
  }
}

async function doRun() {
  if (!builtYaml.value) return
  try {
    runState.value = 'running'
    buildStatus.value = 'Starting dataflow...'
    const result = await runDataflow(builtYaml.value, 'studio-dataflow')
    runState.value = result.status === 'running' ? 'running' : 'failed'
    buildStatus.value = result.status === 'running' ? 'Dataflow running' : `Start failed: ${result.lastMessage}`
  } catch (e) {
    runState.value = 'failed'
    buildStatus.value = `Start failed: ${e instanceof Error ? e.message : 'Unknown error'}`
  }
}

async function doStop() {
  try {
    await stopRuntime()
    runState.value = 'stopped'
    buildStatus.value = 'Dataflow stopped'
  } catch {
    // Ignore stop errors
    runState.value = 'stopped'
    buildStatus.value = 'Dataflow stopped'
  }
}

async function validateBuild() {
  const g: DataflowGraph = {
    nodes: buildGraph.value.nodes.map(n => ({
      id: n.id, operator_id: n.operatorId, runtime: n.runtime,
      inputs: n.inputs, outputs: n.outputs, position: n.position,
    })),
    edges: buildGraph.value.edges.map(e => ({
      id: e.id, source_node: e.sourceNode, source_port: e.sourcePort,
      target_node: e.targetNode, target_port: e.targetPort,
    })),
  }
  try {
    const result = await validateDataflow(g)
    buildValid.value = result.valid; buildChecked.value = true
    buildStatus.value = result.valid ? 'Valid' : result.errors.join('; ')
  } catch {
    buildStatus.value = 'Validation failed — check backend'
    buildValid.value = false; buildChecked.value = true
  }
}

// --- Schema checking ---
const edgeStyles = ref<Record<string, { color: string; tooltip: string }>>({})
const schemaChecking = ref(false)

async function checkAllEdges() {
  schemaChecking.value = true
  const styles: Record<string, { color: string; tooltip: string }> = {}
  for (const edge of buildGraph.value.edges) {
    const srcNode = buildGraph.value.nodes.find(n => n.id === edge.sourceNode)
    const tgtNode = buildGraph.value.nodes.find(n => n.id === edge.targetNode)
    if (!srcNode || !tgtNode) continue
    try {
      const resp = await checkSchema({
        source_operator: srcNode.operatorId, source_port: edge.sourcePort,
        sink_operator: tgtNode.operatorId, sink_port: edge.targetPort,
      })
      styles[edge.id] = {
        color: resp.level === 'incompatible' ? 'var(--accent-red)' : resp.level === 'warning' ? 'var(--accent-yellow)' : resp.level === 'unknown' ? 'var(--text-muted-dark)' : 'var(--accent-green)',
        tooltip: resp.detail,
      }
    } catch {
      styles[edge.id] = { color: 'var(--text-muted-dark)', tooltip: 'Schema check unavailable' }
    }
  }
  edgeStyles.value = styles
  schemaChecking.value = false
}

// Re-check schema when edges change
watch(() => buildGraph.value.edges.length, () => { checkAllEdges() })

function clearBuild() {
  buildGraph.value = { nodes: [], edges: [] }
  selectedBuildNode.value = null; selectedBuildEdge.value = null
  buildStatus.value = 'New graph'; buildValid.value = false; buildChecked.value = false
  generatedYaml.value = ''
}

async function loadDataflow(id: string) {
  selectedDataflowId.value = id
  const result = await getDataflowDefinition(id, emptyDefinition)
  definition.value = result.source === 'connected' ? result.data : null
  apiSource.value = result.source
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
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
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

/* ── Build mode layout (M01) ── */
.build-layout {
  display: flex; flex: 1; min-height: 0;
}
.build-canvas-wrap {
  flex: 1; display: flex; flex-direction: column;
  min-width: 0; background: var(--canvas-base);
}
.build-toolbar {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px;
  background: var(--panel-surface);
  border-bottom: 1px solid var(--hairline);
  flex-shrink: 0;
}
.build-tb-btn {
  padding: 6px 14px; border-radius: 6px; font-size: 12px; font-weight: 510;
  background: rgba(0, 212, 255, 0.12); color: var(--accent-cyan);
  border: 1px solid rgba(0, 212, 255, 0.2); cursor: pointer;
  transition: background 120ms ease;
}
.build-tb-btn:hover { background: rgba(0, 212, 255, 0.2); }
.build-tb-btn.secondary { background: var(--card-surface); color: var(--text-body); border-color: var(--hairline); }
.build-tb-btn.secondary:hover { background: var(--card-hover); }
.build-tb-sep { width: 1px; height: 16px; background: var(--hairline); }
.build-tb-spacer { flex: 1; }
.build-tb-status { font-size: 11px; color: var(--text-muted-dark); }
.build-tb-status.valid { color: var(--accent-green); }
.build-tb-status.invalid { color: var(--accent-red); }
.build-statusbar {
  display: flex; align-items: center; gap: 16px;
  padding: 6px 14px; font-size: 11px; color: var(--text-muted-dark);
  background: var(--panel-surface); border-top: 1px solid var(--hairline);
  flex-shrink: 0;
}
.build-zoom { margin-left: auto; }
.graph-view-canvas { flex: 1; min-height: 0; }
.graph-view-canvas :deep(.canvas-wrap) { position: absolute; inset: 0; }

</style>
