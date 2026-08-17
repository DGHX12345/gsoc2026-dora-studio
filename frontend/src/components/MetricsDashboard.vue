<template>
  <section class="view-stack">
    <!-- M11.5: monitoring control bar -->
    <article class="panel monitoring-bar">
      <div class="monitoring-row">
        <div class="monitoring-master">
          <span class="monitoring-label">{{ t.monitoring.title }}</span>
          <span class="monitoring-master-label">{{ t.monitoring.masterLabel }}</span>
          <button
            :class="['monitoring-switch', { on: masterEnabled }]"
            type="button"
            @click="toggleMaster(!masterEnabled)"
          >{{ masterEnabled ? t.monitoring.on : t.monitoring.off }}</button>
        </div>
        <div class="monitoring-target" :class="{ on: nodeMetricsEnabled }">
          <span class="monitoring-target-label">{{ t.monitoring.nodeMetrics }}</span>
          <button
            :class="['monitoring-switch', 'small', { on: nodeMetricsEnabled }]"
            type="button"
            @click="toggleTarget('nodeMetrics', !nodeMetricsEnabled)"
          >{{ nodeMetricsEnabled ? t.monitoring.on : t.monitoring.off }}</button>
          <span class="monitoring-stat">
            {{ nodeMetricsEnabled ? `${monitoring?.nodeMetrics.sampleCount ?? 0} ${t.monitoring.samples}` : t.monitoring.statusOff }}
          </span>
        </div>
        <div class="monitoring-target" :class="{ on: otelSpansEnabled }">
          <span class="monitoring-target-label">{{ t.monitoring.otelSpans }}</span>
          <button
            :class="['monitoring-switch', 'small', { on: otelSpansEnabled }]"
            type="button"
            @click="toggleTarget('otelSpans', !otelSpansEnabled)"
          >{{ otelSpansEnabled ? t.monitoring.on : t.monitoring.off }}</button>
          <span class="monitoring-stat">
            {{ otelSpansEnabled ? `${monitoring?.otelSpans.sampleCount ?? 0} ${t.monitoring.samples}` : t.monitoring.statusOff }}
          </span>
        </div>
      </div>
    </article>

    <!-- Summary bar -->
    <div class="metric-grid">
      <article :class="['metric-card', 'large-metric', nodeCount > 0 ? 'success' : 'warning']">
        <span>Nodes monitored</span>
        <strong>{{ nodeCount || 'None' }}</strong>
        <small>{{ nodeCount > 0 ? `from ${uniqueDataflows} dataflow(s)` : 'No nodes running' }}</small>
      </article>
      <article :class="['metric-card', 'large-metric', '']">
        <span>Avg CPU</span>
        <strong>{{ avgCpu.toFixed(1) }}%</strong>
        <small>{{ cpuStatusText }}</small>
      </article>
      <article :class="['metric-card', 'large-metric', '']">
        <span>Total Memory</span>
        <strong>{{ totalMemoryMb >= 1024 ? (totalMemoryMb / 1024).toFixed(1) + ' GB' : totalMemoryMb.toFixed(0) + ' MB' }}</strong>
        <small>across {{ nodeCount }} nodes</small>
      </article>
      <article :class="['metric-card', 'large-metric', errorNodes > 0 ? 'warning' : 'success']">
        <span>Health</span>
        <strong>{{ errorNodes > 0 ? `${errorNodes} warning(s)` : 'All healthy' }}</strong>
        <details v-if="errorNodes > 0" class="health-details">
          <summary>{{ t.monitoring.healthDetails }}</summary>
          <ul class="health-reasons">
            <li v-for="reason in healthReasons" :key="reason">{{ reason }}</li>
          </ul>
        </details>
        <small v-else>{{ healthDetail }}</small>
      </article>
    </div>

    <!-- Time series chart -->
    <article class="panel">
      <div class="panel-header">
        <h2>CPU / Memory Timeline</h2>
        <span class="pill">{{ chartTimeRange }}s window</span>
      </div>
      <div v-if="!nodeMetricsEnabled" class="empty-state monitoring-off-state">
        <strong>{{ t.monitoring.disabledTitle }}</strong>
        <p>{{ t.monitoring.disabledHint }}</p>
        <button class="monitoring-enable-btn" type="button" @click="toggleTarget('nodeMetrics', true)">
          {{ t.monitoring.enable }}
        </button>
      </div>
      <div v-else-if="nodeCount === 0" class="empty-state">
        No metrics data available. Start a dora dataflow to see performance charts.
      </div>
      <div v-else class="chart-container">
        <canvas ref="chartCanvasEl" class="metrics-canvas" width="900" height="260"></canvas>
        <div class="chart-legend">
          <span v-for="(color, nodeId) in nodeColors" :key="nodeId" class="chart-legend-item" :title="nodeId">
            <span class="legend-swatch" :style="{ background: color }"></span>
            {{ shortenNodeId(nodeId) }}
          </span>
          <span v-if="nodeCount === 0" class="chart-legend-item muted">No data</span>
        </div>
      </div>
    </article>

    <!-- Per-node gauge cards -->
    <div class="split-grid" style="margin-top: 0;">
      <article class="panel">
        <div class="panel-header">
          <h2>Node Metrics</h2>
          <span :class="['pill', nodeCount > 0 ? 'success' : 'warning']">{{ nodeCount }} nodes</span>
        </div>
        <div v-if="!nodeMetricsEnabled" class="empty-state monitoring-off-state">
          <strong>{{ t.monitoring.disabledTitle }}</strong>
          <p>{{ t.monitoring.disabledHint }}</p>
          <button class="monitoring-enable-btn" type="button" @click="toggleTarget('nodeMetrics', true)">
            {{ t.monitoring.enable }}
          </button>
        </div>
        <div v-else-if="nodeCount === 0" class="empty-state">
          No node metrics available. Metrics are collected from <code>dora node list --format json</code> when nodes are running.
        </div>
        <div v-else class="node-gauge-grid">
          <div v-for="sum in nodes" :key="sum.nodeId" class="node-gauge-card">
            <div class="node-gauge-header">
              <span class="node-id-label" :title="sum.nodeId">{{ shortenNodeId(sum.nodeId) }}</span>
              <span :class="['status-chip', statusChipClass(sum.current.status)]">{{ sum.current.status }}</span>
            </div>
            <div class="gauge-row">
              <div class="gauge-item">
                <span class="gauge-label">CPU</span>
                <div class="gauge-bar-track">
                  <div
                    class="gauge-bar-fill"
                    :style="{ width: Math.min(sum.current.cpuPercent, 100) + '%', background: cpuGradient(sum.current.cpuPercent) }"
                  ></div>
                </div>
                <span class="gauge-value">{{ sum.current.cpuPercent.toFixed(1) }}%</span>
              </div>
              <div class="gauge-item">
                <span class="gauge-label">Mem</span>
                <div class="gauge-bar-track">
                  <div
                    class="gauge-bar-fill"
                    :style="{ width: Math.min((sum.current.memoryMb / 1024) * 100, 100) + '%', background: memGradient(sum.current.memoryMb) }"
                  ></div>
                </div>
                <span class="gauge-value">{{ formatMem(sum.current.memoryMb) }}</span>
              </div>
            </div>
            <div class="node-gauge-footer">
              <small>Restarts: {{ sum.current.restartCount }}</small>
              <small v-if="sum.current.pid">PID: {{ sum.current.pid }}</small>
              <small v-if="sum.dataflowName">{{ sum.dataflowName }}</small>
            </div>
          </div>
        </div>
      </article>

      <!-- Topic-level data: unavailable notice -->
      <article class="panel">
        <div class="panel-header">
          <h2>Topic Metrics</h2>
          <span class="pill warning">unavailable</span>
        </div>
        <div class="empty-state">
          <p>Per-topic latency, queue depth, and frame drop rate require <code>dora topic hz</code> and
          <code>dora topic info</code>. These commands need a running dataflow with the debug flag
          <code>_unstable_debug.publish_all_messages_to_zenoh: true</code> set in its YAML descriptor.</p>
          <p style="margin-top: 0.5rem;">The <code>dora top</code> command does not exist in dora 0.5+.
          Topic-level metrics will become available once a suitable data source is integrated.</p>
        </div>
      </article>
    </div>

    <!-- OTel flame graph (M08) -->
    <article class="panel">
      <div class="panel-header">
        <h2>Trace Flame Graph</h2>
        <span :class="['pill', otelConnected ? 'success' : 'warning']">
          {{ otelConnected ? `${otelSpanCount} spans` : 'OTel not connected' }}
        </span>
      </div>
      <div v-if="!otelSpansEnabled" class="empty-state monitoring-off-state">
        <strong>{{ t.monitoring.disabledTitle }}</strong>
        <p>{{ t.monitoring.disabledHint }}</p>
        <button class="monitoring-enable-btn" type="button" @click="toggleTarget('otelSpans', true)">
          {{ t.monitoring.enable }}
        </button>
      </div>
      <template v-else>
      <div class="fg-controls-row">
        <input
          v-model="spanSearch"
          class="fg-search"
          type="text"
          placeholder="Search span names..."
        />
        <label class="fg-node-label">
          Node
          <select v-model="selectedNode" class="fg-node-select">
            <option value="">All nodes</option>
            <option v-for="n in otelNodes" :key="n" :value="n">{{ shortenNodeId(n) }}</option>
          </select>
        </label>
        <div class="fg-time-btns">
          <button
            v-for="opt in TIME_RANGE_OPTIONS"
            :key="opt.seconds"
            :class="['fg-time-btn', { active: timeRangeSecs === opt.seconds }]"
            @click="timeRangeSecs = opt.seconds"
          >{{ opt.label }}</button>
        </div>
      </div>

      <div v-if="!otelConnected && otelSpanCount === 0" class="empty-state">
        <p>OTel trace backend not reachable ({{ otelEndpoint }}).</p>
        <p style="margin-top: 0.5rem;">dora nodes export spans via OTLP gRPC. To enable flame graphs:</p>
        <ol class="fg-setup-steps">
          <li>Run a Jaeger-compatible backend, e.g. <code>docker run -d -p 4317:4317 -p 16686:16686 jaegertracing/all-in-one:latest</code></li>
          <li>Start dora nodes with <code>OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317</code></li>
          <li>Studio queries the Jaeger API at <code>{{ otelEndpoint }}</code> (override with <code>DORA_OTEL_QUERY_ENDPOINT</code>)</li>
        </ol>
        <p style="margin-top: 0.5rem;">{{ t.monitoring.otelPushHint }}</p>
      </div>
      <div v-else>
        <FlameGraph
          :spans="filteredSpans"
          :search-query="spanSearch"
          :node-colors="otelNodeColors"
        />
      </div>
      </template>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  getMetricsNodes,
  getMonitoringStatus,
  getOtelSpans,
  getOtelStatus,
  setMonitoringToggle,
  type MonitoringStatusResponse,
  type NodeMetricSummaryResponse,
  type NodeMetricSampleResponse,
  type OtelSpanResponse,
} from '../api'
import { useI18n } from '../i18n'
import FlameGraph from './FlameGraph.vue'

