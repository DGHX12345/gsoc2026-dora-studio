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
        <button
          v-for="item in navItems"
          :key="item.id"
          :class="{ active: activeView === item.id }"
          @click="activeView = item.id"
        >
          <span>{{ item.icon }}</span>
          {{ item.label }}
        </button>
      </nav>

      <button class="theme-toggle" @click="toggleTheme">
        <span class="theme-icon">{{ darkMode ? '\u263E' : '\u2600' }}</span>
        {{ darkMode ? '深色模式' : '浅色模式' }}
      </button>

      <div class="sidebar-footer">
        <span :class="['status-light', (coordinatorConnected || runtimeActive) ? 'online' : 'offline']"></span>
        <div>
          <strong>
            {{ runtimeActive ? 'Dataflow 运行中' : coordinatorConnected ? 'DORA 已连接' : 'DORA 未连接' }}
          </strong>
          <p>
            {{ runtimeActive ? `PID ${runtimePid} · 日志收集中` : coordinatorConnected ? `协调器运行中 · ${runningFlows} 个 dataflow` : '启动 dora up 或运行 dataflow' }}
          </p>
        </div>
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
          <button>{{ t.app.exportReport }}</button>
        </div>
      </header>

      <DashboardView v-if="activeView === 'dashboard'" />
      <DataflowExplorer v-else-if="activeView === 'explorer'" />
      <RunMonitorView v-else-if="activeView === 'monitor'" />
      <LogsEventsView v-else-if="activeView === 'logs'" />
      <VisualizationView v-else-if="activeView === 'visualization'" />
      <MotionPlannerView v-else />
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
import VisualizationView from './components/VisualizationView.vue'
import { getCoordinatorStatus, getRuntimeStatus, type CoordinatorStatusResponse, type RuntimeStateResponse } from './api'
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
let coordinatorTimer: number | undefined

async function pollStatus() {
  // Check coordinator
  try {
    const coord = await getCoordinatorStatus({
      connected: false, version: '', runningDataflows: 0, activeNodes: 0, dataflows: [],
    })
    coordinatorConnected.value = coord.data.connected
    runningFlows.value = coord.data.runningDataflows
  } catch {
    coordinatorConnected.value = false
  }

  // Check runtime (dora run subprocess)
  try {
    const rt = await getRuntimeStatus({
      status: 'stopped', pid: null, lastMessage: '', dataflowId: null, dataflowPath: null,
    })
    runtimeActive.value = rt.data.status === 'running'
    runtimePid.value = rt.data.pid
  } catch {
    runtimeActive.value = false
  }
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
  { id: 'dashboard' as ViewId, icon: '01', ...t.value.nav.dashboard },
  { id: 'explorer' as ViewId, icon: '02', ...t.value.nav.explorer },
  { id: 'monitor' as ViewId, icon: '03', ...t.value.nav.monitor },
  { id: 'logs' as ViewId, icon: '04', ...t.value.nav.logs },
  { id: 'visualization' as ViewId, icon: '05', ...t.value.nav.visualization },
  { id: 'motion' as ViewId, icon: '06', ...t.value.nav.motion },
])

const activeView = ref<ViewId>('explorer')
const currentItem = computed(() => navItems.value.find((item) => item.id === activeView.value) ?? navItems.value[0])
</script>
