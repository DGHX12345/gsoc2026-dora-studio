<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  autodetectLerobotProfile,
  getAttributionChain,
  getAttributionSummary,
  getLerobotAttribution,
  getLerobotProfiles,
  scanLerobotDataset,
  type AttributionChainResponse,
  type AttributionStepResponse,
  type AttributionSummaryResponse,
  type LerobotDatasetResponse,
  type LerobotProfileResponse,
} from '../api'
import { useI18n } from '../i18n'

const props = defineProps<{
  recordingId: string
  currentTimestamp: number
  /** M13 D4.1: true while the parent drives the profile's own robot model
   * (B601) for the "Show in 3D" preview — the Nano-fallback note hides. */
  previewOnProfileModel?: boolean
}>()

const emit = defineEmits<{
  'seek-timestamp': [timestampNs: number]
  'apply-action': [vector: number[], profileRobot: string | null]
}>()

const { t } = useI18n()

type AttributionSource = 'drec' | 'lerobot' | 'live'

const collapsed = ref(false)
const summary = ref<AttributionSummaryResponse | null>(null)
const summaryLoading = ref(false)
const summaryError = ref<string | null>(null)
const source = ref<AttributionSource>('drec')

const selectedTs = ref<number | null>(null)
const detail = ref<AttributionChainResponse | null>(null)
const detailLoading = ref(false)
const detailError = ref<string | null>(null)

// --- LeRobot source state (M10) ---
const lerobotPath = ref('')
const lerobotScanning = ref(false)
const lerobotError = ref<string | null>(null)
const dataset = ref<LerobotDatasetResponse | null>(null)
const profiles = ref<LerobotProfileResponse[]>([])
const selectedProfile = ref('')
const selectedEpisode = ref<number | null>(null)
const pageOffset = ref(0)
const pageSize = 200
const lerobotTotal = ref(0)
const lerobotChains = ref<AttributionChainResponse[]>([])
const lerobotSummaries = ref<{ timestampNanos: number; success: boolean | null; stepCount: number }[]>([])
const lerobotAngleUnit = ref<'radians' | 'degrees'>('radians')

const expandedText = ref<Record<string, boolean>>({})

// Token stream (D3): simulated playback of the recorded response text.
const streamVisibleTokens = ref(0)
const streamTotalTokens = ref(0)
const streamRunning = ref(false)
let streamTimer: number | undefined

function formatTime(ns: number): string {
  const totalMs = Math.floor(ns / 1_000_000)
  const ms = totalMs % 1000
  const totalS = Math.floor(totalMs / 1000)
  const s = totalS % 60
  const m = Math.floor(totalS / 60)
  return `${m}:${String(s).padStart(2, '0')}.${String(ms).padStart(3, '0')}`
}

async function loadSummary() {
  if (!props.recordingId) return
  summaryLoading.value = true
  summaryError.value = null
  try {
    summary.value = await getAttributionSummary(props.recordingId)
  } catch (e) {
    summaryError.value = e instanceof Error ? e.message : 'Failed to load attribution data'
  } finally {
    summaryLoading.value = false
  }
}

watch(() => props.recordingId, () => {
  summary.value = null
  selectedTs.value = null
  detail.value = null
  stopTokenStream()
  loadSummary()
}, { immediate: true })

watch(source, () => {
  selectedTs.value = null
  detail.value = null
  detailError.value = null
  stopTokenStream()
})

const chains = computed(() => (
  source.value === 'lerobot' ? lerobotSummaries.value : summary.value?.chains ?? []
))

const selectedIndex = computed(() => {
  if (selectedTs.value === null) return -1
  return chains.value.findIndex((c) => c.timestampNanos === selectedTs.value)
})

const nearestIndex = computed(() => {
  if (!chains.value.length) return -1
  let best = 0
  let bestDist = Math.abs(chains.value[0].timestampNanos - props.currentTimestamp)
  for (let i = 1; i < chains.value.length; i++) {
    const dist = Math.abs(chains.value[i].timestampNanos - props.currentTimestamp)
    if (dist < bestDist) { bestDist = dist; best = i }
  }
  return best
})

