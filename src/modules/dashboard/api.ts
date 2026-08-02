import type { AccountHealthDailyRow, DashboardStats } from './types'
import { messageFromUnknownError } from '../../shared/errors'
import {
  collectValidationReport,
  desktopRpc,
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
    const budget = data?.budget

    return {
      totalInstances: data?.totalInstances ?? 0,
      runningInstances: data?.runningInstances ?? 0,
      proxyCount: data?.proxyCount ?? 0,
      coreCount: data?.coreCount ?? 0,
      memUsedMB: data?.memUsedMB ?? 0,
      maxProfileLimit,
      appVersion: data?.appVersion ?? 'unknown',
      maxConcurrentInstances: budget?.maxConcurrentInstances ?? 0,
      remainingSlots: budget?.remainingSlots ?? -1,
    }
  } catch (error: unknown) {
    console.error('fetchDashboardStats error:', error)
  }

  return {
    totalInstances: 0,
    runningInstances: 0,
    proxyCount: 0,
    coreCount: 0,
    memUsedMB: 0,
    maxProfileLimit: DEFAULT_UNLIMITED,
    appVersion: 'unknown',
    maxConcurrentInstances: 0,
    remainingSlots: -1,
  }
}

export async function fetchAccountHealthTrend(profileId: string, windowDays = 7): Promise<AccountHealthDailyRow[]> {
  if (!profileId.trim()) {
    return []
  }
  try {
    const rows = await desktopRpc<AccountHealthDailyRow[]>('AccountHealthTrend', [profileId, 'challenge_rate', windowDays])
    return Array.isArray(rows) ? rows : []
  } catch (error: unknown) {
    console.error('fetchAccountHealthTrend error:', error)
    return [{
      day: new Date().toISOString().slice(0, 10),
      challengeRate: 0,
      successRate: 0,
      detectorPassRate: 0,
      sampleN: 0,
      status: 'insufficient_data',
    }]
  }
}

export async function fetchEvidenceReportHistory(): Promise<DesktopEvidenceReportHistory> {
  try {
    return await listEvidenceReports()
  } catch (error: unknown) {
    console.error('fetchEvidenceReportHistory error:', error)
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
  } catch (error: unknown) {
    console.error('fetchReleaseSmokeContract error:', error)
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
  } catch (error: unknown) {
    console.error('reloadConfig error:', error)
  }
}

export async function generateCDKeys(count: number): Promise<{ success: boolean, keys: string[], message?: string }> {
  try {
    const keys = await generateDesktopCdKeys(count)
    return { success: true, keys: keys || [] }
  } catch (error: unknown) {
    return { success: false, keys: [], message: messageFromUnknownError(error, '生成失败') }
  }
}
