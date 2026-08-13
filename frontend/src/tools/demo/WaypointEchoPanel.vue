<template>
  <div class="tool-control-panel">
    <p class="tool-control-desc">{{ tool.description }}</p>
    <div v-if="stats" class="tool-stat-grid">
      <div class="tool-stat">
        <strong>{{ stats.pointCount }}</strong>
        <span>points</span>
      </div>
      <div class="tool-stat">
        <strong>{{ stats.lastSource }}</strong>
        <span>source</span>
      </div>
    </div>
    <p v-else class="tool-control-hint">No waypoint data received yet — load the tool demo .drec and start replay.</p>
    <div class="tool-port-list">
      <code v-for="port in tool.subscribePorts" :key="String(port.outputIdPattern)">
        {{ String(port.outputIdPattern) }}
      </code>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ViewportTool } from '../types';
import type { WaypointEchoTool, WaypointStats } from './WaypointEchoTool';

const props = defineProps<{ tool: ViewportTool }>();

const stats = computed<WaypointStats | null>(() => {
  const tool = props.tool as WaypointEchoTool;
  return typeof tool.stats === 'function' ? tool.stats() : null;
});
</script>

<style scoped>
.tool-control-panel {
  display: flex; flex-direction: column; gap: 8px;
  font-size: 13px;
}
.tool-control-desc { margin: 0; color: var(--text-body); }
.tool-control-hint { margin: 0; color: var(--text-muted-dark); }
.tool-stat-grid {
  display: flex; gap: 16px;
}
.tool-stat {
  display: flex; flex-direction: column; gap: 2px;
  padding: 6px 10px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 6px;
}
.tool-stat strong {
  font-size: 15px; font-family: monospace; color: var(--accent-cyan);
  word-break: break-all;
}
.tool-stat span { font-size: 11px; color: var(--text-muted-dark); }
.tool-port-list {
  display: flex; flex-wrap: wrap; gap: 6px;
}
.tool-port-list code {
  font-size: 11px;
  padding: 2px 6px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline);
  border-radius: 4px;
  color: var(--text-body);
}
</style>
