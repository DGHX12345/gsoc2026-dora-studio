<template>
  <section class="explorer-layout">
    <aside class="panel sidebar-panel">
      <div class="panel-header">
        <h2>{{ t.explorer.projects }}</h2>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>

      <div v-if="apiError" class="empty-state">{{ apiError }}</div>

      <div v-else-if="projects.length === 0" class="empty-state">
        No dataflow YAML files found under examples/.
      </div>

      <button class="sidebar-add-project" @click="addProject" title="Scan an additional project directory for dataflows and nodes">
        {{ t.explorer.addProjectDir }}
      </button>

      <div class="flows-scroll">
        <div class="project-groups-label">{{ t.explorer.dataflows }}</div>
        <div v-for="project in projects" :key="project.path" class="project-group">
          <div class="project-group-header">
            <span class="project-group-name">{{ project.name }}</span>
            <span v-if="project.builtin" class="project-group-builtin">builtin</span>
            <span class="project-group-count">{{ project.dataflows.length }}</span>
            <button
              v-if="!project.builtin"
              class="project-group-remove"
              :title="t.explorer.removeProjectConfirm"
              @click="removeProject(project)"
            >✕</button>
          </div>
          <div v-if="!project.builtin && project.dataflows.length === 0" class="project-group-missing">
            {{ t.explorer.projectMissing }}
          </div>
          <button
            v-for="flow in project.dataflows"
            :key="flow.id"
            :class="['flow-file', { active: selectedDataflowId === flow.id }]"
            :title="flow.name"
            @click="selectDataflow(flow.id)"
          >
            <span class="flow-name-row">
              <strong>{{ flow.name }}</strong>
              <span :class="['status-chip', flow.status]">{{ flow.status }}</span>
            </span>
            <small>{{ flow.nodeCount }} nodes &middot; {{ flow.edgeCount }} edges</small>
          </button>
        </div>
      </div>

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
          <div v-if="viewMode === 'source'" class="view-tabs source-subtabs">
            <button :class="['view-tab', { active: sourceSubView === 'canvas' }]" @click="sourceSubView = 'canvas'">{{ t.explorer.canvas }}</button>
            <button :class="['view-tab', { active: sourceSubView === 'text' }]" @click="sourceSubView = 'text'">{{ t.explorer.text }}</button>
          </div>
          <p v-if="viewMode === 'source'">Raw YAML from {{ definition?.relativePath ?? 'dataflow descriptor' }}</p>
          <p v-else>Build dataflows visually — drag nodes, connect ports, generate YAML</p>
        </div>
        <span class="pill">{{ definition?.source?.split('\n').length ?? 0 }} lines</span>
      </div>

      <div v-if="viewMode === 'build'" class="build-layout">
        <NodePalette :entries="paletteEntries" @drag-start="onPaletteDrag" @add-manual="addManualNode" />
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

      <template v-else-if="viewMode === 'source'">
        <div v-if="sourceSubView === 'canvas' && definition" class="build-layout">
          <NodePalette :entries="paletteEntries" @drag-start="onPaletteDrag" @add-manual="addManualNode" />
          <div class="build-canvas-wrap">
            <div class="build-toolbar">
              <button class="build-tb-btn" @click="saveCurrent" :title="t.explorer.edgeStatusHint">{{ t.explorer.save }}</button>
              <button class="build-tb-btn secondary" @click="saveAsCurrent" title="Generate a new dataflow YAML at an absolute path">{{ t.explorer.saveAs }}</button>
              <span class="build-tb-sep"></span>
              <span class="build-tb-status">{{ saveStatus || t.explorer.edgeStatusHint }}</span>
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
          <aside class="source-side-panel">
            <div v-if="selectedBuildEdge && edgeStyles[selectedBuildEdge]" class="edge-props">
              <div class="edge-props-header">{{ t.explorer.connection }}</div>
              <div class="edge-props-reason">{{ edgeStyles[selectedBuildEdge].tooltip }}</div>
              <button
                v-if="edgeStyles[selectedBuildEdge].color === 'var(--accent-red)'"
                class="edge-props-btn"
                @click="createRuleForSelectedEdge"
              >{{ t.explorer.declareRule }}</button>
            </div>
            <PortTypePanel :node="portPanelNode" @update-port="onPortUpdate" />
            <TypeRulesPanel :rules="typeRules" @update:rules="onTypeRulesUpdate" />
          </aside>
        </div>
        <div v-else class="source-fallback">
          <div v-if="selectedDataflowId && !definition" class="parse-note">
            {{ t.explorer.unparseable }}
          </div>
          <div class="source-viewer">
            <pre><code>{{ definition?.source ?? 'No source available.' }}</code></pre>
          </div>
        </div>
      </template>
    </article>

  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  getDataflowDefinition,
  getProjects,
  getPalette,
  addProjectDir,
  deleteProjectDir,
  submitManualNode,
  buildDataflow,
  validateDataflow,
  checkSchemaUrn,
  saveDataflow,
  saveDataflowAs,
  runDataflow,
  stopRuntime,
  type ApiSource,
  type DataflowDefinitionResponse,
  type DataflowGraph,
  type ProjectSummaryResponse,
  type PaletteEntry,
  type PalettePort,
  type ManualNodeSpec,
  type TypeRule,
  type SaveIssue,
} from '../api'
import DataflowCanvas from './DataflowCanvas.vue'
import NodePalette from './NodePalette.vue'
import PortTypePanel from './PortTypePanel.vue'
import TypeRulesPanel from './TypeRulesPanel.vue'
import { definitionToGraph, graphToPayload } from '../dataflow-convert'
import { edgeLevel, edgeColor, buildRulePatch } from '../edge-status'
import { issuesToEdgeStyles, parseSaveError } from '../save-issues'
import { useI18n } from '../i18n'
import type { DataflowGraph as CanvasGraph } from './DataflowCanvas.vue'

