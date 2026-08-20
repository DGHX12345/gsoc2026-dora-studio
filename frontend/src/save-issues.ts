import type { SaveIssue } from './api'
import type { DataflowGraph } from './components/DataflowCanvas.vue'

export type EdgeStyle = { color: string; tooltip: string }

/** Map backend save issues onto canvas edges. Node-addressed issues
 *  (the common backend shape) match the target node + port exactly;
 *  node-less issues fall back to port-name matching on either side. */
export function issuesToEdgeStyles(
  issues: SaveIssue[],
  graph: DataflowGraph,
  blocking: boolean
): Record<string, EdgeStyle> {
  const styles: Record<string, EdgeStyle> = {}
  for (const issue of issues) {
    for (const edge of graph.edges) {
      const matches = issue.nodeId
        ? edge.targetNode === issue.nodeId && (issue.portId ? edge.targetPort === issue.portId : true)
        : issue.portId
          ? edge.sourcePort === issue.portId || edge.targetPort === issue.portId
          : true
      if (matches) {
        styles[edge.id] = {
          color: blocking ? 'var(--accent-red)' : 'var(--accent-yellow)',
          tooltip: issue.message,
        }
      }
    }
  }
  return styles
}

/** Parse the SaveResponse JSON that the backend embeds as a string in the
 *  422 ApiError message ("API request failed: 422 — {json}"). Returns
 *  null when the message is not the save envelope. */
export function parseSaveError(message: string): { ok: false; path: string; warnings: SaveIssue[]; errors: SaveIssue[] } | null {
  const start = message.indexOf('{')
  if (start < 0) return null
  try {
    const parsed = JSON.parse(message.slice(start))
    if (parsed && typeof parsed === 'object' && Array.isArray(parsed.errors)) {
      return parsed
    }
  } catch {
    return null
  }
  return null
}
