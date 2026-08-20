<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { getRuntimeNodeStatuses, reloadNode, type NodeRuntimeStatusResponse } from '../api'

export interface PortSpec { type?: string; description?: string; source?: string }
export interface NodeSpec {
  id: string; operatorId: string; runtime: string; path?: string;
  inputs: Record<string, PortSpec>; outputs: Record<string, PortSpec>;
  position: { x: number; y: number };
}
export interface EdgeSpec {
  id: string; sourceNode: string; sourcePort: string;
  targetNode: string; targetPort: string;
}
export interface DataflowGraph { nodes: NodeSpec[]; edges: EdgeSpec[] }

const props = defineProps<{
  graph: DataflowGraph
  selectedNode: string | null
  selectedEdge: string | null
  edgeStyles?: Record<string, { color: string; tooltip?: string }>
  dataflowId?: string
}>()

const emit = defineEmits<{
  'update:graph': [graph: DataflowGraph]
  'select-node': [nodeId: string | null]
  'select-edge': [edgeId: string | null]
  'node-dblclick': [nodeId: string]
}>()

const svgEl = ref<SVGSVGElement>()
const tx = ref(0)
const ty = ref(0)
const scale = ref(1)
const dragging = ref<{ type: 'canvas' | 'node'; nodeId?: string; dx: number; dy: number; sx: number; sy: number } | null>(null)
const connecting = ref<{ nodeId: string; portName: string; isOutput: boolean } | null>(null)
const mousePos = ref({ x: 0, y: 0 })
const svgRect = ref<DOMRect | null>(null)

const NODE_W = 280
const PORT_R = 7
const PORT_GAP = 24
const MIN_SCALE = 0.15
const MAX_SCALE = 2.5
const HEADER_H = 36
const OP_ROW_H = 20
const HEADER_OFFSET = HEADER_H + OP_ROW_H + 6

// --- Node status polling (M03) ---
const nodeStatuses = ref<Record<string, string>>({})
let statusTimer: ReturnType<typeof setInterval> | null = null

function statusColor(status: string): string {
  return { running: '#22c55e', crashed: '#ef4444', reloading: '#eab308', degraded: '#f97316', exited: '#6b7280', unknown: '#6b7280' }[status] ?? '#6b7280'
}

function statusLabel(status: string): string {
  return { running: 'running', crashed: 'crashed', reloading: 'reloading', degraded: 'degraded', exited: 'exited', unknown: 'unknown' }[status] ?? status
}

async function pollStatuses() {
  if (!props.dataflowId) return
  try {
    const result = await getRuntimeNodeStatuses(props.dataflowId, [])
    if (result.source === 'connected') {
      for (const s of result.data) {
        // Map backend node_id to canvas node id.
        // Backend may return:
        //  - per-node IDs from WS (e.g. "camera") that match canvas node IDs
        //  - dataflow-level ID (e.g. "studio-dataflow") as fallback
        if (props.graph.nodes.some(n => n.id === s.nodeId)) {
          // Direct match: per-node status
          nodeStatuses.value[s.nodeId] = s.status
        } else if (s.status === 'running') {
          // Dataflow-level status: apply to all graph nodes
          for (const n of props.graph.nodes) {
            nodeStatuses.value[n.id] = s.status
          }
        }
      }
    }
  } catch {
    // Ignore polling errors
  }
}

watch(() => props.dataflowId, (newId) => {
  if (statusTimer) { clearInterval(statusTimer); statusTimer = null }
  nodeStatuses.value = {}
  if (newId) {
    pollStatuses()
    statusTimer = setInterval(pollStatuses, 2000)
  }
})

// --- Context menu (M03) ---
const contextMenu = ref<{ x: number; y: number; nodeId: string; runtime: string } | null>(null)
const reloadingNode = ref<string | null>(null)
const reloadMessage = ref<string | null>(null)

