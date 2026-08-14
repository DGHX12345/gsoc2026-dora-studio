<template>
  <div class="tool-panel">
    <div class="tool-panel-header">
      <h3>{{ t.tools.title }}</h3>
      <button class="tool-panel-close" type="button" :title="t.tools.close" @click="$emit('close')">✕</button>
    </div>

    <div v-if="tools.length === 0" class="tool-panel-empty">
      <strong>{{ t.tools.empty }}</strong>
    </div>

    <div v-for="category in categories" :key="category.id" class="tool-category">
      <template v-if="category.tools.length > 0">
        <div class="tool-category-label">{{ category.label }}</div>
        <div v-for="tool in category.tools" :key="tool.id" :class="['tool-row', statusOf(tool.id)]">
          <div class="tool-row-main">
            <div class="tool-row-title">
              <span :class="['tool-status-dot', statusOf(tool.id)]"></span>
              <strong>{{ tool.displayName }}</strong>
              <span
                v-if="isRecommended(tool.id)"
                class="tool-recommend-pill"
                :title="t.tools.recommendationHint"
              >{{ t.tools.recommendation }}</span>
            </div>
            <small v-if="tool.description">{{ tool.description }}</small>
          </div>
          <div class="tool-row-actions">
            <span :class="['pill', 'sm', statusOf(tool.id)]">{{ t.tools[statusOf(tool.id)] }}</span>
            <button
              v-if="statusOf(tool.id) !== 'attached'"
              class="tool-toggle-btn"
              type="button"
              @click="$emit('toggle-tool', tool.id, true)"
            >{{ t.tools.attach }}</button>
            <button
              v-else
              class="tool-toggle-btn detach"
              type="button"
              @click="$emit('toggle-tool', tool.id, false)"
            >{{ t.tools.detach }}</button>
          </div>
          <div v-if="statusOf(tool.id) === 'attached'" class="tool-controls">
            <details :open="expanded.has(tool.id)" @toggle="onToggle(tool.id, $event)">
              <summary>{{ t.tools.controls }}</summary>
              <div class="tool-controls-body">
                <component :is="tool.panelComponent" v-if="tool.panelComponent" :tool="tool" />
                <p v-else class="tool-controls-hint">{{ t.tools.noControls }}</p>
              </div>
            </details>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from '../i18n';
import { toolRegistry } from '../tools/registry';
import type { ToolCategory, ViewportTool } from '../tools/types';

export interface ToolRecommendation {
  toolId: string;
  matchedPorts: { nodeId: string; outputId: string }[];
}

const props = defineProps<{
  recommendations?: ToolRecommendation[];
}>();

defineEmits<{
  close: [];
  'toggle-tool': [id: string, enable: boolean];
}>();

const { t } = useI18n();

const tools = ref<ViewportTool[]>(toolRegistry.list());
const expanded = ref(new Set<string>());
const version = ref(0);

const unsubscribe = toolRegistry.subscribe(() => {
  tools.value = toolRegistry.list();
  version.value += 1;
});

onBeforeUnmount(unsubscribe);

const categories = computed(() => {
  const order: ToolCategory[] = ['visualization', 'diagnostics', 'planning'];
  const labels: Record<ToolCategory, string> = {
    visualization: t.value.tools.categoryVisualization,
    diagnostics: t.value.tools.categoryDiagnostics,
    planning: t.value.tools.categoryPlanning,
  };
  return order.map((id) => ({
    id,
    label: labels[id],
    tools: tools.value.filter((tool) => tool.category === id),
  }));
});

const recommendationIds = computed(() => new Set(props.recommendations?.map((r) => r.toolId) ?? []));

function statusOf(id: string) {
  version.value; // track registry changes for reactivity
  return toolRegistry.statusOf(id);
}

function isRecommended(id: string) {
  return recommendationIds.value.has(id);
}

function onToggle(id: string, event: Event) {
  const details = event.target as HTMLDetailsElement;
  if (details.open) expanded.value.add(id);
  else expanded.value.delete(id);
}
</script>

<style scoped>
.tool-panel {
  position: absolute; top: 44px; right: 12px;
  width: 320px; max-height: calc(100% - 140px);
  display: flex; flex-direction: column;
  background: color-mix(in srgb, var(--card-surface) 92%, transparent);
  backdrop-filter: blur(8px);
  border: 1px solid var(--hairline);
  border-radius: 10px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  z-index: 20;
  overflow-y: auto;
}
.tool-panel-header {
  position: sticky; top: 0;
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 12px;
  background: color-mix(in srgb, var(--card-surface) 92%, transparent);
  border-bottom: 1px solid var(--hairline);
}
.tool-panel-header h3 { margin: 0; font-size: 15px; color: var(--text-heading); }
.tool-panel-close {
  background: none; border: none; color: var(--text-body);
  font-size: 14px; cursor: pointer; padding: 2px 6px; border-radius: 4px;
}
.tool-panel-close:hover { background: var(--card-hover); color: var(--text-heading); }
.tool-panel-empty {
  padding: 20px 12px; text-align: center; color: var(--text-muted-dark);
}
.tool-category { padding: 0 12px; }
.tool-category-label {
  padding: 10px 0 6px;
  font-size: 11px; font-weight: 600; letter-spacing: 0.08em;
  text-transform: uppercase; color: var(--text-muted-dark);
}
.tool-row {
  display: flex; flex-direction: column; gap: 6px;
  padding: 10px;
  margin-bottom: 8px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 8px;
}
.tool-row.attached { border-color: color-mix(in srgb, var(--accent-cyan) 40%, var(--hairline)); }
.tool-row.error { border-color: color-mix(in srgb, var(--accent-red) 40%, var(--hairline)); }
.tool-row-main { display: flex; flex-direction: column; gap: 4px; }
.tool-row-title { display: flex; align-items: center; gap: 8px; }
.tool-row-title strong { font-size: 14px; color: var(--text-heading); }
.tool-row-main small { font-size: 12px; color: var(--text-muted-dark); }
.tool-status-dot {
  width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
}
.tool-status-dot.attached { background: var(--accent-green); }
.tool-status-dot.detached { background: var(--hairline); }
.tool-status-dot.error { background: var(--accent-red); }
.tool-recommend-pill {
  font-size: 11px; font-weight: 600;
  padding: 2px 8px; border-radius: 999px;
  background: color-mix(in srgb, var(--accent-yellow) 22%, transparent);
  color: var(--accent-yellow);
}
.tool-row-actions {
  display: flex; align-items: center; justify-content: flex-end; gap: 8px;
}
.tool-toggle-btn {
  padding: 7px 14px; border: 1px solid var(--hairline); border-radius: 6px;
  font-size: 13px; cursor: pointer;
  background: var(--canvas-base); color: var(--text-body);
}
.tool-toggle-btn:hover { background: var(--card-hover); color: var(--text-heading); }
.tool-toggle-btn.detach { color: var(--accent-red); border-color: var(--accent-red); }
.tool-controls details {
  border-top: 1px solid var(--hairline);
  padding-top: 8px;
}
.tool-controls summary {
  font-size: 12px; color: var(--text-body); cursor: pointer;
}
.tool-controls-body { padding-top: 8px; }
.tool-controls-hint { margin: 0; font-size: 12px; color: var(--text-muted-dark); }
.pill.attached { background: color-mix(in srgb, var(--accent-green) 20%, transparent); color: var(--accent-green); }
.pill.detached { background: var(--canvas-base); color: var(--text-muted-dark); }
.pill.error { background: color-mix(in srgb, var(--accent-red) 20%, transparent); color: var(--accent-red); }
</style>
