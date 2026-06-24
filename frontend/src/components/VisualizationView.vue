<template>
  <section class="view-stack">
    <div class="panel large-action-panel">
      <div>
        <p class="eyebrow">Visualization</p>
        <h2>3D Visualization</h2>
        <p class="muted">基于 dviz + Rerun 的机器人 3D 可视化视口。dviz 通过 Zenoh 发布点云、TF 帧、机器人模型等数据。</p>
      </div>
      <div class="control-row">
        <span :class="['pill', dviz.running ? 'success' : 'warning']">
          {{ dviz.running ? 'dviz running' : 'dviz not detected' }}
        </span>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>
    </div>

    <div class="metric-grid">
      <article :class="['metric-card', 'large-metric', dviz.installed ? 'success' : 'warning']">
        <span>dviz 状态</span>
        <strong>{{ dviz.installed ? (dviz.running ? '运行中' : '已安装') : '未安装' }}</strong>
        <small>{{ dviz.message }}</small>
      </article>
      <article class="metric-card large-metric">
        <span>3D Viewport</span>
        <strong>Week 8</strong>
        <small>Rerun web viewer iframe 将在此处嵌入。</small>
      </article>
      <article class="metric-card large-metric">
        <span>Point Cloud Topics</span>
        <strong>—</strong>
        <small>自动从 dviz Zenoh topics 发现。</small>
      </article>
      <article class="metric-card large-metric">
        <span>TF Tree</span>
        <strong>—</strong>
        <small>坐标帧树将在 Week 9 实现。</small>
      </article>
    </div>

    <article class="panel">
      <div class="panel-header">
        <h2>可视化功能路线</h2>
        <span class="pill">Phase B-D</span>
      </div>
      <div class="hook-list">
        <span>Week 8: Rerun 3D 视口嵌入</span>
        <span>Week 8: 点云 Topic 列表 + 配置面板</span>
        <span>Week 9: TF 帧树面板</span>
        <span>Week 9: ROS bag 回放控制</span>
        <span>Week 12: 错误处理 + 联调</span>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getDvizStatus, type ApiSource, type DvizStatusResponse } from '../api'

const fallbackDviz: DvizStatusResponse = {
  installed: false,
  running: false,
  binaryPath: null,
  message: 'Backend API is not connected.',
}

const dviz = ref<DvizStatusResponse>(fallbackDviz)
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))

onMounted(async () => {
  const result = await getDvizStatus(fallbackDviz)
  dviz.value = result.data
  apiSource.value = result.source
})
</script>
