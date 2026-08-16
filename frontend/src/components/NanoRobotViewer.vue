<template>
  <div ref="viewportRef" class="nano-robot-viewer" :class="viewerState">
    <canvas ref="canvasRef" class="nano-robot-viewer-canvas" :aria-label="viewerLabel"></canvas>

    <div class="nano-robot-viewer-status" :class="viewerState">
      <span>{{ statusBadge }}</span>
      <strong>{{ viewerMessage }}</strong>
      <small>{{ viewerDetails }}</small>
    </div>

    <div class="nano-robot-viewer-hint">Drag to orbit • Scroll to zoom • Right-drag to pan</div>
  </div>
</template>

<script setup lang="ts">
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { STLLoader } from 'three/examples/jsm/loaders/STLLoader.js'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  NANO_ARM_JOINT_AXES,
  NANO_ARM_JOINT_LIMITS,
  NANO_ARM_JOINT_NAMES,
  parseNanoArmXml,
  resolveNanoArmJointName,
  type NanoArmJointName,
  type NanoArmJointState,
} from '../lib/nanoArmModel'
import type { NanoRobotBasePose } from '../lib/nanoRobotMotion'

type NanoArmMeshAsset = {
  file: string
  scale: THREE.Vector3
}

type NanoArmGeomTransform = {
  position: THREE.Vector3
  quaternion: THREE.Quaternion
}

type NanoArmGeomSpec =
  | ({
      kind: 'mesh'
      meshName: string
      color: THREE.Color
      opacity: number
    } & NanoArmGeomTransform)
  | ({
      kind: 'cylinder'
      radius: number
      halfLength: number
      color: THREE.Color
      opacity: number
    } & NanoArmGeomTransform)
  | ({
      kind: 'sphere'
      radius: number
      color: THREE.Color
      opacity: number
    } & NanoArmGeomTransform)

type NanoArmBodySpec = {
  name: string
  position: THREE.Vector3
  quaternion: THREE.Quaternion
  geoms: NanoArmGeomSpec[]
  joint?: {
    name: NanoArmJointName
    axis: THREE.Vector3
  }
  children: NanoArmBodySpec[]
}

type NanoArmModelSpec = {
  meshAssets: Record<string, NanoArmMeshAsset>
  meshGeometries: Map<string, THREE.BufferGeometry>
  rootBodies: NanoArmBodySpec[]
  jointOrder: NanoArmJointName[]
}

const props = defineProps<{
  xmlUrl: string
  assetBaseUrl: string
  jointValues: NanoArmJointState
  basePose: NanoRobotBasePose
  viewerLabel: string
  /** M13: hide only the robot MODEL while tool-mounted models (B601)
   * take over the viewport — the canvas/scene/camera stay alive for the
   * tools. Hiding the whole viewer would black out the tool rendering. */
  modelVisible?: boolean
}>()

const emit = defineEmits<{
  loaded: [jointOrder: NanoArmJointName[]]
}>()

const viewportRef = ref<HTMLDivElement | null>(null)
const canvasRef = ref<HTMLCanvasElement | null>(null)
const viewerState = ref<'loading' | 'ready' | 'error'>('loading')
const viewerMessage = ref('Loading nano_full.xml from the backend models path...')
const viewerDetails = ref('Waiting for the Nano full robot asset bundle to become available.')

const statusBadge = computed(() => {
  if (viewerState.value === 'ready') {
    return 'ready'
  }

  if (viewerState.value === 'error') {
    return 'error'
  }

  return 'loading'
})

let renderer: THREE.WebGLRenderer | null = null
let scene: THREE.Scene | null = null
let camera: THREE.PerspectiveCamera | null = null
let controls: OrbitControls | null = null
let resizeObserver: ResizeObserver | null = null
let animationFrame = 0
let disposed = false
let modelRoot: THREE.Group | null = null
let jointNodes = new Map<NanoArmJointName, THREE.Group>()
let jointAxes = new Map<NanoArmJointName, THREE.Vector3>()
const stlLoader = new STLLoader()

function normalizeBaseUrl(value: string) {
  return value.replace(/\/+$/, '') + '/'
}

function parseNumberTuple(value: string | null, length: number, fallback: number[]) {
  const numbers = (value ?? '')
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((item) => Number(item))
    .filter((item) => Number.isFinite(item))

  return Array.from({ length }, (_, index) => numbers[index] ?? fallback[index] ?? 0)
}

