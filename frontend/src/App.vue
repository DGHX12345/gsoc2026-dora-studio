<template>
  <div class="app-shell">
    <aside class="app-sidebar">
      <div class="brand-block">
        <img class="brand-logo" src="/dora-logo.jpg" alt="DORA" />
        <div>
          <strong>dora-studio</strong>
          <span>{{ t.app.prototype }}</span>
        </div>
      </div>

      <nav>
        <!-- Dashboard: standalone, prominent -->
        <button
          :class="['nav-item nav-primary', { active: activeView === 'dashboard' }]"
          @click="activeView = 'dashboard'"
        >
          <span class="nav-icon">{{ navItems[0].icon }}</span>
          {{ navItems[0].label }}
        </button>

        <!-- Dora section -->
        <div class="nav-section">
          <div class="nav-section-label">{{ t.sections.dora }}</div>
        </div>
        <button
          v-for="item in doraItems"
          :key="item.id"
          :class="['nav-item', { active: activeView === item.id }]"
          @click="activeView = item.id"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          {{ item.label }}
        </button>

        <!-- Robot section -->
        <div class="nav-section">
          <div class="nav-section-label">{{ t.sections.robot }}</div>
        </div>
        <button
          v-for="item in robotItems"
          :key="item.id"
          :class="['nav-item', { active: activeView === item.id }]"
          @click="activeView = item.id"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          {{ item.label }}
        </button>
      </nav>

      <div class="sidebar-spacer"></div>

      <button class="theme-toggle" @click="toggleTheme">
        <span class="theme-icon">{{ darkMode ? '\u263E' : '\u2600' }}</span>
        {{ darkMode ? 'Dark' : 'Light' }}
      </button>

      <div class="sidebar-footer">
        <span :class="['status-light', (coordinatorConnected || runtimeActive) ? 'online' : 'offline']"></span>
        <div>
          <strong>
            {{ runtimeActive ? 'Dataflow running' : coordinatorConnected ? 'Dora connected' : 'Dora not connected' }}
          </strong>
          <p>
            {{ runtimeActive ? `PID ${runtimePid} · capturing logs` : coordinatorConnected ? `Coordinator active · ${runningFlows} dataflow(s)` : 'Start dora daemon or run a dataflow' }}
          </p>
        </div>
        <button
          v-if="sessionRunning"
          class="footer-stop"
          :disabled="sessionBusy"
          @click="stopSessionHandler"
        >
          {{ t.session.stop }}
        </button>
      </div>
    </aside>

    <main class="main-area">
      <header class="topbar">
        <div>
          <p class="eyebrow">{{ currentItem.section }}</p>
          <h1>{{ currentItem.title }}</h1>
        </div>
        <div class="topbar-actions">
          <span>{{ t.app.currentFile }}</span>
          <button class="secondary language-toggle" @click="toggleLocale">
            {{ t.app.languageLabel }}
          </button>
          <button @click="downloadDemoReport">{{ t.app.exportReport }}</button>
        </div>
      </header>

      <DashboardView v-if="activeView === 'dashboard'" @navigate="(v: ViewId) => activeView = v" />
      <DataflowExplorer v-else-if="activeView === 'explorer'" />
      <RunMonitorView v-else-if="activeView === 'monitor'" @open-replay="openReplayInVisualization" />
      <LogsEventsView v-else-if="activeView === 'logs'" />
      <ReplayTimeline v-else-if="activeView === 'replay'" />
      <MetricsDashboard v-else-if="activeView === 'metrics'" />
      <!-- M15 B6: the viewport stays mounted across page switches
           (v-show, not v-if) so the live feed and tool attachments
           survive; hidden pages render nothing (on-demand rendering). -->
      <VisualizationView ref="visualizationRef" v-show="activeView === 'visualization'" />
      <MotionPlannerView v-show="activeView === 'motion'" />
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import DashboardView from './components/DashboardView.vue'
import DataflowExplorer from './components/DataflowExplorer.vue'
import MotionPlannerView from './components/MotionPlannerView.vue'
import RunMonitorView from './components/RunMonitorView.vue'
import LogsEventsView from './components/LogsEventsView.vue'
import MetricsDashboard from './components/MetricsDashboard.vue'
import ReplayTimeline from './components/ReplayTimeline.vue'
import VisualizationView from './components/VisualizationView.vue'
import {
  getCoordinatorStatus,
  getRuntimeStatus,
  getSessionStatus,
  stopSession,
  type CoordinatorStatusResponse,
  type RuntimeStateResponse,
  type SessionStatusResponse,
} from './api'
import { useI18n } from './i18n'
import type { ViewId } from './types'

const { t, toggleLocale } = useI18n()

const darkMode = ref(false)

function toggleTheme() {
  darkMode.value = !darkMode.value
  document.documentElement.setAttribute('data-theme', darkMode.value ? 'dark' : 'light')
  localStorage.setItem('dora-studio-theme', darkMode.value ? 'dark' : 'light')
}