// --- M11.5: monitoring control ---

const { t } = useI18n()

const MONITORING_STORAGE_KEY = 'dora-studio-monitoring'
const monitoring = ref<MonitoringStatusResponse | null>(null)

const nodeMetricsEnabled = computed(() => monitoring.value?.nodeMetrics.enabled ?? false)
const otelSpansEnabled = computed(() => monitoring.value?.otelSpans.enabled ?? false)
const masterEnabled = computed(() => nodeMetricsEnabled.value && otelSpansEnabled.value)

function persistMonitoring() {
  try {
    localStorage.setItem(MONITORING_STORAGE_KEY, JSON.stringify({
      nodeMetrics: nodeMetricsEnabled.value,
      otelSpans: otelSpansEnabled.value,
    }))
  } catch { /* storage unavailable */ }
}

async function applyToggle(body: { nodeMetrics?: boolean; otelSpans?: boolean }) {
  try {
    monitoring.value = await setMonitoringToggle(body)
    persistMonitoring()
  } catch {
    // Backend offline — status polling will resync later
  }
}

function toggleTarget(target: 'nodeMetrics' | 'otelSpans', enabled: boolean) {
  void applyToggle({ [target]: enabled })
}

function toggleMaster(enabled: boolean) {
  void applyToggle({ nodeMetrics: enabled, otelSpans: enabled })
}

