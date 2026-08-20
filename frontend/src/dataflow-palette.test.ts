import { aggregatePalette } from './dataflow-palette'
import type { PaletteEntry } from './api'

const entries: PaletteEntry[] = [
  { id: 'cam', operator: 'cam.py', path: 'cam.py', runtime: 'python', project: 'a', manual: false, inputs: [], outputs: [{ name: 'image', urn: 'std/media/v1/Image' }] },
  { id: 'cam2', operator: 'cam.py', path: 'cam.py', runtime: 'python', project: 'b', manual: false, inputs: [], outputs: [{ name: 'image' }] },
  { id: 'conv', operator: 'conv', path: '/tmp/conv.py', runtime: 'python', project: 'manual', manual: true, inputs: [{ name: 'in', urn: 'std/media/v1/Image' }], outputs: [] },
]

const grouped = aggregatePalette(entries)
if (grouped.length !== 2) throw new Error('expected 2 deduped entries, got ' + grouped.length)
const cam = grouped.find(g => g.operator === 'cam.py')
if (!cam || cam.outputs[0].urn !== 'std/media/v1/Image') throw new Error('richest entry must win')
const manualGroup = grouped.find(g => g.operator === 'conv')
if (!manualGroup?.manual) throw new Error('manual flag lost')

const byProject = aggregatePalette(entries, { groupBy: 'project' })
if (!byProject.find(g => g.operator === 'cam.py' && g.project === 'a')) throw new Error('project grouping failed')

console.log('dataflow-palette tests passed')
