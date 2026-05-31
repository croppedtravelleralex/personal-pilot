import type { DashboardStats } from './types'
import { listEvidenceReports } from '../../services/desktop'
import type { DesktopEvidenceReportHistory } from '../../types/desktop'

const getBindings = async () => {
  try {
    return await import('../../wailsjs/go/main/App')
  } catch {
    return null
  }
}

const DEFAULT_UNLIMITED = Number.POSITIVE_INFINITY

export async function fetchDashboardStats(): Promise<DashboardStats> {
  const bindings: any = await getBindings()
  if (bindings?.GetDashboardStats) {
    try {
      const data = await bindings.GetDashboardStats()
      const licenseStatus = bindings.GetLicenseStatus ? await bindings.GetLicenseStatus() : null
      const rawLimit = Number(licenseStatus?.maxLimit ?? 0)
      const maxProfileLimit = rawLimit > 0 ? rawLimit : DEFAULT_UNLIMITED

      return {
        totalInstances: data?.totalInstances ?? 0,
        runningInstances: data?.runningInstances ?? 0,
        proxyCount: data?.proxyCount ?? 0,
        coreCount: data?.coreCount ?? 0,
        memUsedMB: data?.memUsedMB ?? 0,
        maxProfileLimit,
        appVersion: data?.appVersion ?? 'unknown',
      }
    } catch (e) {
      console.error('fetchDashboardStats error:', e)
    }
  }

  return {
    totalInstances: 0,
    runningInstances: 0,
    proxyCount: 0,
    coreCount: 0,
    memUsedMB: 0,
    maxProfileLimit: DEFAULT_UNLIMITED,
    appVersion: 'unknown',
  }
}

export async function fetchEvidenceReportHistory(): Promise<DesktopEvidenceReportHistory> {
  try {
    return await listEvidenceReports()
  } catch (e) {
    console.error('fetchEvidenceReportHistory error:', e)
    return {
      generatedAt: new Date().toISOString(),
      reportCount: 0,
      reports: [],
      summary: 'Evidence report history unavailable',
    }
  }
}

export async function reloadConfig(): Promise<void> {
  const bindings: any = await getBindings()
  if (bindings?.ReloadConfig) {
    try {
      await bindings.ReloadConfig()
    } catch (e) {
      console.error('reloadConfig error:', e)
    }
  }
}

export async function generateCDKeys(count: number): Promise<{ success: boolean, keys: string[], message?: string }> {
  const bindings: any = await getBindings()
  if (bindings?.GenerateCDKeys) {
    try {
      const keys = await bindings.GenerateCDKeys(count)
      return { success: true, keys: keys || [] }
    } catch (e: any) {
      return { success: false, keys: [], message: e.message || '生成失败' }
    }
  }
  return { success: false, keys: [], message: '系统 API 未就绪' }
}