async function pollMonitoringStatus() {
  try {
    monitoring.value = await getMonitoringStatus()
  } catch {
    // keep last known state
  }
}

// --- State ---

const nodes = ref<NodeMetricSummaryResponse[]>([])
let pollTimer: number | undefined
const CHART_WINDOW_SECS = 60

// --- OTel state (M08) ---

const otelSpans = ref<OtelSpanResponse[]>([])
const otelConnected = ref(false)
const otelSpanCount = ref(0)
const otelEndpoint = ref('http://localhost:16686')
const spanSearch = ref('')
const selectedNode = ref('')
const timeRangeSecs = ref(120)
let otelTimer: number | undefined

const TIME_RANGE_OPTIONS = [
  { label: 'Last 30s', seconds: 30 },
  { label: 'Last 2min', seconds: 120 },
  { label: 'Last 5min', seconds: 300 },
  { label: 'Last 1h', seconds: 3600 },
]

const otelNodes = computed(() => {
  const set = new Set<string>()
  for (const s of otelSpans.value) set.add(s.nodeId)
  return [...set].sort()
})

const otelNodeColors = computed(() => {
  const map: Record<string, string> = {}
  otelNodes.value.forEach((n, i) => { map[n] = NODE_PALETTE[i % NODE_PALETTE.length] })
  return map
})

