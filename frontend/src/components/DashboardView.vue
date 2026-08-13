<template>
  <section class="view-stack">
    <!-- Quick Start panel -->
    <article v-if="!coordinatorConnected && !runtimeActive" class="panel quickstart-panel">
      <div class="panel-header">
        <h2>Quick Start</h2>
        <span class="pill warning">No connection</span>
      </div>
      <p class="muted">Studio needs either the dora daemon or a running dataflow to show live data.</p>
      <div class="quickstart-actions">
        <button
          class="daemon-btn"
          :disabled="daemonState !== 'stopped'"
          @click="startDaemonHandler"
        >
          {{ daemonLabel }}
        </button>
        <span class="quickstart-sep">or</span>
        <button class="secondary" @click="$emit('navigate', 'monitor')">
          Start a dataflow directly &rarr;
        </button>
      </div>
    </article>

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
        <div v-if="!coordinatorConnected && daemonState === 'running'" class="empty-state">
          Daemon is running. Waiting for coordinator to become available...
        </div>
        <div v-else-if="!coordinatorConnected" class="empty-state">
          Coordinator is not available. Use Quick Start above to launch the dora daemon.
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
  getCoordinatorStatus,
  getDaemonStatus,
  getDvizStatus,
  getMoveitStatus,
  getRuntimeLogs,
  getRuntimeStatus,
  getSystemStatus,
  startDaemon,
  type CoordinatorDataflowResponse,
  type DvizStatusResponse,
  type MoveitStatusResponse,
} from '../api'

import type { ViewId } from '../types'

defineEmits<{ navigate: [view: ViewId] }>()

const coordinatorConnected = ref(false)
const coordinatorVersion = ref('')
const coordinatorDataflows = ref<CoordinatorDataflowResponse[]>([])
const daemonState = ref<'stopped' | 'starting' | 'running'>('stopped')
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

const daemonLabel = computed(() => {
  if (daemonState.value === 'starting') return 'Starting daemon...'
  if (daemonState.value === 'running') return 'Daemon is running'
  return 'Start dora daemon'
})

const emptyStatus = { coordinator: '', daemon: '', version: '', runningDataflows: 0, activeNodes: 0, errorCount: 0 } as const

let refreshTimer: number | undefined

async function refreshDashboard() {
  const [sysResult, coordResult, rtResult, logResult, dvizResult, moveitResult, daemonResult] = await Promise.all([
    getSystemStatus(emptyStatus),
    getCoordinatorStatus({ connected: false, version: '', runningDataflows: 0, activeNodes: 0, dataflows: [] }),
    getRuntimeStatus({ status: 'stopped', pid: null, lastMessage: '', dataflowId: null, dataflowPath: null }),
    getRuntimeLogs([]),
    getDvizStatus({ installed: false, running: false, binaryPath: null, message: 'Unable to check dviz status.' }),
    getMoveitStatus({ installed: false, running: false, message: 'Unable to check moveit status.' }),
    getDaemonStatus({ running: false, pid: null }),
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

  // Track daemon state independently from coordinator
  if (daemonState.value !== 'starting') {
    daemonState.value = daemonResult.data.running ? 'running' : 'stopped'
  }
}

async function startDaemonHandler() {
  daemonState.value = 'starting'
  try {
    await startDaemon()
    daemonState.value = 'running'
    await refreshDashboard()
  } catch {
    daemonState.value = 'stopped'
  }
}

onMounted(async () => {
  await refreshDashboard()
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
  gap: 16px;
  margin-top: 18px;
}

.quickstart-sep {
  color: var(--text-muted, #94a3b8);
  font-size: 14px;
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