const { t } = useI18n()

const emptyDefinition: DataflowDefinitionResponse = {
  id: '', name: '', relativePath: '', source: '', nodeCount: 0, edgeCount: 0, nodes: [],
}

const projects = ref<ProjectSummaryResponse[]>([])
const paletteEntries = ref<PaletteEntry[]>([])
const typeRules = ref<TypeRule[]>([])
const editingDefinition = ref<DataflowDefinitionResponse | null>(null)
const sourceSubView = ref<'canvas' | 'text'>('canvas')
const saveStatus = ref('')

const definition = ref<DataflowDefinitionResponse | null>(null)
const selectedDataflowId = ref('')
const viewMode = ref<'source' | 'build'>('build')
const apiSource = ref<ApiSource>('fallback')
const apiError = ref('')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? t.value.explorer.apiConnected : t.value.explorer.backendUnavailable))

// --- Canvas graph state (shared by Build mode and the Source canvas editor) ---
const buildGraph = ref<CanvasGraph>({ nodes: [], edges: [] })
const selectedBuildNode = ref<string | null>(null)
const selectedBuildEdge = ref<string | null>(null)
const buildStatus = ref('New graph')
const buildValid = ref(false)
const buildChecked = ref(false)
const generatedYaml = ref('')
const builtYaml = ref('')
const runState = ref<'running' | 'stopped' | 'failed'>('stopped')

const portPanelNode = computed(() => buildGraph.value.nodes.find(n => n.id === selectedBuildNode.value) ?? null)

function onPaletteDrag(_entry: PaletteEntry) { /* visual feedback handled by browser */ }

function onBuildGraphUpdate(graph: CanvasGraph) {
  buildGraph.value = graph
  buildChecked.value = false
}

// --- Sidebar: projects + palette loading ---

async function loadProjects() {
  try {
    const result = await getProjects()
    projects.value = result.projects
    apiSource.value = 'connected'
    apiError.value = ''
  } catch (e) {
    projects.value = []
    apiSource.value = 'fallback'
    apiError.value = e instanceof Error ? e.message : 'Backend API is unavailable.'
  }
}