const filteredSpans = computed(() => {
  const cutoffMicros = Date.now() * 1000 - timeRangeSecs.value * 1_000_000
  return otelSpans.value.filter(s => {
    if (selectedNode.value && s.nodeId !== selectedNode.value) return false
    if (s.startMicros + s.durationMicros < cutoffMicros) return false
    return true
  })
})

async function pollOtel() {
  try {
    const status = await getOtelStatus({
      endpoint: 'http://localhost:16686', connected: false, spanCount: 0, lastError: null,
    })
    otelConnected.value = status.data.connected
    otelSpanCount.value = status.data.spanCount
    otelEndpoint.value = status.data.endpoint
  } catch {
    otelConnected.value = false
  }

  try {
    const spans = await getOtelSpans(undefined, 500)
    otelSpans.value = spans
  } catch {
    // keep existing spans on transient errors
  }
}

// --- Derived ---

const nodeCount = computed(() => nodes.value.length)
const uniqueDataflows = computed(() => {
  const names = new Set(nodes.value.map(n => n.dataflowName).filter(Boolean))
  return names.size || 0
})
const avgCpu = computed(() => {
  if (nodes.value.length === 0) return 0
  return nodes.value.reduce((s, n) => s + n.current.cpuPercent, 0) / nodes.value.length
})
const totalMemoryMb = computed(() =>
  nodes.value.reduce((s, n) => s + n.current.memoryMb, 0)
)
const errorNodes = computed(() =>
  nodes.value.filter(n =>
    n.current.status !== 'Running' || n.current.cpuPercent > 90 || n.current.memoryMb > 4096
  ).length
)
const cpuStatusText = computed(() => avgCpu.value > 80 ? 'High load' : avgCpu.value > 50 ? 'Moderate' : 'Normal')
const healthReasons = computed(() => {
  const reasons: string[] = []
  nodes.value.forEach(n => {
    if (n.current.status !== 'Running') reasons.push(`${shortenNodeId(n.nodeId)}: ${n.current.status}`)
    if (n.current.cpuPercent > 90) reasons.push(`${shortenNodeId(n.nodeId)}: high CPU`)
    if (n.current.memoryMb > 4096) reasons.push(`${shortenNodeId(n.nodeId)}: high mem`)
  })
  return reasons
})
const healthDetail = computed(() =>
  healthReasons.value.length > 0 ? healthReasons.value.join('; ') : 'All nodes running normally'
)
const chartTimeRange = computed(() => CHART_WINDOW_SECS)

// --- Node colors for chart ---

const NODE_PALETTE = ['#5b9bd5', '#ed7d31', '#a5a5a5', '#ffc000', '#4472c4', '#70ad47', '#264478', '#9b59b6', '#1abc9c', '#e74c3c'] as const
const nodeColors = computed(() => {
  const map: Record<string, string> = {}
  nodes.value.forEach((n, i) => { map[n.nodeId] = NODE_PALETTE[i % NODE_PALETTE.length] })
  return map
})

// --- Helpers ---

function shortenNodeId(id: string) {
  if (id.length <= 12) return id
  return id.slice(0, 8) + '...' + id.slice(-4)
}

