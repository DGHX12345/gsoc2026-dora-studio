<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { PlaybackEngine, type PlaybackSpeed, type StreamEntry } from '../playback'
import {
  openRecording,
  getRecordingEntries,
  getRecordingStreams,
  closeRecording,
  type StreamInfoResponse,
} from '../api'

// --- State ---
const engine = new PlaybackEngine()
const recordingPath = ref('')
const recordingId = ref('')
const streams = ref<StreamInfoResponse[]>([])
const currentEntries = ref<StreamEntry[]>([])
const openedInfo = ref<{ messageCount: number; durationNanos: number; streamCount: number } | null>(null)
const errorMsg = ref('')
const timelineEl = ref<HTMLDivElement>()
const scrubDragging = ref(false)

// --- Playback refs ---
const isPlaying = ref(false)
const currentTime = ref(0)
const speed = ref<PlaybackSpeed>(1)
const currentTimeFormatted = computed(() => engine.formatTime(currentTime.value))
const durationFormatted = computed(() => engine.formatTime(engine.durationNanos))

// --- Zoom state ---
const zoomLevel = ref(1) // 1 = fit all; higher = zoomed in
const zoomOptions = [0.5, 1, 2, 5, 10, 50]
const viewOffset = ref(0) // pan offset when zoomed, in nanos

// Visible time window based on zoom
const viewWindow = computed(() => {
  if (!engine.durationNanos || zoomLevel.value === 0) return engine.durationNanos
  return engine.durationNanos / zoomLevel.value
})

const viewStart = computed(() => {
  // Center on current time, clamped to valid range
  const center = currentTime.value
  const half = viewWindow.value / 2
  const raw = center - half + viewOffset.value
  return Math.max(0, Math.min(raw, engine.durationNanos - viewWindow.value))
})

const viewEnd = computed(() => viewStart.value + viewWindow.value)

// Convert a timestamp to a percentage within the visible window
function timeToPct(ns: number): number {
  if (!viewWindow.value) return 0
  return ((ns - viewStart.value) / viewWindow.value) * 100
}

// --- Bookmark state ---
const bookmarkInput = ref('')

// Wire engine callbacks
engine.onTick((t) => {
  currentTime.value = t
  if (recordingId.value) fetchEntriesAt(t).catch(() => {})
})
engine.onStateChange((s) => {
  isPlaying.value = s === 'playing'
})

// Pan the zoomed timeline with scroll wheel
function onTimelineWheel(e: WheelEvent) {
  e.preventDefault()
  viewOffset.value += e.deltaX * 1_000_000 + e.deltaY * 5_000_000
  // Clamp
  const maxOffset = viewWindow.value / 2
  viewOffset.value = Math.max(-maxOffset, Math.min(maxOffset, viewOffset.value))
}

// --- Recording management ---
async function doOpen() {
  errorMsg.value = ''
  if (!recordingPath.value) { errorMsg.value = 'Enter a file path.'; return }
  try {
    const result = await openRecording(recordingPath.value)
    recordingId.value = result.id
    engine.duration = result.durationNanos
    openedInfo.value = { messageCount: result.messageCount, durationNanos: result.durationNanos, streamCount: result.streamCount }

    // Load streams
    const s = await getRecordingStreams(result.id)
    streams.value = s.streams
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : 'Failed to open'
    recordingId.value = ''
  }
}

async function doClose() {
  if (!recordingId.value) return
  try { await closeRecording(recordingId.value) } catch { /* ignore */ }
  recordingId.value = ''
  openedInfo.value = null
  streams.value = []
  currentEntries.value = []
  engine.stop()
}

async function fetchEntriesAt(timestamp: number) {
  if (!recordingId.value) return
  const result = await getRecordingEntries(recordingId.value, { offset: 0, limit: 200 })
  // Filter entries near the current timestamp (within ~1 frame at 30fps)
  const frameWindow = 33_333_333
  const nearby = result.entries.filter(e =>
    Math.abs(e.timestampNanos - timestamp) < frameWindow
  )
  if (nearby.length > 0) currentEntries.value = nearby
}

// --- Timeline click / scrub ---
function onTimelineClick(e: MouseEvent) {
  if (!timelineEl.value || !engine.durationNanos) return
  const rect = timelineEl.value.getBoundingClientRect()
  const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width))
  // pct maps to position within the visible window
  const ts = viewStart.value + pct * viewWindow.value
  engine.seek(ts, true)
  scrubDragging.value = false
}

function onTimelineMouseDown(e: MouseEvent) {
  scrubDragging.value = true
  onTimelineClick(e)
}

function onTimelineMouseMove(e: MouseEvent) {
  if (!scrubDragging.value) return
  onTimelineClick(e)
}