async function loadPalette() {
  try {
    const result = await getPalette()
    paletteEntries.value = result.entries
  } catch {
    paletteEntries.value = []
  }
}

async function addProject() {
  const path = window.prompt('Project directory path:')
  if (!path) return
  try {
    await addProjectDir(path)
    await loadProjects()
  } catch (e) {
    window.alert(e instanceof Error ? e.message : 'Failed to add project directory')
  }
}

// Non-builtin project groups get a remove entry: a missing directory (or a
// directory the user no longer wants scanned) can be dropped from settings.
async function removeProject(project: ProjectSummaryResponse) {
  if (!window.confirm(t.value.explorer.removeProjectConfirm)) return
  try {
    await deleteProjectDir(project.path)
    await loadProjects()
  } catch (e) {
    window.alert(e instanceof Error ? e.message : 'Failed to remove project directory')
  }
}

async function addManualNode() {
  const id = window.prompt('Node id:')
  if (!id) return
  const path = window.prompt('Node path (e.g. nodes/camera.py):')
  if (!path) return
  const inputsText = window.prompt('Inputs (comma-separated "name=urn, name=urn"):') ?? ''
  const outputsText = window.prompt('Outputs (comma-separated "name=urn, name=urn"):') ?? ''
  const parsePorts = (text: string): PalettePort[] => text
    .split(',')
    .map(part => part.trim())
    .filter(Boolean)
    .map(part => {
      const [name, urn] = part.split('=')
      return urn ? { name: name.trim(), urn: urn.trim() } : { name: part.trim() }
    })
  const spec: ManualNodeSpec = { id, path, inputs: parsePorts(inputsText), outputs: parsePorts(outputsText) }
  try {
    await submitManualNode(spec)
    await loadPalette()
  } catch (e) {
    window.alert(e instanceof Error ? e.message : 'Failed to add node')
  }
}

// --- Dataflow loading ---

async function loadDataflow(id: string) {
  selectedDataflowId.value = id
  saveStatus.value = ''
  const result = await getDataflowDefinition(id, emptyDefinition)
  definition.value = result.source === 'connected' ? result.data : null
  if (definition.value) {
    editingDefinition.value = definition.value
    buildGraph.value = definitionToGraph(definition.value)
    typeRules.value = definition.value.typeRules ?? []
    sourceSubView.value = 'canvas'
    // Sidebar selection always lands on the Source canvas editor so the
    // Save / Save As toolbar is visible regardless of the active tab.
    viewMode.value = 'source'
    await checkAllEdges()
  }
}

async function selectDataflow(id: string) {
  await loadDataflow(id)
}

// --- Build mode actions (unchanged) ---

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

function clearBuild() {
  buildGraph.value = { nodes: [], edges: [] }
  selectedBuildNode.value = null; selectedBuildEdge.value = null
  buildStatus.value = 'New graph'; buildValid.value = false; buildChecked.value = false
  generatedYaml.value = ''
}

// --- Edge schema checking (always URN-based; checkSchemaUrn handles missing URNs) ---

const edgeStyles = ref<Record<string, { color: string; tooltip: string }>>({})
const schemaChecking = ref(false)
let checkSeq = 0

async function checkAllEdges() {
  const seq = ++checkSeq
  schemaChecking.value = true
  const styles: Record<string, { color: string; tooltip: string }> = {}
  for (const edge of buildGraph.value.edges) {
    const srcNode = buildGraph.value.nodes.find(n => n.id === edge.sourceNode)
    const tgtNode = buildGraph.value.nodes.find(n => n.id === edge.targetNode)
    if (!srcNode || !tgtNode) continue
    const srcUrn = srcNode.outputs[edge.sourcePort]?.type
    const tgtUrn = tgtNode.inputs[edge.targetPort]?.type
    try {
      const resp = await checkSchemaUrn({ source_urn: srcUrn, sink_urn: tgtUrn, type_rules: typeRules.value })
      const level = edgeLevel(resp)
      styles[edge.id] = { color: edgeColor(level), tooltip: resp.detail + (resp.suggestion ? ` — ${resp.suggestion}` : '') }
    } catch {
      styles[edge.id] = { color: 'var(--text-muted-dark)', tooltip: 'Schema check unavailable' }
    }
  }
  if (seq !== checkSeq) return   // a newer run superseded this one
  edgeStyles.value = styles
  schemaChecking.value = false
}

