<template>
  <section class="view-stack">
    <!-- Session status bar -->
    <article class="panel session-bar">
      <div class="session-bar-left">
        <span :class="['pill', sessionPillClass]">{{ sessionPillText }}</span>
        <span class="muted session-bar-meta">
          {{ t.session.versionLabel }}: {{ session.version || '—' }}
          <template v-if="session.running"> · {{ session.dataflowCount }} {{ t.session.dataflowCountLabel }}</template>
        </span>
      </div>
      <div class="session-bar-right">
        <p v-if="!session.lifecycleSupported" class="session-upgrade-hint-inline">{{ upgradeHint }}</p>
        <button
          :disabled="!canStart && !canStop"
          @click="session.running ? stopSessionHandler() : startSessionHandler()"
        >
          {{ sessionButtonLabel }}
        </button>
      </div>
    </article>

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
          <option value="__custom__">Custom YAML path…</option>
        </select>
        <input
          v-if="useCustomPath"
          v-model="customPath"
          class="custom-path-input"
          type="text"
          placeholder="examples/my-flow/dataflow.yml"
          @input="apiError = ''"
        />
      </label>
      <p v-if="apiError" class="muted">{{ apiError }}</p>
      <div class="control-row">
        <button class="secondary" @click="refreshRuntime">Refresh</button>
        <button @click="startDataflow" :disabled="!canStartFlow">Start</button>
        <button class="secondary" @click="restartDataflow" :disabled="!canStartFlow">Restart</button>
        <button class="danger-button" @click="stopDataflow" :disabled="!canStopFlow">Stop</button>
        <button
          :class="recordingBtnClass"
          :disabled="recordingBtnState === 'disabled'"
          @click="recordingBtnState === 'recording' ? stopRecordingHandler() : startRecordingHandler()"
        >
          {{ recordingBtnState === 'recording' ? t.recording.stopRecording : t.recording.record }}
        </button>
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
        <span class="pill">Requires dora daemon</span>
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

    <article class="panel">
      <div class="panel-header">
        <h2>{{ t.recording.listTitle }}</h2>
        <button class="secondary" @click="refreshRecordings">Refresh</button>
      </div>
      <div v-if="recordings.length === 0" class="empty-state">{{ t.recording.empty }}</div>
      <div v-else class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Frames</th>
              <th>Size</th>
              <th>Time</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="rec in recordings" :key="rec.path">
              <td><strong>{{ rec.name }}</strong></td>
              <td>{{ rec.frameCount ?? '—' }}</td>
              <td>{{ formatBytes(rec.sizeBytes) }}</td>
              <td>{{ formatRecordingTime(rec.createdAtMillis) }}</td>
              <td>
                <button class="secondary" @click="$emit('openReplay', rec.path)">
                  {{ t.recording.openInReplay }}
                </button>
              </td>
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
import { computed, onMounted, onUnmounted, ref } from 'vue'
import {
  getDataflowDefinition,
  getDataflows,
  getNodes,
  getRecordingList,
  getRuntimeStatus,
  getSessionStatus,
  restartDataflowRuntime,
  startDataflowRuntime,
  startRecordingCapture,
  startRuntimeByPath,
  startSession,
  stopDataflowRuntime,
  stopRecordingCapture,
  stopSession,
  type ApiSource,
  type DataflowSummaryResponse,
  type NodeMetricsResponse,
  type RecordingCaptureStatusResponse,
  type RecordingListEntryResponse,
  type RuntimeStateResponse,
  type SessionStatusResponse,
} from '../api'
import { useI18n } from '../i18n'
import {
  canStartDataflow,
  canStartSession,
  canStopDataflow,
  canStopSession,
  formatBytes,
  formatRecordingTime,
  recordingAction,
  sessionUiState,
  type SessionBusy,
} from '../session-ui'

defineEmits<{ openReplay: [path: string] }>()

const { t } = useI18n()

const emptyNodes: NodeMetricsResponse[] = []
const emptyRuntime: RuntimeStateResponse = { status: 'stopped', pid: null, lastMessage: '', dataflowId: null, dataflowPath: null }
const emptyDataflows: DataflowSummaryResponse[] = []
const emptySession: SessionStatusResponse = {
  status: 'stopped', running: false, coordinatorConnected: false, coordinatorStatus: 'unavailable',
  pid: null, version: '', lifecycleSupported: true, dataflowCount: 0, message: '',
}

const nodes = ref<NodeMetricsResponse[]>([])
const runtime = ref<RuntimeStateResponse>(emptyRuntime)
const dataflows = ref<DataflowSummaryResponse[]>([])
const selectedDataflowId = ref('')
const customPath = ref('')
const useCustomPath = computed(() => selectedDataflowId.value === '__custom__')
const apiError = ref('')
const apiSource = ref<ApiSource>('fallback')
const session = ref<SessionStatusResponse>(emptySession)
const sessionBusy = ref<SessionBusy>('idle')
const recording = ref<RecordingCaptureStatusResponse>({
  status: 'idle', outputPath: null, dataflowPath: null, startedAtMillis: null, frameCount: null, message: '',
})
const recordings = ref<RecordingListEntryResponse[]>([])
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'Connected' : 'Backend unavailable'))
const selectedDataflow = computed(
  () => dataflows.value.find((flow) => flow.id === selectedDataflowId.value) ?? dataflows.value[0],
)
const runtimeStatusText = computed(() => {
  if (runtime.value.status === 'running') return 'Running'
  if (runtime.value.status === 'failed') return 'Failed'
  if (runtime.value.status === 'unavailable') return 'Unavailable'
  return 'Stopped'
})

const statusText: Record<string, string> = {
  running: 'Running',
  degraded: 'Degraded',
  failed: 'Failed',
  stopped: 'Stopped',
}

