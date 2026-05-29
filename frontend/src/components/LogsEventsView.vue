<template>
  <section class="view-stack">
    <div class="panel log-toolbar large-action-panel">
      <div>
        <p class="eyebrow">Logs & Events</p>
        <h2>运行信号中心</h2>
        <p class="muted">这里会轮询 backend runtime logs，用真实 dora run 输出测试日志分区。</p>
      </div>
      <div class="filter-row">
        <span>全部节点</span>
        <span>三类分区</span>
        <span :class="apiSource === 'connected' ? 'api-connected' : 'api-fallback'">{{ apiSourceText }}</span>
      </div>
    </div>

    <div class="log-level-grid">
      <article class="log-level-panel info-panel">
        <div class="log-level-header">
          <div>
            <span class="log-icon">i</span>
            <h2>常规日志</h2>
            <p>正常运行事件和数据流进展。</p>
          </div>
          <strong>{{ groupedLogs.info.length }}</strong>
        </div>
        <div class="log-list">
          <div v-for="log in previewLogs.info" :key="`${log.time}-${log.node}-${log.message}`" class="level-log-line info">
            <time>{{ log.time }}</time>
            <strong>{{ log.node }}</strong>
            <p>{{ log.message }}</p>
          </div>
        </div>
        <button v-if="groupedLogs.info.length > previewLimit" class="view-all-button info" @click="openLogModal('info')">
          查看全部日志
        </button>
      </article>

      <article class="log-level-panel warn-panel">
        <div class="log-level-header">
          <div>
            <span class="log-icon">!</span>
            <h2>警告日志</h2>
            <p>需要关注但尚未中断运行的异常趋势。</p>
          </div>
          <strong>{{ groupedLogs.warn.length }}</strong>
        </div>
        <div class="log-list">
          <div v-for="log in previewLogs.warn" :key="`${log.time}-${log.node}-${log.message}`" class="level-log-line warn">
            <time>{{ log.time }}</time>
            <strong>{{ log.node }}</strong>
            <p>{{ log.message }}</p>
          </div>
        </div>
        <button v-if="groupedLogs.warn.length > previewLimit" class="view-all-button warn" @click="openLogModal('warn')">
          查看全部日志
        </button>
      </article>

      <article class="log-level-panel error-panel">
        <div class="log-level-header">
          <div>
            <span class="log-icon">×</span>
            <h2>错误日志</h2>
            <p>影响链路输出或需要立即定位的问题。</p>
          </div>
          <strong>{{ groupedLogs.error.length }}</strong>
        </div>
        <div class="log-list">
          <div v-for="log in previewLogs.error" :key="`${log.time}-${log.node}-${log.message}`" class="level-log-line error">
            <time>{{ log.time }}</time>
            <strong>{{ log.node }}</strong>
            <p>{{ log.message }}</p>
          </div>
        </div>
        <button v-if="groupedLogs.error.length > previewLimit" class="view-all-button error" @click="openLogModal('error')">
          查看全部日志
        </button>
      </article>
    </div>

    <article class="panel terminal-panel large-terminal compact-terminal">
      <div class="panel-header">
        <h2>原始合并流</h2>
        <button v-if="logs.length > previewLimit" class="terminal-view-all" @click="openLogModal('all')">查看全部日志</button>
      </div>
      <div v-for="log in previewAllLogs" :key="`${log.time}-${log.node}-${log.message}`" :class="['log-line', log.level]">
        <time>{{ log.time }}</time>
        <span>{{ levelText[log.level] }}</span>
        <strong>{{ log.node }}</strong>
        <p>{{ log.message }}</p>
      </div>
    </article>

    <Teleport to="body">
      <div v-if="modalType" class="log-modal-backdrop" @click.self="closeLogModal">
        <section class="log-modal-panel">
          <div class="log-modal-header">
            <div>
              <p class="eyebrow">Logs Detail</p>
              <h2>{{ modalTitle }}</h2>
              <span>{{ modalLogs.length }} 条日志</span>
            </div>
            <button class="secondary" @click="closeLogModal">关闭</button>
          </div>
          <div class="log-modal-list">
            <div v-for="log in modalLogs" :key="`${log.time}-${log.node}-${log.message}`" :class="['log-line', log.level]">
              <time>{{ log.time }}</time>
              <span>{{ levelText[log.level] }}</span>
              <strong>{{ log.node }}</strong>
              <p>{{ log.message }}</p>
            </div>
          </div>
        </section>
      </div>
    </Teleport>

    <div class="split-grid">
      <article class="panel">
        <div class="panel-header">
          <h2>当前调试重点</h2>
          <span class="pill warning">队列</span>
        </div>
        <p class="muted">
          detector 节点处于退化状态，因为 pending messages 持续增长。警告区会优先暴露这种趋势，错误区只保留真正影响输出链路的问题。
        </p>
      </article>

      <article class="panel">
        <div class="panel-header">
          <h2>后续扩展入口</h2>
          <span class="pill">预留</span>
        </div>
        <div class="hook-list">
          <span>Trace 时间线</span>
          <span>Topic 预览</span>
          <span>数据集记录</span>
          <span>训练导出</span>
        </div>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { getRuntimeLogs, type ApiSource } from '../api'
import { logs as fallbackLogs, type LogLevel, type StudioLog } from '../data/mockStudio'

type ModalType = LogLevel | 'all'

const previewLimit = 5
const logs = ref<StudioLog[]>(fallbackLogs)
const modalType = ref<ModalType | null>(null)
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))

const levelText: Record<LogLevel, string> = {
  info: '信息',
  warn: '警告',
  error: '错误',
}

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
  if (modalType.value === 'info') return '全部常规日志'
  if (modalType.value === 'warn') return '全部警告日志'
  if (modalType.value === 'error') return '全部错误日志'
  return '全部原始合并流'
})

let refreshTimer: number | undefined

function latest(items: StudioLog[]) {
  return items.slice(-previewLimit).reverse()
}

function openLogModal(type: ModalType) {
  modalType.value = type
}

function closeLogModal() {
  modalType.value = null
}

async function refreshLogs() {
  const result = await getRuntimeLogs(fallbackLogs)
  logs.value = result.data.length > 0 ? result.data : fallbackLogs
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
