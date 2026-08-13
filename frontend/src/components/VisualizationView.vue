<template>
  <section class="viz-layout">
    <!-- Collapsible left sidebar -->
    <aside :class="['viz-panel', 'viz-left', { collapsed: sidebarCollapsed }]">
      <div class="viz-panel-header viz-left-header">
        <h2>Data Sources</h2>
        <div class="viz-left-header-actions">
          <span :class="['pill', 'sm', backendConnected ? 'success' : 'stopped']">
            {{ backendConnected ? 'connected' : 'offline' }}
          </span>
          <button class="viz-refresh-button" type="button" :disabled="isRefreshing" @click="loadVisualizationData">
            {{ isRefreshing ? '...' : '↻' }}
          </button>
        </div>
      </div>

      <div v-show="!sidebarCollapsed" class="viz-sidebar-body">
        <div class="viz-search">
          <input v-model="topicSearch" type="text" placeholder="Filter topics..." />
        </div>

        <div class="robot-profile-card">
          <div class="robot-profile-title">
            <div>
              <span>Robot Profile</span>
              <strong>{{ robotProfile.name }}</strong>
            </div>
            <button class="robot-profile-toggle" type="button" @click="robotModulesExpanded = !robotModulesExpanded">
              {{ robotModulesExpanded ? 'Hide' : `Modules (${robotProfile.modules.length})` }}
            </button>
          </div>
          <p v-show="!robotModulesExpanded">{{ robotProfile.summary }}</p>
          <div v-if="robotModulesExpanded" class="robot-module-list">
            <button
              v-for="module in visibleRobotModules"
              :key="module.id"
              :class="['robot-module-chip', { selected: selectedRobotModule?.id === module.id }]"
              type="button"
              @click="selectRobotModule(module.id)"
            >
              <span :class="['robot-module-dot', module.status]"></span>
              <div>
                <strong>{{ module.name }}</strong>
                <small>{{ module.kind }} · {{ module.status }}</small>
              </div>
            </button>
          </div>
        </div>

        <div class="display-list">
          <div
            v-for="display in displayData.displays"
            :key="display.id"
            :class="['display-item', { enabled: display.enabled, inactive: !display.enabled }]"
          >
            <input type="checkbox" :checked="display.enabled" @change="toggleDisplay(display.id)" />
            <span :class="['display-dot', display.color]"></span>
            <button class="display-label" type="button" :disabled="!display.sourceTopic" @click="inspectDisplayTopic(display)">
              <strong>{{ display.name }}</strong>
              <small>{{ display.summary }}</small>
            </button>
            <span :class="['display-status', display.status]">{{ display.status }}</span>
          </div>
        </div>

        <div class="viz-section">
          <div class="viz-section-header">
            <h3>Topics</h3>
            <span class="viz-section-source">{{ filteredTopics.length }}/{{ topicData.topics.length }}</span>
          </div>
          <button
            v-for="topic in filteredTopics"
            :key="topic.name"
            :class="['topic-preview-box', { selected: selectedTopic?.name === topic.name }]"
            type="button"
            @click="selectTopic(topic.name)"
          >
            <div class="topic-title">
              <code>{{ topic.name }}</code>
              <span :class="['topic-status', topic.status]">{{ topic.status }}</span>
            </div>
            <span>{{ topic.dataType }} · {{ topic.summary }}</span>
          </button>
        </div>

        <div :class="['viz-data-state', dataStateKind]">
          <strong>{{ dataStateTitle }}</strong>
          <span>{{ dataStateMessage }}</span>
        </div>
        <div class="viz-status-row">
          <span class="status-dot" :class="backendConnected ? 'on' : 'off'"></span>
          <span>{{ connectionLabel }}</span>
        </div>
      </div>
    </aside>

    <!-- Full-viewport 3D area -->
    <article class="viz-panel viz-center">
      <!-- Thin top bar -->
      <div class="viz-topbar">
        <button class="viz-sidebar-toggle" @click="sidebarCollapsed = !sidebarCollapsed" :title="sidebarCollapsed ? 'Show panel' : 'Hide panel'">
          {{ sidebarCollapsed ? '☰' : '✕' }}
        </button>
        <span class="viz-robot-label">{{ robotProfile.name }}</span>
        <span class="pill success">RobotModel</span>
        <span class="viz-topbar-spacer"></span>
        <!-- Live / Replay toggle -->
        <button
          :class="['pill', viewportMode === 'live' ? 'success' : 'info']"
          @click="viewportMode = viewportMode === 'live' ? 'replay' : 'live'"
        >{{ viewportMode === 'live' ? 'Live' : 'Replay' }}</button>
      </div>

      <!-- 3D Viewer fills remaining space -->
      <div class="viz-robot-viewer-card">
        <NanoRobotViewer
          :xml-url="nanoArmResources.xmlUrl"
          :asset-base-url="nanoArmResources.assetBaseUrl"
          :joint-values="effectiveJointState"
          :base-pose="effectiveBasePose"
          viewer-label="Nano RobotModel"
        />

        <!-- M09/M10: attribution bar (replay mode, above the floating replay bar) -->
        <AttributionBar
          v-if="viewportMode === 'replay'"
          :recording-id="replayRecordingId"
          :current-timestamp="replayCurrentTime"
          @seek-timestamp="(ts: number) => replayEngine?.seek(ts, true)"
          @apply-action="applyActionVector"
        />

        <!-- Floating replay bar (overlay at bottom of viewport) -->
        <div v-if="viewportMode === 'replay'" class="viz-replay-bar">
          <input
            v-model="replayPath"
            class="rp-path-input"
            placeholder=".drec file path"
            :disabled="replayActive"
            @keyup.enter="startReplay"
          />
          <button v-if="!replayActive" class="rp-btn" @click="startReplay">Load</button>
          <button v-else class="rp-btn rp-btn-close" @click="stopReplay">✕</button>
          <span v-if="replayError" class="rp-error">{{ replayError }}</span>

          <template v-if="replayActive">
            <span class="rp-time">{{ replayTimeFormatted }} / {{ replayDurationFormatted }}</span>
            <button class="rp-btn" @click="replayEngine?.play()" title="Play (Space)">▶</button>
            <button class="rp-btn" @click="replayEngine?.pause()" title="Pause">⏸</button>
            <button class="rp-btn" @click="replayEngine?.stop()" title="Stop">⏹</button>
            <input
              type="range" min="0" :max="replayDuration"
              :value="replayCurrentTime"
              class="rp-scrubber"
              @input="(e) => replayEngine?.seek(Number((e.target as HTMLInputElement).value))"
            />
          </template>
        </div>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { BACKEND_BASE_URL, getDvizDisplays, getDvizSnapshot, getDvizStatus, getDvizTopics, getRobotProfile, openRecording, type ApiResult, type ApiSource, type DvizDisplayResponse, type DvizDisplaysResponse, type DvizSnapshotResponse, type DvizStatusResponse, type DvizTopicResponse, type DvizTopicsResponse, type RobotModuleResponse, type RobotProfileResponse } from '../api'