function parseVector3(value: string | null, fallback: [number, number, number] = [0, 0, 0]) {
  const [x, y, z] = parseNumberTuple(value, 3, fallback)
  return new THREE.Vector3(x, y, z)
}

function parseMuJoCoQuaternion(value: string | null, fallback: [number, number, number, number] = [1, 0, 0, 0]) {
  const [w, x, y, z] = parseNumberTuple(value, 4, fallback)
  return new THREE.Quaternion(x, y, z, w).normalize()
}

function parseEulerQuaternion(value: string | null, fallback: [number, number, number] = [0, 0, 0]) {
  const [x, y, z] = parseNumberTuple(value, 3, fallback)
  return new THREE.Quaternion().setFromEuler(new THREE.Euler(x, y, z, 'XYZ'))
}

function parseElementQuaternion(element: Element) {
  const quat = element.getAttribute('quat')
  if (quat) {
    return parseMuJoCoQuaternion(quat)
  }

  return parseEulerQuaternion(element.getAttribute('euler'))
}

function parseColorAndOpacity(value: string | null, fallback: [number, number, number, number] = [0.75, 0.75, 0.75, 1]) {
  const [r, g, b, a] = parseNumberTuple(value, 4, fallback)
  return {
    color: new THREE.Color(r, g, b),
    opacity: a,
  }
}

function parseMeshAssets(doc: Document) {
  const entries = Array.from(doc.querySelectorAll('asset mesh'))
    .map((meshElement) => {
      const name = meshElement.getAttribute('name')
      const file = meshElement.getAttribute('file')

      if (!name || !file) {
        return null
      }

      return [
        name,
        {
          file,
          scale: parseVector3(meshElement.getAttribute('scale'), [1, 1, 1]),
        },
      ] as const
    })
    .filter((entry): entry is readonly [string, NanoArmMeshAsset] => entry !== null)

  return Object.fromEntries(entries) as Record<string, NanoArmMeshAsset>
}

function parseBodyElement(bodyElement: Element): NanoArmBodySpec {
  const geoms: NanoArmGeomSpec[] = []
  const children: NanoArmBodySpec[] = []
  let joint: NanoArmBodySpec['joint']

  for (const child of Array.from(bodyElement.children)) {
    if (child.tagName === 'joint') {
      const modelJointName = child.getAttribute('name')
      const jointName = modelJointName ? resolveNanoArmJointName(modelJointName) : undefined
      if (jointName) {
        joint = {
          name: jointName,
          axis: parseVector3(child.getAttribute('axis'), NANO_ARM_JOINT_AXES[jointName]).normalize(),
        }
      }
      continue
    }

    if (child.tagName === 'geom') {
      const geomType = child.getAttribute('type')
      const rgba = parseColorAndOpacity(child.getAttribute('rgba'))
      const transform = {
        position: parseVector3(child.getAttribute('pos')),
        quaternion: parseElementQuaternion(child),
      }

      if (geomType === 'mesh') {
        const meshName = child.getAttribute('mesh')
        if (meshName) {
          geoms.push({
            kind: 'mesh',
            meshName,
            color: rgba.color,
            opacity: rgba.opacity,
            ...transform,
          })
        }
      } else if (geomType === 'cylinder') {
        const [radius = 0.04, halfLength = 0.01] = parseNumberTuple(child.getAttribute('size'), 2, [0.04, 0.01])
        geoms.push({
          kind: 'cylinder',
          radius,
          halfLength,
          color: rgba.color,
          opacity: rgba.opacity,
          ...transform,
        })
      } else if (geomType === 'sphere') {
        const [radius = 0.015] = parseNumberTuple(child.getAttribute('size'), 1, [0.015])
        geoms.push({
          kind: 'sphere',
          radius,
          color: rgba.color,
          opacity: rgba.opacity,
          ...transform,
        })
      }
      continue
    }

    if (child.tagName === 'body') {
      children.push(parseBodyElement(child))
    }
  }

  return {
    name: bodyElement.getAttribute('name') ?? 'nano-body',
    position: parseVector3(bodyElement.getAttribute('pos')),
    quaternion: parseElementQuaternion(bodyElement),
    geoms,
    joint,
    children,
  }
}