function onTimelineMouseUp() {
  scrubDragging.value = false
}

// --- Progress position (within zoomed view) ---
const progressPct = computed(() => timeToPct(currentTime.value))

// --- Activity markers (show all entries as dots in zoomed view) ---
const activityMarkers = computed(() => {
  if (!openedInfo.value || !openedInfo.value.messageCount) return []
  const markers: { pct: number; count: number }[] = []
  // Sample evenly across the visible window
  const buckets = 80
  const step = viewWindow.value / buckets
  for (let i = 0; i < buckets; i++) {
    const ts = viewStart.value + step * i
    const pct = timeToPct(ts)
    if (pct >= 0 && pct <= 100) {
      markers.push({ pct, count: 1 })
    }
  }
  return markers
})

// --- Keyboard ---
function onKeyDown(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
  switch (e.key) {
    case ' ':
      e.preventDefault()
      if (isPlaying.value) engine.pause(); else engine.play()
      break
    case 'ArrowRight':
      e.preventDefault()
      engine.stepForward(e.shiftKey ? 10 : e.ctrlKey ? 100 : 1)
      break
    case 'ArrowLeft':
      e.preventDefault()
      engine.stepBackward(e.shiftKey ? 10 : e.ctrlKey ? 100 : 1)
      break
    case '1': engine.setSpeed(0.5); speed.value = 0.5; break
    case '2': engine.setSpeed(1); speed.value = 1; break
    case '3': engine.setSpeed(2); speed.value = 2; break
    case '4': engine.setSpeed(5); speed.value = 5; break
  }
}

onMounted(() => window.addEventListener('keydown', onKeyDown))
onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown)
  doClose()
})
</script>