async function selectChain(ts: number, seek = false) {
  selectedTs.value = ts
  detailError.value = null
  expandedText.value = {}
  stopTokenStream()
  if (source.value === 'lerobot') {
    detail.value = lerobotChains.value.find((c) => c.timestampNanos === ts) ?? null
    detailError.value = detail.value ? null : 'chain not found in loaded page'
    return
  }
  if (seek) emit('seek-timestamp', ts)
  detailLoading.value = true
  try {
    detail.value = await getAttributionChain(props.recordingId, ts)
  } catch (e) {
    detail.value = null
    detailError.value = e instanceof Error ? e.message : 'Failed to load detail'
  } finally {
    detailLoading.value = false
  }
}

function moveChain(dir: 1 | -1) {
  const idx = selectedIndex.value
  if (idx < 0) return
  const next = idx + dir
  if (next < 0 || next >= chains.value.length) return
  selectChain(chains.value[next].timestampNanos, true)
}

// --- LeRobot source actions (M10) ---

async function runLerobotScan() {
  if (!lerobotPath.value) return
  lerobotScanning.value = true
  lerobotError.value = null
  try {
    dataset.value = await scanLerobotDataset(lerobotPath.value)
    const [profilesResult, autodetectResult] = await Promise.all([
      getLerobotProfiles(),
      autodetectLerobotProfile(lerobotPath.value).catch(() => null),
    ])
    profiles.value = profilesResult.profiles
    selectedProfile.value = autodetectResult?.suggestedProfile ?? profiles.value[0]?.name ?? ''
    selectedEpisode.value = dataset.value.episodes[0]?.index ?? null
    pageOffset.value = 0
    await loadLerobotEpisode()
  } catch (e) {
    lerobotError.value = e instanceof Error ? e.message : 'scan failed'
    dataset.value = null
  } finally {
    lerobotScanning.value = false
  }
}

async function loadLerobotEpisode() {
  if (!dataset.value || selectedEpisode.value === null) return
  lerobotError.value = null
  try {
    const result = await getLerobotAttribution(
      lerobotPath.value, selectedEpisode.value, pageOffset.value, pageSize,
      selectedProfile.value || undefined,
    )
    lerobotChains.value = result.chains
    lerobotSummaries.value = result.summaries
    lerobotTotal.value = result.total
    lerobotAngleUnit.value = result.angleUnit ?? 'radians'
    selectedTs.value = null
    detail.value = null
    stopTokenStream()
  } catch (e) {
    lerobotError.value = e instanceof Error ? e.message : 'load failed'
  }
}

async function lerobotPage(dir: 1 | -1) {
  const next = pageOffset.value + dir * pageSize
  if (next < 0 || next >= lerobotTotal.value) return
  pageOffset.value = next
  await loadLerobotEpisode()
}

function onShowIn3d() {
  if (selectedTs.value === null) return
  if (source.value === 'lerobot') {
    const action = detail.value?.steps.find((s) => s.kind === 'parsedAction')
    if (action?.kind === 'parsedAction') {
      const degToRad = Math.PI / 180
      const vector = lerobotAngleUnit.value === 'degrees'
        ? action.vector.map((v) => v * degToRad)
        : action.vector
      // M13 D4.1: the parent switches the preview to the profile's own
      // robot model (B601) when the MoveIt tool has it loaded.
      const profileRobot = profiles.value.find((p) => p.name === selectedProfile.value)?.robot ?? null
      emit('apply-action', vector, profileRobot)
    }
  } else {
    emit('seek-timestamp', selectedTs.value)
  }
  collapsed.value = true
}

// --- Token stream (simulated 50 tokens/sec over the recorded text) ---

const responseStep = computed(() => (
  detail.value?.steps.find((s): s is Extract<AttributionStepResponse, { kind: 'llmResponse' }> => s.kind === 'llmResponse') ?? null
))