function onCanvasContextMenu(e: MouseEvent) {
  updateRect()
  const w = screenToWorld(e.clientX, e.clientY)
  // Hit-test: find node under cursor
  for (const node of props.graph.nodes) {
    const nh = nodeHeight(node)
    if (w.x >= node.position.x && w.x <= node.position.x + NODE_W &&
        w.y >= node.position.y && w.y <= node.position.y + nh) {
      contextMenu.value = { x: e.clientX, y: e.clientY, nodeId: node.id, runtime: node.runtime }
      return
    }
  }
}

function closeContextMenu() {
  contextMenu.value = null
}

async function doReload() {
  if (!contextMenu.value || !props.dataflowId) return
  const nodeId = contextMenu.value.nodeId
  closeContextMenu()

  reloadingNode.value = nodeId
  reloadMessage.value = null
  nodeStatuses.value[nodeId] = 'reloading'

  try {
    const result = await reloadNode(props.dataflowId, { nodeId })
    reloadMessage.value = result.message
    // Optimistically set to running; polling will confirm
    setTimeout(() => {
      if (nodeStatuses.value[nodeId] === 'reloading') {
        nodeStatuses.value[nodeId] = 'running'
      }
    }, 1500)
  } catch (e) {
    nodeStatuses.value[nodeId] = 'crashed'
    reloadMessage.value = e instanceof Error ? e.message : 'Reload failed'
  }
  reloadingNode.value = null
}

function updateRect() {
  svgRect.value = svgEl.value?.getBoundingClientRect() ?? null
}
onMounted(() => {
  updateRect(); window.addEventListener('resize', updateRect)
  if (props.dataflowId) {
    pollStatuses()
    statusTimer = setInterval(pollStatuses, 2000)
  }
})
onUnmounted(() => {
  window.removeEventListener('resize', updateRect)
  if (statusTimer) { clearInterval(statusTimer); statusTimer = null }
})

function screenToWorld(sx: number, sy: number) {
  const r = svgRect.value
  if (!r) return { x: 0, y: 0 }
  return { x: (sx - r.left - tx.value) / scale.value, y: (sy - r.top - ty.value) / scale.value }
}

function nodePortPositions(node: NodeSpec) {
  const nx = node.position.x; const ny = node.position.y
  const inputs = Object.keys(node.inputs).map((name, i) => ({ name, x: nx, y: ny + HEADER_OFFSET + PORT_GAP * (i + 0.5) }))
  const outputs = Object.keys(node.outputs).map((name, i) => ({ name, x: nx + NODE_W, y: ny + HEADER_OFFSET + PORT_GAP * (i + 0.5) }))
  return { inputs, outputs }
}

function nodeHeight(node: NodeSpec): number {
  const pc = Math.max(Object.keys(node.inputs).length, Object.keys(node.outputs).length, 1)
  return HEADER_OFFSET + pc * PORT_GAP + 8
}

/// Truncate display label for long auto-generated IDs. SVG <text> cannot be
/// ellipsized with CSS, so truncation is done in JS; the full label is shown
/// via a <title> hover tooltip on the element.
function displayLabel(id: string, defaultLabel: string, maxLen = 22): string {
  if (id.length > maxLen) return id.slice(0, maxLen - 3) + '...'
  return defaultLabel || id
}

function edgePath(edge: EdgeSpec): string {
  const src = props.graph.nodes.find(n => n.id === edge.sourceNode)
  const tgt = props.graph.nodes.find(n => n.id === edge.targetNode)
  if (!src || !tgt) return ''
  const sp = nodePortPositions(src); const tp = nodePortPositions(tgt)
  const from = sp.outputs.find(o => o.name === edge.sourcePort) ?? sp.outputs[0]
  const to = tp.inputs.find(i => i.name === edge.targetPort) ?? tp.inputs[0]
  if (!from || !to) return ''
  const dx = Math.max(80, Math.abs(to.x - from.x) * 0.5)
  return `M ${from.x} ${from.y} C ${from.x + dx} ${from.y}, ${to.x - dx} ${to.y}, ${to.x} ${to.y}`
}

function runtimeColor(r: string): string {
  return { python: '#3b82f6', rust: '#f59e0b', c: '#8b5cf6', cpp: '#a78bfa' }[r] ?? '#6b7280'
}

