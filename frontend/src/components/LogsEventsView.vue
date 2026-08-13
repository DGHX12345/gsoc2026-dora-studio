<template>
  <section class="view-stack">
    <div class="panel log-toolbar large-action-panel">
      <div>
        <p class="eyebrow">Logs &amp; Events</p>
        <h2>Runtime Logs</h2>
        <p class="muted">Live stdout and stderr from the running dataflow process.</p>
      </div>
      <div class="filter-row">
        <span :class="apiSource === 'connected' ? 'api-connected' : 'api-fallback'">{{ apiSourceText }}</span>
      </div>
    </div>

    <div class="log-level-grid">
      <article class="log-level-panel info-panel">
        <div class="log-level-header">
          <div>
            <span class="log-icon">i</span>
            <h2>Info</h2>
            <p>Normal operation and progress events.</p>
          </div>
          <strong>{{ groupedLogs.info.length }}</strong>
        </div>
        <div v-if="groupedLogs.info.length === 0" class="empty-log-panel">
          No info logs yet.
        </div>
        <div v-else class="log-list">
          <LogLine v-for="log in previewLogs.info" :key="logKey(log)" :log="log" variant="level" />
        </div>
        <button v-if="groupedLogs.info.length > previewLimit" class="view-all-button info" @click="openLogModal('info')">
          View all info logs
        </button>
      </article>

      <article class="log-level-panel warn-panel">
        <div class="log-level-header">
          <div>
            <span class="log-icon">!</span>
            <h2>Warnings</h2>
            <p>Anomalies worth attention but not yet breaking execution.</p>
          </div>
          <strong>{{ groupedLogs.warn.length }}</strong>
        </div>
        <div v-if="groupedLogs.warn.length === 0" class="empty-log-panel">
          No warnings yet.
        </div>
        <div v-else class="log-list">
          <LogLine v-for="log in previewLogs.warn" :key="logKey(log)" :log="log" variant="level" />
        </div>
        <button v-if="groupedLogs.warn.length > previewLimit" class="view-all-button warn" @click="openLogModal('warn')">
          View all warnings
        </button>
      </article>

      <article class="log-level-panel error-panel">
        <div class="log-level-header">
          <div>
            <span class="log-icon">&times;</span>
            <h2>Errors</h2>
            <p>Issues affecting output or requiring immediate attention.</p>
          </div>
          <strong>{{ groupedLogs.error.length }}</strong>
        </div>
        <div v-if="groupedLogs.error.length === 0" class="empty-log-panel">
          No errors yet.
        </div>
        <div v-else class="log-list">
          <LogLine v-for="log in previewLogs.error" :key="logKey(log)" :log="log" variant="level" />
        </div>
        <button v-if="groupedLogs.error.length > previewLimit" class="view-all-button error" @click="openLogModal('error')">
          View all errors
        </button>
      </article>
    </div>

    <details class="panel terminal-panel large-terminal compact-terminal collapsible">
      <summary class="panel-header">
        <h2>All Logs (Raw Output)</h2>
        <button v-if="logs.length > previewLimit" class="terminal-view-all" @click.prevent="openLogModal('all')">View all</button>
      </summary>
      <div v-if="logs.length === 0" class="empty-terminal">
        No log output yet. Start a dataflow from Run &amp; Monitor to see logs here.
      </div>
      <LogLine v-for="log in previewAllLogs" :key="logKey(log)" :log="log" variant="terminal" />
    </details>

    <Teleport to="body">
      <div v-if="modalType" class="log-modal-backdrop" @click.self="closeLogModal">
        <section class="log-modal-panel">
          <div class="log-modal-header">
            <div>
              <p class="eyebrow">Logs Detail</p>
              <h2>{{ modalTitle }}</h2>
              <span>{{ modalLogs.length }} entries</span>
            </div>
            <button class="secondary" @click="closeLogModal">Close</button>
          </div>
          <div class="log-modal-list">
            <LogLine v-for="log in modalLogs" :key="logKey(log)" :log="log" variant="terminal" />
          </div>
        </section>
      </div>
    </Teleport>
  </section>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, onMounted, onUnmounted, ref, type PropType } from 'vue'
