<template>
  <section class="explorer-layout">
    <aside class="panel sidebar-panel">
      <div class="panel-header">
        <h2>Dataflows</h2>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>
      <button class="flow-file active">robot-perception-demo.yml</button>
      <button class="flow-file">warehouse-pick-place.yml</button>
      <button class="flow-file">camera-logger.yml</button>

      <div class="diagnostics-box">
        <h3>诊断信息</h3>
        <div v-for="item in diagnostics" :key="item.message" :class="['diagnostic', item.severity]">
          {{ item.message }}
        </div>
      </div>
    </aside>

    <article class="panel graph-panel">
      <div class="panel-header">
        <div>
          <h2>Dataflow 结构图</h2>
          <p>由 descriptor graph API 生成的图，用于确认交互和信息架构。</p>
        </div>
        <span class="pill running">{{ nodes.length }} 个节点 · {{ graphEdges.length }} 条边</span>
      </div>

      <div class="graph-canvas">
        <svg class="edge-layer" viewBox="0 0 1000 390" preserveAspectRatio="none">
          <defs>
            <marker id="arrow" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto" markerUnits="strokeWidth">
              <path d="M0,0 L0,6 L9,3 z" fill="#7c8aa5" />
            </marker>
          </defs>
          <path v-for="edge in graphEdges" :key="edge.id" :d="edgePath(edge.from, edge.to)" marker-end="url(#arrow)" />
        </svg>

        <button
          v-for="node in nodes"
          :key="node.id"
          :class="['graph-node', node.status, { selected: selectedNode.id === node.id }]"
          :style="{ left: `${node.x}px`, top: `${node.y}px` }"
          @click="selectedId = node.id"
        >
          <span>{{ node.kind }}</span>
          <strong>{{ node.label }}</strong>
          <small>{{ node.outputs.length }} 个输出 · {{ node.pending }} 条 pending</small>
        </button>
      </div>
    </article>

    <aside class="panel inspector-panel">
      <div class="panel-header">
        <h2>节点详情</h2>
        <span :class="['pill', selectedNode.status]">{{ selectedNode.status }}</span>
      </div>
      <div class="inspector-title">
        <strong>{{ selectedNode.label }}</strong>
        <span>{{ selectedNode.kind }}</span>
      </div>
      <p class="muted">{{ selectedNode.note }}</p>

      <div class="inspector-section">
        <h3>输入</h3>
        <code v-for="input in selectedNode.inputs" :key="input">{{ input }}</code>
      </div>

      <div class="inspector-section">
        <h3>输出</h3>
        <code v-for="output in selectedNode.outputs" :key="output">{{ output }}</code>
        <p v-if="selectedNode.outputs.length === 0" class="muted">没有输出。</p>
      </div>

      <div class="mini-metrics">
        <div><span>CPU</span><b>{{ selectedNode.cpu }}%</b></div>
        <div><span>内存</span><b>{{ selectedNode.memory }} MB</b></div>
        <div><span>重启</span><b>{{ selectedNode.restarts }}</b></div>
        <div><span>Pending</span><b>{{ selectedNode.pending }}</b></div>
      </div>
    </aside>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getDataflowGraph, type ApiSource, type DataflowGraphResponse } from '../api'
import {
  dataflowEdges,
  dataflowNodes,
  diagnostics as fallbackDiagnostics,
  type StudioEdge,
  type StudioNode,
} from '../data/mockStudio'

const fallbackGraph: DataflowGraphResponse = {
  nodes: dataflowNodes,
  edges: dataflowEdges,
  diagnostics: fallbackDiagnostics,
}

const nodes = ref<StudioNode[]>(fallbackGraph.nodes)
const graphEdges = ref<StudioEdge[]>(fallbackGraph.edges)
const diagnostics = ref(fallbackGraph.diagnostics)
const selectedId = ref('detector')
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))
const selectedNode = computed(() => nodes.value.find((node) => node.id === selectedId.value) ?? nodes.value[0])

function edgePath(from: string, to: string) {
  const source = nodes.value.find((node) => node.id === from)
  const target = nodes.value.find((node) => node.id === to)

  if (!source || !target) return ''

  const startX = source.x + 220
  const startY = source.y + 56
  const endX = target.x
  const endY = target.y + 56
  const midX = (startX + endX) / 2

  return `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`
}

onMounted(async () => {
  const result = await getDataflowGraph('robot-perception-demo', fallbackGraph)
  nodes.value = result.data.nodes
  graphEdges.value = result.data.edges
  diagnostics.value = result.data.diagnostics
  apiSource.value = result.source
})
</script>
