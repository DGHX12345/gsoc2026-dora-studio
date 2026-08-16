import { computed, readonly, ref } from 'vue'

export type Locale = 'zh' | 'en'

type NavItemText = {
  label: string
  section: string
  title: string
}

type Messages = {
  app: {
    prototype: string
    runtimeTitle: string
    runtimeSubtitle: string
    exportReport: string
    currentFile: string
    languageLabel: string
  }
  sections: {
    dora: string
    robot: string
  }
  nav: Record<'dashboard' | 'explorer' | 'monitor' | 'logs' | 'replay' | 'visualization' | 'motion' | 'metrics', NavItemText>
  attribution: {
    title: string
    chains: string
    source: string
    sourceDrec: string
    sourceLerobot: string
    sourceLive: string
    sourceLiveHint: string
    sourceLerobotHint: string
    empty: string
    emptyHint: string
    unparseable: string
    showIn3d: string
    stepFrame: string
    stepPrompt: string
    stepResponse: string
    stepAction: string
    stepExecution: string
    tokens: string
    model: string
    latency: string
    confidence: string
    success: string
    failed: string
    expandText: string
    collapseText: string
    tokenStream: string
    replayStream: string
    noDetail: string
    datasetPath: string
    scan: string
    scanning: string
    episodes: string
    profile: string
    autoDetect: string
    loadEpisode: string
    frames: string
    noImageData: string
    notAvailable: string
    page: string
    of: string
    datasetScanFailed: string
    nanoPreviewNote: string
  }
  tools: {
    title: string
    close: string
    attached: string
    detached: string
    error: string
    attach: string
    detach: string
    recommendation: string
    recommendationHint: string
    empty: string
    categoryVisualization: string
    categoryDiagnostics: string
    categoryPlanning: string
    controls: string
    noControls: string
    dviz: {
      hint: string
      points: string
      alternative: string
      stale: string
      show: string
      hide: string
      snap: string
      target: string
      costmapTitle: string
      costmapVisible: string
      cells: string
      perCell: string
    }
    moveit: {
      model: string
      noModel: string
      state_loading: string
      state_loaded: string
      state_unavailable: string
      player: string
      play: string
      pause: string
      stepBack: string
      stepForward: string
      syncToTimeline: string
      stale: string
      endEffector: string
      joints: string
      collisionScene: string
      showWireframes: string
      collisions: string
      ghosts: string
      status: string
      planOk: string
      planFail: string
      execution: string
      idle: string
    }
  }
  monitoring: {
    title: string
    masterLabel: string
    on: string
    off: string
    nodeMetrics: string
    otelSpans: string
    samples: string
    disabledTitle: string
    disabledHint: string
    enable: string
    statusOff: string
    statusOn: string
  }
  liveFeed: {
    label: string
    off: string
    on: string
    error: string
    hint: string
  }
  motionConsole: {
    feedOn: string
    feedOff: string
    targetLabel: string
    targetX: string
    targetY: string
    targetZ: string
    plannerLabel: string
    planLabel: string
    planOk: string
    planFail: string
    executionLabel: string
    executing: string
    idle: string
    autoHint: string
    addBox: string
    removeBox: string
    invalidTarget: string
    sendFailed: string
    sentSeq: string
    modeLabel: string
    modeManual: string
    modeAuto: string
  }
}

