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
  nav: Record<'dashboard' | 'explorer' | 'monitor' | 'logs' | 'visualization' | 'motion', NavItemText>
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
    nav: {
      dashboard: { label: '总览面板', section: '系统总览', title: 'Studio 总览面板' },
      explorer: { label: '数据流浏览', section: '结构图', title: '查看 dataflow 结构' },
      monitor: { label: '运行监控', section: '运行时', title: '运行并观测 dataflow' },
      logs: { label: '日志事件', section: '调试', title: '集中查看运行信号' },
      visualization: { label: '3D 可视化', section: '可视化', title: '机器人 3D 可视化视口' },
      motion: { label: '运动规划', section: '运动', title: '运动规划与场景管理' },
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
    nav: {
      dashboard: { label: 'Dashboard', section: 'Overview', title: 'Studio dashboard' },
      explorer: { label: 'Dataflow Explorer', section: 'Graph', title: 'Inspect dataflow structure' },
      monitor: { label: 'Run & Monitor', section: 'Runtime', title: 'Run and observe dataflows' },
      logs: { label: 'Logs & Events', section: 'Debug', title: 'Centralized runtime signals' },
      visualization: { label: 'Visualization', section: 'Visualization', title: '3D robot visualization' },
      motion: { label: 'Motion Planner', section: 'Motion', title: 'Motion planning & scene management' },
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
