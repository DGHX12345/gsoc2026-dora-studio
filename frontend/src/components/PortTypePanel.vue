<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { getTypeCatalog, type TypeCatalogEntry } from '../api'
import { useI18n } from '../i18n'
import type { NodeSpec } from './DataflowCanvas.vue'

const { t } = useI18n()

const props = defineProps<{ node: NodeSpec | null }>()
const emit = defineEmits<{ 'update-port': [portName: string, isInput: boolean, urn: string] }>()

const catalog = ref<TypeCatalogEntry[]>([])
const search = ref('')
onMounted(async () => {
  try {
    const result = await getTypeCatalog()
    catalog.value = result.types
  } catch {
    catalog.value = []
  }
})

const grouped = computed(() => {
  const q = search.value.toLowerCase()
  const groups: Record<string, TypeCatalogEntry[]> = {}
  for (const entry of catalog.value) {
    if (q && !(entry.urn.toLowerCase().includes(q) || entry.name.toLowerCase().includes(q))) continue
    ;(groups[entry.category] ??= []).push(entry)
  }
  return groups
})

function pick(urn: string, portName: string, isInput: boolean) {
  emit('update-port', portName, isInput, urn)
}
</script>

<template>
  <div v-if="props.node" class="port-type-panel">
    <div class="ptp-header">{{ t.explorer.portTypes.replace('{id}', props.node.id) }}</div>
    <input v-model="search" class="ptp-search" :placeholder="t.explorer.searchType" />
    <div class="ptp-section" v-if="Object.keys(props.node.inputs).length">
      <div class="ptp-label">Inputs</div>
      <div v-for="(port, name) in props.node.inputs" :key="name" class="ptp-port">
        <span class="ptp-name">{{ name }}</span>
        <select class="ptp-select" :value="port.type ?? ''" @change="pick(($event.target as HTMLSelectElement).value, name, true)">
          <option value="">{{ port.type ? t.explorer.clearType : t.explorer.selectType }}</option>
          <optgroup v-for="(entries, category) in grouped" :key="category" :label="category">
            <option v-for="entry in entries" :key="entry.urn" :value="entry.urn">{{ entry.name }} — {{ entry.urn }}</option>
          </optgroup>
        </select>
      </div>
    </div>
    <div class="ptp-section" v-if="Object.keys(props.node.outputs).length">
      <div class="ptp-label">Outputs</div>
      <div v-for="(port, name) in props.node.outputs" :key="name" class="ptp-port">
        <span class="ptp-name">{{ name }}</span>
        <select class="ptp-select" :value="port.type ?? ''" @change="pick(($event.target as HTMLSelectElement).value, name, false)">
          <option value="">{{ port.type ? t.explorer.clearType : t.explorer.selectType }}</option>
          <optgroup v-for="(entries, category) in grouped" :key="category" :label="category">
            <option v-for="entry in entries" :key="entry.urn" :value="entry.urn">{{ entry.name }} — {{ entry.urn }}</option>
          </optgroup>
        </select>
      </div>
    </div>
    <div v-if="!catalog.length" class="ptp-empty">{{ t.explorer.catalogUnavailable }}</div>
  </div>
</template>

<style scoped>
.port-type-panel { padding: 12px; border-left: 1px solid var(--hairline); background: var(--panel-surface); overflow-y: auto; min-width: 260px; }
.ptp-header { font-size: 12px; font-weight: 600; color: var(--text-heading); margin-bottom: 8px; }
.ptp-search { width: 100%; padding: 6px 8px; font-size: 12px; background: var(--card-surface); border: 1px solid var(--hairline); border-radius: 6px; color: var(--text-body); margin-bottom: 8px; }
.ptp-label { font-size: 12px; color: var(--text-muted-dark); margin: 8px 0 4px; }
.ptp-port { display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px; }
.ptp-name { font-size: 12px; color: var(--text-body); }
.ptp-select { padding: 6px 8px; font-size: 12px; background: var(--card-surface); border: 1px solid var(--hairline); border-radius: 6px; color: var(--text-body); }
.ptp-empty { font-size: 12px; color: var(--text-muted-dark); padding: 8px 0; }
</style>
