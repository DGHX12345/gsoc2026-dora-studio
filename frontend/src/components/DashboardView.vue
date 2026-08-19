<template>
  <section class="view-stack">
    <!-- Session control panel -->
    <article class="panel quickstart-panel session-panel">
      <div class="panel-header">
        <h2>{{ t.session.panelTitle }}</h2>
        <span :class="['pill', sessionPillClass]">{{ sessionPillText }}</span>
      </div>
      <p class="muted session-meta">
        <span>{{ t.session.versionLabel }}: {{ session.version || '—' }}</span>
        <span v-if="session.running">{{ t.session.dataflowCountLabel }}: {{ session.dataflowCount }}</span>
        <span v-else-if="session.coordinatorConnected">{{ t.session.externalNote }}</span>
      </p>
      <p v-if="!session.lifecycleSupported" class="session-upgrade-hint">
        {{ upgradeHint }}
      </p>
      <div class="quickstart-actions">
        <button
          class="daemon-btn"
          :disabled="!canStart && !canStop"
          @click="session.running ? requestStop() : startSessionHandler()"
        >
          {{ sessionButtonLabel }}
        </button>
        <template v-if="confirmingStop">
          <button class="danger-button" @click="stopSessionHandler">
            {{ t.session.confirm }}
          </button>
          <button class="secondary" @click="confirmingStop = false">
            {{ t.session.cancel }}
          </button>
        </template>
        <template v-else>
          <span class="quickstart-sep">or</span>
          <button class="secondary" @click="$emit('navigate', 'monitor')">
            Start a dataflow directly &rarr;
          </button>
        </template>
      </div>
      <p v-if="confirmingStop" class="muted session-confirm-note">
        {{ t.session.confirmStopMessage }}
      </p>
    </article>

    <!-- dora version environment card (M17): collapsed by default, the
         summary shows the badge and active version at a glance. -->
    <details class="panel env-panel">
      <summary class="env-summary">
        <span class="env-summary-header">
          <h2>{{ t.doraEnv.title }}</h2>
          <span :class="['pill', envBadgeClass]">{{ envBadgeText }}</span>
        </span>
        <span class="env-summary-tail">
          <span class="muted env-active-version">{{ activeVersionText }}</span>
          <span class="env-expand-hint env-hint-closed">{{ t.doraEnv.expandHint }}</span>
          <span class="env-expand-hint env-hint-open">{{ t.doraEnv.collapseHint }}</span>
          <span class="env-chevron">&#9656;</span>
        </span>
      </summary>
      <p v-if="overriddenByEnv" class="env-override-hint">{{ t.doraEnv.envOverride }}</p>
      <p v-else-if="envBadge === 'degraded'" class="env-degraded-hint">{{ t.doraEnv.degradedHint }}</p>
      <ul class="env-list">
        <li
          v-for="item in doraItems"
          :key="item.path"
          :class="['env-item', { active: item.active }]"
        >
          <div class="env-item-info">
            <strong>{{ item.version }}</strong>
            <small>{{ item.path }}</small>
          </div>
          <span v-if="item.active" class="env-active-tag">{{ t.doraEnv.active }}</span>
          <button
            v-if="canSwitch(item)"
            class="secondary"
            @click="switchDora(item.path)"
          >
            {{ t.doraEnv.switch }}
          </button>
          <button
            v-else-if="!item.active"
            class="env-delete"
            @click="removeCandidate(item.path)"
          >
            {{ t.doraEnv.delete }}
          </button>
        </li>
      </ul>
      <div class="env-actions">
        <input
          v-model="newCandidatePath"
          type="text"
          :placeholder="t.doraEnv.addPlaceholder"
          @input="switchNote = ''"
        />
        <button
          class="secondary"
          :disabled="!newCandidatePath.trim()"
          @click="addCandidate"
        >
          {{ t.doraEnv.add }}
        </button>
      </div>
      <p v-if="switchNote" class="muted env-note">{{ switchNote }}</p>
    </details>

    <div class="metric-grid">
      <article :class="['metric-card', 'large-metric', coordinatorConnected ? 'success' : 'warning']">
        <span>Coordinator</span>
        <strong>{{ coordinatorConnected ? 'Connected' : 'Unavailable' }}</strong>
        <small>{{ coordinatorVersion || 'dora daemon not detected' }}</small>
      </article>
      <article :class="['metric-card', 'large-metric', runtimeStatus === 'running' ? 'success' : '']">
        <span>Runtime</span>
        <strong>{{ runtimeStatusText }}</strong>
        <small>{{ runtimeLastMessage }}</small>
      </article>
      <article :class="['metric-card', 'large-metric', dviz.installed ? 'success' : '']">
        <span>3D Viz (dviz)</span>
        <strong>{{ dviz.installed ? (dviz.running ? 'Running' : 'Installed') : 'Not installed' }}</strong>
        <small>{{ dviz.message }}</small>
      </article>
      <article :class="['metric-card', 'large-metric', moveit.installed ? 'success' : '']">
        <span>Motion (moveit)</span>
        <strong>{{ moveit.installed ? (moveit.running ? 'Running' : 'Installed') : 'Not installed' }}</strong>
        <small>{{ moveit.message }}</small>
      </article>
    </div>

    <div class="split-grid">
      <article class="panel">
        <div class="panel-header">
          <h2>Coordinator Dataflows</h2>
          <span :class="['pill', coordinatorConnected ? 'success' : 'warning']">
            {{ coordinatorConnected ? `${coordinatorDataflows.length} dataflows` : 'unavailable' }}
          </span>
        </div>
        <div v-if="!coordinatorConnected && session.running" class="empty-state">
          Session is starting. Waiting for the coordinator to become available...
        </div>
        <div v-else-if="!coordinatorConnected" class="empty-state">
          Coordinator is not available. Start a session above to launch dora.
        </div>
        <div v-else-if="coordinatorDataflows.length === 0" class="empty-state">
          No dataflows registered with the coordinator.
        </div>
        <div v-else class="coordinator-flow-list">
          <div v-for="df in coordinatorDataflows" :key="df.id" class="coordinator-flow-item">
            <strong>{{ df.name }}</strong>
            <div>
              <span>{{ df.nodes }} nodes</span>
              <span :class="['status-chip', df.status]">{{ df.status }}</span>
            </div>
          </div>
        </div>
      </article>

      <article class="panel">
        <div class="panel-header">
          <h2>Recent Runtime Logs</h2>
          <span :class="['pill', runtimeStatus === 'running' ? 'success' : '']">
            {{ runtimeStatus === 'running' ? 'streaming' : 'idle' }}
          </span>
        </div>
        <div v-if="runtimeStatus !== 'running'" class="empty-state">
          No dataflow is running. Start one from Run &amp; Monitor to see live logs.
        </div>
        <ul v-else-if="recentLogs.length > 0" class="event-list large-events">
          <li v-for="log in recentLogs" :key="`${log.timestamp}-${log.node}`">
            <span :class="['dot', log.level]"></span>
            <div>
              <strong>{{ log.node }}</strong>
              <p>{{ log.message }}</p>
            </div>
            <time>{{ log.time }}</time>
          </li>
        </ul>
        <div v-else class="empty-state">Waiting for log output...</div>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import {
  addDoraCandidate,
  deleteDoraCandidate,
  getCoordinatorStatus,
  getDoraVersions,
  getDvizStatus,
  getMoveitStatus,
  getRuntimeLogs,
  getRuntimeStatus,
  getSessionStatus,
  getSystemStatus,
  startSession,
  stopSession,
  switchDoraVersion,
  type CoordinatorDataflowResponse,
  type DoraVersionItemResponse,
  type DvizStatusResponse,
  type MoveitStatusResponse,
  type SessionStatusResponse,
} from '../api'
import { useI18n } from '../i18n'
import {
  canStartSession,
  canStopSession,
  canSwitchItem,
  sessionUiState,
  versionBadge,
  type SessionBusy,
} from '../session-ui'

