<template>
  <div class="moveit-panel">
    <!-- Robot model -->
    <section class="mp-section">
      <div class="mp-row">
        <label class="mp-label" for="mp-model">{{ t.tools.moveit.model }}</label>
        <select id="mp-model" class="mp-select" :value="modelSelection" @change="onModelChange">
          <option v-for="entry in availableModels" :key="entry.id" :value="entry.id">
            {{ modelLabel(entry.id) }}
          </option>
          <option value="">{{ t.tools.moveit.noModel }}</option>
        </select>
      </div>
      <div v-if="robotState" class="mp-row">
        <span class="pill" :class="`mp-${robotState}`">{{ t.tools.moveit[`state_${robotState}`] }}</span>
        <span v-if="modelName" class="mp-muted mono">{{ modelName }}</span>
      </div>
    </section>

    <!-- Trajectory player -->
    <section v-if="snapshot.trajectory" class="mp-section">
      <div class="mp-section-title">{{ t.tools.moveit.player }}</div>
      <div class="mp-row">
        <button class="mp-btn" type="button" @click="togglePlay">
          {{ snapshot.player.playing ? t.tools.moveit.pause : t.tools.moveit.play }}
        </button>
        <button class="mp-btn" type="button" :title="t.tools.moveit.stepBack" @click="moveit.stepTrajectory(-1)">◀</button>
        <button class="mp-btn" type="button" :title="t.tools.moveit.stepForward" @click="moveit.stepTrajectory(1)">▶</button>
        <select class="mp-select mp-speed" :value="snapshot.player.speed" @change="onSpeedChange">
          <option :value="0.5">0.5×</option>
          <option :value="1">1×</option>
          <option :value="2">2×</option>
        </select>
      </div>
      <div class="mp-row">
        <label class="mp-check">
          <input
            type="checkbox"
            :checked="snapshot.player.syncToTimeline"
            @change="onSyncChange"
          />
          {{ t.tools.moveit.syncToTimeline }}
        </label>
        <span class="mp-muted">
          {{ snapshot.player.waypointIndex + 1 }} / {{ snapshot.player.waypointCount }}
        </span>
        <span v-if="snapshot.trajectory.stale" class="pill mp-stale">{{ t.tools.moveit.stale }}</span>
      </div>
    </section>

    <!-- End effector -->
    <section v-if="snapshot.endEffector" class="mp-section">
      <div class="mp-section-title">{{ t.tools.moveit.endEffector }}</div>
      <div class="mp-ee mono">
        <span>x {{ snapshot.endEffector.x.toFixed(3) }}</span>
        <span>y {{ snapshot.endEffector.y.toFixed(3) }}</span>
        <span>z {{ snapshot.endEffector.z.toFixed(3) }}</span>
      </div>
    </section>

    <!-- Joint table -->
    <section v-if="snapshot.currentJointValues" class="mp-section">
      <div class="mp-section-title">{{ t.tools.moveit.joints }}</div>
      <table class="mp-joints">
        <tbody>
          <tr v-for="(value, i) in snapshot.currentJointValues" :key="i">
            <td class="mono">{{ snapshot.jointLabels[i] ?? `J${i}` }}</td>
            <td class="mono mp-value">{{ value.toFixed(3) }}</td>
          </tr>
        </tbody>
      </table>
    </section>

    <!-- Collision scene -->
    <section v-if="snapshot.scene" class="mp-section">
      <div class="mp-section-title">{{ t.tools.moveit.collisionScene }}</div>
      <label class="mp-check">
        <input type="checkbox" :checked="snapshot.collisionVisible" @change="onCollisionToggle" />
        {{ t.tools.moveit.showWireframes }}
      </label>
      <div v-if="snapshot.sceneCollisions.length > 0" class="mp-warn">
        {{ t.tools.moveit.collisions }}:
        <span v-for="pair in snapshot.sceneCollisions" :key="`${pair.a}-${pair.b}`" class="mono">
          {{ pair.a }}↔{{ pair.b }}
        </span>
      </div>
    </section>

    <!-- Ghost poses -->
    <section v-if="robotState === 'loaded' && snapshot.trajectory" class="mp-section">
      <div class="mp-section-title">{{ t.tools.moveit.ghosts }} ({{ snapshot.ghostCount }})</div>
      <input
        class="mp-range"
        type="range"
        min="0"
        max="20"
        :value="snapshot.ghostCount"
        @input="onGhostCount"
      />
    </section>

    <!-- Plan / execution status -->
    <section v-if="snapshot.planStatus || snapshot.execution" class="mp-section">
      <div class="mp-section-title">{{ t.tools.moveit.status }}</div>
      <div v-if="snapshot.planStatus" class="mp-row">
        <span class="pill" :class="snapshot.planStatus.status.success ? 'mp-loaded' : 'mp-error'">
          {{ snapshot.planStatus.status.success ? t.tools.moveit.planOk : t.tools.moveit.planFail }}
        </span>
        <span v-if="snapshot.planStatus.status.message" class="mp-muted">{{ snapshot.planStatus.status.message }}</span>
      </div>
      <div v-if="snapshot.execution" class="mp-row">
        <span class="mp-muted">{{ t.tools.moveit.execution }}</span>
        <span v-if="snapshot.execution.status.is_executing" class="mp-progress mono">
          {{ Math.round(snapshot.execution.status.progress * 100) }}%
        </span>
        <span v-else class="mp-muted">{{ t.tools.moveit.idle }}</span>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from '../../i18n';