import { buildNanoArmModelResources, createNanoArmJointState } from '../lib/nanoArmModel'
import { PlaybackEngine } from '../playback'
import { ReplayScene, type RobotJointState, type RobotBasePose } from '../replay-scene'
import { createNanoRobotBasePose } from '../lib/nanoRobotMotion'
import NanoRobotViewer from './NanoRobotViewer.vue'
import AttributionBar from './AttributionBar.vue'

const fallbackDvizStatus: DvizStatusResponse = {
  installed: false,
  running: false,
  binaryPath: null,
  message: 'Backend API is not connected.',
}

const fallbackTopicData: DvizTopicsResponse = {
  source: 'frontend fallback',
  message: 'Backend API is not connected; showing frontend fallback topics.',
  topics: [
    {
      name: '/world/points',
      dataType: 'PointCloud',
      source: 'frontend fallback',
      status: 'ready',
      messageRateHz: 12,
      lastSeen: 'demo frame',
      summary: '1,024 pts',
    },
    {
      name: '/world/tf',
      dataType: 'Transform3D',
      source: 'frontend fallback',
      status: 'ready',
      messageRateHz: 30,
      lastSeen: 'demo frame',
      summary: '5 frames',
    },
  ],
}

const fallbackDisplayData: DvizDisplaysResponse = {
  source: 'frontend fallback',
  message: 'Backend API is not connected; showing frontend fallback displays.',
  displays: [
    {
      id: 'grid',
      name: 'Grid',
      dataType: 'Viewport',
      enabled: true,
      sourceTopic: null,
      status: 'ready',
      summary: 'Ground grid · 10×10',
      color: 'gray',
    },
    {
      id: 'axes',
      name: 'Axes',
      dataType: 'Viewport',
      enabled: true,
      sourceTopic: null,
      status: 'ready',
      summary: 'RGB axes · 1.0m',
      color: 'red',
    },
    {
      id: 'tf',
      name: 'TF Frames',
      dataType: 'Transform3D',
      enabled: true,
      sourceTopic: '/world/tf',
      status: 'ready',
      summary: 'Frame tree · 5 active frames',
      color: 'green',
    },
    {
      id: 'pointcloud',
      name: 'PointCloud',
      dataType: 'PointCloud',
      enabled: false,
      sourceTopic: '/world/points',
      status: 'idle',
      summary: 'Point cloud data · color/intensity',
      color: 'blue',
    },
  ],
}

