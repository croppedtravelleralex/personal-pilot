export type ProxyHealthState =
  | "healthy"
  | "warning"
  | "failed"
  | "unknown"
  | "queued"
  | "checking";

export type ProxyProtocol = "http" | "https" | "socks5" | string;

export type ProxySource = string;

export type ProxyUsageFilter = "all" | "used" | "unused" | "active";

export type ProxyBatchScope = "filtered" | "selected";

export type ProxySortField = "updated" | "health" | "usage" | "name";

export type ProxyDataSource = "desktop" | "fallback" | "mixed";

export type ProxyChangeFeedbackPhase = "running" | "accepted" | "success" | "error";

export type ProxyWriteOutcomeLabel =
  | "accepted"
  | "write-failed"
  | "rollback-flagged"
  | "write-pending"
  | "blocked";

export interface ProxyRotationSummary {
  residencyStatus: string;
  rotationMode: string;
  sessionKey: string | null;
  requestedProvider: string | null;
  requestedRegion: string | null;
  stickyTtlSeconds: number | null;
  expiresAt: string | null;
  note: string | null;
  trackingTaskId: string | null;
}

export interface ProxyUsageLink {
  id: string;
  profileId: string;
  profileName: string;
  groupName: string;
  profileStatus: "running" | "ready" | "paused";
  assignedAt: string | null;
}

export interface ProxyHealthSnapshot {
  state: ProxyHealthState;
  summary: string;
  lastCheckAt: string | null;
  latencyMs: number | null;
  exitIp: string | null;
  regionLabel: string | null;
  failureReason: string | null;
  batchState: "idle" | "queued" | "running" | "completed";
}

export interface ProxyRowModel {
  id: string;
  name: string;
  endpoint: string;
  port: number;
  protocol: ProxyProtocol;
  source: ProxySource;
  sourceLabel: string;
  providerLabel: string;
  authLabel: string;
  tags: string[];
  note: string;
  exitIp: string | null;
  regionLabel: string | null;
  usageCount: number;
  activeUsageCount: number;
  usageLinks: ProxyUsageLink[];
  rotation: ProxyRotationSummary;
  health: ProxyHealthSnapshot;
  lastUpdatedAt: string | null;
}

export interface ProxyFilterState {
  searchInput: string;
  appliedSearch: string;
  healthFilter: "all" | ProxyHealthState;
  sourceFilter: "all" | string;
  usageFilter: ProxyUsageFilter;
  regionFilter: string;
  tagFilter: string;
}

export interface ProxyTableState {
  sortField: ProxySortField;
}

export interface ProxyBatchCheckState {
  phase: "idle" | "queued" | "running" | "completed" | "blocked" | "error";
  scope: ProxyBatchScope;
  targetIds: string[];
  targetCount: number;
  completedCount: number;
  requestId: number;
  feedbackTone: "neutral" | "success" | "warning" | "error";
  lastMessage: string;
  lastStartedAt: string | null;
  lastFinishedAt: string | null;
}

export interface ProxyIpChangeFeedback {
  proxyId: string;
  phase: ProxyChangeFeedbackPhase;
  message: string;
  status: string | null;
  mode: string | null;
  sessionKey: string | null;
  requestedProvider: string | null;
  requestedRegion: string | null;
  stickyTtlSeconds: number | null;
  note: string | null;
  residencyStatus: string | null;
  rotationMode: string | null;
  trackingTaskId: string | null;
  expiresAt: string | null;
  updatedAt: string | null;
}

export interface ProxyChangeIpState {
  phase: "idle" | "running" | "completed" | "blocked" | "error";
  requestId: number;
  targetIds: string[];
  completedCount: number;
  // Legacy field name kept for store compatibility; rendered as "accepted writes" in UI.
  succeededCount: number;
  failedCount: number;
  activeProxyId: string | null;
  feedbackTone: "neutral" | "success" | "warning" | "error";
  lastMessage: string;
  lastStartedAt: string | null;
  lastFinishedAt: string | null;
  results: Record<string, ProxyIpChangeFeedback>;
}

export interface ProxyDetailSnapshot {
  proxyId: string;
  health: ProxyHealthSnapshot;
  usageLinks: ProxyUsageLink[];
  rotation: ProxyRotationSummary | null;
  source: ProxyDataSource;
}
