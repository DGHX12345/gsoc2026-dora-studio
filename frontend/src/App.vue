<template>
  <div class="app-shell">
    <aside class="app-sidebar">
      <div class="brand-block">
        <div class="brand-mark">DS</div>
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

      <div class="sidebar-footer">
        <span class="status-light"></span>
        <div>
          <strong>{{ t.app.runtimeTitle }}</strong>
          <p>{{ t.app.runtimeSubtitle }}</p>
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
      <LogsEventsView v-else />
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import DashboardView from './components/DashboardView.vue'
import DataflowExplorer from './components/DataflowExplorer.vue'
import RunMonitorView from './components/RunMonitorView.vue'
import LogsEventsView from './components/LogsEventsView.vue'
import { useI18n } from './i18n'
import type { ViewId } from './types'

const { t, toggleLocale } = useI18n()

const navItems = computed(() => [
  { id: 'dashboard' as ViewId, icon: '01', ...t.value.nav.dashboard },
  { id: 'explorer' as ViewId, icon: '02', ...t.value.nav.explorer },
  { id: 'monitor' as ViewId, icon: '03', ...t.value.nav.monitor },
  { id: 'logs' as ViewId, icon: '04', ...t.value.nav.logs },
])

const activeView = ref<ViewId>('explorer')
const currentItem = computed(() => navItems.value.find((item) => item.id === activeView.value) ?? navItems.value[0])
</script>