import type { ViewportTool } from '../types';
import { getRobotConfig } from './joint-config';
import { fetchModelCatalog, MoveItTool, type ModelCatalogEntry } from './MoveItTool';

const props = defineProps<{ tool: ViewportTool }>();

const moveit = props.tool as MoveItTool;

const { t } = useI18n();

const snapshot = ref(moveit.getSnapshot());
const availableModels = ref<ModelCatalogEntry[]>([]);
const modelSelection = ref(snapshot.value.robotId ?? '');

const unsubscribe = moveit.subscribe(() => {
  snapshot.value = moveit.getSnapshot();
  if (!modelSelection.value && snapshot.value.robotId) modelSelection.value = snapshot.value.robotId;
});

const robotState = computed(() => snapshot.value.robotState);
const modelName = computed(() => snapshot.value.modelName);

onMounted(async () => {
  try {
    availableModels.value = await fetchModelCatalog();
  } catch {
    availableModels.value = []; // honest empty: selector shows only 无模型
  }
});

onBeforeUnmount(unsubscribe);

function modelLabel(id: string): string {
  return getRobotConfig(id)?.label ?? id;
}

function onModelChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  modelSelection.value = value;
  if (value === '') {
    moveit.unloadRobot();
  } else {
    moveit.setRobot(value);
  }
}

function togglePlay() {
  moveit.setTrajectoryPlayback({ playing: !snapshot.value.player.playing });
}

function onSpeedChange(event: Event) {
  const speed = Number((event.target as HTMLSelectElement).value);
  moveit.setTrajectoryPlayback({ playing: snapshot.value.player.playing, speed });
}

function onSyncChange(event: Event) {
  moveit.setSyncToTimeline((event.target as HTMLInputElement).checked);
}

function onCollisionToggle(event: Event) {
  moveit.setCollisionVisible((event.target as HTMLInputElement).checked);
}

function onGhostCount(event: Event) {
  moveit.setGhostCount(Number((event.target as HTMLInputElement).value));
}
</script>

<style scoped>
.moveit-panel { display: flex; flex-direction: column; gap: 10px; }
.mp-section {
  display: flex; flex-direction: column; gap: 6px;
  padding: 10px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 8px;
}
.mp-section-title { font-size: 12px; font-weight: 600; color: var(--text-heading); }
.mp-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
/* Control text is black-on-gray in the light theme (user feedback
   2026-08-14: gray backgrounds need black text); the dark theme keeps
   its light heading color. --text-primary flips per theme. */
.mp-label { font-size: 14px; color: var(--text-primary); }
.mp-select {
  flex: 1; min-width: 0;
  padding: 6px 8px; font-size: 14px;
  background: var(--bg-surface); color: var(--text-primary);
  border: 1px solid var(--hairline); border-radius: 6px;
}
.mp-select option { background: var(--bg-surface); color: var(--text-primary); }
.mp-speed { flex: 0 0 auto; }
.mp-btn {
  padding: 8px 14px; font-size: 14px; cursor: pointer;
  background: var(--bg-surface); color: var(--text-primary);
  border: 1px solid var(--hairline); border-radius: 6px;
}
.mp-btn:hover { background: var(--card-hover); }
.mp-check { font-size: 14px; color: var(--text-primary); display: flex; align-items: center; gap: 6px; }

[data-theme="dark"] .mp-label,
[data-theme="dark"] .mp-select,
[data-theme="dark"] .mp-select option,
[data-theme="dark"] .mp-btn,
[data-theme="dark"] .mp-check {
  color: var(--text-heading);
}
.mp-muted { font-size: 12px; color: var(--text-muted-dark); }
.mp-ee { display: flex; gap: 12px; font-size: 13px; color: var(--text-body); }
.mp-joints { width: 100%; border-collapse: collapse; }
.mp-joints td {
  padding: 3px 6px; font-size: 13px;
  color: var(--text-body); border-bottom: 1px solid var(--hairline);
}
.mp-value { text-align: right; }
.mp-warn {
  font-size: 12px; color: var(--accent-yellow);
  display: flex; gap: 8px; flex-wrap: wrap;
}
.mp-range { width: 100%; accent-color: var(--accent-cyan); }
.mp-progress { font-size: 13px; color: var(--text-body); }
.pill {
  font-size: 12px; font-weight: 600; padding: 2px 10px; border-radius: 999px;
  background: var(--canvas-base); color: var(--text-muted-dark);
}
.pill.mp-loaded { background: color-mix(in srgb, var(--accent-green) 20%, transparent); color: var(--accent-green); }
.pill.mp-loading { background: color-mix(in srgb, var(--accent-yellow) 20%, transparent); color: var(--accent-yellow); }
.pill.mp-unavailable, .pill.mp-error { background: color-mix(in srgb, var(--accent-red) 20%, transparent); color: var(--accent-red); }
.pill.mp-stale { background: color-mix(in srgb, var(--accent-yellow) 20%, transparent); color: var(--accent-yellow); }
</style>