function formatMem(mb: number) {
  if (mb >= 1024) return (mb / 1024).toFixed(1) + ' GB'
  return mb.toFixed(0) + ' MB'
}

function statusChipClass(status: string) {
  const s = status.toLowerCase()
  if (s === 'running') return 'success'
  if (s === 'stopped' || s === 'exited') return 'warning'
  return 'muted'
}

function cpuGradient(v: number) {
  if (v > 80) return 'var(--col-error, #e74c3c)'
  if (v > 50) return 'var(--col-warning, #f39c12)'
  return 'var(--col-accent, #5b9bd5)'
}

function memGradient(mb: number) {
  if (mb > 4096) return 'var(--col-error, #e74c3c)'
  if (mb > 2048) return 'var(--col-warning, #f39c12)'
  return 'var(--col-accent, #5b9bd5)'
}

// --- Canvas chart ---

const chartCanvasEl = ref<HTMLCanvasElement | null>(null)

function drawChart() {
  const canvas = chartCanvasEl.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const dpr = window.devicePixelRatio || 1
  const rect = canvas.getBoundingClientRect()
  const W = rect.width * dpr
  const H = rect.height * dpr
  canvas.width = W
  canvas.height = H
  ctx.scale(dpr, dpr)

  const pad = { top: 24, right: 16, bottom: 28, left: 48 }
  const pw = rect.width - pad.left - pad.right
  const ph = rect.height - pad.top - pad.bottom

  // Clear
  ctx.clearRect(0, 0, rect.width, rect.height)

  // Background grid
  ctx.strokeStyle = 'var(--col-border, rgba(255,255,255,0.06))'
  ctx.lineWidth = 0.5
  const gridLines = 5
  for (let i = 0; i <= gridLines; i++) {
    const y = pad.top + (ph / gridLines) * i
    ctx.beginPath()
    ctx.moveTo(pad.left, y)
    ctx.lineTo(pad.left + pw, y)
    ctx.stroke()
  }

  // Y axis labels
  ctx.fillStyle = 'var(--col-muted, #888)'
  ctx.font = '10px system-ui'
  ctx.textAlign = 'right'
  for (let i = 0; i <= gridLines; i++) {
    const y = pad.top + (ph / gridLines) * i
    const val = 100 - (100 / gridLines) * i
    ctx.fillText(val + '%', pad.left - 6, y + 4)
  }

  // X axis labels
  ctx.textAlign = 'center'
  const xSteps = 4
  for (let i = 0; i <= xSteps; i++) {
    const x = pad.left + (pw / xSteps) * i
    const secs = -CHART_WINDOW_SECS + (CHART_WINDOW_SECS / xSteps) * i
    ctx.fillText(secs + 's', x, pad.top + ph + 18)
  }

  // Data lines
  const now = Math.floor(Date.now() / 1000)
  nodes.value.forEach((node, nodeIdx) => {
    const history = node.history
    if (history.length < 2) return

    const color = NODE_PALETTE[nodeIdx % NODE_PALETTE.length]

    // Draw CPU line
    ctx.strokeStyle = color
    ctx.lineWidth = 1.5
    ctx.beginPath()
    let first = true
    for (const sample of history) {
      const age = now - sample.timestampSecs
      if (age > CHART_WINDOW_SECS || age < 0) continue
      const x = pad.left + pw * (1 - age / CHART_WINDOW_SECS)
      const y = pad.top + ph * (1 - Math.min(sample.cpuPercent, 100) / 100)
      if (first) { ctx.moveTo(x, y); first = false }
      else { ctx.lineTo(x, y) }
    }
    ctx.stroke()

    // Draw memory line (dashed)
    ctx.strokeStyle = color
    ctx.lineWidth = 1
    ctx.setLineDash([4, 3])
    ctx.beginPath()
    first = true
    for (const sample of history) {
      const age = now - sample.timestampSecs
      if (age > CHART_WINDOW_SECS || age < 0) continue
      const x = pad.left + pw * (1 - age / CHART_WINDOW_SECS)
      // Memory: normalize to 0-100% using 4096MB as max
      const memPercent = Math.min((sample.memoryMb / 4096) * 100, 100)
      const y = pad.top + ph * (1 - memPercent / 100)
      if (first) { ctx.moveTo(x, y); first = false }
      else { ctx.lineTo(x, y) }
    }
    ctx.stroke()
    ctx.setLineDash([])
  })
}

