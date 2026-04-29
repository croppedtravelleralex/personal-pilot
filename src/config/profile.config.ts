import { projectConfig } from './project.config'

export type ProfileIconKey =
  | 'book-open'
  | 'globe'
  | 'message-square'
  | 'github'
  | 'mail'
  | 'external-link'

export interface ProfileChannelConfig {
  name: string
  description: string
  detail: string
  href?: string
  icon?: ProfileIconKey
}

export interface AuthorProfileConfig {
  name: string
  initial: string
  title: string
  bio: string
  location: string
  joinDate: string
  email: string
  website: string
  github: string
  skills: string[]
  channels: ProfileChannelConfig[]
}

export interface ProjectProfileActionConfig {
  label: string
  href: string
  icon: ProfileIconKey
}

export interface ProjectProfileConfig {
  name: string
  introBadge: string
  introText: string
  techStack: string[]
  description: string
  actions: ProjectProfileActionConfig[]
}

export interface RemoteAuthorSourceConfig {
  authorURL: string
  timeoutMs: number
}

export interface ProfilePageLocalConfig {
  remoteAuthor: RemoteAuthorSourceConfig
  defaultAuthor: AuthorProfileConfig
  project: ProjectProfileConfig
}

export const profilePageConfig: ProfilePageLocalConfig = {
  remoteAuthor: {
    authorURL: '',
    timeoutMs: 1000,
  },
  defaultAuthor: {
    name: 'personal-pilot',
    initial: 'P',
    title: 'Local Desktop Workspace',
    bio: 'A local-first browser workspace with explicit core selection and unlimited local instances.',
    location: 'Local Machine',
    joinDate: '2026',
    email: 'support@personal-pilot.local',
    website: '',
    github: '',
    skills: ['Wails', 'React', 'TypeScript'],
    channels: [
      {
        name: 'Local Runtime',
        description: 'Runs entirely on your machine',
        detail: 'No upstream account binding',
        icon: 'globe',
      },
    ],
  },
  project: {
    name: projectConfig.name,
    introBadge: projectConfig.name,
    introText: '用于多账号隔离、代理绑定和本地环境管理的桌面浏览器工具。',
    techStack: ['Wails', 'React', 'TypeScript'],
    description: '当前版本聚焦本地实例管理、代理池、可选浏览器内核和快速启动能力。',
    actions: [],
  },
}

export default profilePageConfig
