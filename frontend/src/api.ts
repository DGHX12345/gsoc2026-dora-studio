import type { DiagnosticSeverity, NodeStatus, StudioEdge, StudioLog, StudioNode } from './data/mockStudio'

export type ApiSource = 'connected' | 'fallback'

export type ApiResult<T> = {
  data: T
  source: ApiSource
  error?: string
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
  project?: string
}

export type DataflowDefinitionNodeResponse = {
  id: string
  // M18 (Task 4.1): project-loaded dataflows may omit path; per-port URN
  // types are present when the source YAML declares input/output types.
  path?: string | null
  inputs: string[]
  outputs: string[]
  inputTypes?: Record<string, string>
  outputTypes?: Record<string, string>
}

export type DataflowDefinitionResponse = {
  id: string
  name: string
  relativePath: string
  source: string
  nodeCount: number
  edgeCount: number
  nodes: DataflowDefinitionNodeResponse[]
  project?: string
  typeRules?: TypeRule[]
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

export type RuntimeStatus = 'running' | 'stopped' | 'failed' | 'unavailable'

export type RuntimeStateResponse = {
  status: RuntimeStatus
  pid: number | null
  lastMessage: string
  dataflowId: string | null
  dataflowPath: string | null
}

const DEFAULT_API_BASE_URL = 'http://127.0.0.1:3001/api'
// Optional chaining keeps this module importable under tsx/node (no
// import.meta.env there) — tools import BACKEND_BASE_URL in tests.
const configuredApiBaseUrl = import.meta.env?.VITE_DORA_STUDIO_API_URL as string | undefined
const API_BASE_URL = normalizeApiBaseUrl(configuredApiBaseUrl || DEFAULT_API_BASE_URL)
export const BACKEND_BASE_URL = new URL(API_BASE_URL).origin

function normalizeApiBaseUrl(value: string) {
  return value.replace(/\/+$/, '')
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_BASE_URL}${path}`, init)

  if (!response.ok) {
    let detail = ''
    try {
      const body = await response.json()
      if (body && typeof body.error === 'string') detail = ` — ${body.error}`
    } catch { /* non-JSON error body */ }
    throw new Error(`API request failed: ${response.status}${detail}`)
  }

  return response.json() as Promise<T>
}

async function withFallback<T>(path: string, fallback: T): Promise<ApiResult<T>> {
  try {
    return {
      data: await fetchJson<T>(path),
      source: 'connected',
    }
  } catch (error) {
    return {
      data: fallback,
      source: 'fallback',
      error: error instanceof Error ? error.message : 'Backend API is unavailable.',
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

export function startRuntimeByPath(path: string) {
  return fetchJson<RuntimeStateResponse>('/runtime/start-path', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function stopRuntime() {
  return fetchJson<RuntimeStateResponse>('/runtime/stop', { method: 'POST' })
}

export function startDataflowRuntime(id: string) {
  return fetchJson<RuntimeStateResponse>(`/dataflows/${id}/start`, { method: 'POST' })
}

export function stopDataflowRuntime(id: string) {
  return fetchJson<RuntimeStateResponse>(`/dataflows/${id}/stop`, { method: 'POST' })
}

export function restartDataflowRuntime(id: string) {
  return fetchJson<RuntimeStateResponse>(`/dataflows/${id}/restart`, { method: 'POST' })
}

// --- Week 5: Coordinator, dviz, moveit ---

export type CoordinatorDataflowResponse = {
  id: string
  name: string
  status: string
  nodes: number
}

export type CoordinatorStatusResponse = {
  connected: boolean
  version: string
  runningDataflows: number
  activeNodes: number
  dataflows: CoordinatorDataflowResponse[]
}

export type DvizStatusResponse = {
  installed: boolean
  running: boolean
  binaryPath: string | null
  message: string
}

export type DvizTopicResponse = {
  name: string
  dataType: string
  source: string
  status: string
  messageRateHz: number
  lastSeen: string
  summary: string
}

export type DvizTopicsResponse = {
  source: string
  message: string
  topics: DvizTopicResponse[]
}

export type DvizDisplayResponse = {
  id: string
  name: string
  dataType: string
  enabled: boolean
  sourceTopic: string | null
  status: string
  summary: string
  color: string
}

export type DvizDisplaysResponse = {
  source: string
  message: string
  displays: DvizDisplayResponse[]
}

export type DvizSnapshotSummaryResponse = {
  topicCount: number
  readyTopicCount: number
  idleTopicCount: number
  displayCount: number
  enabledDisplayCount: number
}

export type DvizSnapshotResponse = {
  source: string
  message: string
  status: DvizStatusResponse
  summary: DvizSnapshotSummaryResponse
}

export type RobotModuleResponse = {
  id: string
  name: string
  kind: string
  role: string
  transport: string
  frame: string
  status: string
  summary: string
  required: boolean
  sourceTopics: string[]
  linkedDisplays: string[]
}

export type RobotWorkflowResponse = {
  id: string
  name: string
  status: string
  owner: string
  summary: string
}

export type RobotProfile = {
  id: string
  name: string
  family: string
  summary: string
  simulationOwner: string
  viewportRole: string
  modules: RobotModuleResponse[]
  workflows: RobotWorkflowResponse[]
  visualizationDisplays: string[]
  planningCapabilities: string[]
}

export type RobotProfileResponse = {
  source: string
  message: string
  profile: RobotProfile
}

export type MoveitStatusResponse = {
  installed: boolean
  running: boolean
  message: string
}

export type MoveitSnapshotFreshnessResponse = {
  status: string
  lastUpdated: string
  sourceLabel: string
}

export type MoveitJointStateResponse = {
  name: string
  value: number
  unit: string
  lowerLimit: number
  upperLimit: number
  status: string
  source: string
}

export type MoveitEndEffectorPoseResponse = {
  frame: string
  position: [number, number, number]
  quaternion: [number, number, number, number]
  source: string
}

export type MoveitSceneObjectResponse = {
  name: string
  shape: string
  dims: string
  dimensions: number[]
  frame: string
  status: string
}

export type MoveitPlanningSceneResponse = {
  status: string
  objectCount: number
  objects: MoveitSceneObjectResponse[]
}

export type MoveitTrajectorySummaryResponse = {
  status: string
  waypointCount: number
  durationSeconds: number
  message: string
}

export type MoveitVisualModelResponse = {
  modelId: string
  name: string
  format: string
  source: string
  jointOrder: string[]
}

export type MoveitSnapshotResponse = {
  source: string
  message: string
  robotProfileId: string
  robotConfigId: string
  simulationOwner: string
  viewportRole: string
  freshness: MoveitSnapshotFreshnessResponse
  joints: MoveitJointStateResponse[]
  endEffectorPose: MoveitEndEffectorPoseResponse
  scene: MoveitPlanningSceneResponse
  trajectory: MoveitTrajectorySummaryResponse
  visualModel: MoveitVisualModelResponse
}

export function getCoordinatorStatus(fallback: CoordinatorStatusResponse) {
  return withFallback('/coordinator/status', fallback)
}

export function getDvizStatus(fallback: DvizStatusResponse) {
  return withFallback('/dviz/status', fallback)
}

export function getDvizTopics(fallback: DvizTopicsResponse) {
  return withFallback('/dviz/topics', fallback)
}

export function getDvizDisplays(fallback: DvizDisplaysResponse) {
  return withFallback('/dviz/displays', fallback)
}

export function getDvizSnapshot(fallback: DvizSnapshotResponse) {
  return withFallback('/dviz/snapshot', fallback)
}

export function getRobotProfile(fallback: RobotProfileResponse) {
  return withFallback('/robot/profile', fallback)
}

export function getMoveitStatus(fallback: MoveitStatusResponse) {
  return withFallback('/moveit/status', fallback)
}

export function getMoveitSnapshot(fallback: MoveitSnapshotResponse) {
  return withFallback('/moveit/snapshot', fallback)
}

// --- daemon ---

export type DaemonStatusResponse = {
  running: boolean
  pid: number | null
}

export function getDaemonStatus(fallback: DaemonStatusResponse) {
  return withFallback('/daemon/status', fallback)
}

export function startDaemon() {
  return fetchJson<DaemonStatusResponse>('/daemon/start', { method: 'POST' })
}

export function stopDaemon() {
  return fetchJson<DaemonStatusResponse>('/daemon/stop', { method: 'POST' })
}

// --- dora version manager (M17) ---

export type DoraVersionItemResponse = {
  path: string
  version: string
  compatible: boolean
  active: boolean
}

export type DoraVersionsResponse = {
  active: string
  overriddenByEnv: boolean
  items: DoraVersionItemResponse[]
}

export function getDoraVersions(fallback: DoraVersionsResponse) {
  return withFallback('/dora/versions', fallback)
}

export function switchDoraVersion(path: string) {
  return fetchJson<{ ok: boolean }>('/dora/switch', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function addDoraCandidate(path: string) {
  return fetchJson<{ ok: boolean }>('/dora/candidates/add', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function deleteDoraCandidate(path: string) {
  return fetchJson<{ ok: boolean }>('/dora/candidates/delete', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

// --- session lifecycle (M16.5) ---

export type SessionStatusResponse = {
  status: string
  running: boolean
  coordinatorConnected: boolean
  coordinatorStatus: string
  pid: number | null
  version: string
  lifecycleSupported: boolean
  dataflowCount: number
  message: string
}

export function getSessionStatus(fallback: SessionStatusResponse) {
  return withFallback('/session/status', fallback)
}

export function startSession() {
  return fetchJson<SessionStatusResponse>('/session/start', { method: 'POST' })
}

export function stopSession() {
  return fetchJson<SessionStatusResponse>('/session/stop', { method: 'POST' })
}

// --- recording capture (M16.5 D4) ---

export type RecordingCaptureStatusResponse = {
  status: string
  outputPath: string | null
  dataflowPath: string | null
  startedAtMillis: number | null
  frameCount: number | null
  message: string
}

export type RecordingListEntryResponse = {
  name: string
  path: string
  sizeBytes: number
  createdAtMillis: number
  frameCount: number | null
}

export function startRecordingCapture(dataflowPath: string) {
  return fetchJson<RecordingCaptureStatusResponse>('/recording/capture', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ dataflowPath }),
  })
}

export function stopRecordingCapture() {
  return fetchJson<RecordingCaptureStatusResponse>('/recording/stop', { method: 'POST' })
}

export function getRecordingList(fallback: RecordingListEntryResponse[]) {
  return withFallback('/recording/list', fallback)
}

// --- dataflow builder (M01) ---

export type DataflowGraph = {
  nodes: Array<{
    id: string; operator_id: string; runtime: string;
    inputs: Record<string, { type?: string; description?: string }>;
    outputs: Record<string, { type?: string; description?: string }>;
    position?: { x: number; y: number };
  }>
  edges: Array<{
    id: string; source_node: string; source_port: string;
    target_node: string; target_port: string;
  }>
}

export type BuildResponse = { yaml: string; node_count: number; edge_count: number }
export type ValidateResponse = { valid: boolean; errors: string[] }
export type ParseResponse = { graph: DataflowGraph }

const JSON_HEADER = { 'Content-Type': 'application/json' }

export function buildDataflow(graph: DataflowGraph) {
  return fetchJson<BuildResponse>('/dataflow/build', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify(graph),
  })
}

export function validateDataflow(graph: DataflowGraph) {
  return fetchJson<ValidateResponse>('/dataflow/validate', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify(graph),
  })
}

export function parseDataflow(yaml: string) {
  return fetchJson<ParseResponse>('/dataflow/parse', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ yaml }),
  })
}

// --- schema registry (M02) ---

export type SchemaCheckRequest = {
  source_operator: string; source_port: string;
  sink_operator: string; sink_port: string;
}
export type SchemaCheckResponse = {
  compatible: boolean
  level: string
  detail: string
  // M18 (Task 4.1): URN-based /schema/check queries may add these fields.
  urn?: string
  rule?: TypeRule | null
  suggestion?: string
}
export type OperatorSchemas = { operator: string; inputs: Array<{ port_name: string; port_type: string; description?: string }>; outputs: Array<{ port_name: string; port_type: string; description?: string }> }

export function checkSchema(req: SchemaCheckRequest) {
  return fetchJson<SchemaCheckResponse>('/schema/check', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify(req) })
}

export function getOperatorSchema(name: string) {
  return fetchJson<OperatorSchemas>(`/schema/operator/${encodeURIComponent(name)}`)
}

// --- runtime node status (M03) ---

export type NodeRuntimeStatusResponse = {
  nodeId: string
  status: string
  uptimeSecs: number | null
  restartCount: number
  cpuUsage: number | null
  memoryMb: number | null
  pendingMessages: number | null
}

export function getRuntimeNodeStatuses(dataflowId: string, fallback: NodeRuntimeStatusResponse[]) {
  return withFallback(`/runtime/nodes/${encodeURIComponent(dataflowId)}`, fallback)
}

export type ReloadRequest = { nodeId: string; operatorId?: string }

export function reloadNode(dataflowId: string, req: ReloadRequest) {
  return fetchJson<{ ok: boolean; nodeId: string; message: string }>(
    `/runtime/nodes/${encodeURIComponent(dataflowId)}/reload`,
    { method: 'POST', headers: JSON_HEADER, body: JSON.stringify(req) }
  )
}

export function runDataflow(yaml: string, name?: string) {
  return fetchJson<RuntimeStateResponse>('/dataflow/run', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ yaml, name }),
  })
}

// --- recording API (M04/M05) ---

export type RecordingOpenedResponse = {
  id: string
  dataflowId: string
  version: number
  startNanos: number
  messageCount: number
  durationNanos: number
  streamCount: number
}

export type StreamInfoResponse = {
  nodeId: string
  outputId: string
  entryCount: number
  timeRange: [number, number]
}

export type SeekEntryResponse = {
  byteOffset: number
  timestampNanos: number
  nodeId: string
  outputId: string
  eventBytes?: number[]
}

export type RecordingEntriesResponse = {
  entries: SeekEntryResponse[]
  offset: number
  limit: number
  total: number
}

export function getRecordingEntriesWithData(
  id: string,
  params: { node?: string; output?: string; offset?: number; limit?: number } = {}
) {
  const qs = new URLSearchParams()
  if (params.node) qs.set('node', params.node)
  if (params.output) qs.set('output', params.output)
  if (params.offset !== undefined) qs.set('offset', String(params.offset))
  if (params.limit !== undefined) qs.set('limit', String(params.limit))
  qs.set('include_data', 'true')
  const q = qs.toString()
  return fetchJson<RecordingEntriesResponse>(`/recording/${encodeURIComponent(id)}/entries?${q}`)
}

export function openRecording(path: string) {
  return fetchJson<RecordingOpenedResponse>('/recording/open', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function getRecordingStreams(id: string) {
  return fetchJson<{ streams: StreamInfoResponse[] }>(`/recording/${encodeURIComponent(id)}/streams`)
}

export function seekRecording(id: string, timestamp: number) {
  return fetchJson<SeekEntryResponse>(`/recording/${encodeURIComponent(id)}/seek?timestamp=${timestamp}`)
}

export function getRecordingEntries(
  id: string,
  params: { node?: string; output?: string; offset?: number; limit?: number } = {}
) {
  const qs = new URLSearchParams()
  if (params.node) qs.set('node', params.node)
  if (params.output) qs.set('output', params.output)
  if (params.offset !== undefined) qs.set('offset', String(params.offset))
  if (params.limit !== undefined) qs.set('limit', String(params.limit))
  const q = qs.toString()
  return fetchJson<RecordingEntriesResponse>(`/recording/${encodeURIComponent(id)}/entries${q ? '?' + q : ''}`)
}

export function closeRecording(id: string) {
  return fetchJson<{ ok: boolean }>(`/recording/${encodeURIComponent(id)}/close`, { method: 'POST' })
}

// --- attribution (M09) ---

export type AttributionStepResponse =
  | { kind: 'sensorFrame'; topic: string; width: number; height: number; encoding: string }
  | { kind: 'prompt'; text: string; tokenCount: number }
  | { kind: 'llmResponse'; text: string; tokenCount: number; model: string; latencyMs: number }
  | { kind: 'parsedAction'; actionType: string; vector: number[]; confidence: number | null }
  | { kind: 'executionResult'; success: boolean; errorMessage: string | null }

export type AttributionChainResponse = {
  timestampNanos: number
  steps: AttributionStepResponse[]
}

export type AttributionChainSummaryResponse = {
  timestampNanos: number
  success: boolean | null
  stepCount: number
}

export type UnparseableStreamResponse = {
  nodeId: string
  outputId: string
  reason: string
}

export type AttributionSummaryResponse = {
  chains: AttributionChainSummaryResponse[]
  unparseableStreams: UnparseableStreamResponse[]
}

export function getAttributionSummary(recordingId: string) {
  return fetchJson<AttributionSummaryResponse>(
    `/recording/${encodeURIComponent(recordingId)}/attribution`,
  )
}

export function getAttributionChain(recordingId: string, timestampNanos: number) {
  return fetchJson<AttributionChainResponse>(
    `/recording/${encodeURIComponent(recordingId)}/attribution/chain?timestamp=${timestampNanos}`,
  )
}

// --- lerobot (M10) ---

export type LerobotStatusResponse = {
  pythonAvailable: boolean
  pyarrowAvailable: boolean
  message: string
}

export type LerobotEpisodeResponse = {
  index: number
  rows: number
  startNs: number
  endNs: number
}

export type LerobotDatasetResponse = {
  name: string
  layout: string
  columns: string[]
  episodes: LerobotEpisodeResponse[]
  tasks: Record<number, string>
  hasImageColumns: boolean
}

export type LerobotFrameResponse = {
  frameIndex: number
  timestampNs: number
  taskIndex: number | null
  action: number[]
  state: number[]
}

export type LerobotFramesResponse = { frames: LerobotFrameResponse[]; total: number }

export type LerobotProfileResponse = { name: string; robot: string }

export type LerobotAutodetectResponse = {
  columns: string[]
  suggestedProfile: string | null
  score: number | null
}

export type LerobotAttributionResponse = {
  chains: AttributionChainResponse[]
  summaries: { timestampNanos: number; success: boolean | null; stepCount: number }[]
  total: number
  profile: string
  tasks: Record<number, string>
  angleUnit: 'radians' | 'degrees'
}

export function getLerobotStatus() {
  return fetchJson<LerobotStatusResponse>('/lerobot/status')
}

export function scanLerobotDataset(path: string) {
  return fetchJson<LerobotDatasetResponse>('/lerobot/scan', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function getLerobotProfiles() {
  return fetchJson<{ profiles: LerobotProfileResponse[] }>('/lerobot/profiles')
}

export function autodetectLerobotProfile(path: string) {
  return fetchJson<LerobotAutodetectResponse>('/lerobot/autodetect', {
    method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }),
  })
}

export function getLerobotAttribution(
  path: string, episode: number, offset = 0, limit = 200, profile?: string,
) {
  return fetchJson<LerobotAttributionResponse>('/lerobot/attribution', {
    method: 'POST', headers: JSON_HEADER,
    body: JSON.stringify({ path, episode, offset, limit, ...(profile ? { profile } : {}) }),
  })
}

// --- metrics (M07) ---

export type NodeMetricSampleResponse = {
  timestampSecs: number
  cpuPercent: number
  memoryMb: number
  status: string
  restartCount: number
  pid: number | null
}

export type NodeMetricSummaryResponse = {
  nodeId: string
  dataflowName: string | null
  current: NodeMetricSampleResponse
  history: NodeMetricSampleResponse[]
}

export function getMetricsNodes(fallback: NodeMetricSummaryResponse[]) {
  return withFallback('/metrics/nodes', fallback)
}

export function getMetricsNodeHistory(nodeId: string, windowSecs?: number) {
  const qs = windowSecs !== undefined ? `?window=${windowSecs}` : ''
  return fetchJson<NodeMetricSampleResponse[]>(
    `/metrics/nodes/${encodeURIComponent(nodeId)}/history${qs}`
  )
}

// --- Monitoring control (M11.5) ---

export type MonitoringTargetStatus = {
  enabled: boolean
  sampleCount: number
  lastPollAt: number | null
}

export type MonitoringStatusResponse = {
  nodeMetrics: MonitoringTargetStatus
  otelSpans: MonitoringTargetStatus & {
    connected: boolean
    spanCount: number
    endpoint: string
  }
}

export function getMonitoringStatus() {
  return fetchJson<MonitoringStatusResponse>('/monitoring/status')
}

export function setMonitoringToggle(body: { nodeMetrics?: boolean; otelSpans?: boolean }) {
  return fetchJson<MonitoringStatusResponse>('/monitoring/toggle', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

// --- OTel spans (M08) ---

export type OtelSpanResponse = {
  spanId: string
  parentSpanId: string | null
  traceId: string
  nodeId: string
  operationName: string
  startMicros: number
  durationMicros: number
  attributes: Record<string, string>
}

export type SpanNodeResponse = {
  span: OtelSpanResponse
  children: SpanNodeResponse[]
}

export type OtelStatusResponse = {
  endpoint: string
  connected: boolean
  spanCount: number
  lastError: string | null
}

export function getOtelStatus(fallback: OtelStatusResponse) {
  return withFallback('/otel/status', fallback)
}

export function getOtelSpans(node?: string, limit = 200) {
  const qs = new URLSearchParams()
  if (node) qs.set('node', node)
  qs.set('limit', String(limit))
  return fetchJson<OtelSpanResponse[]>(`/otel/spans?${qs.toString()}`)
}

export function getOtelTrace(traceId: string) {
  return fetchJson<SpanNodeResponse[]>(`/otel/trace/${encodeURIComponent(traceId)}`)
}

// --- Live API (M15 B3) ---

export type LiveFrame = {
  node_id: string
  output_id: string
  timestamp: number
  payload: {
    values?: number[]
    json?: unknown
    bytes_base64?: string
    metadata?: Record<string, unknown>
  }
}

export type LiveRecentResponse = {
  frames: LiveFrame[]
}

export function getLiveRecent(sinceTs: number, limit = 500) {
  const qs = new URLSearchParams()
  qs.set('since_ts', String(sinceTs))
  qs.set('limit', String(limit))
  return fetchJson<LiveRecentResponse>(`/live/recent?${qs.toString()}`)
}

// --- Live command API (M15 B6) ---

export type LiveCommandRequest = {
  kind: string
  planner?: string
  target?: number[]
  action?: string
  object?: Record<string, unknown>
}

export function postLiveCommand(command: LiveCommandRequest) {
  return fetchJson<{ seq: number; kind: string }>('/live/command', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(command),
  })
}

// --- M18 Dataflow Explorer 2.0 ---
// Note: URNs can contain query params (e.g. `a/b?c`) — when calling the
// `/types/:urn` wildcard route, always encodeURIComponent(urn) in the URL.

export type TypeField = { name: string; fieldType: string }
export type TypeParam = { name: string; default?: string }
export type TypeCatalogEntry = {
  urn: string; name: string; category: string; arrow: string;
  description?: string; fields: TypeField[]; params: TypeParam[]
}

export type ProjectSummaryResponse = {
  name: string; path: string; builtin: boolean; dataflowCount: number
  dataflows: DataflowSummaryResponse[]
}

export type PalettePort = { name: string; urn?: string }
export type PaletteEntry = {
  id: string; operator: string; path?: string; runtime: string
  project: string; manual: boolean; inputs: PalettePort[]; outputs: PalettePort[]
}

export type ManualNodeSpec = {
  id: string; path: string; description?: string
  inputs: PalettePort[]; outputs: PalettePort[]
}

export type TypeRule = { from: string; to: string }

export type SchemaCheckUrnRequest = {
  source_urn?: string; sink_urn?: string; type_rules?: TypeRule[]
}
// URN-based /schema/check responses reuse SchemaCheckResponse, declared and
// extended in the schema registry (M02) section above.

export type SaveIssue = { nodeId?: string; portId?: string; message: string }
export type SaveResponse = { ok: boolean; path: string; warnings: SaveIssue[]; errors: SaveIssue[] }

export function getProjects() {
  return fetchJson<{ projects: ProjectSummaryResponse[] }>('/projects/list')
}
export function getPalette() {
  return fetchJson<{ entries: PaletteEntry[] }>('/palette')
}
export function addProjectDir(path: string) {
  return fetchJson<{ ok: boolean }>('/projects/add', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }) })
}
export function deleteProjectDir(path: string) {
  return fetchJson<{ ok: boolean }>('/projects/delete', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ path }) })
}
export function submitManualNode(node: ManualNodeSpec) {
  return fetchJson<{ ok: boolean }>('/projects/nodes', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify(node) })
}
export function getTypeCatalog() {
  return fetchJson<{ types: TypeCatalogEntry[] }>('/types/catalog')
}
export function checkSchemaUrn(req: SchemaCheckUrnRequest) {
  return fetchJson<SchemaCheckResponse>('/schema/check', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify(req) })
}
export function saveDataflow(id: string, graph: unknown) {
  return fetchJson<SaveResponse>(`/dataflows/${encodeURIComponent(id)}/save`, { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ graph }) })
}
export function saveDataflowAs(graph: unknown, targetPath: string) {
  return fetchJson<SaveResponse>('/dataflows/save-as', { method: 'POST', headers: JSON_HEADER, body: JSON.stringify({ graph, targetPath }) })
}