// --- Polling ---

async function poll() {
  const result = await getMetricsNodes([])
  if (result.data && result.data.length > 0) {
    // Sort by nodeId for stable chart colors
    nodes.value = [...result.data].sort((a, b) => a.nodeId.localeCompare(b.nodeId))
    await nextTick()
    drawChart()
  } else {
    nodes.value = []
    await nextTick()
    drawChart()
  }
}

let resizeObserver: ResizeObserver | undefined

onMounted(() => {
  poll()
  pollTimer = window.setInterval(poll, 2000)

  pollOtel()
  otelTimer = window.setInterval(pollOtel, 5000)

  pollMonitoringStatus()
  window.setInterval(pollMonitoringStatus, 2000)

  // Restore persisted monitoring state — the backend boots with monitoring
  // off, so re-apply any target the user had left on.
  try {
    const stored = localStorage.getItem(MONITORING_STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored) as { nodeMetrics?: boolean; otelSpans?: boolean }
      if (parsed.nodeMetrics || parsed.otelSpans) {
        void applyToggle({
          nodeMetrics: parsed.nodeMetrics,
          otelSpans: parsed.otelSpans,
        })
      }
    }
  } catch { /* ignore corrupted storage */ }

  if (chartCanvasEl.value) {
    resizeObserver = new ResizeObserver(() => drawChart())
    resizeObserver.observe(chartCanvasEl.value)
  }
})

onUnmounted(() => {
  if (pollTimer) window.clearInterval(pollTimer)
  if (otelTimer) window.clearInterval(otelTimer)
  if (resizeObserver) resizeObserver.disconnect()
})
</script>