function onMouseDown(e: MouseEvent) {
  if (e.button === 2) return // ignore right-click (handled by contextmenu)
  updateRect()
  const w = screenToWorld(e.clientX, e.clientY)

  // 1. Check port hit (highest priority)
  for (const node of props.graph.nodes) {
    for (const p of nodePortPositions(node).outputs) {
      if (Math.hypot(w.x - p.x, w.y - p.y) < PORT_R + 6) {
        connecting.value = { nodeId: node.id, portName: p.name, isOutput: true }; return
      }
    }
    for (const p of nodePortPositions(node).inputs) {
      if (Math.hypot(w.x - p.x, w.y - p.y) < PORT_R + 6) {
        connecting.value = { nodeId: node.id, portName: p.name, isOutput: false }; return
      }
    }
  }

  // 2. Check edge hit (before node hit, since edges span between nodes)
  let hitEdge: string | null = null
  for (const edge of props.graph.edges) {
    const path = edgePath(edge)
    if (!path) continue
    // Sample points along the bezier and check proximity
    const src = props.graph.nodes.find(n => n.id === edge.sourceNode)
    const tgt = props.graph.nodes.find(n => n.id === edge.targetNode)
    if (!src || !tgt) continue
    const from = nodePortPositions(src).outputs.find(o => o.name === edge.sourcePort) ?? nodePortPositions(src).outputs[0]
    const to = nodePortPositions(tgt).inputs.find(i => i.name === edge.targetPort) ?? nodePortPositions(tgt).inputs[0]
    if (!from || !to) continue
    // Approximate: check distance to line segment midpoints
    const steps = 8
    for (let i = 0; i <= steps; i++) {
      const t = i / steps
      // Cubic bezier at t
      const dx = Math.abs(to.x - from.x) * 0.5
      const cx1 = from.x + dx; const cy1 = from.y
      const cx2 = to.x - dx; const cy2 = to.y
      const bx = (1-t)**3 * from.x + 3*(1-t)**2 * t * cx1 + 3*(1-t) * t**2 * cx2 + t**3 * to.x
      const by = (1-t)**3 * from.y + 3*(1-t)**2 * t * cy1 + 3*(1-t) * t**2 * cy2 + t**3 * to.y
      if (Math.hypot(w.x - bx, w.y - by) < 10) {
        hitEdge = edge.id; break
      }
    }
    if (hitEdge) break
  }
  if (hitEdge) {
    emit('select-edge', hitEdge); emit('select-node', null)
    dragging.value = null; connecting.value = null
    return
  }

  // 3. Check node hit
  for (const node of props.graph.nodes) {
    const nh = nodeHeight(node)
    if (w.x >= node.position.x && w.x <= node.position.x + NODE_W && w.y >= node.position.y && w.y <= node.position.y + nh) {
      dragging.value = { type: 'node', nodeId: node.id, dx: w.x - node.position.x, dy: w.y - node.position.y, sx: e.clientX, sy: e.clientY }
      emit('select-node', node.id); emit('select-edge', null)
      return
    }
  }

  // 4. Canvas drag
  emit('select-node', null); emit('select-edge', null)
  dragging.value = { type: 'canvas', dx: 0, dy: 0, sx: e.clientX - tx.value, sy: e.clientY - ty.value }
  connecting.value = null
}

function onMouseMove(e: MouseEvent) {
  mousePos.value = { x: e.clientX, y: e.clientY }
  if (!dragging.value) return
  if (dragging.value.type === 'canvas') {
    tx.value = e.clientX - dragging.value.sx
    ty.value = e.clientY - dragging.value.sy
  } else if (dragging.value.type === 'node') {
    const w = screenToWorld(e.clientX, e.clientY)
    const id = dragging.value.nodeId!
    const newNodes: NodeSpec[] = props.graph.nodes.map(n =>
      n.id === id ? { ...n, position: { x: w.x - dragging.value!.dx, y: w.y - dragging.value!.dy } } : n
    )
    emit('update:graph', { ...props.graph, nodes: newNodes })
  }
}

