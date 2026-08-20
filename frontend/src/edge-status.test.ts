import { edgeLevel, edgeColor, buildRulePatch } from './edge-status'
import type { SchemaCheckResponse, TypeRule } from './api'

function resp(level: string, rule: TypeRule | null = null): SchemaCheckResponse {
  return { compatible: level !== 'incompatible' && level !== 'unknown', level, detail: '', rule }
}

if (edgeLevel(resp('compatible')) !== 'green') throw new Error('compatible should be green')
if (edgeLevel(resp('rule')) !== 'yellow') throw new Error('rule should be yellow')
if (edgeLevel(resp('warning')) !== 'yellow') throw new Error('legacy warning should be yellow')
if (edgeLevel(resp('incompatible')) !== 'red') throw new Error('incompatible should be red')
if (edgeLevel(resp('unknown')) !== 'gray') throw new Error('unknown should be gray')

if (edgeColor('green') !== 'var(--accent-green)') throw new Error('green token wrong')
if (edgeColor('yellow') !== 'var(--accent-yellow)') throw new Error('yellow token wrong')
if (edgeColor('red') !== 'var(--accent-red)') throw new Error('red token wrong')
if (edgeColor('gray') !== 'var(--text-muted-dark)') throw new Error('gray token wrong')

const existing: TypeRule[] = [{ from: 'a/v1/T', to: 'b/v1/U' }]
const patched = buildRulePatch(existing, 'a/v1/T', 'b/v1/U')
if (patched.length !== 1) throw new Error('duplicate rule not deduped')
const added = buildRulePatch(existing, 'c/v1/X', 'd/v1/Y')
if (added.length !== 2) throw new Error('new rule not appended')
if (added[1].from !== 'c/v1/X') throw new Error('appended rule wrong')

console.log('edge-status tests passed')
