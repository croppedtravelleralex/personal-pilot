// Settings 模块 API
import {
  exportSystemConfig as exportSystemConfigFromDesktop,
  importSystemConfig as importSystemConfigFromDesktop,
  initializeSystemData as initializeSystemDataFromDesktop,
} from '../../services/desktop'
import type { DesktopBackupActionResult } from '../../types/desktop'
import type { AppSettings } from './types'
import { defaultSettings } from './types'

// 本地存储 key
const SETTINGS_KEY = 'app_settings'

export type BackupActionResult = DesktopBackupActionResult

// 获取设置
export async function fetchSettings(): Promise<AppSettings> {
  try {
    const stored = localStorage.getItem(SETTINGS_KEY)
    if (stored) {
      return { ...defaultSettings, ...JSON.parse(stored) }
    }
  } catch (error) {
    console.error('Failed to load settings:', error)
  }
  return defaultSettings
}

// 保存设置
export async function saveSettings(settings: AppSettings): Promise<boolean> {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
    return true
  } catch (error) {
    console.error('Failed to save settings:', error)
    return false
  }
}

// 重置设置
export async function resetSettings(): Promise<AppSettings> {
  localStorage.removeItem(SETTINGS_KEY)
  return defaultSettings
}

export async function initializeSystemData(): Promise<BackupActionResult> {
  return (await initializeSystemDataFromDesktop()) || {}
}

export async function exportSystemConfig(): Promise<BackupActionResult> {
  return (await exportSystemConfigFromDesktop()) || {}
}

export async function importSystemConfig(resetFirst: boolean): Promise<BackupActionResult> {
  return (await importSystemConfigFromDesktop(resetFirst)) || {}
}