// Re-check schema when edges change — the signature watch also fires when an
// edge is rewired between different endpoints (not just when the count
// changes). Type-rule changes re-check as well.
watch(
  () => buildGraph.value.edges.map(e => `${e.sourceNode}/${e.sourcePort}->${e.targetNode}/${e.targetPort}`).join('|'),
  () => { checkAllEdges() }
)
watch(() => typeRules.value, () => { checkAllEdges() })

// --- Source editor: save write-back ---

async function saveCurrent() {
  if (!editingDefinition.value) return
  saveStatus.value = t.value.explorer.saving
  try {
    const result = await saveDataflow(editingDefinition.value.id, graphToPayload(buildGraph.value, typeRules.value))
    if (!result.ok) {
      saveStatus.value = t.value.explorer.saveBlocked.replace('{count}', String(result.errors.length))
      applySaveIssues(result.errors, true)
    } else {
      // Reload first: loadDataflow resets saveStatus and re-runs the schema
      // edge checks, so the success message must be set after it completes.
      await loadDataflow(editingDefinition.value.id)
      saveStatus.value = `${t.value.explorer.saved.replace('{path}', result.path)}${result.warnings.length ? ` (${result.warnings.length} warning(s))` : ''}`
      applySaveIssues(result.warnings, false)
    }
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e)
    saveStatus.value = t.value.explorer.saveFailed.replace('{message}', message)
    // The 422 body arrives as a JSON string inside the ApiError error field
    // ("API request failed: 422 — {"ok":false,...}"); parse it so per-edge
    // save errors surface on the canvas.
    const parsed = parseSaveError(message)
    if (parsed && !parsed.ok) applySaveIssues(parsed.errors, true)
  }
}

function applySaveIssues(issues: SaveIssue[], blocking: boolean) {
  edgeStyles.value = { ...edgeStyles.value, ...issuesToEdgeStyles(issues, buildGraph.value, blocking) }
}

async function saveAsCurrent() {
  const target = window.prompt('Save dataflow as (absolute path):')
  if (!target) return
  saveStatus.value = t.value.explorer.saving
  try {
    const result = await saveDataflowAs(graphToPayload(buildGraph.value, typeRules.value), target)
    saveStatus.value = result.ok
      ? t.value.explorer.savedAs.replace('{path}', result.path)
      : t.value.explorer.saveBlocked.replace('{count}', String(result.errors.length))
  } catch (e) {
    saveStatus.value = t.value.explorer.saveFailed.replace('{message}', e instanceof Error ? e.message : String(e))
  }
}

// --- Source editor: port types + type rules ---

function onPortUpdate(portName: string, isInput: boolean, urn: string) {
  if (!selectedBuildNode.value) return
  const node = buildGraph.value.nodes.find(n => n.id === selectedBuildNode.value)
  if (!node) return
  const ports = isInput ? node.inputs : node.outputs
  if (ports[portName]) {
    if (urn) ports[portName] = { type: urn }
    else delete ports[portName].type
  }
  buildChecked.value = false
  checkAllEdges()
}

function onTypeRulesUpdate(rules: TypeRule[]) {
  typeRules.value = rules
}

function createRuleForSelectedEdge() {
  const edge = buildGraph.value.edges.find(e => e.id === selectedBuildEdge.value)
  if (!edge) return
  const srcNode = buildGraph.value.nodes.find(n => n.id === edge.sourceNode)
  const tgtNode = buildGraph.value.nodes.find(n => n.id === edge.targetNode)
  const from = srcNode?.outputs[edge.sourcePort]?.type
  const to = tgtNode?.inputs[edge.targetPort]?.type
  if (!from || !to) return
  typeRules.value = buildRulePatch(typeRules.value, from, to)
  checkAllEdges()
}

