import type { PaletteEntry } from './api'

/** Dedupe palette entries by path (rich entry wins) and optionally split
 *  per project. Manual nodes are never deduped against scanned ones. */
export function aggregatePalette(
  entries: PaletteEntry[],
  options: { groupBy?: 'project' | 'none' } = {}
): PaletteEntry[] {
  const groupBy = options.groupBy ?? 'none'
  const buckets = new Map<string, PaletteEntry>()
  const order: string[] = []
  for (const entry of entries) {
    const key = entry.manual
      ? `manual:${entry.id}`
      : groupBy === 'project'
        ? `${entry.project}:${entry.path ?? entry.operator}`
        : `scan:${entry.path ?? entry.operator}`
    const existing = buckets.get(key)
    if (!existing) {
      buckets.set(key, entry)
      order.push(key)
    } else {
      buckets.set(key, richer(existing, entry))
    }
  }
  return order.map(key => buckets.get(key) as PaletteEntry)
}

function richer(a: PaletteEntry, b: PaletteEntry): PaletteEntry {
  const aTyped = [...a.inputs, ...a.outputs].filter(p => p.urn).length
  const bTyped = [...b.inputs, ...b.outputs].filter(p => p.urn).length
  if (bTyped > aTyped) return b
  if (bTyped < aTyped) return a
  return a.inputs.length + a.outputs.length >= b.inputs.length + b.outputs.length ? a : b
}