const sessionUi = computed(() => sessionUiState(session.value, sessionBusy.value))
const canStart = computed(() => canStartSession(session.value, sessionBusy.value))
const canStop = computed(() => canStopSession(session.value, sessionBusy.value))
const canStartFlow = computed(() => {
  if (!canStartDataflow(runtime.value.status, session.value)) return false
  if (useCustomPath.value) return customPath.value.trim().length > 0
  return true
})
const canStopFlow = computed(() => canStopDataflow(runtime.value.status, session.value))

const sessionPillClass = computed(() => {
  if (sessionUi.value === 'running') return 'success'
  if (sessionUi.value === 'error') return 'failed'
  if (sessionUi.value === 'unavailable' || sessionUi.value === 'starting' || sessionUi.value === 'stopping') return 'warning'
  return 'stopped'
})

const sessionPillText = computed(() => {
  switch (sessionUi.value) {
    case 'running': return t.value.session.running
    case 'starting': return t.value.session.starting
    case 'stopping': return t.value.session.stopping
    case 'error': return t.value.session.error
    case 'unavailable': return t.value.session.unavailable
    default: return t.value.session.stopped
  }
})

const sessionButtonLabel = computed(() => {
  if (sessionBusy.value === 'starting') return t.value.session.starting
  if (sessionBusy.value === 'stopping') return t.value.session.stopping
  return session.value.running ? t.value.session.stop : t.value.session.start
})

const upgradeHint = computed(() =>
  t.value.session.upgradeHint.replace('{version}', session.value.version || 'unknown'),
)

const recordingBtnState = computed(() =>
  recordingAction(
    recording.value.status,
    session.value.lifecycleSupported &&
      session.value.running &&
      (useCustomPath.value ? customPath.value.trim().length > 0 : !!selectedDataflow.value),
  ),
)
const recordingBtnClass = computed(() => {
  if (recordingBtnState.value === 'recording') return 'danger-button'
  if (recordingBtnState.value === 'disabled') return 'secondary'
  return ''
})

let refreshTimer: number | undefined

async function refreshSessionAndRecordings() {
  const [sessionResult, recordingsResult] = await Promise.all([
    getSessionStatus(emptySession),
    getRecordingList([]),
  ])
  session.value = sessionResult.data
  if (recordingsResult.source === 'connected') {
    recordings.value = recordingsResult.data
  }
}

async function refreshRecordings() {
  const result = await getRecordingList([])
  if (result.source === 'connected') {
    recordings.value = result.data
  }
}

async function refreshRuntime() {
  const result = await getRuntimeStatus(emptyRuntime)
  runtime.value = result.data
  apiSource.value = result.source
  await refreshSelectedNodes()
}

async function startSessionHandler() {
  sessionBusy.value = 'starting'
  try {
    session.value = await startSession()
  } catch {
    // next poll reports the honest state
  }
  sessionBusy.value = 'idle'
  await refreshSessionAndRecordings()
}

async function stopSessionHandler() {
  sessionBusy.value = 'stopping'
  try {
    session.value = await stopSession()
  } catch {
    // next poll reports the honest state
  }
  sessionBusy.value = 'idle'
  await refreshSessionAndRecordings()
}

async function startDataflow() {
  try {
    if (useCustomPath.value) {
      runtime.value = await startRuntimeByPath(customPath.value.trim())
    } else {
      runtime.value = await startDataflowRuntime(selectedDataflowId.value)
    }
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

async function startRecordingHandler() {
  try {
    if (useCustomPath.value) {
      recording.value = await startRecordingCapture(customPath.value.trim())
    } else {
      if (!selectedDataflow.value) return
      const definition = await getDataflowDefinition(selectedDataflow.value.id, {
        id: selectedDataflow.value.id,
        name: selectedDataflow.value.name,
        relativePath: '',
        source: '',
        nodeCount: 0,
        edgeCount: 0,
        nodes: [],
      })
      recording.value = await startRecordingCapture(definition.data.relativePath)
    }
    if (recording.value.status === 'failed' || recording.value.status === 'unavailable') {
      apiError.value = recording.value.message
    }
  } catch {
    apiError.value = 'Failed to start recording. Is the backend running?'
  }
  await refreshRecordings()
}

async function stopRecordingHandler() {
  try {
    recording.value = await stopRecordingCapture()
  } catch {
    apiError.value = 'Failed to stop recording. Is the backend running?'
  }
  await refreshRecordings()
}

async function refreshSelectedNodes() {
  if (!selectedDataflowId.value || useCustomPath.value) return
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

  await refreshSessionAndRecordings()
  refreshTimer = window.setInterval(refreshSessionAndRecordings, 5000)
})

onUnmounted(() => {
  if (refreshTimer) window.clearInterval(refreshTimer)
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

.session-bar {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  justify-content: space-between;
}

.session-bar-left,
.session-bar-right {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
}

.session-bar-meta {
  font-size: 13px;
}

.session-upgrade-hint-inline {
  color: var(--accent-red, #ef4444);
  font-size: 13px;
}

.custom-path-input {
  background: var(--bg-surface, #f8fafc);
  border: 1px solid var(--hairline, #e2e8f0);
  border-radius: 8px;
  color: var(--text-body, #1e293b);
  font-size: 13px;
  min-width: 240px;
  padding: 8px 12px;
}

[data-theme="dark"] .custom-path-input {
  background: var(--bg-surface, #0f172a);
  color: var(--text-body, #e2e8f0);
}

.metric-card strong {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty-state {
  color: var(--text-muted, #94a3b8);
  font-size: 14px;
  padding: 22px 0;
  text-align: center;
}

[data-theme="dark"] .empty-state {
  color: var(--text-muted-dark, #64748b);
}
</style>
