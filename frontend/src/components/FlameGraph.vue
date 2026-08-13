<template>
  <div class="flame-graph-root">
    <!-- Toolbar -->
    <div class="fg-toolbar">
      <span class="fg-breadcrumb">
        <button class="fg-crumb" @click="resetZoom">root</button>
        <template v-for="(crumb, i) in zoomPath" :key="crumb.spanId">
          <span class="fg-sep">&rsaquo;</span>
          <button class="fg-crumb" @click="zoomToPath(i)">{{ crumb.operationName }}</button>
        </template>
      </span>
      <span class="fg-info">{{ flatVisible.length }} spans · {{ formatUs(rootDuration) }} total</span>
    </div>

    <!-- Empty state -->
    <div v-if="spans.length === 0" class="fg-empty">
      No trace spans available. Start dora nodes with OTel export configured
      (OTEL_EXPORTER_OTLP_ENDPOINT) and a Jaeger-compatible backend to see flame graphs.
    </div>

    <!-- Flame graph body -->
    <div v-else class="fg-body" @click="onBackgroundClick">
      <div class="fg-row" v-for="(row, depth) in layoutRows" :key="depth">
        <div
          v-for="cell in row"
          :key="cell.span.spanId"
          class="fg-cell"
          :class="{ 'fg-match': cell.matchesSearch, 'fg-faded': !cell.matchesSearch && hasSearch }"
          :style="{
            left: cell.leftPct + '%',
            width: Math.max(cell.widthPct, 0.15) + '%',
            background: nodeColors[cell.span.nodeId] || '#5b9bd5',
            opacity: 0.55 + 0.45 * (cell.depth / Math.max(maxDepth, 1)),
          }"
          :title="cellTitle(cell)"
          @click.stop="zoomInto(cell.span)"
          @mouseenter="onHover(cell)"
          @mouseleave="onHoverEnd"
        >
          <span class="fg-label">{{ cell.span.operationName }}</span>
        </div>
      </div>
    </div>

    <!-- Hover tooltip -->
    <div
      v-if="hovered"
      class="fg-tooltip"
      :style="{ left: hoverX + 'px', top: hoverY + 'px' }"
    >
      <strong>{{ hovered.span.operationName }}</strong>
      <span>{{ hovered.span.nodeId }}</span>
      <span>duration: {{ formatUs(hovered.span.durationMicros) }}</span>
      <span>children: {{ hovered.childrenCount }}</span>
      <span v-for="(v, k) in hovered.span.attributes" :key="k" class="fg-attr">{{ k }}: {{ v }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { OtelSpanResponse } from '../api'

const props = defineProps<{
  spans: OtelSpanResponse[]
  searchQuery: string
  nodeColors: Record<string, string>
}>()

// --- Tree building ---

type Cell = {
  span: OtelSpanResponse
  depth: number
  leftPct: number
  widthPct: number
  matchesSearch: boolean
  childrenCount: number
}

type Tree = { span: OtelSpanResponse; children: Tree[] }

function buildTrees(flat: OtelSpanResponse[]): Tree[] {
  const byId = new Map<string, OtelSpanResponse>()
  for (const s of flat) byId.set(s.spanId, s)

  const roots: Tree[] = []
  const childrenOf = new Map<string, OtelSpanResponse[]>()
  for (const s of flat) {
    const parentId = s.parentSpanId
    if (parentId && byId.has(parentId)) {
      const list = childrenOf.get(parentId) ?? []
      list.push(s)
      childrenOf.set(parentId, list)
    }
  }

  function build(span: OtelSpanResponse): Tree {
    const kids = (childrenOf.get(span.spanId) ?? [])
      .sort((a, b) => a.startMicros - b.startMicros)
      .map(build)
    return { span, children: kids }
  }

  for (const s of flat) {
    const parentId = s.parentSpanId
    if (!parentId || !byId.has(parentId)) {
      roots.push(build(s))
    }
  }
  // Sort roots by start time
  return roots.sort((a, b) => a.span.startMicros - b.span.startMicros)
}

// --- Zoom state ---

const zoomPath = ref<OtelSpanResponse[]>([])

function zoomInto(span: OtelSpanResponse) {
  zoomPath.value.push(span)
}

function zoomToPath(index: number) {
  zoomPath.value = zoomPath.value.slice(0, index + 1)
}

function resetZoom() {
  zoomPath.value = []
}

function onBackgroundClick() {
  if (zoomPath.value.length > 0) {
    zoomPath.value.pop()
  }
}

// --- Layout computation ---

const hasSearch = computed(() => props.searchQuery.trim().length > 0)

const layout = computed(() => {
  const trees = buildTrees(props.spans)
  const zoomRoot = zoomPath.value.length > 0
    ? zoomPath.value[zoomPath.value.length - 1]
    : null

  // Determine visible root(s) and time window
  let rootSpan: OtelSpanResponse | null = zoomRoot
  let visibleRoots: Tree[] = trees
  let windowStart = 0
  let windowEnd = 0

  if (zoomRoot) {
    // Find the zoom root subtree
    function findSubtree(node: Tree): Tree | null {
      if (node.span.spanId === zoomRoot!.spanId) return node
      for (const c of node.children) {
        const found = findSubtree(c)
        if (found) return found
      }
      return null
    }
    const subtree = trees.map(findSubtree).find(Boolean)
    if (subtree) {
      visibleRoots = [subtree]
      rootSpan = subtree.span
    }
  }

  if (visibleRoots.length > 0) {
    windowStart = Math.min(...visibleRoots.map(r => r.span.startMicros))
    windowEnd = Math.max(...visibleRoots.map(r => r.span.startMicros + r.span.durationMicros))
  }

  const windowDuration = Math.max(windowEnd - windowStart, 1)
  const query = props.searchQuery.trim().toLowerCase()

  const rows: Cell[][] = []
  const maxDepthTracker = { value: 0 }

  function layoutTree(node: Tree, depth: number) {
    maxDepthTracker.value = Math.max(maxDepthTracker.value, depth)
    const row = rows[depth] ?? (rows[depth] = [])
    const leftPct = ((node.span.startMicros - windowStart) / windowDuration) * 100
    const widthPct = (node.span.durationMicros / windowDuration) * 100
    row.push({
      span: node.span,
      depth,
      leftPct,
      widthPct,
      matchesSearch: query.length === 0 || node.span.operationName.toLowerCase().includes(query),
      childrenCount: node.children.length,
    })
    for (const child of node.children) layoutTree(child, depth + 1)
  }

  for (const root of visibleRoots) layoutTree(root, 0)

  return { rows, maxDepth: maxDepthTracker.value, windowDuration, rootSpan }
})

const layoutRows = computed(() => layout.value.rows)
const maxDepth = computed(() => layout.value.maxDepth)
const rootDuration = computed(() => layout.value.windowDuration)
const flatVisible = computed(() => layout.value.rows.flat())

// --- Hover ---

const hovered = ref<Cell | null>(null)
const hoverX = ref(0)
const hoverY = ref(0)

function onHover(cell: Cell) {
  hovered.value = cell
}

function onHoverEnd() {
  hovered.value = null
}

function cellTitle(cell: Cell) {
  const attrs = Object.entries(cell.span.attributes)
    .map(([k, v]) => `${k}=${v}`)
    .join(', ')
  return `${cell.span.operationName} — ${formatUs(cell.span.durationMicros)}${attrs ? ` [${attrs}]` : ''}`
}

function formatUs(us: number) {
  if (us >= 1_000_000) return (us / 1_000_000).toFixed(2) + ' s'
  if (us >= 1000) return (us / 1000).toFixed(1) + ' ms'
  return us.toFixed(0) + ' µs'
}

function onMouseMove(event: MouseEvent) {
  if (hovered.value) {
    hoverX.value = event.clientX + 12
    hoverY.value = event.clientY + 12
  }
}

// Attach mousemove listener via template? Use component-level listener.
// Simpler: use fixed position relative to container via CSS hover.
// The tooltip uses viewport coordinates; add listener in mounted.
import { onMounted, onUnmounted } from 'vue'

function handleMove(event: MouseEvent) {
  onMouseMove(event)
}

onMounted(() => {
  window.addEventListener('mousemove', handleMove)
})

onUnmounted(() => {
  window.removeEventListener('mousemove', handleMove)
})
</script>

<style scoped>
.flame-graph-root {
  position: relative;
  font-size: 11px;
}

.fg-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--col-border, rgba(255,255,255,0.06));
  margin-bottom: 8px;
}