const fallbackSnapshotData: DvizSnapshotResponse = {
  source: 'frontend fallback',
  message: 'Backend API is not connected; using frontend fallback snapshot.',
  status: fallbackDvizStatus,
  summary: {
    topicCount: fallbackTopicData.topics.length,
    readyTopicCount: fallbackTopicData.topics.filter((topic) => topic.status === 'ready').length,
    idleTopicCount: fallbackTopicData.topics.filter((topic) => topic.status === 'idle').length,
    displayCount: fallbackDisplayData.displays.length,
    enabledDisplayCount: fallbackDisplayData.displays.filter((display) => display.enabled).length,
  },
}

const fallbackRobotProfile: RobotProfileResponse = {
  source: 'frontend fallback',
  message: 'Backend API is not connected; showing the frontend fallback robot profile.',
  profile: {
    id: 'nano-full-family',
    name: 'Nano Full Family',
    family: 'nano manipulator platform',
    summary: 'Capability-first profile for the Nano full robot, camera modules, optional base, and optional lidar.',
    simulationOwner: 'dora-moveit2 / MuJoCo',
    viewportRole: 'Studio mirrors moveit-side simulated state; it does not own simulation.',
    modules: [
      {
        id: 'left-arm',
        name: 'Left Nano Arm',
        kind: 'arm',
        role: 'manipulation',
        transport: 'dora dataflow',
        frame: 'left_arm_base',
        status: 'ready',
        summary: 'Primary manipulator slot with gripper-ready joint state.',
        required: true,
        sourceTopics: ['/robot/model', '/world/tf'],
        linkedDisplays: ['robotmodel', 'tf'],
      },
      {
        id: 'camera-array',
        name: 'OpenCV Camera Array',
        kind: 'camera',
        role: 'perception / recording',
        transport: 'OpenCV node',
        frame: 'camera_mounts',
        status: 'ready',
        summary: 'Variable camera count; target profile supports up to four camera slots.',
        required: true,
        sourceTopics: ['/world/points', '/world/markers'],
        linkedDisplays: ['pointcloud', 'markers'],
      },
      {
        id: 'mobile-base',
        name: 'Mobile Base',
        kind: 'mobility',
        role: 'navigation',
        transport: 'profile slot',
        frame: 'base_link',
        status: 'optional',
        summary: 'Reserved interface for base control once verified.',
        required: false,
        sourceTopics: ['/world/tf', '/world/markers'],
        linkedDisplays: ['tf', 'markers'],
      },
      {
        id: 'lidar',
        name: 'Lidar',
        kind: 'sensor',
        role: 'scan / mapping',
        transport: 'profile slot',
        frame: 'lidar_link',
        status: 'optional',
        summary: 'Reserved LaserScan source for dviz display linking.',
        required: false,
        sourceTopics: ['/world/laser'],
        linkedDisplays: ['laserscan'],
      },
    ],
    workflows: [
      {
        id: 'teleop',
        name: 'Teleoperation',
        status: 'planned',
        owner: 'dorobot dataflow',
        summary: 'Manual control path for Nano full robot operation.'
      },
      {
        id: 'recording',
        name: 'Data Collection',
        status: 'planned',
        owner: 'dorobot dataflow',
        summary: 'Camera and robot-state recording workflow for dataset capture.',
      },
      {
        id: 'planning',
        name: 'Motion Planning',
        status: 'planned',
        owner: 'dora-moveit2',
        summary: 'IK, planning, execution, and MuJoCo simulation stay moveit-owned.',
      },
    ],
    visualizationDisplays: ['RobotModel', 'TF Frames', 'PointCloud', 'LaserScan', 'Markers'],
    planningCapabilities: ['robot config selection', 'IK readiness', 'trajectory preview', 'moveit-owned MuJoCo state mirror'],
  },
}