import type { ViewId } from '../types'

defineEmits<{ navigate: [view: ViewId] }>()

const { t } = useI18n()

const coordinatorConnected = ref(false)
const coordinatorVersion = ref('')
const coordinatorDataflows = ref<CoordinatorDataflowResponse[]>([])
const session = ref<SessionStatusResponse>({
  status: 'stopped', running: false, coordinatorConnected: false, coordinatorStatus: 'unavailable',
  pid: null, version: '', lifecycleSupported: true, dataflowCount: 0, message: '',
})
const sessionBusy = ref<SessionBusy>('idle')
const confirmingStop = ref(false)
const runtimeStatus = ref('stopped')
const runtimeActive = computed(() => runtimeStatus.value === 'running')
const runtimeLastMessage = ref('')
const dviz = ref<DvizStatusResponse>({ installed: false, running: false, binaryPath: null, message: 'Checking...' })
const moveit = ref<MoveitStatusResponse>({ installed: false, running: false, message: 'Checking...' })
const recentLogs = ref<{ time: string; timestamp: string; node: string; level: string; message: string }[]>([])

const runtimeStatusText = computed(() => {
  if (runtimeStatus.value === 'running') return 'Running'
  if (runtimeStatus.value === 'failed') return 'Failed'
  return 'Stopped'
})

