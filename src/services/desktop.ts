import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'

type InvokeArgs = Record<string, unknown> | undefined

export interface DesktopInvokeOptions {
  timeoutMs?: number
}

export interface CoreStatus {
  running: boolean
  eventUrl?: string | null
  bridgeUrl?: string | null
  pid?: number | null
}

export interface DesktopEnvironment {
  buildType: string
  platform: string
  arch: string
}

export type DesktopUnlisten = () => void
export type DesktopEventHandler<T> = (payload: T) => void

const DEFAULT_TIMEOUT_MS = 10000

export class DesktopInvokeError extends Error {
  command: string
  causeRaw: unknown

  constructor(command: string, causeRaw: unknown) {
    super(normalizeDesktopError(command, causeRaw))
    this.name = 'DesktopInvokeError'
    this.command = command
    this.causeRaw = causeRaw
  }
}

function normalizeDesktopError(command: string, causeRaw: unknown): string {
  if (causeRaw instanceof Error) {
    return `${command}: ${causeRaw.message}`
  }
  if (typeof causeRaw === 'string') {
    return `${command}: ${causeRaw}`
  }
  return `${command}: desktop command failed`
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null
  const timeoutPromise = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`timeout after ${timeoutMs}ms`))
    }, timeoutMs)
  })
  try {
    return await Promise.race([promise, timeoutPromise])
  } finally {
    if (timer) clearTimeout(timer)
  }
}

export async function desktopInvoke<T>(
  command: string,
  args?: InvokeArgs,
  options?: DesktopInvokeOptions,
): Promise<T> {
  const timeoutMs = options?.timeoutMs ?? DEFAULT_TIMEOUT_MS
  try {
    return await withTimeout(invoke<T>(command, args), timeoutMs)
  } catch (error) {
    throw new DesktopInvokeError(command, error)
  }
}

export function desktopListen<T>(
  eventName: string,
  handler: DesktopEventHandler<T>,
): Promise<DesktopUnlisten> {
  return listen<T>(eventName, (event) => handler(event.payload))
}

export function desktopCoreStart(): Promise<CoreStatus> {
  return desktopInvoke<CoreStatus>('core_start', undefined, { timeoutMs: 30000 })
}

export function desktopCoreStatus(): Promise<CoreStatus> {
  return desktopInvoke<CoreStatus>('core_status')
}

export function desktopCoreStop(): Promise<void> {
  return desktopInvoke<void>('core_stop', undefined, { timeoutMs: 15000 })
}

export function desktopRpc<T>(
  method: string,
  args: unknown[] = [],
  options?: DesktopInvokeOptions,
): Promise<T> {
  return desktopInvoke<T>('core_rpc', { method, args }, options ?? { timeoutMs: 120000 })
}

export function desktopEnvironment(): Promise<DesktopEnvironment> {
  return desktopInvoke<DesktopEnvironment>('app_environment')
}

export function desktopWindowHide(): Promise<void> {
  return desktopInvoke<void>('app_window_hide')
}

export function desktopWindowShow(): Promise<void> {
  return desktopInvoke<void>('app_window_show')
}

export function desktopWindowMinimize(): Promise<void> {
  return desktopInvoke<void>('app_window_minimize')
}

export function desktopQuit(): Promise<void> {
  return desktopInvoke<void>('app_quit', undefined, { timeoutMs: 15000 })
}

export function desktopQuitAppOnly(): Promise<void> {
  return desktopInvoke<void>('app_quit_app_only', undefined, { timeoutMs: 30000 })
}

export function desktopQuitFull(): Promise<void> {
  return desktopInvoke<void>('app_quit_full', undefined, { timeoutMs: 30000 })
}

export async function desktopSaveBackupPath(defaultFilename: string): Promise<string | null> {
  const selected = await save({
    title: '导出配置',
    defaultPath: defaultFilename,
    filters: [{ name: 'ZIP 文件', extensions: ['zip'] }],
  })
  return typeof selected === 'string' && selected.trim() ? selected : null
}

export async function desktopOpenBackupPath(): Promise<string | null> {
  const selected = await open({
    title: '加载配置',
    multiple: false,
    filters: [{ name: 'ZIP 文件', extensions: ['zip'] }],
  })
  if (Array.isArray(selected)) {
    const first = selected[0]
    return typeof first === 'string' && first.trim() ? first : null
  }
  return typeof selected === 'string' && selected.trim() ? selected : null
}

export const desktop = {
  coreStart: desktopCoreStart,
  coreStatus: desktopCoreStatus,
  coreStop: desktopCoreStop,
  environment: desktopEnvironment,
  invoke: desktopInvoke,
  listen: desktopListen,
  openBackupPath: desktopOpenBackupPath,
  quit: desktopQuit,
  quitAppOnly: desktopQuitAppOnly,
  quitFull: desktopQuitFull,
  rpc: desktopRpc,
  saveBackupPath: desktopSaveBackupPath,
  windowHide: desktopWindowHide,
  windowMinimize: desktopWindowMinimize,
  windowShow: desktopWindowShow,
}