async function loadNanoArmModel() {
  const response = await fetch(props.xmlUrl)
  if (!response.ok) {
    throw new Error(`Failed to load nano_full.xml (${response.status})`)
  }

  const xmlText = await response.text()
  const parsedXml = parseNanoArmXml(xmlText)
  const xmlDocument = new DOMParser().parseFromString(xmlText, 'application/xml')

  if (xmlDocument.querySelector('parsererror')) {
    throw new Error('nano_full.xml could not be parsed as XML.')
  }

  const meshAssets = parseMeshAssets(xmlDocument)
  const meshGeometries = new Map<string, THREE.BufferGeometry>()
  const assetBaseUrl = normalizeBaseUrl(props.assetBaseUrl)

  await Promise.all(
    Object.entries(meshAssets).map(async ([meshName, meshAsset]) => {
      const geometryUrl = new URL(meshAsset.file, assetBaseUrl).toString()
      const geometry = await stlLoader.loadAsync(geometryUrl)
      geometry.computeVertexNormals()
      geometry.computeBoundingBox()
      meshGeometries.set(meshName, geometry)
    }),
  )

  return {
    meshAssets,
    meshGeometries,
    rootBodies: Array.from(xmlDocument.querySelectorAll('worldbody > body')).map((bodyElement) => parseBodyElement(bodyElement)),
    jointOrder: parsedXml.jointOrder,
  } satisfies NanoArmModelSpec
}

function createMeshMaterial(color: THREE.Color, opacity: number) {
  return new THREE.MeshStandardMaterial({
    color,
    metalness: 0.08,
    roughness: 0.72,
    side: THREE.DoubleSide,
    transparent: opacity < 1,
    opacity,
  })
}

function createPrimitiveMesh(geom: NanoArmGeomSpec) {
  if (geom.kind === 'cylinder') {
    const geometry = new THREE.CylinderGeometry(geom.radius, geom.radius, geom.halfLength * 2, 28)
    geometry.rotateX(Math.PI / 2)
    return new THREE.Mesh(geometry, createMeshMaterial(geom.color, geom.opacity))
  }

  if (geom.kind === 'sphere') {
    const geometry = new THREE.SphereGeometry(geom.radius, 28, 20)
    return new THREE.Mesh(geometry, createMeshMaterial(geom.color, geom.opacity))
  }

  return null
}

function buildGeomObject(geom: NanoArmGeomSpec, model: NanoArmModelSpec) {
  if (geom.kind === 'mesh') {
    const geometry = model.meshGeometries.get(geom.meshName)
    const meshAsset = model.meshAssets[geom.meshName]

    if (!geometry || !meshAsset) {
      throw new Error(`Mesh asset "${geom.meshName}" referenced by nano_full.xml could not be loaded.`)
    }

    const mesh = new THREE.Mesh(geometry, createMeshMaterial(geom.color, geom.opacity))
    mesh.position.copy(geom.position)
    mesh.quaternion.copy(geom.quaternion)
    mesh.scale.copy(meshAsset.scale)
    return mesh
  }

  const primitive = createPrimitiveMesh(geom)
  if (!primitive) {
    throw new Error('Unsupported Nano robot geometry.')
  }

  primitive.position.copy(geom.position)
  primitive.quaternion.copy(geom.quaternion)
  return primitive
}

function buildBodyNode(body: NanoArmBodySpec, parent: THREE.Object3D, model: NanoArmModelSpec) {
  const bodyGroup = new THREE.Group()
  bodyGroup.name = body.name
  bodyGroup.position.copy(body.position)
  bodyGroup.quaternion.copy(body.quaternion)
  parent.add(bodyGroup)

  let attachmentParent: THREE.Object3D = bodyGroup

  if (body.joint) {
    const rotationFrame = new THREE.Group()
    rotationFrame.name = `${body.joint.name}-rotation`
    bodyGroup.add(rotationFrame)

    attachmentParent = rotationFrame
    jointNodes.set(body.joint.name, rotationFrame)
    jointAxes.set(body.joint.name, body.joint.axis.clone().normalize())
  }

  for (const geom of body.geoms) {
    attachmentParent.add(buildGeomObject(geom, model))
  }

  for (const childBody of body.children) {
    buildBodyNode(childBody, attachmentParent, model)
  }
}

function clearModelRoot() {
  if (!scene || !modelRoot) {
    return
  }

  const geometries = new Set<THREE.BufferGeometry>()
  const materials = new Set<THREE.Material>()

  modelRoot.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      geometries.add(object.geometry)
      const meshMaterials = Array.isArray(object.material) ? object.material : [object.material]
      meshMaterials.forEach((material) => materials.add(material))
    }
  })

  scene.remove(modelRoot)
  geometries.forEach((geometry) => geometry.dispose())
  materials.forEach((material) => material.dispose())
  modelRoot = null
  jointNodes = new Map()
  jointAxes = new Map()
}

