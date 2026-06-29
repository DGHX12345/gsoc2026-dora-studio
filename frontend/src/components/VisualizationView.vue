<template>
  <section class="viz-layout">
    <aside class="viz-panel viz-left">
      <div class="viz-panel-header">
        <h2>Data Sources</h2>
        <span :class="['pill', dviz.running ? 'success' : 'stopped']">
          {{ dviz.running ? 'dviz live' : 'stopped' }}
        </span>
      </div>

      <div class="viz-search">
        <input type="text" placeholder="搜索 display..." disabled />
      </div>

      <div class="display-list">
        <label
          v-for="display in displays"
          :key="display.id"
          :class="['display-item', { enabled: display.enabled }]"
        >
          <input type="checkbox" :checked="display.enabled" disabled />
          <span :class="['display-dot', display.color]"></span>
          <div class="display-label">
            <strong>{{ display.name }}</strong>
            <small>{{ display.summary }}</small>
          </div>
          <span class="display-status">{{ display.enabled ? 'ON' : 'OFF' }}</span>
        </label>
      </div>

      <div class="viz-section">
        <h3>Topic Preview</h3>
        <div class="topic-preview-box" v-for="topic in mockTopics" :key="topic.name">
          <code>{{ topic.name }}</code>
          <span>{{ topic.summary }}</span>
        </div>
      </div>

      <div class="viz-status-row">
        <span class="status-dot" :class="dviz.running ? 'on' : 'off'"></span>
        <span>{{ dviz.running ? 'ZENOH connected' : 'ZENOH disconnected' }}</span>
        <span class="viz-version">{{ dviz.installed ? 'dviz detected' : 'dviz not found' }}</span>
      </div>
    </aside>

    <article class="viz-panel viz-center">
      <div class="viz-panel-header">
        <h2>3D Viewport</h2>
        <span class="pill">Rerun SDK</span>
      </div>
      <div class="viewport-placeholder">
        <div class="viewport-grid-bg"></div>
        <div class="viewport-center">
          <div class="viewport-icon">R</div>
          <strong>Rerun Web Viewer</strong>
          <p>3D 可视化视口将在 Week 8 接入 Rerun iframe</p>
          <span class="viewport-hint">点云 · TF 帧 · 机器人模型 · 激光雷达 · 标记</span>
        </div>
      </div>
      <div class="viewport-controls">
        <button disabled>重置视角</button>
        <button disabled>俯视</button>
        <button disabled>跟随</button>
        <span class="fps-indicator">— fps</span>
      </div>
    </article>

    <aside class="viz-panel viz-right">
      <div class="viz-panel-header">
        <h2>Display Properties</h2>
      </div>

      <div v-for="group in propertyGroups" :key="group.name" class="prop-group">
        <div class="prop-group-header">
          <span :class="['display-dot', group.color]"></span>
          <strong>{{ group.name }}</strong>
        </div>

        <div v-for="prop in group.props" :key="prop.label" class="prop-row">
          <label>{{ prop.label }}</label>
          <div class="prop-control">
            <input
              v-if="prop.type === 'slider'"
              type="range"
              :value="prop.value"
              :min="prop.min"
              :max="prop.max"
              disabled
            />
            <span v-if="prop.type === 'slider'" class="prop-value">{{ prop.value }}</span>
            <div v-else-if="prop.type === 'color'" class="prop-color">
              <span class="color-swatch" :style="{ background: prop.value }"></span>
              <span>{{ prop.value }}</span>
            </div>
            <span v-else class="prop-value disabled">{{ prop.value }}</span>
          </div>
        </div>
      </div>
    </aside>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { getDvizStatus, type DvizStatusResponse } from '../api'

const dviz = ref<DvizStatusResponse>({
  installed: false, running: false, binaryPath: null,
  message: 'Backend API is not connected.',
})

const displays = [
  { id: 'grid', name: 'Grid', color: 'gray', enabled: true, summary: '地面网格 · 10×10' },
  { id: 'axes', name: 'Axes', color: 'red', enabled: true, summary: 'RGB 坐标轴 · 1.0m' },
  { id: 'tf', name: 'TF Frames', color: 'green', enabled: true, summary: '帧树 · 5 个活跃帧' },
  { id: 'pointcloud', name: 'PointCloud', color: 'blue', enabled: false, summary: '点云数据 · 彩色/强度' },
  { id: 'laserscan', name: 'LaserScan', color: 'orange', enabled: false, summary: '2D 激光 · 360°' },
  { id: 'markers', name: 'Markers', color: 'purple', enabled: false, summary: '箭头/立方体/球体/文字' },
  { id: 'robotmodel', name: 'RobotModel', color: 'cyan', enabled: false, summary: 'URDF 模型 · 关节联动' },
]

const mockTopics = [
  { name: '/world/points', summary: 'PointCloud · 1,024 pts' },
  { name: '/world/tf', summary: 'Transform3D · 5 frames' },
]

type PropItem =
  | { type: 'slider'; label: string; value: string; min: string; max: string }
  | { type: 'color'; label: string; value: string }
  | { type: 'text'; label: string; value: string }

type PropGroup = { name: string; color: string; props: PropItem[] }

const propertyGroups: PropGroup[] = [
  {
    name: 'Grid', color: 'gray',
    props: [
      { type: 'slider' as const, label: 'Cell Count', value: '10', min: '2', max: '50' },
      { type: 'slider' as const, label: 'Cell Size', value: '1.0', min: '0.1', max: '10.0' },
      { type: 'color' as const, label: 'Color', value: '#7c8aa5' },
    ],
  },
  {
    name: 'PointCloud', color: 'blue',
    props: [
      { type: 'slider' as const, label: 'Point Size', value: '0.02', min: '0.001', max: '0.5' },
      { type: 'color' as const, label: 'Flat Color', value: '#ffffff' },
      { type: 'slider' as const, label: 'Decay Time', value: '3.0', min: '0', max: '30' },
      { type: 'slider' as const, label: 'Queue Size', value: '10', min: '1', max: '50' },
    ],
  },
  {
    name: 'TF Frames', color: 'green',
    props: [
      { type: 'slider' as const, label: 'Axis Scale', value: '0.3', min: '0.1', max: '5.0' },
      { type: 'slider' as const, label: 'Timeout', value: '5.0', min: '1.0', max: '30.0' },
    ],
  },
]

onMounted(async () => {
  const result = await getDvizStatus({
    installed: false, running: false, binaryPath: null,
    message: 'Backend API is not connected.',
  })
  dviz.value = result.data
})
</script>
