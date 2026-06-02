import profilePageConfig from '../../config/profile.config'
import { fetchRemoteAuthorProfileFromDesktop } from '../../services/desktop'
import type { AuthorProfile, IconKey, ProfileChannel, ProfilePageData, ProfileProject } from './types'

const PROFILE_ICON_KEYS: IconKey[] = [
  'book-open',
  'globe',
  'message-square',
  'github',
  'mail',
  'external-link',
]

const CHANNEL_ICON_BY_NAME: Record<string, IconKey> = {
  掘金: 'book-open',
  个人博客: 'globe',
  博客: 'globe',
  公众号: 'message-square',
  微信公众号: 'message-square',
  github: 'github',
  邮件: 'mail',
}

export function createDefaultProfilePageData(): ProfilePageData {
  return {
    author: cloneAuthor(profilePageConfig.defaultAuthor),
    project: cloneProject(profilePageConfig.project),
    meta: {
      source: 'default',
    },
  }
}

export async function loadProfilePageData(): Promise<ProfilePageData> {
  const defaultData = createDefaultProfilePageData()
  const authorURL = profilePageConfig.remoteAuthor.authorURL.trim()
  const timeoutMs = profilePageConfig.remoteAuthor.timeoutMs

  if (!authorURL) {
    return defaultData
  }

  try {
    const payload = await fetchRemoteAuthorPayload(authorURL, timeoutMs)
    return {
      author: normalizeAuthorProfile(payload, defaultData.author),
      project: defaultData.project,
      meta: {
        source: 'remote',
      },
    }
  } catch (error: unknown) {
    return {
      ...defaultData,
      meta: {
        source: 'default',
        message: errorMessage(error) || '远程作者配置不可用，已切换为默认资料。',
      },
    }
  }
}

async function fetchRemoteAuthorPayload(authorURL: string, timeoutMs: number): Promise<Record<string, unknown>> {
  try {
    const payload = await fetchRemoteAuthorProfileFromDesktop(authorURL, timeoutMs)
    return isRecord(payload) ? payload : {}
  } catch {
    return await fetchRemoteAuthorPayloadViaBrowser(authorURL, timeoutMs)
  }
}

async function fetchRemoteAuthorPayloadViaBrowser(authorURL: string, timeoutMs: number): Promise<Record<string, unknown>> {
  const controller = new AbortController()
  const timer = window.setTimeout(() => controller.abort(), clampTimeout(timeoutMs))

  try {
    const response = await fetch(authorURL, {
      method: 'GET',
      headers: {
        Accept: 'application/json',
      },
      signal: controller.signal,
    })

    if (!response.ok) {
      throw new Error(`远程作者配置返回异常状态码: ${response.status}`)
    }

    const payload = await response.json()
    if (!isRecord(payload)) {
      throw new Error('远程作者配置格式无效')
    }

    return payload
  } catch (error: unknown) {
    if (isAbortError(error)) {
      throw new Error('远程作者配置请求超时')
    }
    throw error
  } finally {
    window.clearTimeout(timer)
  }
}

function normalizeAuthorProfile(payload: Record<string, unknown>, fallback: AuthorProfile): AuthorProfile {
  const source = extractAuthorPayload(payload)
  const name = normalizeString(source.name, fallback.name)
  const initial = normalizeString(source.initial, name.charAt(0) || fallback.initial).charAt(0) || fallback.initial

  return {
    name,
    initial,
    title: normalizeString(source.title, fallback.title),
    bio: normalizeString(source.bio, fallback.bio),
    location: normalizeString(source.location, fallback.location),
    joinDate: normalizeString(source.joinDate, fallback.joinDate),
    email: normalizeString(source.email, fallback.email),
    website: normalizeString(source.website, fallback.website),
    github: normalizeString(source.github, fallback.github),
    skills: normalizeStringArray(source.skills, fallback.skills),
    channels: normalizeChannels(source.channels, fallback.channels),
  }
}

function normalizeChannels(value: unknown, fallback: ProfileChannel[]): ProfileChannel[] {
  if (!Array.isArray(value)) {
    return cloneChannels(fallback)
  }

  const channels = value
    .map((item) => normalizeChannel(item))
    .filter((item): item is ProfileChannel => !!item)

  return channels.length > 0 ? channels : cloneChannels(fallback)
}

function normalizeChannel(value: unknown): ProfileChannel | null {
  if (!isRecord(value)) {
    return null
  }

  const name = normalizeString(value.name)
  if (!name) {
    return null
  }

  const href = normalizeOptionalString(value.href ?? value.url)
  const detail = normalizeString(value.detail ?? value.value, href ? stripProtocol(href) : '')

  return {
    name,
    description: normalizeString(value.description),
    detail,
    href,
    icon: normalizeIconKey(value.icon, name),
  }
}

function normalizeIconKey(value: unknown, name: string): IconKey | undefined {
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase() as IconKey
    if (PROFILE_ICON_KEYS.includes(normalized)) {
      return normalized
    }
  }

  return CHANNEL_ICON_BY_NAME[name] || CHANNEL_ICON_BY_NAME[name.toLowerCase()]
}

function extractAuthorPayload(payload: Record<string, unknown>): Record<string, unknown> {
  if (isRecord(payload.author)) {
    return payload.author
  }
  return payload
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function isAbortError(error: unknown): boolean {
  if (error instanceof DOMException) {
    return error.name === 'AbortError'
  }
  return isRecord(error) && error.name === 'AbortError'
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  if (isRecord(error) && typeof error.message === 'string') {
    return error.message
  }
  return ''
}

function normalizeString(value: unknown, fallback = ''): string {
  if (typeof value !== 'string') {
    return fallback
  }

  const trimmed = value.trim()
  return trimmed || fallback
}

function normalizeOptionalString(value: unknown): string | undefined {
  if (typeof value !== 'string') {
    return undefined
  }

  const trimmed = value.trim()
  return trimmed || undefined
}

function normalizeStringArray(value: unknown, fallback: string[]): string[] {
  if (!Array.isArray(value)) {
    return [...fallback]
  }

  const items = value
    .map((item) => normalizeString(item))
    .filter(Boolean)

  return items.length > 0 ? items : [...fallback]
}

function stripProtocol(value: string): string {
  return value.replace(/^https?:\/\//, '').replace(/\/$/, '')
}

function clampTimeout(timeoutMs: number): number {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    return 3000
  }
  return Math.min(timeoutMs, 15000)
}

function cloneAuthor(author: AuthorProfile): AuthorProfile {
  return {
    ...author,
    skills: [...author.skills],
    channels: cloneChannels(author.channels),
  }
}

function cloneChannels(channels: ProfileChannel[]): ProfileChannel[] {
  return channels.map((channel) => ({ ...channel }))
}

function cloneProject(project: ProfileProject): ProfileProject {
  return {
    ...project,
    techStack: [...project.techStack],
    actions: project.actions.map((action) => ({ ...action })),
  }
}