const responseTokens = computed(() => {
  const text = responseStep.value?.text ?? ''
  return text.length ? text.split(/\s+/) : []
})

const streamedText = computed(() => responseTokens.value.slice(0, streamVisibleTokens.value).join(' '))

function startTokenStream() {
  if (!responseStep.value || streamRunning.value) return
  streamTotalTokens.value = responseTokens.value.length
  streamVisibleTokens.value = 0
  streamRunning.value = true
  const intervalMs = 20 // 50 tokens/sec
  streamTimer = window.setInterval(() => {
    streamVisibleTokens.value += 1
    if (streamVisibleTokens.value >= streamTotalTokens.value) stopTokenStream()
  }, intervalMs)
}

function stopTokenStream() {
  streamRunning.value = false
  if (streamTimer !== undefined) {
    window.clearInterval(streamTimer)
    streamTimer = undefined
  }
}

function toggleText(key: string) {
  expandedText.value[key] = !expandedText.value[key]
}

function isExpanded(key: string) {
  return !!expandedText.value[key]
}

function textDisplay(text: string, key: string) {
  if (text.length <= 200 || isExpanded(key)) return text
  return `${text.slice(0, 200)}…`
}

onUnmounted(stopTokenStream)

const sourceOptions = computed(() => [
  { value: 'drec' as AttributionSource, label: t.value.attribution.sourceDrec, disabled: false, hint: '' },
  { value: 'lerobot' as AttributionSource, label: t.value.attribution.sourceLerobot, disabled: false, hint: '' },
  { value: 'live' as AttributionSource, label: t.value.attribution.sourceLive, disabled: true, hint: t.value.attribution.sourceLiveHint },
])

const currentSourceHint = computed(() => (
  sourceOptions.value.find((o) => o.value === source.value)?.hint ?? ''
))
</script>