<template>
  <section class="replay-layout">
    <!-- Open panel -->
    <div class="replay-topbar">
      <input
        v-model="recordingPath"
        class="replay-path-input"
        placeholder="Path to .drec file (e.g. /tmp/recording.drec)"
        :disabled="!!recordingId"
        @keyup.enter="doOpen"
      />
      <button v-if="!recordingId" class="rp-btn primary" @click="doOpen">Open</button>
      <button v-else class="rp-btn danger" @click="doClose">Close</button>
      <span v-if="errorMsg" class="rp-error">{{ errorMsg }}</span>
    </div>

    <!-- Recording info -->
    <div v-if="openedInfo" class="rp-info-bar">
      <span>{{ openedInfo.messageCount }} messages</span>
      <span>{{ openedInfo.streamCount }} streams</span>
      <span>{{ durationFormatted }}</span>
    </div>

    <!-- Timeline -->
    <div v-if="openedInfo" class="timeline-section">
      <!-- Controls -->
      <div class="tl-controls">
        <button class="tl-ctrl-btn" @click="engine.stepBackward(1)" title="Step back (←)">⏮</button>
        <button class="tl-ctrl-btn play" @click="isPlaying ? engine.pause() : engine.play()" :title="isPlaying ? 'Pause (Space)' : 'Play (Space)'">
          {{ isPlaying ? '⏸' : '▶' }}
        </button>
        <button class="tl-ctrl-btn" @click="engine.stepForward(1)" title="Step forward (→)">⏭</button>
        <button class="tl-ctrl-btn" @click="engine.stop()" title="Stop">⏹</button>

        <span class="tl-sep"></span>

        <!-- Speed -->
        <span class="tl-speed-label">Speed</span>
        <template v-for="s in ([0.5, 1, 2, 5] as PlaybackSpeed[])" :key="s">
          <button
            :class="['tl-ctrl-btn sm', { active: speed === s }]"
            @click="engine.setSpeed(s); speed = s"
          >{{ s }}x</button>
        </template>

        <span class="tl-spacer"></span>

        <!-- Time -->
        <span class="tl-time">{{ currentTimeFormatted }} / {{ durationFormatted }}</span>

        <span class="tl-sep"></span>

        <!-- Zoom -->
        <span class="tl-speed-label">Zoom</span>
        <template v-for="z in zoomOptions" :key="z">
          <button
            :class="['tl-ctrl-btn sm', { active: zoomLevel === z }]"
            @click="zoomLevel = z; viewOffset = 0"
          >{{ z }}x</button>
        </template>
        <span v-if="zoomLevel !== 1" class="tl-speed-label" style="color:var(--accent-cyan)">
          {{ zoomLevel }}x — scroll to pan
        </span>
      </div>

      <!-- Track -->
      <div
        ref="timelineEl"
        class="tl-track"
        @mousedown="onTimelineMouseDown"
        @mousemove="onTimelineMouseMove"
        @mouseup="onTimelineMouseUp"
        @mouseleave="onTimelineMouseUp"
        @wheel.prevent="onTimelineWheel"
      >
        <!-- Mini-map (activity markers) -->
        <div class="tl-minimap">
          <div
            v-for="m in activityMarkers"
            :key="m.pct"
            class="tl-marker"
            :style="{ left: m.pct + '%', opacity: m.count > 1 ? 0.3 : 0.12 }"
          />
        </div>

        <!-- Progress bar -->
        <div class="tl-progress" :style="{ width: progressPct + '%' }" />

        <!-- Scrub handle -->
        <div class="tl-scrub" :style="{ left: progressPct + '%' }" />

        <!-- Bookmarks as colored pins -->
        <div
          v-for="bm in engine.bookmarks"
          :key="bm.id"
          class="tl-bookmark"
          :style="{ left: timeToPct(bm.timestampNanos) + '%' }"
          @click.stop="engine.jumpToBookmark(bm.id)"
        >
          <span class="bm-pin">📍</span>
          <span class="bm-label">{{ bm.label || engine.formatTime(bm.timestampNanos) }}</span>
        </div>
      </div>

      <!-- Bookmark list -->
      <div v-if="engine.bookmarks.length" class="tl-bookmark-list">
        <span class="tl-speed-label">Bookmarks:</span>
        <button
          v-for="bm in engine.bookmarks"
          :key="bm.id"
          class="bm-chip"
          @click="engine.jumpToBookmark(bm.id)"
        >
          {{ bm.label || engine.formatTime(bm.timestampNanos) }}
          <span class="bm-chip-del" @click.stop="engine.removeBookmark(bm.id)">×</span>
        </button>
      </div>

      <!-- Bookmark bar -->
      <div class="tl-bookmark-bar">
        <input
          v-model="bookmarkInput"
          class="bm-input"
          placeholder="Bookmark label..."
          @keyup.enter="(e) => { engine.addBookmark(currentTime, bookmarkInput); bookmarkInput = ''; (e.target as HTMLInputElement).blur() }"
        />
        <button class="tl-ctrl-btn sm" @click="engine.addBookmark(currentTime, bookmarkInput); bookmarkInput = ''">+ Bookmark</button>
        <span v-if="engine.bookmarks.length" class="tl-speed-label">
          {{ engine.bookmarks.length }} bookmark(s)
        </span>
      </div>
    </div>

    <!-- Stream grid (D4) -->
    <div v-if="streams.length" class="stream-grid">
      <h3 class="sg-title">Streams ({{ currentEntries.length }} active)</h3>
      <div class="sg-grid">
        <div
          v-for="s in streams"
          :key="s.nodeId + '/' + s.outputId"
          :class="['sg-card', { active: currentEntries.some(e => e.nodeId === s.nodeId && e.outputId === s.outputId) }]"
        >
          <div class="sg-card-header">
            <span class="sg-node">{{ s.nodeId }}</span>
            <span class="sg-port">/ {{ s.outputId }}</span>
            <span class="sg-count">{{ s.entryCount }}</span>
          </div>
          <!-- Mini sparkline area -->
          <div class="sg-sparkline">
            <div class="sg-sparkline-bar" :style="{ width: (s.entryCount / (openedInfo?.messageCount ?? 1)) * 100 + '%' }" />
          </div>
          <div class="sg-range">
            {{ engine.formatTime(s.timeRange[0]) }} → {{ engine.formatTime(s.timeRange[1]) }}
          </div>
        </div>
      </div>
    </div>

    <!-- Empty state -->
    <div v-if="!openedInfo" class="rp-empty">
      <p>Open a .drec recording to start replay.</p>
      <p class="rp-hint">Use <code>cargo test -- drec</code> to generate test recordings under <code>/tmp/dora-studio-tests/</code></p>
    </div>
  </section>
</template>

<style scoped>
.replay-layout {
  display: flex; flex-direction: column; gap: 16px; padding: 20px; height: 100%;
  overflow-y: auto;
}