const messages: Record<Locale, Messages> = {
  zh: {
    app: {
      prototype: 'GSoC 2026 原型',
      runtimeTitle: 'Mock 运行时',
      runtimeSubtitle: '已准备接入 API',
      exportReport: '导出演示报告',
      currentFile: 'robot-perception-demo.yml',
      languageLabel: '中文',
    },
    sections: {
      dora: 'Dora',
      robot: 'Robot',
    },
    nav: {
      dashboard: { label: '总览面板', section: '系统总览', title: 'Studio 总览面板' },
      explorer: { label: '数据流浏览', section: '结构图', title: '查看 dataflow 结构' },
      monitor: { label: '运行监控', section: '运行时', title: '运行并观测 dataflow' },
      logs: { label: '日志事件', section: '调试', title: '集中查看运行信号' },
      replay: { label: '录制回放', section: '回放', title: '.drec 录制时间轴回放' },
      metrics: { label: '性能指标', section: '性能', title: '节点性能指标面板' },
      visualization: { label: '3D 可视化', section: '可视化', title: '机器人 3D 可视化视口' },
      motion: { label: '运动规划', section: '运动', title: '运动规划与场景管理' },
    },
    attribution: {
      title: '归因链',
      chains: '条归因链',
      source: '数据源',
      sourceDrec: '.drec 录制',
      sourceLerobot: 'LeRobot 数据集 (M10)',
      sourceLive: 'Live dora VLM 节点',
      sourceLiveHint: '需要运行中的 dataflow',
      sourceLerobotHint: 'M10 模块提供',
      empty: '未检测到 VLM 归因数据',
      emptyHint: '加载包含 VLM 算子输出的 .drec 录制',
      unparseable: '无法解析的流',
      showIn3d: '在 3D 中查看',
      stepFrame: '传感器帧',
      stepPrompt: '提示词',
      stepResponse: 'LLM 回复',
      stepAction: '解析动作',
      stepExecution: '执行结果',
      tokens: 'tokens',
      model: '模型',
      latency: '延迟',
      confidence: '置信度',
      success: '成功',
      failed: '失败',
      expandText: '展开全文',
      collapseText: '收起',
      tokenStream: 'Token 流回放',
      replayStream: '重放 token 流',
      noDetail: '详情加载失败',
      datasetPath: '数据集路径',
      scan: '扫描',
      scanning: '扫描中…',
      episodes: 'episodes',
      profile: '配置',
      autoDetect: '自动检测',
      loadEpisode: '加载 Episode',
      frames: '帧',
      noImageData: '无图像数据',
      notAvailable: '数据集中未记录',
      page: '页',
      of: '/',
      datasetScanFailed: '数据集扫描失败',
      nanoPreviewNote: '该动作在 Nano 模型上预览（暂无 B601 模型，接入见 M13）',
    },
    tools: {
      title: '工具',
      close: '关闭',
      attached: '已挂载',
      detached: '未挂载',
      error: '错误',
      attach: '挂载',
      detach: '卸载',
      recommendation: '推荐',
      recommendationHint: '检测到匹配的数据流端口，可挂载此工具',
      empty: '暂无已注册工具',
      categoryVisualization: '可视化',
      categoryDiagnostics: '诊断',
      categoryPlanning: '规划',
      controls: '控制面板',
      noControls: '该工具暂无可配置项（控制面板随后续版本提供）',
      dviz: {
        hint: '暂无路径数据——加载工具演示 .drec 并开始回放',
        points: '个点',
        alternative: '备选',
        stale: '过期',
        show: '显示',
        hide: '隐藏',
        snap: '聚焦',
        target: '目标点',
        costmapTitle: '代价地图',
        costmapVisible: '显示',
        cells: '格',
        perCell: 'm/格',
      },
      moveit: {
        model: '机器人模型',
        noModel: '无模型（坐标图回退）',
        state_loading: '加载中',
        state_loaded: '已加载',
        state_unavailable: '不可用',
        player: '轨迹播放器',
        play: '播放',
        pause: '暂停',
        stepBack: '上一路径点',
        stepForward: '下一路径点',
        syncToTimeline: '同步时间轴',
        stale: '数据过期',
        endEffector: '末端执行器位置',
        joints: '关节角度表',
        collisionScene: '碰撞场景',
        showWireframes: '显示线框',
        collisions: '碰撞对',
        ghosts: '幽灵姿态数',
        status: '规划与执行状态',
        planOk: '规划成功',
        planFail: '规划失败',
        execution: '执行进度',
        idle: '空闲',
      },
    },
    monitoring: {
      title: '监测',
      masterLabel: '主开关',
      on: '开',
      off: '关',
      nodeMetrics: '节点指标',
      otelSpans: 'OTel 火焰图',
      samples: '样本',
      disabledTitle: '监测已关闭',
      disabledHint: '监测按需开启——开启后每 2 秒采集一次节点指标，不开启零开销',
      enable: '一键开启',
      statusOff: '已关闭',
      statusOn: '采集中',
    },
    liveFeed: {
      label: '实时数据',
      off: '已关闭',
      on: '接收中',
      error: '后端不可达',
      hint: '通过 studio_bridge 节点接收运行中 dataflow 的实时端口数据',
    },
    motionConsole: {
      feedOn: '实时状态已连接',
      feedOff: '后端不可达',
      targetLabel: '目标位置 (x, y, z)',
      targetX: 'x 坐标',
      targetY: 'y 坐标',
      targetZ: 'z 坐标',
      plannerLabel: '规划器',
      planLabel: '规划',
      planOk: '成功',
      planFail: '失败',
      executionLabel: '执行',
      executing: '执行中',
      idle: '空闲',
      autoHint: '恢复自动目标（轨道演示）',
      addBox: '添加箱子',
      removeBox: '移除箱子',
      invalidTarget: '目标坐标无效：请输入有限数字',
      sendFailed: '命令发送失败',
      sentSeq: '已发送，序号',
      modeLabel: '模式',
      modeManual: '手动目标',
      modeAuto: '自动轨道',
    },
  },
  en: {
    app: {
      prototype: 'GSoC 2026 prototype',
      runtimeTitle: 'Mock runtime',
      runtimeSubtitle: 'Ready for API integration',
      exportReport: 'Export report',
      currentFile: 'robot-perception-demo.yml',
      languageLabel: 'English',
    },
    sections: {
      dora: 'Dora',
      robot: 'Robot',
    },
    nav: {
      dashboard: { label: 'Dashboard', section: 'Overview', title: 'Studio dashboard' },
      explorer: { label: 'Dataflow Explorer', section: 'Graph', title: 'Inspect dataflow structure' },
      monitor: { label: 'Run & Monitor', section: 'Runtime', title: 'Run and observe dataflows' },
      logs: { label: 'Logs & Events', section: 'Debug', title: 'Centralized runtime signals' },
      replay: { label: 'Replay', section: 'Replay', title: '.drec recording timeline replay' },
      metrics: { label: 'Performance', section: 'Performance', title: 'Node performance metrics' },
      visualization: { label: 'Visualization', section: 'Visualization', title: '3D robot visualization' },
      motion: { label: 'Motion Planner', section: 'Motion', title: 'Motion planning & scene management' },
    },
    attribution: {
      title: 'Attribution',
      chains: 'chains',
      source: 'Source',
      sourceDrec: '.drec recording',
      sourceLerobot: 'LeRobot dataset (M10)',
      sourceLive: 'Live dora VLM node',
      sourceLiveHint: 'requires a running dataflow',
      sourceLerobotHint: 'provided by M10',
      empty: 'No VLM data detected',
      emptyHint: 'Load a .drec recording with VLM operator output',
      unparseable: 'Unparseable streams',
      showIn3d: 'Show in 3D',
      stepFrame: 'Sensor frame',
      stepPrompt: 'Prompt',
      stepResponse: 'LLM response',
      stepAction: 'Parsed action',
      stepExecution: 'Execution',
      tokens: 'tokens',
      model: 'Model',
      latency: 'Latency',
      confidence: 'Confidence',
      success: 'Succeeded',
      failed: 'Failed',
      expandText: 'Expand text',
      collapseText: 'Collapse',
      tokenStream: 'Token stream',
      replayStream: 'Replay token stream',
      noDetail: 'Failed to load detail',
      datasetPath: 'Dataset path',
      scan: 'Scan',
      scanning: 'Scanning…',
      episodes: 'episodes',
      profile: 'Profile',
      autoDetect: 'Auto-detect',
      loadEpisode: 'Load episode',
      frames: 'frames',
      noImageData: 'No image data',
      notAvailable: 'Not available in this dataset',
      page: 'Page',
      of: 'of',
      datasetScanFailed: 'Dataset scan failed',
      nanoPreviewNote: 'Previewed on the Nano model (no B601 model yet; see M13)',
    },
    tools: {
      title: 'Tools',
      close: 'Close',
      attached: 'attached',
      detached: 'detached',
      error: 'error',
      attach: 'Attach',
      detach: 'Detach',
      recommendation: 'Recommended',
      recommendationHint: 'A matching dataflow port was detected',
      empty: 'No tools registered',
      categoryVisualization: 'Visualization',
      categoryDiagnostics: 'Diagnostics',
      categoryPlanning: 'Planning',
      controls: 'Control panel',
      noControls: 'This tool has no controls yet (panel ships in a later milestone)',
      dviz: {
        hint: 'No path data yet — load the tool demo .drec and start replay',
        points: 'points',
        alternative: 'alternative',
        stale: 'stale',
        show: 'Show',
        hide: 'Hide',
        snap: 'Focus',
        target: 'Target',
        costmapTitle: 'Costmap',
        costmapVisible: 'Show',
        cells: 'cells',
        perCell: 'm/cell',
      },
      moveit: {
        model: 'Robot model',
        noModel: 'No model (chart fallback)',
        state_loading: 'loading',
        state_loaded: 'loaded',
        state_unavailable: 'unavailable',
        player: 'Trajectory player',
        play: 'Play',
        pause: 'Pause',
        stepBack: 'Previous waypoint',
        stepForward: 'Next waypoint',
        syncToTimeline: 'Sync to timeline',
        stale: 'stale data',
        endEffector: 'End effector position',
        joints: 'Joint angle table',
        collisionScene: 'Collision scene',
        showWireframes: 'Show wireframes',
        collisions: 'Collision pairs',
        ghosts: 'Ghost pose count',
        status: 'Plan & execution status',
        planOk: 'plan succeeded',
        planFail: 'plan failed',
        execution: 'Execution progress',
        idle: 'idle',
      },
    },
    monitoring: {
      title: 'Monitoring',
      masterLabel: 'Master switch',
      on: 'On',
      off: 'Off',
      nodeMetrics: 'Node metrics',
      otelSpans: 'OTel spans',
      samples: 'samples',
      disabledTitle: 'Monitoring is off',
      disabledHint: 'Monitoring is opt-in — no polling runs until you enable it',
      enable: 'Enable',
      statusOff: 'off',
      statusOn: 'collecting',
    },
    liveFeed: {
      label: 'Live Feed',
      off: 'off',
      on: 'receiving',
      error: 'backend unreachable',
      hint: 'Receive live port data from a running dataflow via the studio_bridge node',
    },
    motionConsole: {
      feedOn: 'live feed connected',
      feedOff: 'backend unreachable',
      targetLabel: 'Target position (x, y, z)',
      targetX: 'x coordinate',
      targetY: 'y coordinate',
      targetZ: 'z coordinate',
      plannerLabel: 'Planner',
      planLabel: 'Plan',
      planOk: 'ok',
      planFail: 'failed',
      executionLabel: 'Execution',
      executing: 'executing',
      idle: 'idle',
      autoHint: 'Resume the automatic orbit target',
      addBox: 'Add Box',
      removeBox: 'Remove Box',
      invalidTarget: 'Invalid target: enter finite numbers',
      sendFailed: 'Command failed to send',
      sentSeq: 'sent, seq',
      modeLabel: 'Mode',
      modeManual: 'manual goal',
      modeAuto: 'auto orbit',
    },
  },
}

const locale = ref<Locale>('zh')
const t = computed(() => messages[locale.value])

export function useI18n() {
  function toggleLocale() {
    locale.value = locale.value === 'zh' ? 'en' : 'zh'
  }

  return {
    locale: readonly(locale),
    t,
    toggleLocale,
  }
}