<template>
  <section class="attr-panel" :class="{ collapsed }">
    <!-- Header -->
    <header class="attr-header">
      <button class="attr-toggle" type="button" @click="collapsed = !collapsed">
        {{ collapsed ? '▸' : '▾' }}
      </button>
      <strong class="attr-title">{{ t.attribution.title }}</strong>
      <span v-if="chains.length" class="attr-count">
        {{ chains.length }} {{ t.attribution.chains }}
      </span>

      <label class="attr-source">
        <span class="attr-source-label">{{ t.attribution.source }}</span>
        <select v-model="source" class="attr-source-select">
          <option
            v-for="opt in sourceOptions"
            :key="opt.value"
            :value="opt.value"
            :disabled="opt.disabled"
          >
            {{ opt.label }}
          </option>
        </select>
      </label>
      <span v-if="currentSourceHint" class="attr-source-hint">{{ currentSourceHint }}</span>
    </header>

    <div v-show="!collapsed" class="attr-body">
      <!-- LeRobot controls (M10) — visible as soon as the source is selected -->
      <div v-if="source === 'lerobot'" class="attr-lerobot">
        <div class="attr-lerobot-row">
          <input
            v-model="lerobotPath"
            class="attr-lerobot-path"
            :placeholder="'/path/to/dataset (e.g. ~/.cache/huggingface/lerobot/my_org/b601_pilot_v1)'"
            @keyup.enter="runLerobotScan"
          />
          <button class="attr-cta" type="button" :disabled="lerobotScanning" @click="runLerobotScan">
            {{ lerobotScanning ? t.attribution.scanning : t.attribution.scan }}
          </button>
        </div>
        <div v-if="lerobotError" class="attr-state error">{{ lerobotError }}</div>
        <div v-if="dataset" class="attr-lerobot-info">
          <span class="attr-chip">{{ dataset.name }}</span>
          <span class="attr-chip">{{ dataset.layout }}</span>
          <span class="attr-chip">{{ dataset.episodes.length }} {{ t.attribution.episodes }}</span>
          <span class="attr-chip mono">{{ dataset.columns.length }} cols</span>
        </div>
        <div v-if="dataset" class="attr-lerobot-row">
          <label class="attr-source">
            <span class="attr-source-label">{{ t.attribution.profile }}</span>
            <select v-model="selectedProfile" class="attr-source-select" @change="pageOffset = 0; loadLerobotEpisode()">
              <option v-for="p in profiles" :key="p.name" :value="p.name">{{ p.robot }} ({{ p.name }})</option>
            </select>
          </label>
          <label class="attr-source">
            <span class="attr-source-label">Episode</span>
            <select v-model="selectedEpisode" class="attr-source-select" @change="pageOffset = 0; loadLerobotEpisode()">
              <option v-for="e in dataset.episodes" :key="e.index" :value="e.index">
                #{{ e.index }} · {{ e.rows }} {{ t.attribution.frames }}
              </option>
            </select>
          </label>
          <span class="attr-detail-spacer"></span>
          <button v-if="lerobotTotal > pageSize" class="attr-nav" type="button" :disabled="pageOffset === 0" @click="lerobotPage(-1)">‹</button>
          <span v-if="lerobotTotal > pageSize" class="attr-chip">
            {{ t.attribution.page }} {{ pageOffset / pageSize + 1 }} {{ t.attribution.of }} {{ Math.ceil(lerobotTotal / pageSize) }}
          </span>
          <button v-if="lerobotTotal > pageSize" class="attr-nav" type="button" :disabled="pageOffset + pageSize >= lerobotTotal" @click="lerobotPage(1)">›</button>
        </div>
      </div>

      <!-- Loading / error / empty states (drec source) -->
      <div v-if="source === 'drec' && summaryLoading" class="attr-state">…</div>
      <div v-else-if="source === 'drec' && summaryError" class="attr-state error">{{ summaryError }}</div>
      <div v-else-if="!chains.length" class="attr-state empty">
        <strong>{{ t.attribution.empty }}</strong>
        <span>{{ t.attribution.emptyHint }}</span>
      </div>

      <template v-else>
        <!-- Icon chain strip -->
        <div class="attr-strip" role="list">
          <button
            v-for="(chain, i) in chains"
            :key="chain.timestampNanos"
            type="button"
            role="listitem"
            :class="[
              'attr-tick',
              chain.success === null ? 'neutral' : chain.success ? 'ok' : 'fail',
              { selected: selectedTs === chain.timestampNanos, nearest: source === 'drec' && i === nearestIndex },
            ]"
            :title="formatTime(chain.timestampNanos)"
            @click="selectChain(chain.timestampNanos)"
          >
            <span class="attr-ticks">
              <svg viewBox="0 0 12 12" class="attr-icon" aria-label="camera"><rect x="1.5" y="3" width="9" height="6.5" rx="1"/><circle cx="6" cy="6.2" r="1.8"/><path d="M4.5 3l.7-1.2h1.6L7.5 3"/></svg>
              <svg viewBox="0 0 12 12" class="attr-icon" aria-label="prompt"><rect x="1" y="2" width="10" height="7" rx="1.6"/><path d="M3 11l1.6-2H11"/></svg>
              <svg viewBox="0 0 12 12" class="attr-icon" aria-label="action"><circle cx="6" cy="6" r="2.1"/><path d="M6 1.2v1.4M6 9.4v1.4M1.2 6h1.4M9.4 6h1.4M2.6 2.6l1 1M8.4 8.4l1 1M9.4 2.6l-1 1M3.6 8.4l-1 1"/></svg>
              <svg v-if="chain.success === true" viewBox="0 0 12 12" class="attr-icon ok" aria-label="ok"><path d="M2.2 6.4l2.6 2.6 5-5.2"/></svg>
              <svg v-else-if="chain.success === false" viewBox="0 0 12 12" class="attr-icon fail" aria-label="fail"><path d="M3 3l6 6M9 3l-6 6"/></svg>
              <svg v-else viewBox="0 0 12 12" class="attr-icon" aria-label="no-result"><path d="M3 6h6"/></svg>
            </span>
            <span class="attr-tick-time">{{ formatTime(chain.timestampNanos) }}</span>
          </button>
        </div>

        <!-- Unparseable streams (honest reporting) -->
        <div v-if="source === 'drec' && summary?.unparseableStreams.length" class="attr-unparseable">
          <strong>{{ t.attribution.unparseable }}</strong>
          <span v-for="s in summary.unparseableStreams" :key="s.nodeId + s.outputId">
            {{ s.nodeId }}/{{ s.outputId }} — {{ s.reason }}
          </span>
        </div>

        <!-- Detail card -->
        <div v-if="selectedTs !== null" class="attr-detail">
          <div class="attr-detail-header">
            <button class="attr-nav" type="button" :disabled="selectedIndex <= 0" @click="moveChain(-1)">‹</button>
            <button class="attr-nav" type="button" :disabled="selectedIndex < 0 || selectedIndex >= chains.length - 1" @click="moveChain(1)">›</button>
            <span class="attr-detail-time">{{ formatTime(selectedTs) }}</span>
            <span
              v-if="detail && chains[selectedIndex]?.success != null"
              :class="['attr-status-pill', chains[selectedIndex]?.success ? 'ok' : 'fail']"
            >
              {{ chains[selectedIndex]?.success ? t.attribution.success : t.attribution.failed }}
            </span>
            <span class="attr-detail-spacer"></span>
            <button class="attr-cta" type="button" @click="onShowIn3d">
              {{ t.attribution.showIn3d }}
            </button>
            <button class="attr-close" type="button" @click="selectedTs = null; detail = null; stopTokenStream()">✕</button>
          </div>

          <div v-if="detailLoading" class="attr-detail-body">…</div>
          <div v-else-if="detailError" class="attr-detail-body error">{{ t.attribution.noDetail }}: {{ detailError }}</div>
          <div v-else-if="detail" class="attr-detail-body">
            <span v-if="source === 'lerobot' && !props.previewOnProfileModel" class="attr-note">{{ t.attribution.nanoPreviewNote }}</span>
            <!-- Step: SensorFrame -->
            <div class="attr-step">
              <span class="attr-step-num">1</span>
              <div class="attr-step-main">
                <strong>{{ t.attribution.stepFrame }}</strong>
                <template v-if="detail.steps[0]?.kind === 'sensorFrame'">
                  <div class="attr-chips">
                    <span class="attr-chip mono">{{ detail.steps[0].topic }}</span>
                    <span class="attr-chip">{{ detail.steps[0].width }} × {{ detail.steps[0].height }}</span>
                    <span class="attr-chip">{{ detail.steps[0].encoding }}</span>
                  </div>
                  <small class="attr-note">image data not recorded — metadata only</small>
                </template>
                <span v-else class="attr-unavailable">{{ t.attribution.notAvailable }}</span>
              </div>
            </div>

            <!-- Step: Prompt -->
            <div class="attr-step">
              <span class="attr-step-num">2</span>
              <div class="attr-step-main">
                <strong>{{ t.attribution.stepPrompt }}</strong>
                <template v-if="detail.steps[1]?.kind === 'prompt'">
                  <span class="attr-chip">{{ detail.steps[1].tokenCount }} {{ t.attribution.tokens }}</span>
                  <p class="attr-text">
                    {{ textDisplay(detail.steps[1].text, 'prompt') }}
                    <button v-if="detail.steps[1].text.length > 200" class="attr-text-toggle" type="button" @click="toggleText('prompt')">
                      {{ isExpanded('prompt') ? t.attribution.collapseText : t.attribution.expandText }}
                    </button>
                  </p>
                </template>
                <span v-else class="attr-unavailable">{{ t.attribution.notAvailable }}</span>
              </div>
            </div>

            <!-- Step: LLM response + token stream -->
            <div class="attr-step">
              <span class="attr-step-num">3</span>
              <div class="attr-step-main">
                <strong>{{ t.attribution.stepResponse }}</strong>
                <template v-if="detail.steps[2]?.kind === 'llmResponse'">
                  <div class="attr-chips">
                    <span class="attr-chip">{{ detail.steps[2].tokenCount }} {{ t.attribution.tokens }}</span>
                    <span class="attr-chip mono">{{ detail.steps[2].model }}</span>
                    <span class="attr-chip">{{ t.attribution.latency }} {{ detail.steps[2].latencyMs }} ms</span>
                  </div>
                  <p class="attr-text">
                    {{ textDisplay(detail.steps[2].text, 'response') }}
                    <button v-if="detail.steps[2].text.length > 200" class="attr-text-toggle" type="button" @click="toggleText('response')">
                      {{ isExpanded('response') ? t.attribution.collapseText : t.attribution.expandText }}
                    </button>
                  </p>

                  <!-- D3: token stream viewer -->
                  <div class="attr-stream">
                    <button class="attr-cta sm" type="button" :disabled="!responseStep || streamRunning" @click="startTokenStream">
                      {{ streamRunning ? `${streamVisibleTokens}/${streamTotalTokens}` : t.attribution.replayStream }}
                    </button>
                    <span v-if="streamRunning" class="attr-stream-label">{{ t.attribution.tokenStream }} · 50 tok/s</span>
                  </div>
                  <div v-if="streamRunning || streamVisibleTokens > 0" class="attr-stream-box mono">
                    {{ streamedText }}<span v-if="streamRunning" class="attr-caret">▌</span>
                  </div>
                </template>
                <span v-else class="attr-unavailable">{{ t.attribution.notAvailable }}</span>
              </div>
            </div>

            <!-- Step: ParsedAction -->
            <div class="attr-step">
              <span class="attr-step-num">4</span>
              <div class="attr-step-main">
                <strong>{{ t.attribution.stepAction }}</strong>
                <template v-if="detail.steps[3]?.kind === 'parsedAction'">
                  <div class="attr-chips">
                    <span class="attr-chip mono">{{ detail.steps[3].actionType }}</span>
                    <span v-if="source === 'lerobot'" class="attr-chip mono">{{ lerobotAngleUnit === 'degrees' ? 'deg' : 'rad' }}</span>
                    <span class="attr-chip">{{ t.attribution.confidence }} {{ detail.steps[3].confidence != null ? (detail.steps[3].confidence * 100).toFixed(0) + '%' : 'n/a' }}</span>
                  </div>
                  <table class="attr-table">
                    <tbody>
                      <tr v-for="(v, i) in detail.steps[3].vector" :key="i">
                        <td class="attr-table-idx">joint_{{ i + 1 }}</td>
                        <td class="mono">{{ v.toFixed(3) }}</td>
                      </tr>
                    </tbody>
                  </table>
                </template>
                <span v-else class="attr-unavailable">{{ t.attribution.notAvailable }}</span>
              </div>
            </div>

            <!-- Step: ExecutionResult -->
            <div class="attr-step">
              <span class="attr-step-num">5</span>
              <div class="attr-step-main">
                <strong>{{ t.attribution.stepExecution }}</strong>
                <template v-if="detail.steps[4]?.kind === 'executionResult'">
                  <span
                    :class="['attr-status-pill', detail.steps[4].success ? 'ok' : 'fail']"
                  >
                    {{ detail.steps[4].success ? t.attribution.success : t.attribution.failed }}
                  </span>
                  <p v-if="detail.steps[4].errorMessage" class="attr-error-text">
                    {{ detail.steps[4].errorMessage }}
                  </p>
                </template>
                <span v-else class="attr-unavailable">{{ t.attribution.notAvailable }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>
  </section>
</template>

<style scoped>
/* Panel */
.attr-panel {
  position: absolute;
  bottom: 64px;
  left: 50%;
  transform: translateX(-50%);
  width: min(96%, 1100px);
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, var(--card-surface) 94%, transparent);
  backdrop-filter: blur(8px);
  border: 1px solid var(--hairline);
  border-radius: 10px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  z-index: 9;
  overflow: hidden;
}