<style scoped>
/* --- M11.5: monitoring control bar --- */
.monitoring-bar { padding: 10px 14px; }
.monitoring-row {
  display: flex; align-items: center; gap: 24px; flex-wrap: wrap;
}
.monitoring-master {
  display: flex; align-items: center; gap: 10px;
}
.monitoring-label {
  font-size: 16px; font-weight: 600; color: var(--text-primary);
}
.monitoring-master-label {
  font-size: 13px; color: var(--text-secondary);
}
.monitoring-switch {
  min-width: 56px; padding: 8px 16px;
  border: 1px solid var(--border-card);
  border-radius: 999px;
  background: var(--bg-surface);
  color: var(--text-primary);
  font-size: 14px; font-weight: 600; cursor: pointer;
}
.monitoring-switch.small { min-width: 44px; padding: 5px 12px; font-size: 13px; }
.monitoring-switch.on {
  background: color-mix(in srgb, #22c55e 22%, transparent);
  border-color: #22c55e;
  color: var(--text-primary);
}
.monitoring-target {
  display: flex; align-items: center; gap: 10px;
  padding: 4px 10px;
  border: 1px solid var(--border-card);
  border-radius: 8px;
}
.monitoring-target.on { border-color: color-mix(in srgb, #22c55e 35%, transparent); }
.monitoring-target-label {
  font-size: 14px; color: var(--text-primary);
}
.monitoring-stat {
  font-size: 13px; font-family: monospace; color: var(--text-secondary);
  min-width: 90px; text-align: right;
}
.monitoring-off-state {
  display: flex; flex-direction: column; align-items: center; gap: 8px;
  padding: 28px 16px !important;
}
.monitoring-off-state strong { font-size: 15px; color: var(--text-primary); }
.monitoring-off-state p {
  margin: 0; font-size: 13px; color: var(--text-secondary);
  max-width: 480px; text-align: center;
}
.monitoring-enable-btn {
  margin-top: 4px;
  padding: 10px 22px;
  border: none; border-radius: 8px;
  background: var(--col-accent, #5b9bd5);
  color: #fff; font-size: 14px; font-weight: 600; cursor: pointer;
}
.monitoring-enable-btn:hover { filter: brightness(1.1); }

.metrics-canvas {
  width: 100%;
  height: 260px;
  display: block;
}

.chart-container {
  background: var(--col-surface, rgba(255,255,255,0.02));
  border-radius: var(--radius-md, 6px);
  overflow: hidden;
}

.chart-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  padding: 8px 16px 12px;
}

.chart-legend-item {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text-secondary, #aaa);
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.legend-swatch {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  display: inline-block;
  flex-shrink: 0;
}

.health-details {
  margin-top: 6px;
}

.health-details summary {
  cursor: pointer;
  font-size: 13px;
  color: var(--text-primary);
  user-select: none;
}

.health-reasons {
  list-style: disc;
  margin: 6px 0 0 18px;
  padding: 0;
  max-height: 140px;
  overflow-y: auto;
}

.health-reasons li {
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-secondary);
}

.node-gauge-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}

.node-gauge-card {
  background: var(--bg-surface, rgba(255,255,255,0.03));
  border: 1px solid var(--border-card, rgba(255,255,255,0.06));
  border-radius: var(--radius-md, 6px);
  padding: 12px;
}

.node-gauge-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.node-id-label {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary, #eee);
  max-width: 62%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-chip {
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 10px;
  font-weight: 600;
}

.status-chip.success {
  background: rgba(39, 174, 96, 0.15);
  color: #27ae60;
}

.status-chip.warning {
  background: rgba(243, 156, 18, 0.15);
  color: #f39c12;
}

.status-chip.muted {
  background: rgba(255,255,255,0.05);
  color: var(--col-muted, #888);
}

.gauge-row {
  display: flex;
  gap: 8px;
}

.gauge-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.gauge-label {
  font-size: 9px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--col-muted, #888);
}

.gauge-bar-track {
  height: 6px;
  background: var(--col-surface-raised, rgba(255,255,255,0.05));
  border-radius: 3px;
  overflow: hidden;
}

.gauge-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.5s ease;
}

.gauge-value {
  font-size: 11px;
  color: var(--col-text-secondary, #aaa);
  text-align: right;
}

.node-gauge-footer {
  display: flex;
  gap: 10px;
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px solid var(--col-border, rgba(255,255,255,0.05));
}

.node-gauge-footer small {
  font-size: 10px;
  color: var(--col-muted, #888);
}

/* --- OTel flame graph controls (M08) --- */

.fg-controls-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 16px;
}

.fg-search {
  background: var(--col-surface, rgba(255,255,255,0.03));
  border: 1px solid var(--col-border, rgba(255,255,255,0.1));
  border-radius: 6px;
  color: var(--col-text, #eee);
  padding: 8px 12px;
  font-size: 13px;
  min-width: 220px;
  flex: 1;
  max-width: 340px;
}

.fg-search:focus {
  outline: 1px solid var(--col-accent, #5b9bd5);
  border-color: var(--col-accent, #5b9bd5);
}

.fg-node-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--col-text, #eee);
  white-space: nowrap;
}

.fg-node-select {
  background: var(--col-surface-raised, rgba(255,255,255,0.06));
  border: 1px solid var(--col-border, rgba(255,255,255,0.16));
  border-radius: 6px;
  color: var(--col-text, #eee);
  padding: 8px 10px;
  font-size: 13px;
  min-width: 160px;
  max-width: 240px;
  cursor: pointer;
}

.fg-node-select option {
  color: #222;
}

.fg-node-select:focus {
  outline: 1px solid var(--col-accent, #5b9bd5);
  border-color: var(--col-accent, #5b9bd5);
}

.fg-time-btns {
  display: flex;
  gap: 4px;
}

.fg-time-btn {
  background: var(--col-surface, rgba(255,255,255,0.03));
  border: 1px solid var(--col-border, rgba(255,255,255,0.1));
  color: var(--col-text-secondary, #aaa);
  font-size: 12px;
  padding: 7px 12px;
  cursor: pointer;
  border-radius: 6px;
  transition: background 0.15s, color 0.15s;
}

.fg-time-btn:hover {
  color: var(--col-text, #eee);
  background: var(--col-surface-raised, rgba(255,255,255,0.06));
}

.fg-time-btn.active {
  background: var(--col-accent, #5b9bd5);
  border-color: var(--col-accent, #5b9bd5);
  color: #fff;
}

.fg-setup-steps {
  text-align: left;
  margin: 8px auto 0;
  max-width: 620px;
  padding-left: 20px;
}

.fg-setup-steps li {
  margin: 6px 0;
  color: var(--col-text-secondary, #aaa);
  font-size: 13px;
  line-height: 1.6;
}
</style>
