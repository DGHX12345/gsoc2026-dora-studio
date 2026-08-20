<script setup lang="ts">
import { ref, computed } from 'vue'
import type { PaletteEntry, PalettePort } from '../api'
import { useI18n } from '../i18n'

const { t } = useI18n()

const props = defineProps<{ entries: PaletteEntry[] }>()
const emit = defineEmits<{
  'drag-start': [entry: PaletteEntry]
  'add-manual': []
}>()

const search = ref('')

const groups = computed(() => {
  const q = search.value.toLowerCase()
  const filtered = props.entries.filter(entry => {
    if (!q) return true
    return entry.operator.toLowerCase().includes(q) ||
      entry.inputs.some(port => port.name.toLowerCase().includes(q)) ||
      entry.outputs.some(port => port.name.toLowerCase().includes(q))
  })
  const byProject = new Map<string, PaletteEntry[]>()
  for (const entry of filtered) {
    const list = byProject.get(entry.project) ?? []
    list.push(entry)
    byProject.set(entry.project, list)
  }
  return Array.from(byProject.entries()).map(([project, entries]) => ({ project, entries }))
})

function portNames(ports: PalettePort[]): string {
  return ports.map(p => p.name).join(', ')
}

function portUrns(ports: PalettePort[]): string {
  const urns = ports.filter(p => p.urn).map(p => `${p.name}: ${p.urn}`)
  return urns.length ? `types; ${urns.join('; ')}` : ''
}

function onDragStart(e: DragEvent, entry: PaletteEntry) {
  e.dataTransfer!.effectAllowed = 'copy'
  e.dataTransfer!.setData('application/json', JSON.stringify(entry))
  emit('drag-start', entry)
}

// Only --accent-cyan/green/red/yellow exist as accent tokens; map runtimes
// onto them (python=cyan brand accent, rust=amber, c/cpp=red/green).
const runtimeColor = (r: string) => ({ python: 'var(--accent-cyan)', rust: 'var(--accent-yellow)', c: 'var(--accent-red)', cpp: 'var(--accent-green)' })[r] ?? 'var(--text-muted-dark)'
</script>

<template>
  <div class="palette">
    <div class="palette-header">Nodes</div>
    <input v-model="search" class="palette-search" placeholder="Search nodes..." />
    <div v-for="group in groups" :key="group.project" class="palette-category">
      <div class="palette-cat-label">
        {{ group.project }}
        <span v-if="group.entries.every(e => e.manual)" class="palette-cat-manual">(manual)</span>
      </div>
      <div
        v-for="entry in group.entries"
        :key="entry.id"
        class="palette-item"
        draggable="true"
        @dragstart="onDragStart($event, entry)"
      >
        <div class="palette-item-head">
          <span class="palette-item-name">{{ entry.operator }}</span>
          <span class="palette-item-right">
            <span v-if="entry.manual" class="palette-item-manual">manual</span>
            <span class="palette-item-runtime" :style="{ color: runtimeColor(entry.runtime) }">{{ entry.runtime }}</span>
          </span>
        </div>
        <div v-if="entry.path" class="palette-item-desc">{{ entry.path }}</div>
        <div class="palette-item-ports">
          <span v-if="entry.inputs.length" class="port-in" :title="portUrns(entry.inputs) || undefined">
            in: {{ portNames(entry.inputs) }}
            <span v-if="entry.inputs.some(p => p.urn)" class="port-typed"></span>
          </span>
          <span v-if="entry.outputs.length" class="port-out" :title="portUrns(entry.outputs) || undefined">
            out: {{ portNames(entry.outputs) }}
            <span v-if="entry.outputs.some(p => p.urn)" class="port-typed"></span>
          </span>
        </div>
      </div>
    </div>
    <div v-if="groups.length === 0" class="palette-empty">No nodes found</div>
    <button class="palette-add-manual" @click="emit('add-manual')">{{ t.explorer.addManual }}</button>
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
.palette-cat-manual { font-size: 12px; color: var(--accent-yellow); text-transform: none; letter-spacing: 0; }
.palette-item {
  margin: 2px 10px; padding: 8px 10px;
  background: var(--card-surface); border: 1px solid var(--hairline);
  border-radius: 6px; cursor: grab;
  transition: border-color 120ms ease, background 120ms ease;
}
.palette-item:hover { border-color: var(--hairline-hover); background: var(--card-hover); }
.palette-item:active { cursor: grabbing; }
.palette-item-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2px; }
.palette-item-right { display: flex; align-items: center; gap: 6px; }
.palette-item-name { font-size: 12px; font-weight: 510; color: var(--text-heading); }
.palette-item-manual {
  font-size: 12px; font-weight: 600; color: var(--accent-yellow);
  border: 1px solid var(--hairline-hover); border-radius: 4px; padding: 0 5px;
}
.palette-item-runtime { font-size: 9px; font-weight: 600; }
.palette-item-desc { font-size: 10px; color: var(--text-muted-dark); margin-bottom: 3px; }
.palette-item-ports { font-size: 9px; display: flex; gap: 8px; }
.port-in { color: var(--accent-green); }
.port-out { color: var(--accent-cyan); }
.port-typed {
  display: inline-block; width: 5px; height: 5px; border-radius: 50%;
  background: var(--accent-cyan); margin-left: 4px; vertical-align: middle;
}
.palette-empty { padding: 24px 16px; text-align: center; color: var(--text-muted-dark); font-size: 12px; }
.palette-add-manual {
  margin: 10px 12px 12px; padding: 8px 10px;
  background: var(--card-surface); border: 1px dashed var(--hairline-hover);
  border-radius: 6px; color: var(--accent-cyan); font-size: 12px; font-weight: 510;
  cursor: pointer; transition: background 120ms ease;
}
.palette-add-manual:hover { background: var(--card-hover); }
</style>
