<template>
  <section class="motion-layout">
    <aside class="motion-panel motion-left">
      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Robot State</h3>
          <span class="pill stopped">simulated</span>
        </div>

        <div class="joint-list">
          <div v-for="joint in joints" :key="joint.name" class="joint-row">
            <label>{{ joint.name }}</label>
            <div class="joint-input">
              <input type="text" :value="joint.value" disabled />
              <span>rad</span>
            </div>
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>End-Effector Pose</h3>
        </div>
        <div class="pose-grid">
          <div class="pose-row" v-for="row in poseRows" :key="row.label">
            <label>{{ row.label }}</label>
            <input v-for="(val, i) in row.values" :key="i" type="text" :value="val" disabled />
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Planning Scene</h3>
          <span class="pill">3 objects</span>
        </div>
        <div class="scene-list">
          <div v-for="obj in sceneObjects" :key="obj.name" class="scene-object">
            <span :class="['scene-icon', obj.shape]"></span>
            <div>
              <strong>{{ obj.name }}</strong>
              <small>{{ obj.dims }}</small>
            </div>
          </div>
        </div>
        <div class="scene-actions">
          <button disabled>+ Add Box</button>
          <button class="secondary" disabled>+ Add Sphere</button>
          <button class="secondary" disabled>− Remove</button>
        </div>
      </div>

      <div class="motion-status">
        <span :class="['status-dot', moveit.running ? 'on' : 'off']"></span>
        {{ moveit.message }}
      </div>
    </aside>

    <article class="motion-panel motion-right">
      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Motion Control</h3>
        </div>

        <div class="goal-tabs">
          <button class="goal-tab active" disabled>Joint Goal</button>
          <button class="goal-tab" disabled>Pose Goal</button>
          <button class="goal-tab" disabled>Cartesian Path</button>
        </div>

        <div class="joint-goal-grid">
          <div v-for="(val, i) in goalJoints" :key="i" class="joint-goal-cell">
            <label>joint_{{ i + 1 }}</label>
            <input type="text" :value="val" disabled />
          </div>
        </div>

        <div class="control-row motion-actions">
          <button class="primary-action" disabled>Plan</button>
          <button class="primary-action execute" disabled>Execute</button>
          <button class="secondary" disabled>Stop</button>
        </div>

        <div class="planner-config">
          <label>Planner</label>
          <select disabled>
            <option>RRT-Connect</option>
            <option>RRT</option>
            <option>PRM</option>
          </select>
          <label>Planning Time</label>
          <input type="text" value="5.0s" disabled />
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Trajectory Preview</h3>
          <span class="pill">0 waypoints</span>
        </div>
        <div class="trajectory-table-wrap">
          <table class="trajectory-table">
            <thead>
              <tr>
                <th>#</th>
                <th v-for="j in 6" :key="j">joint_{{ j }}</th>
                <th>time</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td colspan="8" class="empty-trajectory">
                  规划后将在此显示轨迹路径点
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>IK Solver</h3>
        </div>
        <div class="ik-row">
          <div class="pose-grid">
            <div class="pose-row">
              <label>Position</label>
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
            </div>
            <div class="pose-row">
              <label>Quaternion</label>
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
              <input type="text" value="1.0" disabled />
            </div>
          </div>
          <div class="ik-action">
            <button disabled>Solve IK</button>
            <span class="ik-result">—</span>
          </div>
        </div>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { getMoveitStatus, type MoveitStatusResponse } from '../api'

const moveit = ref<MoveitStatusResponse>({
  installed: false, running: false,
  message: 'Backend API is not connected.',
})

const joints = [
  { name: 'shoulder_pan', value: '0.00' },
  { name: 'shoulder_lift', value: '0.00' },
  { name: 'elbow', value: '0.00' },
  { name: 'wrist_1', value: '0.00' },
  { name: 'wrist_2', value: '0.00' },
  { name: 'wrist_3', value: '0.00' },
]

const poseRows = [
  { label: 'XYZ', values: ['0.00', '0.00', '0.00'] },
  { label: 'Quat', values: ['0.00', '0.00', '0.00', '1.00'] },
]

const sceneObjects = [
  { name: 'table', shape: 'box', dims: '0.8 × 1.2 × 0.05' },
  { name: 'obstacle_A', shape: 'sphere', dims: 'radius 0.15' },
  { name: 'target_box', shape: 'cylinder', dims: 'r=0.1 h=0.2' },
]

const goalJoints = ['0.00', '0.00', '0.00', '0.00', '0.00', '0.00']

onMounted(async () => {
  const result = await getMoveitStatus({
    installed: false, running: false,
    message: 'Backend API is not connected.',
  })
  moveit.value = result.data
})
</script>
