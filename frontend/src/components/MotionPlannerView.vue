<template>
  <section class="motion-layout">
    <aside class="motion-panel motion-left">
      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Robot Profile</h3>
          <span class="pill">{{ robotProfileSource }}</span>
        </div>
        <div class="motion-profile-card">
          <strong>{{ robotProfile.name }}</strong>
          <span>{{ robotProfile.family }}</span>
          <p>{{ robotProfile.summary }}</p>
          <div class="motion-profile-meta">
            <span>{{ armModules.length }} arm slots</span>
            <span>{{ cameraModules.length }} camera slots</span>
            <span>{{ optionalModules.length }} optional modules</span>
            <span>{{ mappedModuleCount }} mapped modules</span>
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Robot State</h3>
          <span class="pill stopped">moveit mirror</span>
        </div>

        <div class="joint-list">
          <div v-for="joint in snapshotJoints" :key="joint.name" class="joint-row">
            <label>{{ joint.name }}</label>
            <div class="joint-input">
              <input type="text" :value="joint.value.toFixed(2)" disabled />
              <span>{{ joint.unit }}</span>
            </div>
            <small class="joint-meta">{{ joint.status }} · {{ joint.lowerLimit.toFixed(2) }}..{{ joint.upperLimit.toFixed(2) }}</small>
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>End-Effector Pose</h3>
        </div>
        <div class="pose-grid">
          <div class="pose-row" v-for="row in snapshotPoseRows" :key="row.label">
            <label>{{ row.label }}</label>
            <input v-for="(val, i) in row.values" :key="i" type="text" :value="val" disabled />
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Planning Scene</h3>
          <span class="pill">{{ moveitSnapshot.scene.objectCount }} objects</span>
        </div>
        <div class="scene-list">
          <div v-for="obj in snapshotSceneObjects" :key="obj.name" class="scene-object">
            <span :class="['scene-icon', obj.shape]"></span>
            <div>
              <strong>{{ obj.name }}</strong>
              <small>{{ obj.dims }} · {{ obj.frame }} · {{ obj.status }}</small>
            </div>
          </div>
        </div>
        <div class="scene-actions">
          <div class="console-field-row console-field-stack">
            <label>{{ t.motionConsole.boxPos }}</label>
            <div class="console-target-inputs">
              <input v-model="boxX" type="text" class="console-input" :title="t.motionConsole.targetX" />
              <input v-model="boxY" type="text" class="console-input" :title="t.motionConsole.targetY" />
              <input v-model="boxZ" type="text" class="console-input" :title="t.motionConsole.targetZ" />
            </div>
          </div>
          <div class="scene-actions-buttons">
            <button :disabled="consoleBusy" @click="addBox">+ {{ t.motionConsole.addBox }}</button>
            <button class="secondary" :disabled="consoleBusy || addedObjects.length === 0" @click="removeLastBox">
              − {{ t.motionConsole.removeBox }}
            </button>
          </div>
        </div>
      </div>

      <div class="motion-status">
        <span :class="['status-dot', moveit.running ? 'on' : 'off']"></span>
        {{ moveit.message }}
      </div>
    </aside>

    <article class="motion-panel motion-right">
      <div class="motion-section mujoco-mirror-section">
        <div class="motion-section-header">
          <h3>Nano Full MuJoCo Visual Mirror</h3>
          <span class="pill warning">mirror only</span>
        </div>
        <div class="mujoco-mirror-card">
          <NanoRobotViewer
            ref="mirrorViewer"
            :xml-url="nanoArmResources.xmlUrl"
            :asset-base-url="nanoArmResources.assetBaseUrl"
            :joint-values="nanoArmJointState"
            :base-pose="nanoRobotBasePose"
            viewer-label="Nano full arm planning preview"
            :model-visible="true"
            @loaded="updateNanoArmJointOrder"
          />
          <div class="mujoco-mirror-details" :title="nanoArmResources.xmlUrl">
            <span>{{ moveitSnapshot.visualModel.name }}</span>
            <strong>{{ moveitSnapshot.robotConfigId }}</strong>
            <p>{{ moveitSnapshot.viewportRole }}</p>
            <div class="robot-display-tags">
              <span v-for="joint in nanoArmJointOrder" :key="joint">{{ joint }}</span>
            </div>
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Motion Control</h3>
          <span class="pill" :class="consoleFeedStatus === 'connected' ? 'success' : 'failed'">
            {{ consoleFeedStatus === 'connected' ? t.motionConsole.feedOn : t.motionConsole.feedOff }}
          </span>
        </div>

        <!-- B6: live console — Plan/Execute/Stop send real requests to the
             running dataflow through the backend command queue. The joint
             grid below stays the local read-only mirror. -->
        <div class="console-field-row">
          <label>{{ t.motionConsole.targetLabel }}</label>
          <div class="console-target-inputs">
            <input v-model="targetX" type="text" class="console-input" :title="t.motionConsole.targetX" />
            <input v-model="targetY" type="text" class="console-input" :title="t.motionConsole.targetY" />
            <input v-model="targetZ" type="text" class="console-input" :title="t.motionConsole.targetZ" />
          </div>
        </div>
        <div class="console-field-row">
          <label>{{ t.motionConsole.plannerLabel }}</label>
          <select v-model="selectedPlanner" class="console-select">
            <option v-for="planner in plannerOptions" :key="planner.id" :value="planner.id">
              {{ planner.label }}
            </option>
          </select>
        </div>

        <div class="control-row motion-actions">
          <button class="primary-action" :disabled="consoleBusy" @click="sendPlan">Plan</button>
          <button class="primary-action execute" :disabled="consoleBusy" @click="sendCommand('execute')">Execute</button>
          <button class="secondary" :disabled="consoleBusy" @click="sendCommand('stop')">Stop</button>
          <button class="secondary" :disabled="consoleBusy" @click="sendCommand('auto')" :title="t.motionConsole.autoHint">Auto</button>
          <span :class="['pill', modeLabel === 'auto' ? 'success' : '']">{{ t.motionConsole.modeLabel }}: {{ modeLabel === 'auto' ? t.motionConsole.modeAuto : t.motionConsole.modeManual }}</span>
        </div>
        <div v-if="consoleError" class="rp-error">{{ consoleError }}</div>
        <div v-if="lastCommandInfo" class="console-sent-info">{{ lastCommandInfo }}</div>

        <div class="console-status">
          <span v-if="livePlanStatus !== null" :class="['pill', (livePlanStatus.success as boolean) ? 'success' : 'failed']">
            {{ t.motionConsole.planLabel }}: {{ (livePlanStatus.success as boolean) ? t.motionConsole.planOk : t.motionConsole.planFail }}
            <template v-if="typeof livePlanStatus.path_length === 'number'"> · {{ livePlanStatus.path_length }}m</template>
          </span>
          <span v-if="liveExecution !== null" :class="['pill', (liveExecution.is_executing as boolean) ? 'success' : '']">
            {{ t.motionConsole.executionLabel }}: {{ (liveExecution.is_executing as boolean) ? t.motionConsole.executing : t.motionConsole.idle }}
            <template v-if="typeof liveExecution.progress === 'number'"> · {{ Math.round((liveExecution.progress as number) * 100) }}%</template>
          </span>
        </div>

        <div class="motion-boundary-card">
          <span>Simulation Owner</span>
          <strong>{{ robotProfile.simulationOwner }}</strong>
          <p>{{ robotProfile.viewportRole }}</p>
        </div>

        <div class="motion-snapshot-card">
          <span>MoveIt Snapshot</span>
          <strong>{{ snapshotSourceLabel }}</strong>
          <p>{{ moveitSnapshot.message }}</p>
          <div class="motion-profile-meta">
            <span>{{ moveitSnapshot.freshness.status }}</span>
            <span>{{ moveitSnapshot.freshness.lastUpdated }}</span>
            <span>{{ moveitSnapshot.freshness.sourceLabel }}</span>
          </div>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>Trajectory Preview</h3>
          <span class="pill">{{ snapshotTrajectory.waypointCount }} waypoints</span>
          <span class="pill">{{ snapshotTrajectory.durationSeconds.toFixed(1) }}s</span>
        </div>
        <div class="trajectory-table-wrap">
          <table class="trajectory-table">
            <thead>
              <tr>
                <th>#</th>
                <th v-for="joint in goalJointRows" :key="joint.name">{{ joint.name }}</th>
                <th>time</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td :colspan="trajectoryColumnCount" class="empty-trajectory">
                  {{ snapshotTrajectory.message }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div class="motion-section">
        <div class="motion-section-header">
          <h3>IK Solver</h3>
        </div>
        <div class="ik-row">
          <div class="pose-grid">
            <div class="pose-row">
              <label>Position</label>
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
            </div>
            <div class="pose-row">
              <label>Quaternion</label>
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
              <input type="text" value="0.0" disabled />
              <input type="text" value="1.0" disabled />
            </div>
          </div>
          <div class="ik-action">
            <button disabled>Solve IK</button>
            <span class="ik-result">—</span>
          </div>
        </div>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import {
  BACKEND_BASE_URL,
  getDataflowGraph,
  getDataflows,
  getLiveRecent,
  getMoveitSnapshot,
  getMoveitStatus,
  getRobotProfile,
  postLiveCommand,
  type ApiSource,
  type MoveitSnapshotResponse,
  type MoveitStatusResponse,
  type RobotProfileResponse,
} from '../api'
import { useI18n } from '../i18n'
import {
  buildExecuteCommand,
  buildPlanCommand,
  buildSceneAddCommand,
  buildSceneRemoveCommand,
  extractConsoleStatus,
  parseTargetInputs,
} from '../live-command'
import {
  buildNanoArmModelResources,
  createNanoArmJointState,
  findNanoArmSnapshotJoint,
  seedNanoArmJointStateFromSnapshot,
  NANO_ARM_JOINT_LIMITS,
  NANO_ARM_JOINT_NAMES,
} from '../lib/nanoArmModel'
import { createNanoRobotBasePose } from '../lib/nanoRobotMotion'
import NanoRobotViewer from './NanoRobotViewer.vue'

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

const fallbackMoveitSnapshot: MoveitSnapshotResponse = {
  source: 'frontend fallback',
  message: 'Backend API is not connected; showing a read-only Nano full MuJoCo mirror fallback.',
  robotProfileId: 'nano-full-family',
  robotConfigId: 'nano-full-arm-demo',
  simulationOwner: 'dora-moveit2 / MuJoCo',
  viewportRole: 'Studio mirrors moveit-side simulated state; it does not own simulation.',
  freshness: {
    status: 'fallback',
    lastUpdated: 'frontend fallback frame',
    sourceLabel: 'deterministic frontend fallback',
  },
  joints: [
    { name: 'shoulder_pan', value: 0.22, unit: 'rad', lowerLimit: -2.62, upperLimit: 2.62, status: 'ready', source: 'frontend moveit mirror' },
    { name: 'shoulder_lift', value: -0.48, unit: 'rad', lowerLimit: -1.9, upperLimit: 1.9, status: 'ready', source: 'frontend moveit mirror' },
    { name: 'elbow_flex', value: 0.86, unit: 'rad', lowerLimit: -2.1, upperLimit: 2.1, status: 'ready', source: 'frontend moveit mirror' },
    { name: 'wrist_flex', value: -0.34, unit: 'rad', lowerLimit: -1.8, upperLimit: 1.8, status: 'ready', source: 'frontend moveit mirror' },
    { name: 'wrist_roll', value: 0.18, unit: 'rad', lowerLimit: -3.14, upperLimit: 3.14, status: 'ready', source: 'frontend moveit mirror' },
    { name: 'gripper', value: 0.03, unit: 'rad', lowerLimit: 0, upperLimit: 0.8, status: 'ready', source: 'frontend moveit mirror' },
  ],
  endEffectorPose: {
    frame: 'left_gripper_tip',
    position: [0.32, -0.08, 0.24],
    quaternion: [0, 0.18, 0, 0.98],
    source: 'frontend moveit mirror',
  },
  scene: {
    status: 'ready',
    objectCount: 3,
    objects: [
      { name: 'workbench', shape: 'box', dims: '0.80 x 1.20 x 0.05', dimensions: [0.8, 1.2, 0.05], frame: 'world', status: 'fixed' },
      { name: 'pick_target', shape: 'cylinder', dims: 'r=0.04 h=0.12', dimensions: [0.04, 0.12], frame: 'world', status: 'target' },
      { name: 'safety_zone', shape: 'sphere', dims: 'r=0.18', dimensions: [0.18], frame: 'left_arm_base', status: 'collision guard' },
    ],
  },
  trajectory: {
    status: 'idle',
    waypointCount: 0,
    durationSeconds: 0,
    message: 'No plan requested; Studio is showing read-only mirror state.',
  },
  visualModel: {
    modelId: 'nano-full-mujoco-mirror',
    name: 'Nano full MuJoCo visual mirror',
    format: 'threejs-stl-viewer',
    source: 'dora-moveit2 mirror contract',
    jointOrder: ['shoulder_pan', 'shoulder_lift', 'elbow_flex', 'wrist_flex', 'wrist_roll', 'gripper'],
  },
}

const moveit = ref<MoveitStatusResponse>({
  installed: false, running: false,
  message: 'Backend API is not connected.',
})
const robotProfileData = ref<RobotProfileResponse>(fallbackRobotProfile)
const robotProfileSource = ref<ApiSource>('fallback')
const moveitSnapshotData = ref<MoveitSnapshotResponse>(fallbackMoveitSnapshot)
const moveitSnapshotSource = ref<ApiSource>('fallback')

const robotProfile = computed(() => robotProfileData.value.profile)
const armModules = computed(() => robotProfile.value.modules.filter((module) => module.kind === 'arm'))
const cameraModules = computed(() => robotProfile.value.modules.filter((module) => module.kind === 'camera'))
const optionalModules = computed(() => robotProfile.value.modules.filter((module) => !module.required))
const mappedModuleCount = computed(() => robotProfile.value.modules.filter((module) => module.linkedDisplays.length > 0).length)
const moveitSnapshot = computed(() => moveitSnapshotData.value)
const snapshotJoints = computed(() => moveitSnapshot.value.joints)
const nanoArmResources = buildNanoArmModelResources(BACKEND_BASE_URL)
const nanoArmJointState = reactive(createNanoArmJointState())
const nanoRobotBasePose = createNanoRobotBasePose()
const nanoArmJointOrder = ref([...NANO_ARM_JOINT_NAMES])
const jointStateSeeded = ref(false)
const nanoArmJointControls = computed(() => nanoArmJointOrder.value.map((name) => {
  const joint = findNanoArmSnapshotJoint(snapshotJoints.value, name)
  const limits = NANO_ARM_JOINT_LIMITS[name]

  return {
    name,
    lower: limits.lower,
    upper: limits.upper,
    unit: joint?.unit ?? 'rad',
    status: joint?.status ?? 'local goal',
  }
}))
const snapshotSceneObjects = computed(() => moveitSnapshot.value.scene.objects)
const snapshotTrajectory = computed(() => moveitSnapshot.value.trajectory)
const snapshotPoseRows = computed(() => [
  {
    label: 'XYZ',
    values: moveitSnapshot.value.endEffectorPose.position.map((value) => value.toFixed(2)),
  },
  {
    label: 'Quat',
    values: moveitSnapshot.value.endEffectorPose.quaternion.map((value) => value.toFixed(2)),
  },
])
const snapshotSourceLabel = computed(() => `${moveitSnapshotSource.value} · ${moveitSnapshot.value.source}`)

const goalJointRows = computed(() => nanoArmJointControls.value)
const trajectoryColumnCount = computed(() => goalJointRows.value.length + 2)

function updateNanoArmJointOrder(jointOrder: typeof nanoArmJointOrder.value) {
  nanoArmJointOrder.value = jointOrder.length > 0 ? jointOrder : [...NANO_ARM_JOINT_NAMES]
}

function seedNanoArmJointState() {
  if (jointStateSeeded.value) {
    return
  }

  Object.assign(nanoArmJointState, seedNanoArmJointStateFromSnapshot(moveitSnapshot.value.joints))
  jointStateSeeded.value = true
}

onMounted(async () => {
  const [moveitResult, robotProfileResult, moveitSnapshotResult] = await Promise.all([
    getMoveitStatus({
      installed: false, running: false,
      message: 'Backend API is not connected.',
    }),
    getRobotProfile(fallbackRobotProfile),
    getMoveitSnapshot(fallbackMoveitSnapshot),
  ])

  moveit.value = moveitResult.data
  robotProfileData.value = robotProfileResult.data
  robotProfileSource.value = robotProfileResult.source
  moveitSnapshotData.value = moveitSnapshotResult.data
  moveitSnapshotSource.value = moveitSnapshotResult.source
  seedNanoArmJointState()

  detectPlanners()
  feedTimer = setInterval(() => void pollConsoleFeed(), 500)
})

onBeforeUnmount(() => {
  if (feedTimer !== null) clearInterval(feedTimer)
})

// --- M15 B6: live planning console ---

const { t } = useI18n()
const targetX = ref('0.55')
const targetY = ref('0.20')
const targetZ = ref('0.30')
// Arm-demo default: inside the ur5e workspace at the arm's link plane
// (the real DH kinematics put the zero-config links at z ~ 0.16), so a
// single added box visibly blocks direct paths.
const boxX = ref('0.45')
const boxY = ref('0.15')
const boxZ = ref('0.16')
const selectedPlanner = ref('simple_planner')
const plannerOptions = ref<{ id: string; label: string }[]>([
  { id: 'simple_planner', label: 'simple_planner (A* grid)' },
])
const consoleBusy = ref(false)
const consoleError = ref<string | null>(null)
const lastCommandInfo = ref<string | null>(null)
const modeLabel = ref<'manual' | 'auto'>('manual')
let commandInfoTimer: ReturnType<typeof setTimeout> | null = null
const consoleFeedStatus = ref<'connected' | 'unavailable'>('unavailable')
const livePlanStatus = ref<Record<string, unknown> | null>(null)
const liveExecution = ref<Record<string, unknown> | null>(null)
const addedObjects = ref<string[]>([])
let lastFeedTs = Date.now() * 1_000_000 - 2_000_000_000
let feedTimer: ReturnType<typeof setInterval> | null = null
const mirrorViewer = ref<InstanceType<typeof NanoRobotViewer> | null>(null)

function flashCommandSent(kind: string, seq: number) {
  // Plan disables the orbit demo; Auto enables it (mirrors the
  // costmap_source mode state).
  if (kind === 'plan') modeLabel.value = 'manual'
  if (kind === 'auto') modeLabel.value = 'auto'

  lastCommandInfo.value = `${kind} → ${t.value.motionConsole.sentSeq} ${seq}`
  if (commandInfoTimer !== null) clearTimeout(commandInfoTimer)
  commandInfoTimer = setTimeout(() => { lastCommandInfo.value = null }, 3000)
}

async function sendCommand(kind: 'execute' | 'stop' | 'auto') {
  consoleBusy.value = true
  consoleError.value = null
  try {
    const result = await postLiveCommand(buildExecuteCommand(kind))
    flashCommandSent(kind, result.seq)
  } catch (e) {
    consoleError.value = e instanceof Error ? e.message : t.value.motionConsole.sendFailed
  } finally {
    consoleBusy.value = false
  }
}

async function sendPlan() {
  const target = parseTargetInputs(targetX.value, targetY.value, targetZ.value)
  if (target === null) {
    consoleError.value = t.value.motionConsole.invalidTarget
    return
  }
  consoleBusy.value = true
  consoleError.value = null
  try {
    const result = await postLiveCommand(buildPlanCommand(target, selectedPlanner.value || undefined))
    flashCommandSent('plan', result.seq)
  } catch (e) {
    consoleError.value = e instanceof Error ? e.message : t.value.motionConsole.sendFailed
  } finally {
    consoleBusy.value = false
  }
}

async function addBox() {
  const position = parseTargetInputs(boxX.value, boxY.value, boxZ.value)
  if (position === null) {
    consoleError.value = t.value.motionConsole.invalidTarget
    return
  }
  const name = `box_${addedObjects.value.length + 1}`
  consoleBusy.value = true
  consoleError.value = null
  try {
    const result = await postLiveCommand(buildSceneAddCommand(name, 'box', position, [0.12, 0.12, 0.24]))
    addedObjects.value.push(name)
    flashCommandSent('scene', result.seq)
  } catch (e) {
    consoleError.value = e instanceof Error ? e.message : t.value.motionConsole.sendFailed
  } finally {
    consoleBusy.value = false
  }
}

async function removeLastBox() {
  const name = addedObjects.value.pop()
  if (!name) return
  consoleBusy.value = true
  consoleError.value = null
  try {
    const result = await postLiveCommand(buildSceneRemoveCommand(name))
    flashCommandSent('scene', result.seq)
  } catch (e) {
    consoleError.value = e instanceof Error ? e.message : t.value.motionConsole.sendFailed
  } finally {
    consoleBusy.value = false
  }
}

async function pollConsoleFeed() {
  try {
    const { frames } = await getLiveRecent(lastFeedTs)
    if (frames.length > 0) {
      lastFeedTs = Math.max(...frames.map((f) => f.timestamp))
    }
    const status = extractConsoleStatus(frames)
    if (status.planStatus !== null) livePlanStatus.value = status.planStatus
    if (status.execution !== null) liveExecution.value = status.execution
    consoleFeedStatus.value = 'connected'
  } catch {
    consoleFeedStatus.value = 'unavailable'
  }
}

/** Detect planner nodes from the discovered dataflow graphs so the
 * selector lists what is actually available (fallback: simple_planner). */
async function detectPlanners() {
  try {
    const { data: dataflows } = await getDataflows([])
    for (const dataflow of dataflows) {
      const { data: graph } = await getDataflowGraph(dataflow.id, {
        nodes: [],
        edges: [],
        diagnostics: [],
      })
      for (const node of graph.nodes) {
        if (!/planner|move_group|ompl/i.test(node.id)) continue
        if (plannerOptions.value.some((p) => p.id === node.id)) continue
        plannerOptions.value.push({ id: node.id, label: `${node.id} (${dataflow.name})` })
      }
    }
  } catch {
    // keep the default option; honest empty detection
  }
}
</script>