/* Header */
.attr-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  flex-shrink: 0;
}
.attr-toggle {
  background: none; border: none; cursor: pointer;
  color: var(--text-body); font-size: 14px; padding: 2px 4px;
}
.attr-title { font-size: 13px; font-weight: 600; color: var(--text-heading); }
.attr-count { font-size: 11px; color: var(--text-muted-dark); }
.attr-source {
  display: flex; align-items: center; gap: 6px; margin-left: auto;
}
.attr-source-label { font-size: 11px; color: var(--text-muted-dark); }
.attr-source-select {
  padding: 4px 8px; font-size: 12px;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 4px;
}
.attr-source-hint { font-size: 11px; color: var(--accent-yellow); }

/* Body */
.attr-body {
  display: flex; flex-direction: column; gap: 10px;
  padding: 0 12px 12px;
  max-height: 46vh;
  overflow-y: auto;
}
.attr-state {
  display: flex; flex-direction: column; gap: 6px; align-items: center;
  padding: 28px 12px; font-size: 13px; color: var(--text-muted-dark);
}
.attr-state strong { font-size: 16px; color: var(--text-body); }
.attr-state.empty::before {
  content: '◌';
  font-size: 22px; color: var(--text-muted-dark); line-height: 1;
}
.attr-state.error, .attr-detail-body.error { color: var(--accent-red); }