/* Top bar */
.replay-topbar {
  display: flex; gap: 8px; align-items: center;
}
.replay-path-input {
  flex: 1; max-width: 500px;
  padding: 8px 12px;
  background: var(--card-surface); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 6px;
  font-size: 13px; font-family: monospace;
}
.replay-path-input:disabled { opacity: 0.5; }
.rp-btn {
  padding: 8px 16px; border: none; border-radius: 6px;
  font-size: 13px; font-weight: 600; cursor: pointer;
}
.rp-btn.primary { background: var(--accent-cyan); color: #000; }
.rp-btn.danger { background: var(--accent-red); color: #fff; }
.rp-error { color: var(--accent-red); font-size: 13px; }

/* Info bar */
.rp-info-bar {
  display: flex; gap: 24px; font-size: 13px; color: var(--text-muted-dark);
}

/* Timeline section */
.timeline-section {
  display: flex; flex-direction: column; gap: 8px;
  background: var(--card-surface); border: 1px solid var(--hairline);
  border-radius: 8px; padding: 12px 16px;
}

/* Controls row */
.tl-controls {
  display: flex; gap: 4px; align-items: center;
}
.tl-ctrl-btn {
  width: 32px; height: 28px; border: 1px solid var(--hairline);
  border-radius: 4px; background: var(--canvas-base); color: var(--text-body);
  font-size: 13px; cursor: pointer; display: flex; align-items: center; justify-content: center;
}
.tl-ctrl-btn:hover { background: var(--card-hover); color: var(--text-heading); }
.tl-ctrl-btn.play { width: 40px; font-size: 16px; }
.tl-ctrl-btn.sm { width: 28px; font-size: 11px; }
.tl-ctrl-btn.active { background: var(--accent-cyan); color: #000; border-color: var(--accent-cyan); }
.tl-sep { width: 1px; height: 20px; background: var(--hairline); margin: 0 8px; }
.tl-spacer { flex: 1; }
.tl-speed-label { font-size: 11px; color: var(--text-muted-dark); margin: 0 4px; }
.tl-time { font-size: 13px; font-family: monospace; color: var(--text-body); font-weight: 600; }

/* Track */
.tl-track {
  position: relative; height: 32px;
  background: var(--canvas-base); border: 1px solid var(--hairline);
  border-radius: 4px; cursor: pointer; user-select: none;
}
.tl-minimap { position: absolute; inset: 0; opacity: 0.15; }
.tl-marker {
  position: absolute; top: 0; width: 2px; height: 100%;
  background: var(--accent-yellow);
}
.tl-progress {
  position: absolute; top: 0; left: 0; height: 100%;
  background: var(--accent-cyan); opacity: 0.2; border-radius: 4px 0 0 4px;
  pointer-events: none;
}
.tl-scrub {
  position: absolute; top: -2px; width: 4px; height: 36px;
  background: var(--accent-red); border-radius: 2px;
  transform: translateX(-50%); pointer-events: none;
}
.tl-bookmark {
  position: absolute; top: -18px; cursor: pointer; pointer-events: all;
  display: flex; flex-direction: column; align-items: center;
  transform: translateX(-50%);
}
.bm-pin { font-size: 14px; }
.bm-label {
  font-size: 9px; color: var(--accent-green); white-space: nowrap;
  background: var(--card-surface); padding: 1px 4px; border-radius: 3px;
  margin-top: -2px;
}

/* Bookmark list */
.tl-bookmark-list {
  display: flex; gap: 6px; align-items: center; flex-wrap: wrap;
}
.bm-chip {
  background: var(--canvas-base); border: 1px solid var(--hairline);
  border-radius: 12px; padding: 2px 8px; font-size: 11px;
  color: var(--accent-green); cursor: pointer; display: flex; align-items: center; gap: 4px;
}
.bm-chip:hover { background: var(--card-hover); }
.bm-chip-del { color: var(--text-muted-dark); font-size: 13px; }
.bm-chip-del:hover { color: var(--accent-red); }

/* Bookmark bar */
.tl-bookmark-bar {
  display: flex; gap: 6px; align-items: center;
}
.bm-input {
  width: 140px; padding: 4px 8px; font-size: 12px;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 4px;
}

/* Stream grid */
.stream-grid {
  display: flex; flex-direction: column; gap: 8px;
}
.sg-title {
  font-size: 14px; font-weight: 600; color: var(--text-heading); margin: 0;
}
.sg-grid {
  display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px;
}
.sg-card {
  background: var(--card-surface); border: 1px solid var(--hairline);
  border-radius: 6px; padding: 10px 12px;
  transition: border-color 150ms ease;
}
.sg-card.active { border-color: var(--accent-cyan); }
.sg-card-header { display: flex; align-items: baseline; gap: 4px; font-size: 13px; }
.sg-node { font-weight: 600; color: var(--text-heading); }
.sg-port { color: var(--text-muted-dark); font-size: 12px; }
.sg-count { margin-left: auto; font-size: 11px; color: var(--text-muted-dark); }
.sg-sparkline {
  height: 4px; background: var(--canvas-base); border-radius: 2px;
  margin: 6px 0;
}
.sg-sparkline-bar {
  height: 100%; background: var(--accent-cyan); opacity: 0.5; border-radius: 2px;
}
.sg-range {
  font-size: 10px; color: var(--text-muted-dark); font-family: monospace;
}

/* Empty state */
.rp-empty {
  text-align: center; color: var(--text-muted-dark); padding: 48px 0;
}
.rp-hint { font-size: 12px; }
.rp-hint code { background: var(--card-surface); padding: 2px 6px; border-radius: 3px; }
</style>
