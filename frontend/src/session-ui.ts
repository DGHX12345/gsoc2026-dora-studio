// Pure button state machine for the session / dataflow / recording
// lifecycle controls (M16.5 D5). Kept free of Vue imports so the
// tools test runner (tsx/node) can exercise it.

export type SessionStatus = {
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

export type SessionBusy = 'idle' | 'starting' | 'stopping'

export type SessionUiState =
  | 'stopped'
  | 'starting'
  | 'stopping'
  | 'running'
  | 'error'
  | 'unavailable'

export function sessionUiState(session: SessionStatus, busy: SessionBusy): SessionUiState {
  if (busy === 'starting') return 'starting'
  if (busy === 'stopping') return 'stopping'
  if (!session.lifecycleSupported) return 'unavailable'
  if (session.running) return 'running'
  if (session.status === 'failed') return 'error'
  return 'stopped'
}

export function canStartSession(session: SessionStatus, busy: SessionBusy): boolean {
  return busy === 'idle' && session.lifecycleSupported && !session.running
}

export function canStopSession(session: SessionStatus, busy: SessionBusy): boolean {
  return busy === 'idle' && session.lifecycleSupported && session.running
}

export function canStartDataflow(runtimeStatus: string, session: SessionStatus): boolean {
  return session.lifecycleSupported && runtimeStatus !== 'running'
}

export function canStopDataflow(runtimeStatus: string, session: SessionStatus): boolean {
  // A failed dataflow can still be stopped: dora stop cleans up any
  // leftover coordinator state and lets the user recover.
  return session.lifecycleSupported && (runtimeStatus === 'running' || runtimeStatus === 'failed')
}

export type RecordingAction = 'record' | 'recording' | 'disabled'

export function recordingAction(recordingStatus: string, canRecord: boolean): RecordingAction {
  if (!canRecord) return 'disabled'
  if (recordingStatus === 'recording') return 'recording'
  return 'record'
}

// --- dora version manager (M17) ---

export type DoraVersionItem = {
  path: string
  version: string
  compatible: boolean
  active: boolean
}

export type VersionBadge = 'compatible' | 'degraded' | 'overridden'

export function versionBadge(items: DoraVersionItem[], overriddenByEnv: boolean): VersionBadge {
  if (overriddenByEnv) return 'overridden'
  const active = items.find((item) => item.active)
  if (!active) return 'degraded'
  return active.compatible ? 'compatible' : 'degraded'
}

export function canSwitchItem(item: DoraVersionItem, overriddenByEnv: boolean): boolean {
  return !overriddenByEnv && !item.active
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unitIndex = -1
  do {
    value /= 1024
    unitIndex += 1
  } while (value >= 1024 && unitIndex < units.length - 1)
  return `${value.toFixed(1)} ${units[unitIndex]}`
}

function pad2(value: number): string {
  return String(value).padStart(2, '0')
}

export function formatRecordingTime(millis: number): string {
  const date = new Date(millis)
  return (
    `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())} ` +
    `${pad2(date.getHours())}:${pad2(date.getMinutes())}:${pad2(date.getSeconds())}`
  )
}