const dviz = ref<DvizStatusResponse>(fallbackDvizStatus)
const topicData = ref<DvizTopicsResponse>(fallbackTopicData)
const displayData = ref<DvizDisplaysResponse>(fallbackDisplayData)
const snapshotData = ref<DvizSnapshotResponse>(fallbackSnapshotData)
const robotProfileData = ref<RobotProfileResponse>(fallbackRobotProfile)
const topicApiSource = ref<ApiSource>('fallback')
const statusApiSource = ref<ApiSource>('fallback')
const displayApiSource = ref<ApiSource>('fallback')
const snapshotApiSource = ref<ApiSource>('fallback')
const robotProfileApiSource = ref<ApiSource>('fallback')
const topicSearch = ref('')
const selectedTopicName = ref<string | null>(fallbackTopicData.topics[0]?.name ?? null)
const selectedRobotModuleId = ref<string | null>(fallbackRobotProfile.profile.modules[0]?.id ?? null)
const robotModulesExpanded = ref(false)
const isRefreshing = ref(false)
const refreshError = ref<string | null>(null)
const nanoArmResources = buildNanoArmModelResources(BACKEND_BASE_URL)
const nanoArmJointState = reactive(createNanoArmJointState())
const nanoRobotBasePose = reactive(createNanoRobotBasePose())

// --- UI state ---
const sidebarCollapsed = ref(false)

// --- M06: Live/Replay toggle ---
const viewportMode = ref<'live' | 'replay'>('live')
const replayPath = ref('/tmp/dora-studio-tests/joint_animation.drec')
const replayActive = ref(false)
const replayRecordingId = ref('')
const replayEngine = ref<PlaybackEngine | null>(null)
const replayScene = ref<ReplayScene | null>(null)
const replayCurrentTime = ref(0)
const replayDuration = ref(1)
const replayTimeFormatted = ref('0:00.000')
const replayDurationFormatted = ref('0:00.000')
const replayError = ref<string | null>(null)
const replayJoints = reactive<RobotJointState>({ ...createNanoArmJointState() })
const replayBasePose = reactive<RobotBasePose>({ x: 0, y: 0, yaw: 0 })

const effectiveJointState = computed(() => {
  if (viewportMode.value === 'replay') {
    // Map RobotJointState (joint_1) → NanoArmJointState (joint1)
    return {
      joint1: replayJoints.joint_1 ?? 0,
      joint2: replayJoints.joint_2 ?? 0,
      joint3: replayJoints.joint_3 ?? 0,
      joint4: replayJoints.joint_4 ?? 0,
      joint5: replayJoints.joint_5 ?? 0,
      joint6: replayJoints.joint_6 ?? 0,
    }
  }
  return nanoArmJointState
})
const effectiveBasePose = computed(() => {
  if (viewportMode.value === 'replay') {
    return { x: replayBasePose.x ?? 0, y: replayBasePose.y ?? 0, yaw: replayBasePose.yaw ?? 0 }
  }
  return nanoRobotBasePose
})

