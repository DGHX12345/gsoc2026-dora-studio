<script setup lang="ts">
import { ref, computed } from 'vue'

interface PaletteEntry {
  operatorId: string; runtime: string; category: string;
  description: string; inputs: string[]; outputs: string[];
}

const emit = defineEmits<{ 'drag-start': [entry: PaletteEntry] }>()

const search = ref('')

const categories: Record<string, PaletteEntry[]> = {
  perception: [
    { operatorId: 'camera_driver', runtime: 'python', category: 'perception', description: 'Camera image capture', inputs: [], outputs: ['image'] },
    { operatorId: 'lidar_driver', runtime: 'rust', category: 'perception', description: 'LiDAR point cloud capture', inputs: [], outputs: ['pointcloud'] },
    { operatorId: 'object_detection', runtime: 'python', category: 'perception', description: 'Detect objects in images', inputs: ['image'], outputs: ['bboxes'] },
  ],
  planning: [
    { operatorId: 'planner', runtime: 'python', category: 'planning', description: 'Path/trajectory planner', inputs: ['scene_update'], outputs: ['trajectory', 'plan_status'] },
    { operatorId: 'path_follower', runtime: 'python', category: 'planning', description: 'Follow generated paths', inputs: ['waypoints', 'pose'], outputs: ['cmd_vel'] },
  ],
  control: [
    { operatorId: 'controller', runtime: 'rust', category: 'control', description: 'Joint/motor controller', inputs: ['joint_commands'], outputs: ['joint_positions'] },
    { operatorId: 'trajectory_executor', runtime: 'python', category: 'control', description: 'Execute joint trajectories', inputs: ['trajectory', 'joint_positions'], outputs: ['joint_commands'] },
  ],
  llm: [
    { operatorId: 'vlm_node', runtime: 'python', category: 'llm', description: 'Vision language model inference', inputs: ['image', 'prompt_template'], outputs: ['response', 'action_vector'] },
    { operatorId: 'llm_node', runtime: 'python', category: 'llm', description: 'LLM text generation', inputs: ['prompt'], outputs: ['response'] },
  ],
  hardware: [
    { operatorId: 'motor_driver', runtime: 'rust', category: 'hardware', description: 'Motor hardware interface', inputs: ['cmd_vel'], outputs: ['odom'] },
    { operatorId: 'gripper_driver', runtime: 'python', category: 'hardware', description: 'Gripper control', inputs: ['gripper_cmd'], outputs: ['gripper_state'] },
  ],
}

const filtered = computed(() => {
  const q = search.value.toLowerCase()
  if (!q) return categories
  const result: Record<string, PaletteEntry[]> = {}
  for (const [cat, entries] of Object.entries(categories)) {
    const filtered = entries.filter(e => e.operatorId.toLowerCase().includes(q) || e.description.toLowerCase().includes(q))
    if (filtered.length) result[cat] = filtered
  }
  return result
})

const runtimeColor = (r: string) => ({ python: '#3b82f6', rust: '#f59e0b', c: '#8b5cf6', cpp: '#a78bfa' })[r] ?? '#6b7280'

function onDragStart(e: DragEvent, entry: PaletteEntry) {
  e.dataTransfer!.effectAllowed = 'copy'
  e.dataTransfer!.setData('application/json', JSON.stringify(entry))
  emit('drag-start', entry)
}
</script>

<template>
  <div class="palette">
    <div class="palette-header">Nodes</div>
    <input v-model="search" class="palette-search" placeholder="Search nodes..." />
    <div v-for="(entries, cat) in filtered" :key="cat" class="palette-category">
      <div class="palette-cat-label">{{ cat }}</div>
      <div
        v-for="entry in entries" :key="entry.operatorId"
        class="palette-item"
        draggable="true"
        @dragstart="onDragStart($event, entry)"
      >
        <div class="palette-item-head">
          <span class="palette-item-name">{{ entry.operatorId }}</span>
          <span class="palette-item-runtime" :style="{ color: runtimeColor(entry.runtime) }">{{ entry.runtime }}</span>
        </div>
        <div class="palette-item-desc">{{ entry.description }}</div>
        <div class="palette-item-ports">
          <span v-if="entry.inputs.length" class="port-in">in: {{ entry.inputs.join(', ') }}</span>
          <span v-if="entry.outputs.length" class="port-out">out: {{ entry.outputs.join(', ') }}</span>
        </div>
      </div>
    </div>
    <div v-if="Object.keys(filtered).length === 0" class="palette-empty">No nodes found</div>
  </div>
</template>

<style scoped>
.palette {
  width: 240px; min-width: 240px;
  background: var(--panel-surface);
  border-right: 1px solid var(--hairline);
  display: flex; flex-direction: column;
  overflow-y: auto; user-select: none;
}
.palette-header {
  padding: 14px 16px 10px;
  font-size: 12px; font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.05em; color: var(--text-muted-dark);
}
.palette-search {
  margin: 0 12px 10px; padding: 7px 10px;
  background: var(--card-surface); border: 1px solid var(--hairline);
  border-radius: 6px; color: var(--text-body); font-size: 12px;
  outline: none;
}
.palette-search:focus { border-color: var(--accent-cyan); }
.palette-search::placeholder { color: var(--text-muted-dark); }
.palette-category { margin-bottom: 4px; }
.palette-cat-label {
  padding: 8px 16px 4px; font-size: 10px; font-weight: 600;
  text-transform: uppercase; letter-spacing: 0.06em;
  color: var(--text-muted-dark);
}
.palette-item {
  margin: 2px 10px; padding: 8px 10px;
  background: var(--card-surface); border: 1px solid var(--hairline);
  border-radius: 6px; cursor: grab;
  transition: border-color 120ms ease, background 120ms ease;
}
.palette-item:hover { border-color: var(--hairline-hover); background: var(--card-hover); }
.palette-item:active { cursor: grabbing; }
.palette-item-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2px; }
.palette-item-name { font-size: 12px; font-weight: 510; color: var(--text-heading); }
.palette-item-runtime { font-size: 9px; font-weight: 600; }
.palette-item-desc { font-size: 10px; color: var(--text-muted-dark); margin-bottom: 3px; }
.palette-item-ports { font-size: 9px; display: flex; gap: 8px; }
.port-in { color: var(--accent-green); }
.port-out { color: var(--accent-cyan); }
.palette-empty { padding: 24px 16px; text-align: center; color: var(--text-muted-dark); font-size: 12px; }
</style>