.fg-breadcrumb {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-wrap: wrap;
}

.fg-crumb {
  background: none;
  border: none;
  color: var(--col-accent, #5b9bd5);
  cursor: pointer;
  font-size: 11px;
  padding: 2px 4px;
  border-radius: 3px;
}

.fg-crumb:hover {
  background: var(--col-surface-raised, rgba(255,255,255,0.06));
}

.fg-sep {
  color: var(--col-muted, #888);
}

.fg-info {
  color: var(--col-muted, #888);
}

.fg-empty {
  padding: 24px;
  text-align: center;
  color: var(--col-muted, #888);
  border: 1px dashed var(--col-border, rgba(255,255,255,0.1));
  border-radius: var(--radius-md, 6px);
}

.fg-body {
  position: relative;
  min-height: 120px;
  max-height: 420px;
  overflow: auto;
  background: var(--col-surface, rgba(255,255,255,0.02));
  border-radius: var(--radius-md, 6px);
  padding: 4px 0;
}

.fg-row {
  position: relative;
  height: 22px;
  margin: 1px 0;
}

.fg-cell {
  position: absolute;
  height: 20px;
  border-radius: 3px;
  cursor: pointer;
  overflow: hidden;
  white-space: nowrap;
  transition: filter 0.1s;
}

.fg-cell:hover {
  filter: brightness(1.3);
  outline: 1px solid rgba(255,255,255,0.5);
}

.fg-match {
  outline: 2px solid #f39c12;
}

.fg-faded {
  opacity: 0.25 !important;
}

.fg-label {
  display: inline-block;
  padding: 3px 6px;
  color: #fff;
  font-size: 10px;
  pointer-events: none;
}

.fg-tooltip {
  position: fixed;
  z-index: 1000;
  background: var(--col-surface-raised, #1e1e2e);
  border: 1px solid var(--col-border, rgba(255,255,255,0.1));
  border-radius: 6px;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-width: 320px;
  box-shadow: 0 4px 16px rgba(0,0,0,0.4);
  pointer-events: none;
  color: var(--col-text, #eee);
}

.fg-tooltip strong {
  font-size: 12px;
}

.fg-tooltip span {
  color: var(--col-text-secondary, #aaa);
}

.fg-attr {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 10px;
  color: var(--col-muted, #888) !important;
}
</style>