function onMouseUp(e: MouseEvent) {
  if (connecting.value) {
    const t = connecting.value
    updateRect()
    const cursor = screenToWorld(e.clientX, e.clientY)
    let bestDist = 24 // max snap distance in world coords
    let bestEdge: EdgeSpec | null = null

    for (const node of props.graph.nodes) {
      if (node.id === t.nodeId) continue
      const ports = nodePortPositions(node)
      const candidates = t.isOutput ? ports.inputs : ports.outputs
      for (const p of candidates) {
        const dist = Math.hypot(cursor.x - p.x, cursor.y - p.y)
        if (dist < bestDist) {
          bestDist = dist
          bestEdge = {
            id: `e_${Date.now()}`,
            sourceNode: t.isOutput ? t.nodeId : node.id,
            sourcePort: t.isOutput ? t.portName : p.name,
            targetNode: t.isOutput ? node.id : t.nodeId,
            targetPort: t.isOutput ? p.name : t.portName,
          }
        }
      }
    }
    if (bestEdge) {
      // Don't create duplicate edges
      const dup = props.graph.edges.find(e =>
        e.sourceNode === bestEdge!.sourceNode && e.sourcePort === bestEdge!.sourcePort &&
        e.targetNode === bestEdge!.targetNode && e.targetPort === bestEdge!.targetPort
      )
      if (!dup) {
        emit('update:graph', { ...props.graph, edges: [...props.graph.edges, bestEdge] })
      }
    }
  }
  dragging.value = null; connecting.value = null
}

const connectingPos = computed(() => {
  if (!connecting.value) return null
  const node = props.graph.nodes.find(n => n.id === connecting.value!.nodeId)
  if (!node) return null
  const ports = nodePortPositions(node)
  const port = connecting.value!.isOutput
    ? ports.outputs.find(o => o.name === connecting.value!.portName)
    : ports.inputs.find(i => i.name === connecting.value!.portName)
  return port ? { x: port.x, y: port.y } : null
})

function onWheel(e: WheelEvent) {
  e.preventDefault()
  updateRect()
  const r = svgRect.value; if (!r) return
  const mx = e.clientX - r.left; const my = e.clientY - r.top
  const factor = e.deltaY > 0 ? 0.88 : 1.12
  const ns = Math.max(MIN_SCALE, Math.min(MAX_SCALE, scale.value * factor))
  // Zoom toward cursor
  tx.value = mx - (mx - tx.value) * (ns / scale.value)
  ty.value = my - (my - ty.value) * (ns / scale.value)
  scale.value = ns
}

function zoomIn() {
  scale.value = Math.min(MAX_SCALE, scale.value * 1.25)
}
function zoomOut() {
  scale.value = Math.max(MIN_SCALE, scale.value * 0.8)
}
function zoomFit() {
  if (props.graph.nodes.length === 0) { tx.value = 0; ty.value = 0; scale.value = 1; return }
  const r = svgRect.value; if (!r) return
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity
  for (const n of props.graph.nodes) {
    minX = Math.min(minX, n.position.x); minY = Math.min(minY, n.position.y)
    maxX = Math.max(maxX, n.position.x + NODE_W); maxY = Math.max(maxY, n.position.y + nodeHeight(n))
  }
  const gw = maxX - minX + 100; const gh = maxY - minY + 100
  const sx = (r.width - 260) / gw  // 260 for palette
  const sy = (r.height - 80) / gh   // 80 for toolbar + statusbar
  scale.value = Math.min(sx, sy, 1.5)
  tx.value = (r.width - 260 - gw * scale.value) / 2 - minX * scale.value + 260
  ty.value = (r.height - gh * scale.value) / 2 - minY * scale.value
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Delete' || e.key === 'Backspace') {
    const tag = (e.target as HTMLElement)?.tagName
    if (tag === 'INPUT' || tag === 'TEXTAREA') return
    e.preventDefault()
    const g = { ...props.graph }
    if (props.selectedEdge) {
      g.edges = g.edges.filter(ed => ed.id !== props.selectedEdge)
      emit('update:graph', g)
      emit('select-edge', null)
    } else if (props.selectedNode) {
      g.nodes = g.nodes.filter(n => n.id !== props.selectedNode)
      g.edges = g.edges.filter(e => e.sourceNode !== props.selectedNode && e.targetNode !== props.selectedNode)
      emit('update:graph', g)
      emit('select-node', null)
    }
  }
}

