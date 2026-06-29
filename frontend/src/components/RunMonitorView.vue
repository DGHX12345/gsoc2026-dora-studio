<template>
  <section class="view-stack">
    <div class="panel run-panel large-action-panel">
      <div>
        <p class="eyebrow">Run & Monitor</p>
        <h2>{{ selectedDataflow?.name ?? 'No dataflow selected' }}</h2>
        <p class="muted">
          从这里启动或停止选中的本地 dataflow，用真实 Dora run 输出测试 Studio。
          <span v-if="runtime.dataflowPath">当前路径：{{ runtime.dataflowPath }}</span>
        </p>
      </div>
      <label class="flow-select">
        <span>目标 Dataflow</span>
        <select v-model="selectedDataflowId" @change="refreshSelectedNodes">
          <option v-for="flow in dataflows" :key="flow.id" :value="flow.id">
            {{ flow.name }}
          </option>
        </select>
      </label>
      <p v-if="apiError" class="muted">{{ apiError }}</p>
      <div class="control-row">
        <button class="secondary" @click="refreshRuntime">刷新状态</button>
        <button @click="startDataflow" :disabled="runtime.status === 'running'">启动示例</button>
        <button class="secondary" @click="restartDataflow">重启示例</button>
        <button class="danger-button" @click="stopDataflow" :disabled="runtime.status !== 'running'">停止示例</button>
      </div>
    </div>

    <div class="metric-grid">
      <article :class="['metric-card', 'large-metric', runtime.status === 'running' ? 'success' : 'warning']">
        <span>真实运行状态</span>
        <strong>{{ runtimeStatusText }}</strong>
        <small>{{ runtime.pid ? `PID ${runtime.pid}` : '无运行进程' }}</small>
      </article>
      <article class="metric-card large-metric"><span>总 CPU</span><strong>113%</strong><small>当前仍为 mock 指标</small></article>
      <article class="metric-card warning large-metric"><span>Pending 消息</span><strong>30</strong><small>detector 队列偏高</small></article>
      <article class="metric-card large-metric"><span>运行消息</span><strong>{{ runtime.status }}</strong><small>{{ runtime.lastMessage }}</small></article>
    </div>

    <article class="panel">
      <div class="panel-header">
        <h2>节点指标</h2>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>节点</th>
              <th>状态</th>
              <th>CPU</th>
              <th>内存</th>
              <th>重启</th>
              <th>Pending</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="node in nodes" :key="node.id">
              <td><strong>{{ node.label }}</strong><span>{{ node.kind }}</span></td>
              <td><span :class="['status-chip', node.status]">{{ statusText[node.status] }}</span></td>
              <td>{{ node.cpu }}%</td>
              <td>{{ node.memory }} MB</td>
              <td>{{ node.restarts }}</td>
              <td>{{ node.pending }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </article>

    <p v-if="runtime.status === 'running'" class="muted" style="text-align: center; padding: 10px 0;">
      运行中 · 切换到 <strong>Logs & Events</strong> 页面查看实时日志输出
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
import { dataflowNodes, type NodeStatus } from '../data/mockStudio'

const fallbackNodes: NodeMetricsResponse[] = dataflowNodes.map(({ id, label, kind, status, cpu, memory, restarts, pending }) => ({
  id,
  label,
  kind,
  status,
  cpu,
  memory,
  restarts,
  pending,
}))

const fallbackRuntime: RuntimeStateResponse = {
  status: 'stopped',
  pid: null,
  lastMessage: 'Backend runtime API is not connected.',
  dataflowId: null,
  dataflowPath: null,
}

const fallbackDataflows: DataflowSummaryResponse[] = [
  {
    id: 'robot-perception-demo',
    name: 'robot-perception-demo.yml',
    status: 'running',
    nodeCount: dataflowNodes.length,
    edgeCount: 0,
  },
]

const nodes = ref<NodeMetricsResponse[]>(fallbackNodes)
const runtime = ref<RuntimeStateResponse>(fallbackRuntime)
const dataflows = ref<DataflowSummaryResponse[]>(fallbackDataflows)
const selectedDataflowId = ref(fallbackDataflows[0].id)
const apiError = ref('')
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))
const selectedDataflow = computed(
  () => dataflows.value.find((flow) => flow.id === selectedDataflowId.value) ?? dataflows.value[0],
)
const runtimeStatusText = computed(() => {
  if (runtime.value.status === 'running') return '运行中'
  if (runtime.value.status === 'failed') return '失败'
  return '已停止'
})

const statusText: Record<NodeStatus, string> = {
  running: '运行中',
  degraded: '退化',
  failed: '失败',
  stopped: '已停止',
}

async function refreshRuntime() {
  const result = await getRuntimeStatus(fallbackRuntime)
  runtime.value = result.data
  apiSource.value = result.source
  await refreshSelectedNodes()
}

async function startDataflow() {
  runtime.value = await startDataflowRuntime(selectedDataflowId.value)
  await refreshSelectedNodes()
}

async function stopDataflow() {
  runtime.value = await stopDataflowRuntime(selectedDataflowId.value)
  await refreshSelectedNodes()
}

async function restartDataflow() {
  runtime.value = await restartDataflowRuntime(selectedDataflowId.value)
  await refreshSelectedNodes()
}

async function refreshSelectedNodes() {
  const result = await getNodes(selectedDataflowId.value, fallbackNodes)
  nodes.value = result.data
  apiSource.value = result.source
  apiError.value = result.error ?? ''
}

onMounted(async () => {
  const [dataflowsResult, runtimeResult] = await Promise.all([
    getDataflows(fallbackDataflows),
    getRuntimeStatus(fallbackRuntime),
  ])

  dataflows.value = dataflowsResult.data
  selectedDataflowId.value = dataflows.value[0]?.id ?? fallbackDataflows[0].id
  runtime.value = runtimeResult.data
  apiSource.value = dataflowsResult.source === 'connected' || runtimeResult.source === 'connected' ? 'connected' : 'fallback'
  apiError.value = dataflowsResult.error ?? runtimeResult.error ?? ''

  await refreshSelectedNodes()
})
</script>
