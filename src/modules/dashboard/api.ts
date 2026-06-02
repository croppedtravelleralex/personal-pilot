import type { DashboardStats } from './types'
import {
  collectValidationReport,
  generateDesktopCdKeys,
  listEvidenceReports,
  readDashboardStats,
  readLicenseStatus,
  readReleaseSmokeContract,
  reloadDesktopConfig,
} from '../../services/desktop'
import type {
  DesktopEvidenceReportHistory,
  DesktopReleaseSmokeContract,
  DesktopValidationBrowserSignal,
  DesktopValidationReport,
} from '../../types/desktop'

const DEFAULT_UNLIMITED = Number.POSITIVE_INFINITY

export async function fetchDashboardStats(): Promise<DashboardStats> {
  try {
    const [data, licenseStatus] = await Promise.all([
      readDashboardStats(),
      readLicenseStatus().catch(() => null),
    ])
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

export async function fetchReleaseSmokeContract(): Promise<DesktopReleaseSmokeContract | null> {
  try {
    return await readReleaseSmokeContract()
  } catch (e) {
    console.error('fetchReleaseSmokeContract error:', e)
    return null
  }
}

export async function collectDesktopWebViewEvidence(
  browserSignals: DesktopValidationBrowserSignal[],
): Promise<DesktopValidationReport> {
  return collectValidationReport(browserSignals)
}

export async function reloadConfig(): Promise<void> {
  try {
    await reloadDesktopConfig()
  } catch (e) {
    console.error('reloadConfig error:', e)
  }
}

export async function generateCDKeys(count: number): Promise<{ success: boolean, keys: string[], message?: string }> {
  try {
    const keys = await generateDesktopCdKeys(count)
    return { success: true, keys: keys || [] }
  } catch (e: any) {
    return { success: false, keys: [], message: e.message || '生成失败' }
  }
}