// Drop from palette (drag payload is a full PaletteEntry: ports are
// {name, urn?} objects carrying the declared type URNs)
function onDragOver(e: DragEvent) { e.preventDefault() }
function onDrop(e: DragEvent) {
  e.preventDefault()
  const json = e.dataTransfer?.getData('application/json')
  if (!json) return
  updateRect()
  const w = screenToWorld(e.clientX, e.clientY)
  const entry = JSON.parse(json) as {
    operator: string; runtime: string; path?: string
    inputs: Array<{ name: string; urn?: string }>; outputs: Array<{ name: string; urn?: string }>
  }
  const toPorts = (ports: Array<{ name: string; urn?: string }>) => {
    const result: Record<string, PortSpec> = {}
    for (const port of ports) {
      result[port.name] = port.urn ? { type: port.urn } : {}
    }
    return result
  }
  const id = `${entry.operator}_${Date.now()}`
  const newNode: NodeSpec = {
    id, operatorId: entry.operator, runtime: entry.runtime, path: entry.path,
    inputs: toPorts(entry.inputs),
    outputs: toPorts(entry.outputs),
    position: { x: w.x - NODE_W / 2, y: w.y - 18 },
  }
  emit('update:graph', { ...props.graph, nodes: [...props.graph.nodes, newNode] })
}

// Auto-fit when nodes change (e.g., graph data loaded)
watch(() => props.graph.nodes.length, () => {
  if (props.graph.nodes.length > 0) {
    // Delay to let DOM settle
    setTimeout(() => { updateRect(); zoomFit() }, 100)
  }
})

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  setTimeout(() => { updateRect(); if (props.graph.nodes.length > 0) zoomFit() }, 200)
})
onUnmounted(() => { window.removeEventListener('keydown', onKeyDown) })

defineExpose({ zoomFit, zoomIn, zoomOut })
</script>

