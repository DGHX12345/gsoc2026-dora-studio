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
