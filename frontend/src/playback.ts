// Playback engine — state machine for .drec replay.
// Drives the timeline, triggers data fetches at current position.

export type PlaybackState = 'stopped' | 'playing' | 'paused' | 'seeking'
export type PlaybackSpeed = 0.5 | 1 | 2 | 5

export interface Bookmark {
  id: string
  timestampNanos: number
  label: string
}

export interface StreamEntry {
  byteOffset: number
  timestampNanos: number
  nodeId: string
  outputId: string
}

export class PlaybackEngine {
  private _state: PlaybackState = 'stopped'
  private _currentTime: number = 0
  private _speed: PlaybackSpeed = 1
  private _durationNanos: number = 0
  private _bookmarks: Bookmark[] = []
  private _rafId: number | null = null
  private _lastFrameTime: number = 0
  private _onTickListeners = new Set<(time: number) => void>()
  private _onStateChange: ((state: PlaybackState) => void) | null = null
  private _seekDebounceTimer: ReturnType<typeof setTimeout> | null = null

  get state() { return this._state }
  get currentTime() { return this._currentTime }
  get speed() { return this._speed }
  get durationNanos() { return this._durationNanos }
  get bookmarks() { return this._bookmarks }

  set duration(ns: number) { this._durationNanos = ns }

  onTick(cb: (time: number) => void) { this._onTickListeners.add(cb) }
  onStateChange(cb: (state: PlaybackState) => void) { this._onStateChange = cb }

  play(speed?: PlaybackSpeed) {
    if (speed) this._speed = speed
    if (this._currentTime >= this._durationNanos) this._currentTime = 0
    this._state = 'playing'
    this._lastFrameTime = performance.now()
    this._onStateChange?.('playing')
    this._tick()
  }

  pause() {
    this._state = 'paused'
    if (this._rafId !== null) { cancelAnimationFrame(this._rafId); this._rafId = null }
    this._onStateChange?.('paused')
  }

  seek(timestampNs: number, immediate = false) {
    this._currentTime = Math.max(0, Math.min(timestampNs, this._durationNanos))
    this._notifyTick(this._currentTime)
    if (!immediate) {
      // Debounce fetches during scrubbing — resolve after 80ms idle
      if (this._seekDebounceTimer) clearTimeout(this._seekDebounceTimer)
      this._seekDebounceTimer = setTimeout(() => {
        this._seekDebounceTimer = null
        this._notifyTick(this._currentTime)
      }, 80)
    }
  }

  stop() {
    if (this._rafId !== null) { cancelAnimationFrame(this._rafId); this._rafId = null }
    this._state = 'stopped'
    this._currentTime = 0
    this._notifyTick(0)
    this._onStateChange?.('stopped')
  }

  stepForward(frames = 1) {
    // Advance by average frame interval (estimated as duration / messageCount if available)
    const step = 33_333_333 * frames // ~30fps default step
    this.seek(this._currentTime + step, true)
  }

  stepBackward(frames = 1) {
    const step = 33_333_333 * frames
    this.seek(this._currentTime - step, true)
  }

  setSpeed(s: PlaybackSpeed) { this._speed = s }

  addBookmark(timestampNanos: number, label = '') {
    const id = crypto.randomUUID()
    this._bookmarks.push({ id, timestampNanos, label })
    this._bookmarks.sort((a, b) => a.timestampNanos - b.timestampNanos)
    return id
  }

  removeBookmark(id: string) {
    this._bookmarks = this._bookmarks.filter(b => b.id !== id)
  }

  jumpToBookmark(id: string) {
    const bm = this._bookmarks.find(b => b.id === id)
    if (bm) this.seek(bm.timestampNanos, true)
  }

  formatTime(ns: number): string {
    const totalMs = Math.floor(ns / 1_000_000)
    const ms = totalMs % 1000
    const totalS = Math.floor(totalMs / 1000)
    const s = totalS % 60
    const m = Math.floor(totalS / 60) % 60
    const h = Math.floor(totalS / 3600)
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}.${String(ms).padStart(3, '0')}`
    return `${m}:${String(s).padStart(2, '0')}.${String(ms).padStart(3, '0')}`
  }

  private _tick() {
    if (this._state !== 'playing') return
    const now = performance.now()
    const deltaMs = now - this._lastFrameTime
    this._lastFrameTime = now
    this._currentTime += deltaMs * 1_000_000 * this._speed
    if (this._currentTime >= this._durationNanos) {
      this._currentTime = this._durationNanos
      this.pause()
      return
    }
    this._notifyTick(this._currentTime)
    this._rafId = requestAnimationFrame(() => this._tick())
  }

  private _notifyTick(t: number) {
    for (const cb of this._onTickListeners) cb(t)
  }
}