/* Chain strip */
.attr-strip {
  display: flex; gap: 6px; overflow-x: auto;
  padding: 4px 2px;
}
.attr-tick {
  display: flex; flex-direction: column; align-items: center; gap: 3px;
  min-width: 34px; padding: 5px 4px;
  background: var(--canvas-base);
  border: 1px solid var(--hairline); border-radius: 6px;
  cursor: pointer; flex-shrink: 0;
  transition: border-color 120ms ease, transform 120ms ease;
}
.attr-tick:hover { border-color: var(--text-muted-dark); }
.attr-tick.selected {
  border-color: var(--accent-cyan);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent-cyan) 40%, transparent);
}
.attr-tick.nearest:not(.selected) { border-color: color-mix(in srgb, var(--accent-cyan) 45%, transparent); }
.attr-tick.ok .attr-icon { color: var(--accent-green); }
.attr-tick.fail .attr-icon { color: var(--accent-red); }
.attr-tick.neutral .attr-icon { color: var(--text-muted-dark); }
.attr-tick.fail { background: color-mix(in srgb, var(--accent-red) 7%, var(--canvas-base)); }
.attr-ticks {
  display: flex; flex-direction: column; gap: 2px; align-items: center;
}
.attr-icon {
  width: 13px; height: 13px; fill: none; stroke: currentColor;
  stroke-width: 1.2; stroke-linecap: round; stroke-linejoin: round;
}
.attr-icon.ok, .attr-icon.fail { stroke-width: 1.6; }
.attr-tick-time { font-size: 9px; font-family: monospace; color: var(--text-muted-dark); }