async function startReplay() {
  if (!replayPath.value) return
  replayError.value = null
  try {
    const info = await openRecording(replayPath.value)
    const engine = new PlaybackEngine()
    engine.duration = info.durationNanos
    replayEngine.value = engine
    replayRecordingId.value = info.id
    replayDuration.value = info.durationNanos
    replayDurationFormatted.value = engine.formatTime(info.durationNanos)
    replayActive.value = true

    const scene = new ReplayScene(info.id)
    replayScene.value = scene

    scene.onFrameChange((frame) => {
      Object.assign(replayJoints, frame.joints)
      Object.assign(replayBasePose, frame.basePose)
    })

    engine.onTick((t) => {
      replayCurrentTime.value = t
      replayTimeFormatted.value = engine.formatTime(t)
    })

    scene.attach(engine)
    engine.seek(0, true)
  } catch (e) {
    console.error('Failed to start replay:', e)
    replayActive.value = false
    const hint = replayPath.value.endsWith('.drec')
      ? ''
      : ' — LeRobot 数据集请用归因条数据源下拉的 LeRobot 项（.drec 回放只接受 .drec 文件）'
    replayError.value = e instanceof Error ? `Load failed: ${e.message}${hint}` : 'Load failed'
  }
}

// M10: apply a LeRobot action vector to the replay-mode viewport (first 6
// joints). Stays in replay mode so the attribution panel remains mounted;
// without an active .drec, replayJoints are the display pose.
function applyActionVector(vector: number[]) {
  const names: (keyof RobotJointState)[] = ['joint_1', 'joint_2', 'joint_3', 'joint_4', 'joint_5', 'joint_6']
  names.forEach((name, i) => {
    if (vector[i] !== undefined) replayJoints[name] = vector[i]
  })
}

function stopReplay() {
  replayEngine.value?.stop()
  replayScene.value?.dispose()
  // Close via API using the recording ID from the open call
  replayEngine.value = null
  replayScene.value = null
  replayActive.value = false
  replayRecordingId.value = ''
  replayError.value = null
  Object.assign(replayJoints, createNanoArmJointState())
  Object.assign(replayBasePose, { x: 0, y: 0, yaw: 0 })
}

const backendConnected = computed(() => (
  statusApiSource.value === 'connected'
  || topicApiSource.value === 'connected'
  || displayApiSource.value === 'connected'
  || snapshotApiSource.value === 'connected'
  || robotProfileApiSource.value === 'connected'
))

const visualizationDataLabel = computed(() => {
  if (topicApiSource.value === 'connected' && topicData.value.source === 'demo') {
    return 'backend demo'
  }

  if (topicApiSource.value === 'connected') {
    return 'backend data'
  }

  return 'fallback data'
})

const connectionLabel = computed(() => {
  if (!backendConnected.value) {
    return 'backend unavailable'
  }

  if (statusApiSource.value === 'connected') {
    return dviz.value.running ? 'dviz process detected' : 'backend connected'
  }

  return 'backend partially connected'
})

const normalizedTopicSearch = computed(() => topicSearch.value.trim().toLowerCase())

const filteredTopics = computed(() => {
  if (!normalizedTopicSearch.value) {
    return topicData.value.topics
  }

  return topicData.value.topics.filter((topic) => {
    const searchable = [topic.name, topic.dataType, topic.status, topic.source, topic.summary]
    return searchable.some((value) => value.toLowerCase().includes(normalizedTopicSearch.value))
  })
})

const selectedTopic = computed(() => {
  if (!selectedTopicName.value) {
    return null
  }

  return topicData.value.topics.find((topic) => topic.name === selectedTopicName.value) ?? null
})

const robotProfile = computed(() => robotProfileData.value.profile)
const visibleRobotModules = computed(() => robotProfile.value.modules)
const selectedRobotModule = computed(() => {
  if (!selectedRobotModuleId.value) {
    return null
  }

  return robotProfile.value.modules.find((module) => module.id === selectedRobotModuleId.value) ?? null
})
const snapshotSummary = computed(() => snapshotData.value.summary)
const refreshLabel = computed(() => (isRefreshing.value ? 'Refreshing...' : 'Refresh'))

