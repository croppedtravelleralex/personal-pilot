export interface DashboardStats {
  totalInstances: number
  runningInstances: number
  proxyCount: number
  coreCount: number
  memUsedMB: number
  maxProfileLimit: number
  appVersion: string
  maxConcurrentInstances: number
  remainingSlots: number
}

export interface AccountHealthDailyRow {
  day: string
  site?: string
  challengeRate: number
  successRate: number
  detectorPassRate: number
  sampleN: number
  status: string
}