<template>
  <div class="canvas-wrap" @dragover="onDragOver" @drop="onDrop">
    <svg
      ref="svgEl"
      class="dataflow-canvas"
      @mousedown="onMouseDown"
      @mousemove="onMouseMove"
      @mouseup="onMouseUp"
      @wheel="onWheel"
      @contextmenu.prevent="onCanvasContextMenu"
    >
      <defs>
        <pattern id="dot-grid" width="24" height="24" patternUnits="userSpaceOnUse">
          <circle cx="12" cy="12" r="0.8" fill="#ffffff" opacity="0.04" />
        </pattern>
        <linearGradient id="edge-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stop-color="#00d4ff" />
          <stop offset="100%" stop-color="#7c3aed" />
        </linearGradient>
        <marker id="arrowhead" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
          <polygon points="0 0, 8 3, 0 6" fill="#7c3aed" />
        </marker>
      </defs>

      <!-- Background with dot grid -->
      <rect x="-5000" y="-5000" width="10000" height="10000" fill="var(--canvas-base)" />
      <rect x="-5000" y="-5000" width="10000" height="10000" fill="url(#dot-grid)" />

      <!-- Transformed scene -->
      <g :transform="`translate(${tx}, ${ty}) scale(${scale})`">
        <!-- Edges -->
        <g v-for="edge in graph.edges" :key="edge.id">
          <title v-if="edgeStyles?.[edge.id]?.tooltip">{{ edgeStyles[edge.id].tooltip }}</title>
          <path
            :d="edgePath(edge)" fill="none"
            :stroke="edgeStyles?.[edge.id]?.color ?? '#7c3aed'"
            :stroke-width="2 / scale"
            :opacity="selectedEdge === edge.id ? 1 : (selectedEdge ? 0.3 : 0.7)"
            marker-end="url(#arrowhead)" class="canvas-edge"
          />
        </g>

        <!-- Connecting line -->
        <line v-if="connectingPos" :x1="connectingPos.x" :y1="connectingPos.y" :x2="(mousePos.x - (svgRect?.left ?? 0) - tx) / scale" :y2="(mousePos.y - (svgRect?.top ?? 0) - ty) / scale" stroke="#00d4ff" :stroke-width="1.5 / scale" stroke-dasharray="6,3" opacity="0.7" />

        <!-- Nodes -->
        <g v-for="node in graph.nodes" :key="node.id">
          <rect :x="node.position.x" :y="node.position.y" :width="NODE_W" :height="nodeHeight(node)" rx="10"
            :fill="selectedNode===node.id ? 'var(--card-active)' : 'var(--card-surface)'"
            :stroke="selectedNode===node.id ? 'var(--accent-cyan)' : 'var(--hairline)'"
            :stroke-width="selectedNode===node.id ? 2 : 1"
            class="canvas-node" />
          <!-- Header bar -->
          <rect :x="node.position.x" :y="node.position.y" :width="NODE_W" height="36" rx="10" fill="var(--hairline)" opacity="0.4" />
          <rect :x="node.position.x" :y="node.position.y + 26" :width="NODE_W" height="10" fill="var(--hairline)" opacity="0.4" />
          <!-- Node name -->
          <text :x="node.position.x + 14" :y="node.position.y + 22" fill="var(--text-heading)" font-size="14" font-weight="600" font-family="system-ui, sans-serif" class="canvas-node-label">{{ displayLabel(node.id, node.id) }}<title>{{ node.id }}</title></text>
          <!-- Status dot (M03) -->
          <circle
            v-if="nodeStatuses[node.id]"
            :cx="node.position.x + NODE_W - 74" :cy="node.position.y + 18" r="4"
            :fill="statusColor(nodeStatuses[node.id])"
            :opacity="nodeStatuses[node.id] === 'running' ? 1 : 0.8"
            :class="{ 'status-pulse': nodeStatuses[node.id] === 'running' }"
          />
          <title v-if="nodeStatuses[node.id]">Status: {{ statusLabel(nodeStatuses[node.id]) }}</title>
          <!-- Runtime badge -->
          <rect :x="node.position.x + NODE_W - 62" :y="node.position.y + 8" width="50" height="20" rx="6" :fill="runtimeColor(node.runtime)" opacity="0.18" />
          <text :x="node.position.x + NODE_W - 37" :y="node.position.y + 22" :fill="runtimeColor(node.runtime)" font-size="12" text-anchor="middle" font-weight="600" font-family="system-ui, sans-serif">{{ node.runtime }}</text>
          <!-- Operator type row -->
          <rect :x="node.position.x" :y="node.position.y + 36" :width="NODE_W" :height="OP_ROW_H" fill="var(--canvas-base)" opacity="0.5" />
          <text :x="node.position.x + 14" :y="node.position.y + 50" fill="var(--text-muted-dark)" font-size="12" font-family="system-ui, sans-serif" class="canvas-node-label">{{ displayLabel(node.operatorId, node.operatorId, 34) }}<title>{{ node.operatorId }}</title></text>
          <!-- Separator line -->
          <line :x1="node.position.x + 8" :y1="node.position.y + HEADER_OFFSET" :x2="node.position.x + NODE_W - 8" :y2="node.position.y + HEADER_OFFSET" stroke="var(--hairline)" stroke-width="0.5" />
          <!-- Input ports -->
          <g v-for="(port, i) in nodePortPositions(node).inputs" :key="'in-'+port.name">
            <circle :cx="port.x" :cy="port.y" :r="PORT_R" fill="var(--accent-green)" stroke="var(--card-surface)" stroke-width="2" class="canvas-port" />
            <text :x="port.x + 14" :y="port.y + 5" fill="var(--text-body)" font-size="12" font-family="system-ui, sans-serif">{{ port.name }}</text>
          </g>
          <!-- Output ports -->
          <g v-for="(port, i) in nodePortPositions(node).outputs" :key="'out-'+port.name">
            <circle :cx="port.x" :cy="port.y" :r="PORT_R" fill="var(--accent-cyan)" stroke="var(--card-surface)" stroke-width="2" class="canvas-port" />
            <text :x="port.x - 14" :y="port.y + 5" fill="var(--text-body)" font-size="12" text-anchor="end" font-family="system-ui, sans-serif">{{ port.name }}</text>
          </g>
        </g>
      </g>
    </svg>

    <!-- Zoom controls -->
    <div class="zoom-controls">
      <button class="zoom-btn" @click="zoomIn" title="Zoom in">+</button>
      <button class="zoom-btn" @click="zoomOut" title="Zoom out">−</button>
      <button class="zoom-btn" @click="zoomFit" title="Fit all">⊡</button>
    </div>

    <!-- Context menu (M03) -->
    <Teleport to="body">
      <div v-if="contextMenu" class="canvas-context-menu" :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }" @mouseleave="closeContextMenu">
        <div class="ctx-header">{{ contextMenu.nodeId }}</div>
        <button
          v-if="contextMenu.runtime === 'python'"
          class="ctx-item"
          :disabled="!dataflowId || reloadingNode === contextMenu.nodeId"
          :title="!dataflowId ? 'Run the dataflow first to enable Hot Reload' : ''"
          @click="doReload"
        >
          {{ reloadingNode === contextMenu.nodeId ? 'Reloading...' : 'Hot Reload' }}
        </button>
        <button class="ctx-item" @click="closeContextMenu">Cancel</button>
      </div>
      <div v-if="contextMenu" class="ctx-backdrop" @click="closeContextMenu"></div>
    </Teleport>
  </div>