/* Unparseable */
.attr-unparseable {
  display: flex; flex-direction: column; gap: 2px;
  padding: 6px 10px;
  border: 1px solid color-mix(in srgb, var(--accent-yellow) 40%, transparent);
  border-radius: 6px;
  font-size: 11px; color: var(--text-muted-dark);
}
.attr-unparseable strong { color: var(--accent-yellow); font-size: 11px; }

/* Detail card */
.attr-detail {
  border: 1px solid var(--hairline); border-radius: 8px;
  background: color-mix(in srgb, var(--canvas-base) 80%, transparent);
  display: flex; flex-direction: column;
  overflow: hidden;
}
.attr-detail-header {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--hairline);
}
.attr-nav {
  width: 30px; height: 28px;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 4px;
  font-size: 15px; cursor: pointer;
}
.attr-nav:disabled { opacity: 0.35; cursor: default; }
.attr-detail-time { font-family: monospace; font-size: 13px; font-weight: 600; color: var(--accent-cyan); }
.attr-detail-spacer { flex: 1; }
.attr-status-pill {
  padding: 2px 10px; border-radius: 10px; font-size: 11px; font-weight: 600;
}
.attr-status-pill.ok { background: color-mix(in srgb, var(--accent-green) 18%, transparent); color: var(--accent-green); }
.attr-status-pill.fail { background: color-mix(in srgb, var(--accent-red) 18%, transparent); color: var(--accent-red); }
.attr-cta {
  padding: 5px 12px; font-size: 12px; font-weight: 600;
  background: var(--accent-cyan); color: #000;
  border: none; border-radius: 5px; cursor: pointer;
}
.attr-cta.sm { padding: 4px 10px; font-size: 11px; background: var(--canvas-base); color: var(--accent-cyan); border: 1px solid var(--accent-cyan); }
.attr-cta:disabled { opacity: 0.4; cursor: default; }
.attr-close {
  background: none; border: none; color: var(--text-muted-dark);
  font-size: 13px; cursor: pointer; padding: 2px 6px;
}
.attr-close:hover { color: var(--text-heading); }

