<template>
  <section class="dashboard-grid">
    <div class="hero-card dashboard-hero">
      <div>
        <p class="eyebrow">Dora Studio</p>
        <h1>面向 Dora dataflow 的可视化观测与调试工作台</h1>
        <p class="hero-copy">
          先通过静态原型明确产品形态，后续逐步接入真实的 Dora descriptor、runtime、logs 和 metrics API。
        </p>
      </div>
      <div class="hero-actions">
        <button>打开 dataflow</button>
        <button class="secondary">连接运行时</button>
      </div>
    </div>

    <div class="metric-grid dashboard-metrics">
      <article class="metric-card success large-metric">
        <span>Coordinator</span>
        <strong>{{ system.coordinator }}</strong>
        <small>{{ system.version }}</small>
      </article>
      <article class="metric-card success large-metric">
        <span>Daemon</span>
        <strong>{{ system.daemon }}</strong>
        <small>心跳延迟 42ms</small>
      </article>
      <article class="metric-card large-metric">
        <span>运行中 Dataflow</span>
        <strong>{{ system.runningDataflows }}</strong>
        <small>{{ system.activeNodes }} 个活跃节点</small>
      </article>
      <article class="metric-card danger large-metric">
        <span>最近错误</span>
        <strong>{{ system.errorCount }}</strong>
        <small>robot_bridge 需要关注</small>
      </article>
    </div>

    <article class="panel active-flow-panel">
      <div class="panel-header">
        <h2>当前 dataflow</h2>
        <span class="pill running">运行中</span>
      </div>
      <div class="flow-summary prominent-flow">
        <strong>robot-perception-demo.yml</strong>
        <span>camera → detector → planner → robot_bridge</span>
      </div>
      <div class="progress-list big-progress">
        <div><span>结构图解析</span><b>已就绪</b></div>
        <div><span>运行时连接</span><b>Mock</b></div>
        <div><span>日志流</span><b>已启用</b></div>
        <div><span>数据采集出口</span><b>已预留</b></div>
      </div>
    </article>

    <article class="panel events-panel">
      <div class="panel-header">
        <h2>最近事件</h2>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>
      <ul class="event-list large-events">
        <li v-for="log in logs.slice(0, 5)" :key="`${log.time}-${log.node}`">
          <span :class="['dot', log.level]"></span>
          <div>
            <strong>{{ log.node }}</strong>
            <p>{{ log.message }}</p>
          </div>
          <time>{{ log.time }}</time>
        </li>
      </ul>
    </article>

    <article class="panel visualization-panel throughput-panel">
      <div class="panel-header">
        <div>
          <h2>数据流吞吐预览</h2>
          <p>这里不是空白区，而是预留给真实 runtime metrics 的可视化区域。</p>
        </div>
        <span class="pill">Mock chart</span>
      </div>
      <div class="line-chart" aria-label="mock throughput chart">
        <span
          v-for="(value, index) in throughputSeries"
          :key="`throughput-${index}`"
          :style="{ height: `${value}px` }"
        ></span>
      </div>
      <div class="chart-footer">
        <span>消息吞吐</span>
        <strong>148 msg/s 峰值</strong>
      </div>
    </article>

    <article class="panel visualization-panel resource-panel">
      <div class="panel-header">
        <div>
          <h2>节点资源占用</h2>
          <p>后续会接入 Dora NodeInfo 中的 CPU、内存和 pending message。</p>
        </div>
        <span class="pill warning">预留</span>
      </div>
      <div class="bar-list">
        <div v-for="bar in resourceBars" :key="bar.label" class="bar-row">
          <span>{{ bar.label }}</span>
          <div><i :style="{ width: `${bar.value}%` }"></i></div>
          <b>{{ bar.value }}%</b>
        </div>
      </div>
    </article>

    <article class="panel visualization-panel debug-panel">
      <div class="panel-header">
        <div>
          <h2>未来 Debug / 数据工作流入口</h2>
          <p>用于承接 topic preview、trace timeline、record/replay 和训练数据导出。</p>
        </div>
        <span class="pill">Roadmap</span>
      </div>
      <div class="debug-roadmap">
        <div><strong>Topic Preview</strong><span>查看关键 topic payload</span></div>
        <div><strong>Trace Timeline</strong><span>定位跨节点延迟</span></div>
        <div><strong>Dataset Recorder</strong><span>采集训练数据</span></div>
        <div><strong>Replay</strong><span>回放并复现实验</span></div>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getLogs, getSystemStatus, type ApiSource } from '../api'
import {
  logs as fallbackLogs,
  resourceBars,
  systemStatus as fallbackSystem,
  throughputSeries,
  type StudioLog,
} from '../data/mockStudio'

const system = ref(fallbackSystem)
const logs = ref<StudioLog[]>(fallbackLogs)
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))

onMounted(async () => {
  const [systemResult, logsResult] = await Promise.all([
    getSystemStatus(fallbackSystem),
    getLogs('robot-perception-demo', fallbackLogs),
  ])

  system.value = systemResult.data
  logs.value = logsResult.data
  apiSource.value = systemResult.source === 'connected' || logsResult.source === 'connected' ? 'connected' : 'fallback'
})
</script>
