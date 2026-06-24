<template>
  <section class="view-stack">
    <div class="panel large-action-panel">
      <div>
        <p class="eyebrow">Motion Planner</p>
        <h2>运动规划控制</h2>
        <p class="muted">通过 dora-moveit2 MoveGroup API 进行运动规划、IK 求解、场景管理和轨迹执行。moveit 节点通过 Dora dataflow 运行。</p>
      </div>
      <div class="control-row">
        <span :class="['pill', moveit.running ? 'success' : 'warning']">
          {{ moveit.running ? 'moveit running' : 'moveit not detected' }}
        </span>
        <span :class="['pill', apiSource === 'connected' ? 'success' : 'warning']">{{ apiSourceText }}</span>
      </div>
    </div>

    <div class="metric-grid">
      <article :class="['metric-card', 'large-metric', moveit.installed ? 'success' : 'warning']">
        <span>dora-moveit2 状态</span>
        <strong>{{ moveit.installed ? (moveit.running ? '运行中' : '已安装') : '未安装' }}</strong>
        <small>{{ moveit.message }}</small>
      </article>
      <article class="metric-card large-metric">
        <span>Motion Control</span>
        <strong>Week 10</strong>
        <small>Plan / Execute / Stop 按钮将在此处。</small>
      </article>
      <article class="metric-card large-metric">
        <span>Planning Scene</span>
        <strong>Week 11</strong>
        <small>碰撞对象管理面板。</small>
      </article>
      <article class="metric-card large-metric">
        <span>Robot State</span>
        <strong>—</strong>
        <small>关节值 + 末端位姿显示。</small>
      </article>
    </div>

    <article class="panel">
      <div class="panel-header">
        <h2>运动规划功能路线</h2>
        <span class="pill">Phase C</span>
      </div>
      <div class="hook-list">
        <span>Week 10: MoveGroup REST API (plan/execute/ik/fk)</span>
        <span>Week 10: Motion 控制面板</span>
        <span>Week 11: Planning Scene 管理 + 轨迹预览</span>
        <span>Week 11: 机器人状态面板</span>
        <span>Week 12: 错误处理 + 联调</span>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getMoveitStatus, type ApiSource, type MoveitStatusResponse } from '../api'

const fallbackMoveit: MoveitStatusResponse = {
  installed: false,
  running: false,
  message: 'Backend API is not connected.',
}

const moveit = ref<MoveitStatusResponse>(fallbackMoveit)
const apiSource = ref<ApiSource>('fallback')
const apiSourceText = computed(() => (apiSource.value === 'connected' ? 'API connected' : 'Using mock fallback'))

onMounted(async () => {
  const result = await getMoveitStatus(fallbackMoveit)
  moveit.value = result.data
  apiSource.value = result.source
})
</script>