</template>

<style scoped>
.canvas-wrap {
  flex: 1; position: relative; overflow: hidden; min-height: 0;
}
.dataflow-canvas {
  width: 100%; height: 100%; display: block;
  cursor: grab; user-select: none;
}
.dataflow-canvas:active { cursor: grabbing; }
.canvas-node { transition: filter 140ms ease; }
.canvas-node:hover { filter: brightness(1.12); }
.canvas-port { transition: r 120ms ease; cursor: crosshair; }
.canvas-port:hover { r: 10; }
.canvas-edge { transition: opacity 150ms ease; cursor: pointer; }

/* Long node labels are truncated in JS (displayLabel); these properties keep
   the intent and prevent wrap/overflow if the label ever renders as HTML.
   SVG <text> cannot ellipsize via CSS, so the full label is in <title>. */
.canvas-node-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.zoom-controls {
  position: absolute; bottom: 12px; right: 12px;
  display: flex; gap: 4px; z-index: 10;
}
.zoom-btn {
  width: 32px; height: 32px; padding: 0;
  background: var(--card-surface); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 6px;
  font-size: 16px; font-weight: 600; cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  transition: background 120ms ease;
}
.zoom-btn:hover { background: var(--card-hover); color: var(--text-heading); }

/* Status pulse animation */
@keyframes status-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
.status-pulse {
  animation: status-pulse 2s ease-in-out infinite;
}

/* Context menu */
.canvas-context-menu {
  position: fixed; z-index: 1000;
  background: var(--card-surface); border: 1px solid var(--hairline);
  border-radius: 8px; padding: 4px; min-width: 160px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.4);
}
.ctx-header {
  padding: 6px 12px; font-size: 12px; font-weight: 600;
  color: var(--text-muted-dark); border-bottom: 1px solid var(--hairline);
  margin-bottom: 4px;
}
.ctx-item {
  display: block; width: 100%; padding: 8px 12px;
  background: none; border: none; border-radius: 4px;
  color: var(--text-body); font-size: 13px; text-align: left;
  cursor: pointer; transition: background 100ms ease;
}
.ctx-item:hover { background: var(--card-hover); color: var(--text-heading); }
.ctx-item:disabled { opacity: 0.4; cursor: default; }
.ctx-backdrop {
  position: fixed; inset: 0; z-index: 999;
}
</style>