import { getRuntimeLogs, type ApiSource } from '../api'
import { type LogLevel, type StudioLog } from '../data/mockStudio'

type ModalType = LogLevel | 'all'

const previewLimit = 5
const logs = ref<StudioLog[]>([])
const modalType = ref<ModalType | null>(null)
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Backend unavailable'))

const levelText: Record<LogLevel, string> = {
  info: 'INFO',
  warn: 'WARN',
  error: 'ERROR',
}

const LogLine = defineComponent({
  props: {
    log: {
      type: Object as PropType<StudioLog>,
      required: true,
    },
    variant: {
      type: String as PropType<'level' | 'terminal'>,
      required: true,
    },
  },
  setup(props) {
    return () => {
      const log = props.log
      const sourceLocation = [log.sourceFile, log.sourceLine].filter(Boolean).join(':')
      const metadata = [log.source, sourceLocation].filter(Boolean).join(' / ')

      if (props.variant === 'level') {
        return h('div', { class: ['level-log-line', log.level], title: log.rawMessage }, [
          h('time', { datetime: log.timestamp }, log.time),
          h('strong', log.node),
          h('p', log.message),
          h('small', metadata),
        ])
      }

      return h('div', { class: ['log-line', log.level], title: log.rawMessage }, [
        h('time', { datetime: log.timestamp }, log.time),
        h('span', levelText[log.level]),
        h('strong', log.node),
        h('p', log.message),
        h('small', metadata),
      ])
    }
  },
})

const groupedLogs = computed<Record<LogLevel, StudioLog[]>>(() => ({
  info: logs.value.filter((log) => log.level === 'info'),
  warn: logs.value.filter((log) => log.level === 'warn'),
  error: logs.value.filter((log) => log.level === 'error'),
}))

const previewLogs = computed<Record<LogLevel, StudioLog[]>>(() => ({
  info: latest(groupedLogs.value.info),
  warn: latest(groupedLogs.value.warn),
  error: latest(groupedLogs.value.error),
}))

const previewAllLogs = computed(() => latest(logs.value))
const modalLogs = computed(() => (modalType.value === 'all' ? logs.value : modalType.value ? groupedLogs.value[modalType.value] : []))
const modalTitle = computed(() => {
  if (modalType.value === 'info') return 'All Info Logs'
  if (modalType.value === 'warn') return 'All Warning Logs'
  if (modalType.value === 'error') return 'All Error Logs'
  return 'All Raw Stream'
})

let refreshTimer: number | undefined

function latest(items: StudioLog[]) {
  return items.slice(-previewLimit).reverse()
}

function logKey(log: StudioLog) {
  return `${log.timestamp}-${log.node}-${log.rawMessage}`
}

function openLogModal(type: ModalType) {
  modalType.value = type
}

function closeLogModal() {
  modalType.value = null
}

async function refreshLogs() {
  const result = await getRuntimeLogs([])
  if (result.source === 'connected') {
    logs.value = result.data
  }
  apiSource.value = result.source
}

onMounted(async () => {
  await refreshLogs()
  refreshTimer = window.setInterval(refreshLogs, 1200)
})

onUnmounted(() => {
  if (refreshTimer) window.clearInterval(refreshTimer)
})
</script>

<style scoped>
.empty-log-panel {
  color: #94a3b8;
  font-size: 14px;
  padding: 18px;
  text-align: center;
}

.empty-terminal {
  color: #64748b;
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 14px;
  padding: 28px 16px;
  text-align: center;
}

[data-theme="dark"] .empty-log-panel {
  color: #64748b;
}

.log-toolbar {
  flex-wrap: wrap;
  gap: 14px;
}

.log-toolbar > div:first-child {
  min-width: 0;
}

.log-level-grid {
  min-width: 0;
}

.log-level-panel {
  min-width: 0;
  overflow: hidden;
}

.compact-terminal {
  min-width: 0;
  overflow: auto;
}

.terminal-panel :deep(.log-line) {
  min-width: 600px;
}
</style>
