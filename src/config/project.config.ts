/**
 * 项目配置文件
 */

export const projectConfig = {
  name: 'personal-pilot',
  shortName: 'personal-pilot',
  description: '本地优先的指纹浏览器工作台，支持本地无限实例',
  primaryColor: 'primary',
}

export interface NavItem {
  name: string
  path: string
  icon: string
}

export interface NavSection {
  title: string
  items: NavItem[]
}

export const navigationConfig: NavSection[] = [
  {
    title: '主菜单',
    items: [
      { name: '控制台', path: '/', icon: 'LayoutDashboard' },
    ],
  },
  {
    title: '指纹浏览器',
    items: [
      { name: '实例列表', path: '/browser/list', icon: 'Monitor' },
      { name: '实例工作台', path: '/browser/synchronizer', icon: 'Layers' },
      { name: '行为录制', path: '/browser/recording', icon: 'Radio' },
      { name: '自动化接口（实验）', path: '/browser/automation', icon: 'Bot' },
      { name: '内核管理', path: '/browser/cores', icon: 'Cpu' },
      { name: '代理池配置', path: '/browser/proxy-pool', icon: 'Globe' },
      { name: '默认书签', path: '/browser/bookmarks', icon: 'Bookmark' },
      { name: '标签管理', path: '/browser/tags', icon: 'Tag' },
      { name: '事件监控', path: '/browser/monitor', icon: 'Activity' },
    ],
  },
  {
    title: '系统维护',
    items: [
      { name: '系统设置', path: '/settings', icon: 'Settings' },
      { name: '使用教程', path: '/system/tutorial', icon: 'BookOpen' },
      { name: '日志查看', path: '/browser/logs', icon: 'FileText' },
      { name: '接口文档', path: '/browser/launch-api', icon: 'BookOpen' },
    ],
  },
]

export const featuresConfig = {
  dashboard: true,
  data: true,
  settings: true,
}

export const uiConfig = {
  pagination: {
    defaultPageSize: 20,
    pageSizeOptions: [10, 20, 50, 100],
  },
  dateFormat: 'YYYY-MM-DD HH:mm:ss',
  locale: 'zh-CN',
}

export default {
  project: projectConfig,
  navigation: navigationConfig,
  features: featuresConfig,
  ui: uiConfig,
}