const coordinatorConnected = ref(false)
const runningFlows = ref(0)
const runtimeActive = ref(false)
const runtimePid = ref<number | null>(null)
const sessionRunning = ref(false)
const sessionBusy = ref(false)
let coordinatorTimer: number | undefined

async function pollStatus() {
  try {
    const coord = await getCoordinatorStatus({
      connected: false, version: '', runningDataflows: 0, activeNodes: 0, dataflows: [],
    })
    coordinatorConnected.value = coord.data.connected
    runningFlows.value = coord.data.runningDataflows
  } catch {
    coordinatorConnected.value = false
  }

  try {
    const rt = await getRuntimeStatus({
      status: 'stopped', pid: null, lastMessage: '', dataflowId: null, dataflowPath: null,
    })
    runtimeActive.value = rt.data.status === 'running'
    runtimePid.value = rt.data.pid
  } catch {
    runtimeActive.value = false
  }

  try {
    const sessionResult = await getSessionStatus({
      status: 'stopped', running: false, coordinatorConnected: false, coordinatorStatus: 'unavailable',
      pid: null, version: '', lifecycleSupported: true, dataflowCount: 0, message: '',
    })
    if (!sessionBusy.value) {
      sessionRunning.value = sessionResult.data.running
    }
  } catch {
    if (!sessionBusy.value) {
      sessionRunning.value = false
    }
  }
}

async function stopSessionHandler() {
  sessionBusy.value = true
  try {
    const result = await stopSession()
    sessionRunning.value = result.running
  } catch {
    sessionRunning.value = false
  }
  sessionBusy.value = false
}

function downloadDemoReport() {
  const lines = [
    '# dora-studio prototype report',
    '',
    `Generated at: ${new Date().toISOString()}`,
    `Current view: ${currentItem.value.title}`,
    `Coordinator: ${coordinatorConnected.value ? 'connected' : 'not connected'}`,
    `Running dataflows: ${runningFlows.value}`,
    `Runtime: ${runtimeActive.value ? `running, PID ${runtimePid.value}` : 'stopped'}`,
    '',
    '## Implemented prototype areas',
    '',
    '- Dashboard overview and recent runtime signals',
    '- Dataflow discovery, graph rendering, node inspection, and diagnostics',
    '- Selected-dataflow runtime control through the backend bridge',
    '- Grouped runtime logs and raw log stream view',
    '- dviz-oriented visualization layout prepared for Phase 2 data forwarding',
    '- dora-moveit2-oriented motion planning layout prepared for Phase 2 APIs',
    '',
    '## Validation commands',
    '',
    '```bash',
    'cargo fmt --manifest-path backend/Cargo.toml --check',
    'cargo test --manifest-path backend/Cargo.toml',
    'npm --prefix frontend run build',
    '```',
  ]

  const blob = new Blob([lines.join('\n')], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = 'dora-studio-prototype-report.md'
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}

onMounted(() => {
  const saved = localStorage.getItem('dora-studio-theme')
  if (saved === 'dark') {
    darkMode.value = true
    document.documentElement.setAttribute('data-theme', 'dark')
  }

  pollStatus()
  coordinatorTimer = window.setInterval(pollStatus, 5000)
})

onUnmounted(() => {
  if (coordinatorTimer) window.clearInterval(coordinatorTimer)
})

const navItems = computed(() => [
  { id: 'dashboard' as ViewId, group: 'overview', icon: '01', ...t.value.nav.dashboard },
  { id: 'explorer' as ViewId, group: 'dora', icon: '02', ...t.value.nav.explorer },
  { id: 'monitor' as ViewId, group: 'dora', icon: '03', ...t.value.nav.monitor },
  { id: 'logs' as ViewId, group: 'dora', icon: '04', ...t.value.nav.logs },
  { id: 'metrics' as ViewId, group: 'dora', icon: '08', ...t.value.nav.metrics },
  { id: 'replay' as ViewId, group: 'dora', icon: '07', ...t.value.nav.replay },
  { id: 'visualization' as ViewId, group: 'robot', icon: '05', ...t.value.nav.visualization },
  { id: 'motion' as ViewId, group: 'robot', icon: '06', ...t.value.nav.motion },
])

const doraItems = computed(() => navItems.value.filter((item) => item.group === 'dora'))
const robotItems = computed(() => navItems.value.filter((item) => item.group === 'robot'))

const activeView = ref<ViewId>('dashboard')
const currentItem = computed(() => navItems.value.find((item) => item.id === activeView.value) ?? navItems.value[0])

type VisualizationExposed = { openReplayFromRecording?: (path: string) => void }
const visualizationRef = ref<VisualizationExposed | null>(null)

function openReplayInVisualization(path: string) {
  activeView.value = 'visualization'
  visualizationRef.value?.openReplayFromRecording?.(path)
}
</script>

<style scoped>
.footer-stop {
  background: var(--accent-red, #ef4444);
  border: none;
  border-radius: 8px;
  color: #ffffff;
  cursor: pointer;
  flex-shrink: 0;
  font-size: 12px;
  padding: 6px 12px;
}

.footer-stop:disabled {
  cursor: default;
  opacity: 0.6;
}
</style>
