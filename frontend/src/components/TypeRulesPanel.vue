<script setup lang="ts">
import type { TypeRule } from '../api'
import { useI18n } from '../i18n'

const { t } = useI18n()
const props = defineProps<{ rules: TypeRule[] }>()
const emit = defineEmits<{ 'update:rules': [rules: TypeRule[]] }>()

function remove(index: number) {
  if (!window.confirm(t.value.explorer.removeRuleConfirm)) return
  const next = props.rules.filter((_, i) => i !== index)
  emit('update:rules', next)
}
</script>

<template>
  <div class="type-rules-panel">
    <div class="trp-header">{{ t.explorer.typeRules.replace('{count}', String(props.rules.length)) }}</div>
    <div v-if="!props.rules.length" class="trp-empty">{{ t.explorer.noRules }}</div>
    <div v-for="(rule, index) in props.rules" :key="`${rule.from}-${rule.to}`" class="trp-rule">
      <span class="trp-rule-text">{{ rule.from }} → {{ rule.to }}</span>
      <button class="trp-remove" title="Remove rule" @click="remove(index)">✕</button>
    </div>
  </div>
</template>

<style scoped>
.type-rules-panel { padding: 12px; border-left: 1px solid var(--hairline); background: var(--panel-surface); overflow-y: auto; min-width: 240px; }
.trp-header { font-size: 12px; font-weight: 600; color: var(--text-heading); margin-bottom: 8px; }
.trp-empty { font-size: 12px; color: var(--text-muted-dark); }
.trp-rule { display: flex; justify-content: space-between; align-items: center; gap: 8px; padding: 6px 8px; margin-bottom: 4px; background: var(--card-surface); border: 1px solid var(--hairline); border-radius: 6px; }
.trp-rule-text { font-size: 12px; color: var(--text-body); }
.trp-remove { background: none; border: none; color: var(--text-muted-dark); cursor: pointer; font-size: 12px; }
.trp-remove:hover { color: var(--accent-red); }
</style>
