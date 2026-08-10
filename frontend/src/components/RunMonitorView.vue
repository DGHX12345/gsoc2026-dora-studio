<template>
  <section class="view-stack">
    <div class="panel run-panel large-action-panel">
      <div>
        <p class="eyebrow">Run &amp; Monitor</p>
        <h2>{{ selectedDataflow?.name ?? 'No dataflow selected' }}</h2>
        <p class="muted">
          Start and stop local dataflows through the dora CLI runtime bridge.
          <span v-if="runtime.dataflowPath">Path: {{ runtime.dataflowPath }}</span>
        </p>
      </div>
      <label class="flow-select">
        <span>Target</span>
        <select v-model="selectedDataflowId" @change="refreshSelectedNodes">
          <option v-for="flow in dataflows" :key="flow.id" :value="flow.id">
            {{ flow.name }}
          </option>
        </select>
      </label>
      <p v-if="apiError" class="muted">{{ apiError }}</p>
      <div class="control-row">
        <button class="secondary" @click="refreshRuntime">Refresh</button>
        <button @click="startDataflow" :disabled="runtime.status === 'running'">Start</button>
        <button class="secondary" @click="restartDataflow">Restart</button>
        <button class="danger-button" @click="stopDataflow" :disabled="runtime.status !== 'running'">Stop</button>
      </div>
    </div>

    <div class="metric-grid">
      <article :class="['metric-card', 'large-metric', runtime.status === 'running' ? 'success' : '']">
        <span>Runtime Status</span>
        <strong>{{ runtimeStatusText }}</strong>
        <small>{{ runtime.pid ? `PID ${runtime.pid}` : 'No running process' }}</small>
      </article>
      <article class="metric-card large-metric">
        <span>Selected Dataflow</span>
        <strong>{{ selectedDataflow?.name ?? 'none' }}</strong>
        <small>{{ selectedDataflow ? `${selectedDataflow.nodeCount} nodes` : '' }}</small>
      </article>
      <article :class="['metric-card', apiSource === 'connected' ? 'success' : 'warning', 'large-metric']">
        <span>API Connection</span>
        <strong>{{ apiSourceText }}</strong>
        <small>{{ apiSource === 'connected' ? 'Backend responding' : 'Check backend is running on :3001' }}</small>
      </article>
      <article class="metric-card large-metric">
        <span>Last Message</span>
        <strong>{{ runtime.status }}</strong>
        <small>{{ runtime.lastMessage }}</small>
      </article>
    </div>

    <article class="panel">
      <div class="panel-header">
        <h2>Node Status</h2>
        <span class="pill">Metrics unavailable</span>
      </div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Node</th>
              <th>Kind</th>
              <th>Status</th>
              <th>CPU</th>
              <th>Memory</th>
              <th>Restarts</th>
              <th>Pending</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="node in nodes" :key="node.id">
              <td><strong>{{ node.label }}</strong></td>
              <td>{{ node.kind }}</td>
              <td><span :class="['status-chip', node.status]">{{ statusText[node.status] ?? node.status }}</span></td>
              <td class="metric-unavailable">--</td>
              <td class="metric-unavailable">--</td>
              <td class="metric-unavailable">--</td>
              <td class="metric-unavailable">--</td>
            </tr>
          </tbody>
        </table>
      </div>
    </article>

    <p v-if="runtime.status === 'running'" class="muted" style="text-align: center; padding: 10px 0;">
      Dataflow is running. Switch to <strong>Logs &amp; Events</strong> to view live output.
    </p>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  getDataflows,
  getNodes,
  getRuntimeStatus,
  restartDataflowRuntime,
  startDataflowRuntime,
  stopDataflowRuntime,
  type ApiSource,
  type DataflowSummaryResponse,
  type NodeMetricsResponse,
  type RuntimeStateResponse,
} from '../api'

const emptyNodes: NodeMetricsResponse[] = []
const emptyRuntime: RuntimeStateResponse = { status: 'stopped', pid: null, lastMessage: '', dataflowId: null, dataflowPath: null }
const emptyDataflows: DataflowSummaryResponse[] = []

const nodes = ref<NodeMetricsResponse[]>([])
const runtime = ref<RuntimeStateResponse>(emptyRuntime)
const dataflows = ref<DataflowSummaryResponse[]>([])
const selectedDataflowId = ref('')
const apiError = ref('')
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'Connected' : 'Backend unavailable'))
const selectedDataflow = computed(
  () => dataflows.value.find((flow) => flow.id === selectedDataflowId.value) ?? dataflows.value[0],
)
const runtimeStatusText = computed(() => {
  if (runtime.value.status === 'running') return 'Running'
  if (runtime.value.status === 'failed') return 'Failed'
  return 'Stopped'
})

const statusText: Record<string, string> = {
  running: 'Running',
  degraded: 'Degraded',
  failed: 'Failed',
  stopped: 'Stopped',
}

async function refreshRuntime() {
  const result = await getRuntimeStatus(emptyRuntime)
  runtime.value = result.data
  apiSource.value = result.source
  await refreshSelectedNodes()
}

async function startDataflow() {
  try {
    runtime.value = await startDataflowRuntime(selectedDataflowId.value)
  } catch {
    apiError.value = 'Failed to start dataflow. Is the backend running?'
  }
  await refreshSelectedNodes()
}

async function stopDataflow() {
  try {
    runtime.value = await stopDataflowRuntime(selectedDataflowId.value)
  } catch {
    apiError.value = 'Failed to stop dataflow. Is the backend running?'
  }
  await refreshSelectedNodes()
}

async function restartDataflow() {
  try {
    runtime.value = await restartDataflowRuntime(selectedDataflowId.value)
  } catch {
    apiError.value = 'Failed to restart dataflow. Is the backend running?'
  }
  await refreshSelectedNodes()
}

async function refreshSelectedNodes() {
  if (!selectedDataflowId.value) return
  const result = await getNodes(selectedDataflowId.value, emptyNodes)
  nodes.value = result.data
  apiSource.value = result.source
  apiError.value = result.error ?? ''
}

onMounted(async () => {
  const [dataflowsResult, runtimeResult] = await Promise.all([
    getDataflows(emptyDataflows),
    getRuntimeStatus(emptyRuntime),
  ])

  dataflows.value = dataflowsResult.data
  runtime.value = runtimeResult.data
  apiSource.value = dataflowsResult.source === 'connected' || runtimeResult.source === 'connected' ? 'connected' : 'fallback'
  apiError.value = dataflowsResult.source === 'fallback' ? (dataflowsResult.error ?? 'Backend API is unavailable.') : ''

  if (dataflows.value.length > 0) {
    selectedDataflowId.value = dataflows.value[0].id
    await refreshSelectedNodes()
  }
})
</script>

<style scoped>
.metric-unavailable {
  color: #94a3b8 !important;
  font-family: "JetBrains Mono", monospace;
  font-size: 14px;
  text-align: center;
}

[data-theme="dark"] .metric-unavailable {
  color: #64748b !important;
}

.run-panel {
  flex-wrap: wrap;
  gap: 14px;
}

.run-panel > div:first-child {
  min-width: 0;
}

.run-panel h2 {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.control-row {
  flex-wrap: wrap;
}

.table-wrap {
  min-width: 0;
}

.metric-grid {
  min-width: 0;
}
</style>