const uiState = computed(() => sessionUiState(session.value, sessionBusy.value))
const canStart = computed(() => canStartSession(session.value, sessionBusy.value))
const canStop = computed(() => canStopSession(session.value, sessionBusy.value))

const sessionPillClass = computed(() => {
  if (uiState.value === 'running') return 'success'
  if (uiState.value === 'error') return 'failed'
  if (uiState.value === 'unavailable' || uiState.value === 'starting' || uiState.value === 'stopping') return 'warning'
  return 'stopped'
})

const sessionPillText = computed(() => {
  switch (uiState.value) {
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

// --- dora version environment card (M17) ---
const doraItems = ref<DoraVersionItemResponse[]>([])
const overriddenByEnv = ref(false)
const newCandidatePath = ref('')
const switchNote = ref('')

const envBadge = computed(() => versionBadge(doraItems.value, overriddenByEnv.value))
const envBadgeClass = computed(() => {
  if (envBadge.value === 'compatible') return 'success'
  if (envBadge.value === 'degraded') return 'failed'
  return 'warning'
})
const envBadgeText = computed(() => {
  if (envBadge.value === 'compatible') return t.value.doraEnv.compatible
  if (envBadge.value === 'degraded') return t.value.doraEnv.degraded
  return t.value.doraEnv.overridden
})
const activeVersionText = computed(() => {
  const active = doraItems.value.find((item) => item.active)
  return active ? active.version : '—'
})
const canSwitch = (item: DoraVersionItemResponse) => canSwitchItem(item, overriddenByEnv.value)

async function refreshDoraVersions() {
  const result = await getDoraVersions({ active: '', overriddenByEnv: false, items: [] })
  if (result.source === 'connected') {
    doraItems.value = result.data.items
    overriddenByEnv.value = result.data.overriddenByEnv
  }
}

async function switchDora(path: string) {
  try {
    await switchDoraVersion(path)
    await refreshDoraVersions()
    const item = doraItems.value.find((entry) => entry.path === path)
    switchNote.value = t.value.doraEnv.switched.replace(
      '{version}',
      item?.version ?? path,
    )
    await refreshDashboard()
  } catch (error) {
    switchNote.value = error instanceof Error ? error.message : 'Switch failed'
  }
}

async function addCandidate() {
  const path = newCandidatePath.value.trim()
  if (!path) return
  try {
    await addDoraCandidate(path)
    newCandidatePath.value = ''
    await refreshDoraVersions()
  } catch (error) {
    switchNote.value = error instanceof Error ? error.message : 'Add failed'
  }
}

async function removeCandidate(path: string) {
  try {
    await deleteDoraCandidate(path)
    await refreshDoraVersions()
    await refreshDashboard()
  } catch (error) {
    switchNote.value = error instanceof Error ? error.message : 'Delete failed'
  }
}

const emptyStatus = { coordinator: '', daemon: '', version: '', runningDataflows: 0, activeNodes: 0, errorCount: 0 } as const

let refreshTimer: number | undefined

async function refreshDashboard() {
  const [sysResult, coordResult, rtResult, logResult, dvizResult, moveitResult, sessionResult] = await Promise.all([
    getSystemStatus(emptyStatus),
    getCoordinatorStatus({ connected: false, version: '', runningDataflows: 0, activeNodes: 0, dataflows: [] }),
    getRuntimeStatus({ status: 'stopped', pid: null, lastMessage: '', dataflowId: null, dataflowPath: null }),
    getRuntimeLogs([]),
    getDvizStatus({ installed: false, running: false, binaryPath: null, message: 'Unable to check dviz status.' }),
    getMoveitStatus({ installed: false, running: false, message: 'Unable to check moveit status.' }),
    getSessionStatus({
      status: 'stopped', running: false, coordinatorConnected: false, coordinatorStatus: 'unavailable',
      pid: null, version: '', lifecycleSupported: true, dataflowCount: 0, message: '',
    }),
  ])

  coordinatorConnected.value = sysResult.source === 'connected' && sysResult.data.coordinator === 'connected'
  coordinatorVersion.value = sysResult.data.version

  if (coordResult.source === 'connected') {
    coordinatorDataflows.value = coordResult.data.dataflows
  }

  runtimeStatus.value = rtResult.data.status
  runtimeLastMessage.value = rtResult.data.lastMessage

  dviz.value = dvizResult.data
  moveit.value = moveitResult.data

  if (logResult.source === 'connected') {
    recentLogs.value = logResult.data.slice(-5).reverse()
  }

  session.value = sessionResult.data
}

function requestStop() {
  confirmingStop.value = true
}

async function startSessionHandler() {
  confirmingStop.value = false
  sessionBusy.value = 'starting'
  try {
    session.value = await startSession()
  } catch {
    // next poll reports the honest state
  }
  sessionBusy.value = 'idle'
  await refreshDashboard()
}

async function stopSessionHandler() {
  confirmingStop.value = false
  sessionBusy.value = 'stopping'
  try {
    session.value = await stopSession()
  } catch {
    // next poll reports the honest state
  }
  sessionBusy.value = 'idle'
  await refreshDashboard()
}

onMounted(async () => {
  await refreshDashboard()
  await refreshDoraVersions()
  refreshTimer = window.setInterval(refreshDashboard, 5000)
})

onUnmounted(() => {
  if (refreshTimer) window.clearInterval(refreshTimer)
})
</script>

<style scoped>
.empty-state {
  color: #94a3b8;
  font-size: 15px;
  line-height: 1.6;
  padding: 28px 0;
  text-align: center;
}

[data-theme="dark"] .empty-state {
  color: #64748b;
}

.quickstart-panel {
  border-color: var(--accent, #3b82f6);
}

.quickstart-actions {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin-top: 18px;
}

.quickstart-sep {
  color: var(--text-muted, #94a3b8);
  font-size: 14px;
}

.session-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 20px;
  margin-top: 10px;
}

.session-meta span {
  color: var(--text-secondary, #475569);
  font-size: 13px;
}

[data-theme="dark"] .session-meta span {
  color: var(--text-muted-dark, #94a3b8);
}

.session-upgrade-hint {
  background: var(--bg-surface, #f1f5f9);
  border: 1px solid var(--hairline, #e2e8f0);
  border-radius: 10px;
  color: var(--accent-red, #ef4444);
  font-size: 13px;
  margin-top: 12px;
  padding: 10px 14px;
}

.session-confirm-note {
  color: var(--accent-yellow, #eab308);
  font-size: 13px;
  margin-top: 12px;
}

.env-panel {
  margin-top: 4px;
}

.env-summary {
  align-items: center;
  cursor: pointer;
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
  justify-content: space-between;
  list-style: none;
}

.env-summary::-webkit-details-marker {
  display: none;
}

.env-summary-header {
  align-items: center;
  display: flex;
  gap: 12px;
}

.env-summary-header h2 {
  font-size: 16px;
}

.env-summary-tail {
  align-items: center;
  display: flex;
  gap: 10px;
}

.env-active-version {
  font-size: 13px;
}

.env-expand-hint {
  color: var(--text-muted, #94a3b8);
  font-size: 12px;
}

[data-theme="dark"] .env-expand-hint {
  color: var(--text-muted-dark, #64748b);
}

.env-hint-open {
  display: none;
}

.env-panel[open] .env-hint-closed {
  display: none;
}

.env-panel[open] .env-hint-open {
  display: inline;
}

.env-chevron {
  color: var(--text-muted, #94a3b8);
  font-size: 12px;
  transition: transform 0.15s ease;
}

.env-panel[open] .env-chevron {
  transform: rotate(90deg);
}

.env-override-hint {
  color: var(--accent-yellow, #eab308);
  font-size: 13px;
  margin-top: 8px;
}

.env-degraded-hint {
  color: var(--accent-red, #ef4444);
  font-size: 13px;
  margin-top: 8px;
}

.env-list {
  display: grid;
  gap: 8px;
  list-style: none;
  margin: 14px 0 0;
  padding: 0;
}

.env-item {
  align-items: center;
  background: var(--bg-surface, #f8fafc);
  border: 1px solid var(--hairline, #e2e8f0);
  border-radius: 10px;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  padding: 10px 14px;
}

.env-item.active {
  border-color: var(--accent, #3b82f6);
}

.env-item-info {
  min-width: 0;
}

.env-item-info strong {
  display: block;
  font-size: 14px;
}

.env-item-info small {
  color: var(--text-muted, #94a3b8);
  display: block;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.env-active-tag {
  color: var(--accent-green, #22c55e);
  flex-shrink: 0;
  font-size: 12px;
}

.env-actions {
  display: flex;
  gap: 10px;
  margin-top: 12px;
}

.env-actions input {
  background: var(--bg-surface, #f8fafc);
  border: 1px solid var(--hairline, #e2e8f0);
  border-radius: 8px;
  color: var(--text-body, #1e293b);
  flex: 1;
  font-size: 13px;
  min-width: 0;
  padding: 8px 12px;
}

[data-theme="dark"] .env-actions input {
  background: var(--bg-surface, #0f172a);
  color: var(--text-body, #e2e8f0);
}

.env-delete {
  background: transparent;
  border: 1px solid var(--hairline, #e2e8f0);
  border-radius: 8px;
  color: var(--text-muted, #94a3b8);
  cursor: pointer;
  flex-shrink: 0;
  font-size: 12px;
  padding: 5px 10px;
}

.env-note {
  margin-top: 10px;
}

.daemon-btn:disabled {
  opacity: 0.6;
}

.coordinator-flow-list {
  display: grid;
  gap: 8px;
}

.coordinator-flow-item {
  align-items: center;
  background: #f8fafd;
  border: 1px solid #edf2f8;
  border-radius: 14px;
  display: flex;
  justify-content: space-between;
  padding: 14px 18px;
}

.coordinator-flow-item strong {
  font-size: 16px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.coordinator-flow-item div {
  align-items: center;
  display: flex;
  flex-shrink: 0;
  gap: 12px;
}

.coordinator-flow-item div > span {
  color: #64748b;
  font-size: 14px;
}

[data-theme="dark"] .coordinator-flow-item {
  background: #0f172a;
  border-color: #334155;
}

[data-theme="dark"] .coordinator-flow-item div > span {
  color: #94a3b8;
}
</style>
