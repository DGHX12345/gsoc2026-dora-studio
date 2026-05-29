<template>
  <section class="view-stack">
    <div class="panel run-panel large-action-panel">
      <div>
        <p class="eyebrow">Run & Monitor</p>
        <h2>robot-perception-demo.yml</h2>
        <p class="muted">从这里启动或停止 examples/robot-perception-test/dataflow.yml，用真实 Dora run 输出测试 Studio。</p>
      </div>
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
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  getNodes,
  getRuntimeStatus,
  startRuntime,
  stopRuntime,
  type ApiSource,
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
}

const nodes = ref<NodeMetricsResponse[]>(fallbackNodes)
const runtime = ref<RuntimeStateResponse>(fallbackRuntime)
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))
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
}

async function startDataflow() {
  runtime.value = await startRuntime()
}

async function stopDataflow() {
  runtime.value = await stopRuntime()
}

async function restartDataflow() {
  await stopDataflow()
  runtime.value = await startRuntime()
}

onMounted(async () => {
  const [nodesResult, runtimeResult] = await Promise.all([
    getNodes('robot-perception-demo', fallbackNodes),
    getRuntimeStatus(fallbackRuntime),
  ])
  nodes.value = nodesResult.data
  runtime.value = runtimeResult.data
  apiSource.value = nodesResult.source === 'connected' || runtimeResult.source === 'connected' ? 'connected' : 'fallback'
})
</script>