function syncRendererSize() {
  if (!renderer || !camera || !viewportRef.value) {
    return
  }

  const { clientWidth, clientHeight } = viewportRef.value
  if (clientWidth === 0 || clientHeight === 0) {
    return
  }

  renderer.setSize(clientWidth, clientHeight, false)
  camera.aspect = clientWidth / clientHeight
  camera.updateProjectionMatrix()
  requestRender()
}

function frameCameraToModel() {
  if (!camera || !controls || !modelRoot) {
    return
  }

  modelRoot.updateMatrixWorld(true)

  const bounds = new THREE.Box3().setFromObject(modelRoot)
  if (bounds.isEmpty()) {
    controls.target.set(0, 0, 0.18)
    camera.position.set(0.75, -0.9, 0.65)
    controls.update()
    return
  }

  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  const radius = Math.max(size.x, size.y, size.z, 0.3)

  controls.target.copy(center)
  camera.position.set(center.x + radius * 1.75, center.y - radius * 2.15, center.z + radius * 1.1)
  camera.near = Math.max(0.01, radius / 100)
  camera.far = Math.max(20, radius * 60)
  camera.updateProjectionMatrix()
  controls.update()
}

function applyBasePose() {
  if (!modelRoot) {
    return
  }

  modelRoot.position.set(props.basePose.x, props.basePose.y, 0)
  modelRoot.rotation.set(0, 0, props.basePose.yaw)
}

function applyJointValues() {
  for (const jointName of NANO_ARM_JOINT_NAMES) {
    const jointNode = jointNodes.get(jointName)
    const jointAxis = jointAxes.get(jointName)

    if (jointNode && jointAxis) {
      const limits = NANO_ARM_JOINT_LIMITS[jointName]
      const value = THREE.MathUtils.clamp(props.jointValues[jointName], limits.lower, limits.upper)
      jointNode.quaternion.setFromAxisAngle(jointAxis, value)
    }
  }
}

async function initializeScene() {
  if (!canvasRef.value) {
    throw new Error('Nano robot viewer canvas is not available.')
  }

  renderer = new THREE.WebGLRenderer({
    canvas: canvasRef.value,
    antialias: true,
    alpha: true,
  })
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
  renderer.setClearColor(0x000000, 0)

  scene = new THREE.Scene()
  scene.up.set(0, 0, 1)

  camera = new THREE.PerspectiveCamera(35, 1, 0.01, 50)
  camera.up.set(0, 0, 1)
  camera.position.set(0.75, -0.9, 0.65)

  controls = new OrbitControls(camera, canvasRef.value)
  controls.enableDamping = true
  controls.dampingFactor = 0.08
  controls.enablePan = true
  controls.target.set(0, 0, 0.18)
  controls.update()

  const ambientLight = new THREE.HemisphereLight(0xddeaff, 0x1c2635, 1.5)
  scene.add(ambientLight)

  const keyLight = new THREE.DirectionalLight(0xffffff, 1.8)
  keyLight.position.set(1.6, -1.3, 2.2)
  scene.add(keyLight)

  const fillLight = new THREE.DirectionalLight(0x8cc7ff, 0.6)
  fillLight.position.set(-1.4, 0.8, 1.0)
  scene.add(fillLight)

  const grid = new THREE.GridHelper(1.2, 24, 0x3a4d68, 0x223146)
  grid.rotation.x = Math.PI / 2
  grid.position.z = 0
  scene.add(grid)

  const axes = new THREE.AxesHelper(0.18)
  axes.position.z = 0.001
  scene.add(axes)

  resizeObserver = new ResizeObserver(() => {
    syncRendererSize()
  })

  if (viewportRef.value) {
    resizeObserver.observe(viewportRef.value)
  }

  syncRendererSize()
}

// On-demand rendering: only re-render when something changes (orbit, joint, resize).
// Saves GPU when the viewport is idle.
let renderNeeded = true
let renderScheduled = false

function requestRender() {
  renderNeeded = true
  if (renderScheduled || disposed) return
  renderScheduled = true
  animationFrame = window.requestAnimationFrame(() => {
    renderScheduled = false
    if (disposed || !renderer || !scene || !camera) return
    if (renderNeeded) {
      controls?.update()
      renderer.render(scene, camera)
      renderNeeded = false
    }
    // If still needed (e.g. animation playing), schedule next frame
    if (renderNeeded) {
      renderScheduled = true
      animationFrame = window.requestAnimationFrame(() => {
        renderScheduled = false
        if (!disposed && renderNeeded && renderer && scene && camera) {
          controls?.update()
          renderer.render(scene, camera)
          renderNeeded = false
        }
      })
    }
  })
}

