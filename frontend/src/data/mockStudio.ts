export type NodeStatus = 'running' | 'degraded' | 'failed' | 'stopped'
export type LogLevel = 'info' | 'warn' | 'error'

export type StudioNode = {
  id: string
  label: string
  kind: string
  status: NodeStatus
  x: number
  y: number
  inputs: string[]
  outputs: string[]
  cpu: number
  memory: number
  restarts: number
  pending: number
  note: string
}

export type StudioEdge = {
  id: string
  from: string
  to: string
  label: string
}

export type StudioLog = {
  time: string
  node: string
  level: LogLevel
  message: string
}

export type DiagnosticSeverity = 'info' | 'warning' | 'error'

export type StudioDiagnostic = {
  severity: DiagnosticSeverity
  message: string
}

export const systemStatus = {
  coordinator: '已连接',
  daemon: '健康',
  version: 'dora 0.x mock',
  runningDataflows: 2,
  activeNodes: 7,
  errorCount: 1,
}

export const throughputSeries = [42, 58, 51, 72, 88, 81, 96, 112, 104, 132, 124, 148]
export const latencySeries = [34, 32, 41, 45, 48, 62, 58, 73, 69, 80, 76, 84]
export const resourceBars = [
  { label: 'Camera', value: 28 },
  { label: 'Detector', value: 76 },
  { label: 'Planner', value: 38 },
  { label: 'Logger', value: 44 },
]

export const dataflowNodes: StudioNode[] = [
  {
    id: 'camera',
    label: 'camera',
    kind: 'Python 数据源',
    status: 'running',
    x: 70,
    y: 130,
    inputs: ['tick: dora/timer/hz/30'],
    outputs: ['frame'],
    cpu: 18,
    memory: 164,
    restarts: 0,
    pending: 2,
    note: '从模拟机器人相机采集图像帧，是后续感知链路的数据入口。',
  },
  {
    id: 'detector',
    label: 'detector',
    kind: 'Rust 算子',
    status: 'degraded',
    x: 330,
    y: 130,
    inputs: ['frame: camera/frame'],
    outputs: ['boxes', 'debug_image'],
    cpu: 61,
    memory: 512,
    restarts: 1,
    pending: 18,
    note: '执行目标检测；当前输入队列增长，用于展示退化节点的调试入口。',
  },
  {
    id: 'planner',
    label: 'planner',
    kind: 'Python 节点',
    status: 'running',
    x: 590,
    y: 80,
    inputs: ['boxes: detector/boxes'],
    outputs: ['cmd_vel'],
    cpu: 22,
    memory: 210,
    restarts: 0,
    pending: 4,
    note: '将检测结果转换为机器人速度控制指令。',
  },
  {
    id: 'logger',
    label: 'logger',
    kind: '数据记录器',
    status: 'running',
    x: 590,
    y: 230,
    inputs: ['frame: camera/frame', 'boxes: detector/boxes', 'logs: dora/logs/error'],
    outputs: ['dataset_chunk'],
    cpu: 12,
    memory: 340,
    restarts: 0,
    pending: 6,
    note: '预留未来数据采集、回放和训练数据集导出的边界。',
  },
  {
    id: 'robot_bridge',
    label: 'robot_bridge',
    kind: 'ROS2 桥接',
    status: 'stopped',
    x: 850,
    y: 80,
    inputs: ['cmd_vel: planner/cmd_vel'],
    outputs: [],
    cpu: 0,
    memory: 0,
    restarts: 0,
    pending: 0,
    note: '桥接节点在 mock 运行中停止，用于展示非活跃节点和错误定位。',
  },
]

export const dataflowEdges: StudioEdge[] = [
  { id: 'e1', from: 'camera', to: 'detector', label: 'frame' },
  { id: 'e2', from: 'detector', to: 'planner', label: 'boxes' },
  { id: 'e3', from: 'camera', to: 'logger', label: 'frame' },
  { id: 'e4', from: 'detector', to: 'logger', label: 'boxes' },
  { id: 'e5', from: 'planner', to: 'robot_bridge', label: 'cmd_vel' },
]

export const diagnostics: StudioDiagnostic[] = [
  { severity: 'warning', message: 'detector 的 pending queue 高于建议阈值。' },
  { severity: 'info', message: 'logger 被设计为未来数据集导出的边界。' },
  { severity: 'error', message: 'robot_bridge 已停止，cmd_vel 没有到达机器人适配层。' },
]

export const logs: StudioLog[] = [
  { time: '10:21:03', node: 'camera', level: 'info', message: '已发布第 1842 帧，频率 30hz' },
  { time: '10:21:04', node: 'detector', level: 'warn', message: '输入队列达到 18 条 pending message' },
  { time: '10:21:05', node: 'planner', level: 'info', message: '根据 3 个检测结果生成 cmd_vel' },
  { time: '10:21:06', node: 'robot_bridge', level: 'error', message: '桥接节点已停止，输出 cmd_vel 被丢弃' },
  { time: '10:21:07', node: 'logger', level: 'info', message: '写入数据集分片 mock-session-0007' },
]
