import type { SchemaCheckResponse, TypeRule } from './api'

export type EdgeLevel = 'green' | 'yellow' | 'red' | 'gray'

/** Map a schema check response to the four-color edge semantics. */
export function edgeLevel(response: SchemaCheckResponse): EdgeLevel {
  switch (response.level) {
    case 'compatible': return 'green'
    case 'rule':
    case 'warning': return 'yellow'
    case 'incompatible': return 'red'
    default: return 'gray'
  }
}

export function edgeColor(level: EdgeLevel): string {
  return {
    green: 'var(--accent-green)',
    yellow: 'var(--accent-yellow)',
    red: 'var(--accent-red)',
    gray: 'var(--text-muted-dark)',
  }[level]
}

/** Add a from→to rule to the dataflow rules, deduplicating. */
export function buildRulePatch(existing: TypeRule[], from: string, to: string): TypeRule[] {
  if (existing.some(rule => rule.from === from && rule.to === to)) return existing
  return [...existing, { from, to }]
}