/* Detail body */
.attr-detail-body {
  display: flex; flex-direction: column; gap: 8px;
  padding: 10px;
  font-size: 12px;
}
.attr-step {
  display: flex; gap: 10px; align-items: flex-start;
}
.attr-step-num {
  flex-shrink: 0;
  width: 20px; height: 20px; border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  font-size: 11px; font-weight: 700;
  background: var(--card-surface); color: var(--accent-cyan);
  border: 1px solid var(--hairline);
  margin-top: 1px;
}
.attr-step-main {
  flex: 1; display: flex; flex-direction: column; gap: 6px;
  min-width: 0;
}
.attr-step-main > strong { font-size: 12px; color: var(--text-heading); }
.attr-chips { display: flex; gap: 6px; flex-wrap: wrap; }
.attr-chip {
  padding: 2px 8px; border-radius: 10px; font-size: 11px;
  background: var(--card-surface); color: var(--text-body);
  border: 1px solid var(--hairline);
}
.attr-chip.mono, .mono { font-family: monospace; }
.attr-note { font-size: 10px; color: var(--text-muted-dark); }
.attr-text {
  margin: 0; line-height: 1.55; color: var(--text-body);
  font-size: 12px; word-break: break-word;
}
.attr-text-toggle {
  background: none; border: none; cursor: pointer;
  color: var(--accent-cyan); font-size: 11px; padding: 0 2px;
}
.attr-table {
  border-collapse: collapse; max-width: 320px;
  font-size: 11px;
}
.attr-table td {
  padding: 3px 10px; border-bottom: 1px solid var(--hairline);
  color: var(--text-body);
}
.attr-table-idx { color: var(--text-muted-dark); }
.attr-error-text { margin: 0; color: var(--accent-red); font-size: 12px; }

/* Token stream */
.attr-stream { display: flex; align-items: center; gap: 8px; }
.attr-stream-label { font-size: 11px; color: var(--accent-cyan); }
.attr-stream-box {
  padding: 6px 10px; border-radius: 5px;
  background: var(--canvas-base); border: 1px solid var(--hairline);
  font-size: 12px; line-height: 1.5; color: var(--accent-cyan);
  word-break: break-word;
  min-height: 18px;
}
.attr-caret { animation: attr-blink 0.9s steps(1) infinite; }
@keyframes attr-blink { 50% { opacity: 0; } }

/* LeRobot source controls (M10) */
.attr-lerobot { display: flex; flex-direction: column; gap: 8px; }
.attr-lerobot-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.attr-lerobot-path {
  flex: 1; min-width: 280px; padding: 7px 10px; font-size: 12px; font-family: monospace;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 5px;
}
.attr-lerobot-info { display: flex; gap: 6px; flex-wrap: wrap; }
.attr-unavailable { color: var(--text-muted-dark); font-size: 12px; font-style: italic; }

.attr-panel.collapsed .attr-body { display: none; }
</style>
