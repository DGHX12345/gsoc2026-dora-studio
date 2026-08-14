<template>
  <div class="dviz-panel">
    <template v-if="snapshot">
      <p v-if="snapshot.paths.length === 0 && !snapshot.costmap && !snapshot.target" class="dviz-hint">
        {{ t.tools.dviz.hint }}
      </p>

      <template v-else>
        <div v-if="snapshot.paths.length > 0" class="dviz-path-list">
          <div v-for="path in snapshot.paths" :key="path.key" class="dviz-path-row">
            <div class="dviz-path-head">
              <span
                :class="['dviz-swatch', path.kind === 'alternative' ? 'alt' : '']"
                :style="{ background: colorCss(path.colorHex) }"
              ></span>
              <div class="dviz-path-name">
                <code>{{ path.nodeId }}/{{ path.outputId }}</code>
                <div class="dviz-badges">
                  <span v-if="path.kind === 'alternative'" class="dviz-badge secondary">
                    {{ t.tools.dviz.alternative }}
                  </span>
                  <span v-if="path.stale" class="dviz-badge stale">
                    {{ t.tools.dviz.stale }}
                  </span>
                </div>
              </div>
            </div>
            <div class="dviz-path-stats">
              {{ path.pointCount }} {{ t.tools.dviz.points }} · {{ formatLength(path.length) }}
            </div>
            <div class="dviz-path-actions">
              <button type="button" class="dviz-btn" @click="togglePath(path.key, !path.visible)">
                {{ path.visible ? t.tools.dviz.hide : t.tools.dviz.show }}
              </button>
              <button type="button" class="dviz-btn" @click="snap(path.key)">
                {{ t.tools.dviz.snap }}
              </button>
            </div>
          </div>
        </div>

        <div v-if="snapshot.target" class="dviz-target-row">
          <span class="dviz-target-label">{{ t.tools.dviz.target }}</span>
          <code class="dviz-target-value">
            {{ formatPosition(snapshot.target.x, snapshot.target.y, snapshot.target.z) }}
          </code>
        </div>

        <div v-if="snapshot.costmap" class="dviz-costmap">
          <div class="dviz-costmap-head">
            <strong>{{ t.tools.dviz.costmapTitle }}</strong>
            <label class="dviz-check">
              <input
                type="checkbox"
                :checked="snapshot.costmap.visible"
                @change="onCostmapVisible($event)"
              />
              <span>{{ t.tools.dviz.costmapVisible }}</span>
            </label>
          </div>
          <label class="dviz-opacity">
            <span>{{ opacityLabel }}</span>
            <input
              type="range"
              min="0"
              max="100"
              step="1"
              :value="opacityPercent"
              @input="onOpacity($event)"
            />
          </label>
          <p class="dviz-costmap-info">
            {{ snapshot.costmap.width }}×{{ snapshot.costmap.height }} {{ t.tools.dviz.cells }} ·
            {{ fmtResolution(snapshot.costmap.resolution) }} {{ t.tools.dviz.perCell }}
          </p>
        </div>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from '../../i18n';
import type { ViewportTool } from '../types';
import type { ToolSnapshot } from './DvizPathTool';
import { DvizPathTool } from './DvizPathTool';
import { formatLength, formatPosition } from './format';

const props = defineProps<{ tool: ViewportTool }>();

const { t } = useI18n();

// Reactivity (plan R7): the ToolPanel does not re-render on batch broadcasts,
// so the panel subscribes to the tool and refreshes a snapshot ref itself.
const snapshot = ref<ToolSnapshot | null>(null);
let unsubscribe: (() => void) | null = null;

function dvizTool(): DvizPathTool {
  return props.tool as DvizPathTool;
}

function refresh() {
  snapshot.value = dvizTool().getSnapshot();
}

onMounted(() => {
  refresh();
  unsubscribe = dvizTool().subscribe(refresh);
});

onBeforeUnmount(() => {
  unsubscribe?.();
  unsubscribe = null;
});

const opacityPercent = computed(() =>
  Math.round((snapshot.value?.costmap?.opacity ?? 0) * 100),
);
const opacityLabel = computed(() => `${opacityPercent.value}%`);