const dataStateKind = computed(() => {
  if (isRefreshing.value) {
    return 'loading'
  }

  if (refreshError.value) {
    return 'error'
  }

  return backendConnected.value ? 'connected' : 'fallback'
})

const dataStateTitle = computed(() => {
  if (isRefreshing.value) {
    return 'Refreshing metadata'
  }

  if (refreshError.value) {
    return 'Fallback metadata active'
  }

  return backendConnected.value ? 'Backend metadata connected' : 'Frontend fallback active'
})

const dataStateMessage = computed(() => {
  if (isRefreshing.value) {
    return 'Reloading dviz status, topics, displays, and snapshot counts.'
  }

  if (refreshError.value) {
    return refreshError.value
  }

  return snapshotData.value.message
})

function selectTopic(name: string) {
  selectedTopicName.value = name
}

function selectRobotModule(id: string) {
  selectedRobotModuleId.value = id
  const module = robotProfile.value.modules.find((item: RobotModuleResponse) => item.id === id)
  const firstTopic = module?.sourceTopics.find((topic) => topicData.value.topics.some((candidate) => candidate.name === topic))

  if (firstTopic) {
    topicSearch.value = ''
    selectedTopicName.value = firstTopic
  }
}

function toggleDisplay(id: string) {
  displayData.value = {
    ...displayData.value,
    displays: displayData.value.displays.map((display) => (
      display.id === id ? { ...display, enabled: !display.enabled } : display
    )),
  }
}

function inspectDisplayTopic(display: DvizDisplayResponse) {
  if (!display.sourceTopic) {
    return
  }

  topicSearch.value = ''
  selectedTopicName.value = display.sourceTopic
}

function ensureSelectedTopic() {
  if (selectedTopicName.value && topicData.value.topics.some((topic) => topic.name === selectedTopicName.value)) {
    return
  }

  selectedTopicName.value = topicData.value.topics[0]?.name ?? null
}

function ensureSelectedRobotModule() {
  if (selectedRobotModuleId.value && robotProfile.value.modules.some((module) => module.id === selectedRobotModuleId.value)) {
    return
  }

  selectedRobotModuleId.value = robotProfile.value.modules[0]?.id ?? null
}

function firstError(...results: ApiResult<unknown>[]) {
  return results.find((result) => result.source === 'fallback' && result.error)?.error ?? null
}

async function loadVisualizationData() {
  isRefreshing.value = true
  refreshError.value = null

  const [statusResult, topicResult, displayResult, snapshotResult, robotProfileResult] = await Promise.all([
    getDvizStatus(fallbackDvizStatus),
    getDvizTopics(fallbackTopicData),
    getDvizDisplays(fallbackDisplayData),
    getDvizSnapshot(fallbackSnapshotData),
    getRobotProfile(fallbackRobotProfile),
  ])

  dviz.value = snapshotResult.source === 'connected' ? snapshotResult.data.status : statusResult.data
  topicData.value = topicResult.data
  displayData.value = displayResult.data
  snapshotData.value = snapshotResult.data
  robotProfileData.value = robotProfileResult.data
  statusApiSource.value = statusResult.source
  topicApiSource.value = topicResult.source
  displayApiSource.value = displayResult.source
  snapshotApiSource.value = snapshotResult.source
  robotProfileApiSource.value = robotProfileResult.source
  refreshError.value = firstError(statusResult, topicResult, displayResult, snapshotResult, robotProfileResult)
  ensureSelectedRobotModule()
  ensureSelectedTopic()
  isRefreshing.value = false
}

onMounted(loadVisualizationData)
</script>

<style scoped>
/* ===== Layout ===== */
.viz-layout {
  display: flex; height: 100%; overflow: hidden;
}