// Keep rendering while user is orbiting
function startContinuousRender() {
  renderNeeded = true
  function step() {
    if (!renderNeeded || disposed) return
    requestRender()
    if (renderNeeded) {
      animationFrame = window.requestAnimationFrame(step)
    }
  }
  step()
}

function stopContinuousRender() {
  renderNeeded = false
}

async function loadAndRenderModel() {
  if (!scene) {
    throw new Error('Nano robot scene has not been initialized.')
  }

  viewerState.value = 'loading'
  viewerMessage.value = 'Loading nano_full.xml from the backend models path...'
  viewerDetails.value = 'Resolving Nano full STL assets and robot joint layout.'

  const model = await loadNanoArmModel()
  if (disposed) {
    return
  }

  clearModelRoot()

  modelRoot = new THREE.Group()
  modelRoot.name = 'nano-full-root'
  modelRoot.visible = props.modelVisible !== false
  scene.add(modelRoot)

  for (const rootBody of model.rootBodies) {
    buildBodyNode(rootBody, modelRoot, model)
  }

  syncRendererSize()
  applyBasePose()
  frameCameraToModel()
  applyJointValues()
  // Hidden mounts (v-show pages) size to 0 and syncRendererSize bails
  // out without requesting a render — ask for one unconditionally so
  // the first visible frame always draws the model.
  requestRender()

  viewerState.value = 'ready'
  viewerMessage.value = 'Loaded nano_full.xml from backend models.'
  viewerDetails.value = `${model.meshGeometries.size} STL meshes • ${model.jointOrder.length} robot joints`
  emit('loaded', model.jointOrder)
}

watch(
  () => props.jointValues,
  () => {
    applyJointValues()
    requestRender()
  },
  { deep: true },
)

watch(
  () => props.basePose,
  () => {
    applyBasePose()
    requestRender()
  },
  { deep: true },
)

watch(
  () => props.modelVisible,
  () => {
    if (modelRoot) modelRoot.visible = props.modelVisible !== false
    requestRender()
  },
)

onMounted(async () => {
  try {
    await initializeScene()
    requestRender()
    await loadAndRenderModel()

    // On-demand rendering: only loop while user interacts with orbit controls
    const canvas = canvasRef.value
    if (canvas) {
      canvas.addEventListener('mousedown', startContinuousRender)
      canvas.addEventListener('touchstart', startContinuousRender)
      canvas.addEventListener('wheel', () => { requestRender(); startContinuousRender() })
      window.addEventListener('mouseup', stopContinuousRender)
      window.addEventListener('touchend', stopContinuousRender)
    }
  } catch (error) {
    viewerState.value = 'error'
    viewerMessage.value = 'Nano full viewer failed to load.'
    viewerDetails.value = error instanceof Error ? error.message : 'Unknown viewer error.'
  }
})

onBeforeUnmount(() => {
  disposed = true
  if (animationFrame !== 0) {
    window.cancelAnimationFrame(animationFrame)
    animationFrame = 0
  }

  resizeObserver?.disconnect()
  resizeObserver = null
  controls?.dispose()
  controls = null

  clearModelRoot()
  renderer?.dispose()
  renderer = null
  scene = null
  camera = null
})

// M11: tool slot support — let the parent mount tools into our scene.
defineExpose({
  getScene: () => scene,
  getCamera: () => camera,
  requestRender,
  // M12 R5: snap the view to a world-space center + radius. Syncs the
  // OrbitControls target so the framing survives the next user drag; falls
  // back to a bare position + lookAt when controls are unavailable.
  focusOn: (center: { x: number; y: number; z: number }, radius: number) => {
    if (!camera) return
    const safeRadius = radius > 0 ? radius : 1
    if (!controls) {
      camera.position.set(
        center.x + safeRadius * 1.75,
        center.y - safeRadius * 2.15,
        center.z + safeRadius * 1.1,
      )
      camera.lookAt(center.x, center.y, center.z)
      requestRender()
      return
    }
    controls.target.set(center.x, center.y, center.z)
    camera.position.set(
      center.x + safeRadius * 1.75,
      center.y - safeRadius * 2.15,
      center.z + safeRadius * 1.1,
    )
    camera.near = Math.max(0.01, safeRadius / 100)
    camera.far = Math.max(20, safeRadius * 60)
    camera.updateProjectionMatrix()
    controls.update()
    requestRender()
  },
})
</script>