/** #rrggbb from a numeric color (e.g. 0x22d3ee). */
function colorCss(hex: number) {
  return `#${hex.toString(16).padStart(6, '0')}`;
}

function togglePath(key: string, visible: boolean) {
  dvizTool().setPathVisible(key, visible);
}

function snap(key: string) {
  dvizTool().snapCameraToPath(key);
}

function onCostmapVisible(event: Event) {
  dvizTool().setCostmapVisible((event.target as HTMLInputElement).checked);
}

function onOpacity(event: Event) {
  const percent = Number((event.target as HTMLInputElement).value);
  dvizTool().setCostmapOpacity(percent / 100);
}

function fmtResolution(v: number) {
  return v.toFixed(2);
}
</script>

<style scoped>
.dviz-panel {
  display: flex; flex-direction: column; gap: 12px;
  font-size: 13px;
}
.dviz-hint {
  margin: 0;
  font-size: 13px; line-height: 1.6;
  color: var(--text-muted-dark);
}
.dviz-path-list {
  display: flex; flex-direction: column; gap: 10px;
}
.dviz-path-row {
  display: flex; flex-direction: column; gap: 8px;
  padding: 10px 12px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 8px;
}
.dviz-path-head {
  display: flex; align-items: flex-start; gap: 10px;
}
.dviz-swatch {
  flex-shrink: 0;
  width: 14px; height: 14px;
  margin-top: 2px;
  border: 1px solid var(--hairline);
  border-radius: 3px;
}
/* Alternative paths render dashed at low opacity in 3D — hint it here. */
.dviz-swatch.alt {
  opacity: 0.5;
  border-style: dashed;
}
.dviz-path-name {
  display: flex; flex-direction: column; gap: 5px;
  min-width: 0;
}
.dviz-path-name code {
  font-family: monospace;
  font-size: 13px; line-height: 1.4;
  color: var(--text-heading);
  word-break: break-all;
}
.dviz-badges {
  display: flex; flex-wrap: wrap; gap: 6px;
}
.dviz-badge {
  font-size: 12px; font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
}
.dviz-badge.secondary {
  background: color-mix(in srgb, var(--accent-cyan) 18%, transparent);
  color: var(--accent-cyan);
}
.dviz-badge.stale {
  background: color-mix(in srgb, var(--accent-yellow) 22%, transparent);
  color: var(--accent-yellow);
}
.dviz-path-stats {
  font-size: 13px;
  color: var(--text-body);
}
.dviz-path-actions {
  display: flex; gap: 8px;
}
.dviz-btn {
  flex: 1;
  padding: 10px 12px;
  font-size: 13px;
  background: var(--card-surface);
  color: var(--text-body);
  border: 1px solid var(--hairline);
  border-radius: 6px;
  cursor: pointer;
}
.dviz-btn:hover {
  background: var(--card-hover);
  color: var(--text-heading);
}
.dviz-target-row {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 12px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 8px;
}
.dviz-target-label {
  flex-shrink: 0;
  font-size: 13px;
  color: var(--text-body);
}
.dviz-target-value {
  font-family: monospace;
  font-size: 13px;
  color: var(--accent-cyan);
  word-break: break-all;
}
.dviz-costmap {
  display: flex; flex-direction: column; gap: 10px;
  padding: 10px 12px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 8px;
}
.dviz-costmap-head {
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
}
.dviz-costmap-head strong {
  font-size: 13px;
  color: var(--text-heading);
}
.dviz-check {
  display: flex; align-items: center; gap: 7px;
  font-size: 13px;
  color: var(--text-body);
  cursor: pointer;
}
.dviz-check input {
  width: 18px; height: 18px;
  accent-color: var(--accent-cyan);
  cursor: pointer;
}
.dviz-opacity {
  display: flex; flex-direction: column; gap: 6px;
}
.dviz-opacity > span {
  font-size: 12px;
  color: var(--text-muted-dark);
}
.dviz-opacity input[type='range'] {
  width: 100%;
  height: 10px;
  accent-color: var(--accent-cyan);
  cursor: pointer;
}
.dviz-costmap-info {
  margin: 0;
  font-family: monospace;
  font-size: 13px;
  color: var(--text-body);
  word-break: break-all;
}
</style>