/* Left sidebar */
.viz-left {
  width: 280px; min-width: 280px; overflow-y: auto; overflow-x: hidden;
  border-right: 1px solid var(--hairline);
  background: var(--canvas-base);
  transition: width 200ms ease, min-width 200ms ease;
}
.viz-left.collapsed {
  width: 44px; min-width: 44px;
}
.viz-left.collapsed h2 { writing-mode: vertical-rl; font-size: 11px; }
.viz-left.collapsed .viz-left-header-actions { display: none; }
.viz-left-header {
  position: sticky; top: 0; z-index: 5;
  background: var(--canvas-base);
}
.viz-left-header-actions {
  display: flex; align-items: center; gap: 6px; flex-shrink: 0;
}
.viz-sidebar-body {
  padding: 0 12px 12px; overflow-y: auto;
}

/* Center viewport */
.viz-center {
  flex: 1; display: flex; flex-direction: column; min-width: 0; min-height: 0;
  position: relative;
}

/* Thin top bar */
.viz-topbar {
  display: flex; align-items: center; gap: 8px;
  padding: 6px 12px;
  background: var(--card-surface);
  border-bottom: 1px solid var(--hairline);
  flex-shrink: 0;
}
.viz-sidebar-toggle {
  background: none; border: none; color: var(--text-body);
  font-size: 16px; cursor: pointer; padding: 2px 6px; border-radius: 4px;
}
.viz-sidebar-toggle:hover { background: var(--card-hover); color: var(--text-heading); }
.viz-robot-label { font-size: 13px; font-weight: 600; color: var(--text-heading); }
.viz-topbar-spacer { flex: 1; }

/* Viewport — override global card padding that creates white strip */
.viz-robot-viewer-card {
  flex: 1; position: relative; min-height: 0;
  /* Override global .viz-robot-viewer-card styles. The global rule keeps a
     300px grid column for the deleted Inspector panel — block layout removes
     the empty black strip on the right. */
  display: block !important;
  padding: 0 !important; margin: 0 !important;
  border-radius: 0 !important; border: none !important;
  background: #0f172a !important; /* dark bg to match 3D viewport */
  box-shadow: none !important;
}
.viz-robot-viewer-card :deep(.nano-robot-viewer) {
  width: 100% !important; height: 100% !important;
  box-shadow: none !important;
}
.viz-robot-viewer-card :deep(canvas) {
  display: block;
}

/* ===== Floating replay bar ===== */
.viz-replay-bar {
  position: absolute; bottom: 12px; left: 50%; transform: translateX(-50%);
  display: flex; align-items: center; gap: 6px;
  padding: 8px 14px;
  background: color-mix(in srgb, var(--card-surface) 92%, transparent);
  backdrop-filter: blur(8px);
  border: 1px solid var(--hairline);
  border-radius: 10px;
  box-shadow: 0 4px 16px rgba(0,0,0,0.3);
  z-index: 10;
}
.rp-path-input {
  width: 340px; padding: 9px 12px;
  font-size: 14px; font-family: monospace;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline); border-radius: 6px;
}
.rp-path-input:disabled { opacity: 0.5; }
.rp-path-input::placeholder { color: var(--text-muted-dark); }
.rp-time {
  font-size: 14px; font-family: monospace; color: var(--accent-cyan); font-weight: 600;
  white-space: nowrap; min-width: 130px; text-align: center;
}
.rp-btn {
  padding: 7px 12px; border: none; border-radius: 5px;
  font-size: 13px; cursor: pointer;
  background: var(--canvas-base); color: var(--text-body);
  border: 1px solid var(--hairline);
}
.rp-btn:hover { background: var(--card-hover); color: var(--text-heading); }
.rp-btn-close { color: var(--accent-red); border-color: var(--accent-red); }
.rp-error { color: var(--accent-red); font-size: 13px; max-width: 300px; }
.rp-scrubber {
  width: 140px; height: 4px;
  -webkit-appearance: none; appearance: none;
  background: var(--hairline); border-radius: 2px; outline: none;
}
.rp-scrubber::-webkit-slider-thumb {
  -webkit-appearance: none; width: 12px; height: 12px;
  border-radius: 50%; background: var(--accent-red); cursor: pointer;
}
.pill.info { background: color-mix(in srgb, var(--accent-cyan) 20%, transparent); color: var(--accent-cyan); }
</style>
