import type { DiagnosticSeverity, NodeStatus, StudioEdge, StudioLog, StudioNode } from './data/mockStudio'

export type ApiSource = 'connected' | 'fallback'

export type ApiResult<T> = {
  data: T
  source: ApiSource
}

export type SystemStatusResponse = {
  coordinator: string
  daemon: string
  version: string
  runningDataflows: number
  activeNodes: number
  errorCount: number
}

export type DataflowSummaryResponse = {
  id: string
  name: string
  status: string
  nodeCount: number
  edgeCount: number
}

export type DataflowDefinitionNodeResponse = {
  id: string
  path: string | null
  inputs: string[]
  outputs: string[]
}

export type DataflowDefinitionResponse = {
  id: string
  name: string
  relativePath: string
  source: string
  nodeCount: number
  edgeCount: number
  nodes: DataflowDefinitionNodeResponse[]
}

export type NodeMetricsResponse = {
  id: string
  label: string
  kind: string
  status: NodeStatus
  cpu: number
  memory: number
  restarts: number
  pending: number
}

export type DiagnosticResponse = {
  severity: DiagnosticSeverity
  message: string
}

export type DataflowGraphResponse = {
  nodes: StudioNode[]
  edges: StudioEdge[]
  diagnostics: DiagnosticResponse[]
}

export type RuntimeStatus = 'running' | 'stopped' | 'failed'

export type RuntimeStateResponse = {
  status: RuntimeStatus
  pid: number | null
  lastMessage: string
}

const API_BASE_URL = 'http://127.0.0.1:3001/api'

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_BASE_URL}${path}`, init)

  if (!response.ok) {
    throw new Error(`API request failed: ${response.status}`)
  }

  return response.json() as Promise<T>
}

async function withFallback<T>(path: string, fallback: T): Promise<ApiResult<T>> {
  try {
    return {
      data: await fetchJson<T>(path),
      source: 'connected',
    }
  } catch {
    return {
      data: fallback,
      source: 'fallback',
    }
  }
}

export function getSystemStatus(fallback: SystemStatusResponse) {
  return withFallback('/system/status', fallback)
}

export function getDataflows(fallback: DataflowSummaryResponse[]) {
  return withFallback('/dataflows', fallback)
}

export function getDataflowDefinition(id: string, fallback: DataflowDefinitionResponse) {
  return withFallback(`/dataflows/${id}/definition`, fallback)
}

export function getNodes(id: string, fallback: NodeMetricsResponse[]) {
  return withFallback(`/dataflows/${id}/nodes`, fallback)
}

export function getLogs(id: string, fallback: StudioLog[]) {
  return withFallback(`/dataflows/${id}/logs`, fallback)
}

export function getDataflowGraph(id: string, fallback: DataflowGraphResponse) {
  return withFallback(`/dataflows/${id}/graph`, fallback)
}

export function getRuntimeStatus(fallback: RuntimeStateResponse) {
  return withFallback('/runtime/status', fallback)
}

export function getRuntimeLogs(fallback: StudioLog[]) {
  return withFallback('/runtime/logs', fallback)
}

export function startRuntime() {
  return fetchJson<RuntimeStateResponse>('/runtime/start', { method: 'POST' })
}

export function stopRuntime() {
  return fetchJson<RuntimeStateResponse>('/runtime/stop', { method: 'POST' })
}