onMounted(async () => {
  await loadProjects()
  await loadPalette()
  const first = projects.value.flatMap(project => project.dataflows)[0]
  if (first) await loadDataflow(first.id)
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

/* Dataflow names must NOT be truncated (user feedback); the :title tooltip
   on the button still shows the full name on hover. The status chip stays
   right-aligned via flex. */
.flow-name-row strong {
  flex: 1;
  min-width: 0;
  white-space: normal;
  overflow-wrap: anywhere;
}

.flow-name-row .status-chip {
  flex-shrink: 0;
}

/* Scrollable flow list: full names render un-truncated, and the list
   scrolls vertically instead of overflowing the sidebar bottom edge. */
.flows-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.flow-file small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Sidebar panel header: keep the status pill inside its container */
.sidebar-panel .panel-header {
  gap: 8px;
}

.sidebar-panel .panel-header h2 {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sidebar-panel .panel-header .pill {
  flex-shrink: 0;
  font-size: 11px;
  max-width: 110px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.view-tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 6px;
}

.source-subtabs {
  margin-top: 4px;
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

/* ── Source editor (M18): grouped sidebar, canvas editor, side panels ── */
.sidebar-add-project {
  margin: 4px 0 8px; padding: 8px 10px; width: 100%;
  background: var(--card-surface); border: 1px dashed var(--hairline-hover);
  border-radius: 6px; color: var(--accent-cyan); font-size: 12px; font-weight: 510;
  cursor: pointer; transition: background 120ms ease;
}
.sidebar-add-project:hover { background: var(--card-hover); }

.project-group { margin-bottom: 6px; }
.project-group-header {
  display: flex; align-items: center; gap: 6px;
  padding: 8px 0 4px; font-size: 12px; font-weight: 600;
  color: var(--text-muted-dark);
}
.project-group-name {
  flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.project-group-builtin {
  font-size: 12px; font-weight: 600; color: var(--text-muted-dark);
  border: 1px solid var(--hairline); border-radius: 4px; padding: 0 4px;
}
.project-group-count { font-size: 12px; color: var(--text-muted-dark); }
.project-groups-label {
  font-size: 12px; font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.05em; color: var(--text-muted-dark);
  padding: 4px 0 2px;
}
.project-group-remove {
  background: none; border: none; color: var(--text-muted-dark);
  cursor: pointer; font-size: 12px; line-height: 1; padding: 0 2px;
}
.project-group-remove:hover { color: var(--accent-red); }
.project-group-missing {
  font-size: 12px; color: var(--text-muted-dark); font-style: italic;
  padding: 0 0 6px;
}

.source-fallback {
  display: flex; flex-direction: column; flex: 1; min-height: 0;
}
.parse-note {
  padding: 10px 14px; font-size: 12px; color: var(--accent-yellow);
  background: var(--card-surface); border-bottom: 1px solid var(--hairline);
  flex-shrink: 0;
}
.source-side-panel {
  display: flex; flex-direction: column;
  width: 264px; min-width: 264px; min-height: 0;
  border-left: 1px solid var(--hairline);
  background: var(--panel-surface);
  overflow-y: auto;
}
.edge-props {
  padding: 12px; border-bottom: 1px solid var(--hairline);
  flex-shrink: 0;
}
.edge-props-header {
  font-size: 12px; font-weight: 600; color: var(--text-heading); margin-bottom: 6px;
}
.edge-props-reason {
  font-size: 12px; color: var(--text-body); line-height: 1.5; margin-bottom: 8px;
}
.edge-props-btn {
  width: 100%; padding: 8px 10px; font-size: 12px; font-weight: 510;
  background: var(--card-surface); color: var(--accent-yellow);
  border: 1px solid var(--hairline); border-radius: 6px; cursor: pointer;
  transition: background 120ms ease;
}
.edge-props-btn:hover { background: var(--card-hover); }
</style>
