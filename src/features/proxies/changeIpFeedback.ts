import type { ProxyIpChangeFeedback } from "./model";

export type ProxyProviderWriteState =
  | "idle"
  | "submitting"
  | "accepted"
  | "verify_pending"
  | "rollback_flagged"
  | "blocked"
  | "failed";

const ROLLBACK_TOKENS = [
  "rollback",
  "rolled_back",
  "rolled back",
  "revert",
  "reverted",
  "compensat",
];

const BLOCKED_TOKENS = ["blocked", "rejected", "denied"];

const FAILED_TOKENS = ["failed", "error", "timed_out", "timeout", "cancelled", "canceled"];

const ACCEPTED_TOKENS = ["queued", "accepted", "scheduled", "submitted"];

function normalize(value: string | null | undefined): string {
  return value?.trim().toLowerCase() ?? "";
}

function collectFeedbackText(result: ProxyIpChangeFeedback): string {
  return [
    result.status,
    result.execution?.status,
    result.execution?.stage,
    result.execution?.detail,
    result.rollback?.status,
    result.rollback?.reason,
    result.providerRefresh?.status,
    result.providerRefresh?.source,
    result.mode,
    result.residencyStatus,
    result.rotationMode,
    result.message,
    result.note,
  ]
    .map((value) => normalize(value))
    .filter(Boolean)
    .join(" ");
}

function includesAny(haystack: string, needles: string[]): boolean {
  return needles.some((needle) => haystack.includes(needle));
}

function pickFirstNonEmpty(...values: Array<string | null | undefined>): string | null {
  for (const value of values) {
    if (typeof value !== "string") {
      continue;
    }
    const normalized = value.trim();
    if (normalized.length > 0) {
      return normalized;
    }
  }

  return null;
}

export interface ProxyProviderWriteEvidence {
  acceptedWrite: boolean | null;
  rollbackSignal: boolean;
  providerSource: string | null;
  requestId: string | null;
  executionStatus: string | null;
  rollbackStatus: string | null;
  providerRefreshStatus: string | null;
  providerRefreshAt: string | null;
}

export function parseProxyTimestamp(value: string | null): number | null {
  if (!value) {
    return null;
  }

  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return numericValue;
  }

  const parsedMs = Date.parse(value);
  if (Number.isNaN(parsedMs)) {
    return null;
  }

  return Math.floor(parsedMs / 1000);
}

export function hasProxyRollbackSignal(result: ProxyIpChangeFeedback | null): boolean {
  if (!result) {
    return false;
  }

  if (result.rollback?.signaled === true) {
    return true;
  }
  if (result.rollback?.signaled === false) {
    return false;
  }

  return includesAny(collectFeedbackText(result), ROLLBACK_TOKENS);
}

export function getProxyProviderRequestId(result: ProxyIpChangeFeedback | null): string | null {
  if (!result) {
    return null;
  }

  return pickFirstNonEmpty(
    result.execution?.requestId ?? null,
    result.providerRefresh?.requestId ?? null,
    result.rollback?.requestId ?? null,
    result.trackingTaskId,
  );
}

export function getProxyProviderSource(result: ProxyIpChangeFeedback | null): string | null {
  if (!result) {
    return null;
  }

  return pickFirstNonEmpty(
    result.providerRefresh?.source ?? null,
    result.execution?.providerSource ?? null,
  );
}

export function getProxyAcceptedWriteSignal(
  result: ProxyIpChangeFeedback | null,
): boolean | null {
  if (!result) {
    return null;
  }

  if (typeof result.execution?.acceptedWrite === "boolean") {
    return result.execution.acceptedWrite;
  }

  const normalized = collectFeedbackText(result);
  if (result.trackingTaskId || includesAny(normalized, ACCEPTED_TOKENS)) {
    return true;
  }

  if (
    result.phase === "error" ||
    includesAny(normalized, FAILED_TOKENS) ||
    includesAny(normalized, BLOCKED_TOKENS)
  ) {
    return false;
  }

  return null;
}

export function getProxyProviderWriteEvidence(
  result: ProxyIpChangeFeedback | null,
): ProxyProviderWriteEvidence {
  if (!result) {
    return {
      acceptedWrite: null,
      rollbackSignal: false,
      providerSource: null,
      requestId: null,
      executionStatus: null,
      rollbackStatus: null,
      providerRefreshStatus: null,
      providerRefreshAt: null,
    };
  }

  return {
    acceptedWrite: getProxyAcceptedWriteSignal(result),
    rollbackSignal: hasProxyRollbackSignal(result),
    providerSource: getProxyProviderSource(result),
    requestId: getProxyProviderRequestId(result),
    executionStatus: pickFirstNonEmpty(
      result.execution?.status ?? null,
      result.status,
    ),
    rollbackStatus: pickFirstNonEmpty(
      result.rollback?.status ?? null,
      result.rollback?.reason ?? null,
    ),
    providerRefreshStatus: pickFirstNonEmpty(
      result.providerRefresh?.status ?? null,
    ),
    providerRefreshAt: pickFirstNonEmpty(result.providerRefresh?.refreshedAt ?? null),
  };
}

export function getProxyProviderWriteState(
  result: ProxyIpChangeFeedback | null,
): ProxyProviderWriteState {
  if (!result) {
    return "idle";
  }

  if (result.phase === "running") {
    return "submitting";
  }

  const normalized = collectFeedbackText(result);
  const acceptedSignal = getProxyAcceptedWriteSignal(result);

  if (hasProxyRollbackSignal(result)) {
    return "rollback_flagged";
  }

  if (result.phase === "error" || includesAny(normalized, FAILED_TOKENS)) {
    return "failed";
  }

  if (includesAny(normalized, BLOCKED_TOKENS)) {
    return "blocked";
  }

  if (acceptedSignal === true) {
    return "accepted";
  }

  if (acceptedSignal === false && result.phase !== "success" && result.phase !== "accepted") {
    return "verify_pending";
  }

  if (result.phase === "success" || result.phase === "accepted") {
    if (acceptedSignal === true || result.trackingTaskId || includesAny(normalized, ACCEPTED_TOKENS)) {
      return "accepted";
    }
    return "verify_pending";
  }

  return "verify_pending";
}

export function getProxyProviderWriteLabel(state: ProxyProviderWriteState): string {
  switch (state) {
    case "idle":
      return "No recent write";
    case "submitting":
      return "Write submitting";
    case "accepted":
      return "Write accepted";
    case "verify_pending":
      return "Awaiting verify";
    case "rollback_flagged":
      return "Rollback flagged";
    case "blocked":
      return "Write blocked";
    case "failed":
      return "Write failed";
    default:
      return "Unknown write state";
  }
}

export function getProxyProviderWriteDetail(result: ProxyIpChangeFeedback | null): string {
  const state = getProxyProviderWriteState(result);
  if (!result) {
    return "No tracked change-IP request yet.";
  }

  const evidence = getProxyProviderWriteEvidence(result);
  const requestInfo = evidence.requestId ? ` request ${evidence.requestId}` : "";
  const sourceInfo = evidence.providerSource ? ` from ${evidence.providerSource}` : "";

  switch (state) {
    case "submitting":
      return "Submitting changeProxyIp request through desktop contract; waiting for execution metadata.";
    case "accepted":
      return evidence.requestId
        ? `Provider write accepted${sourceInfo}; queued under${requestInfo}.`
        : `Provider write accepted${sourceInfo}; request id is pending.`;
    case "verify_pending":
      return "Write finished locally, but provider acceptance is still unconfirmed; wait for detail refresh.";
    case "rollback_flagged":
      return evidence.rollbackStatus
        ? `Rollback signal flagged (${evidence.rollbackStatus}); verify latest detail before reuse.`
        : "Rollback signal flagged; verify latest detail before reuse.";
    case "blocked":
      return "Provider write was blocked/rejected; keep current assignment until follow-up.";
    case "failed":
      return "Local request failed; provider-side write is not confirmed.";
    default:
      return "No tracked provider-write signal.";
  }
}

export function getProxyChangeCooldownWindowSeconds(
  result: ProxyIpChangeFeedback | null,
): number {
  if (!result) {
    return 0;
  }

  const writeState = getProxyProviderWriteState(result);
  if (writeState === "submitting") {
    return 0;
  }

  if (writeState === "rollback_flagged" || writeState === "blocked" || writeState === "failed") {
    return 15 * 60;
  }

  let baseWindowSeconds = 5 * 60;
  const mode = normalize(result.rotationMode ?? result.mode);
  const residencyStatus = normalize(result.residencyStatus);

  if (mode.includes("sticky_rebind")) {
    baseWindowSeconds = 9 * 60;
  } else if (mode.includes("sticky")) {
    baseWindowSeconds = 7 * 60;
  } else if (mode.includes("provider")) {
    baseWindowSeconds = 6 * 60;
  }

  if (
    residencyStatus.includes("provider_rotation_pending") ||
    residencyStatus.includes("provider_override_pending") ||
    residencyStatus.includes("region_override_pending")
  ) {
    baseWindowSeconds = Math.max(baseWindowSeconds, 8 * 60);
  }

  return baseWindowSeconds;
}

export function getProxyChangeCooldownRemainingSeconds(
  result: ProxyIpChangeFeedback | null,
): number | null {
  if (!result) {
    return null;
  }

  const updatedAt = parseProxyTimestamp(result.updatedAt);
  if (!updatedAt) {
    return null;
  }

  const windowSeconds = getProxyChangeCooldownWindowSeconds(result);
  if (windowSeconds <= 0) {
    return 0;
  }

  return windowSeconds - (Math.floor(Date.now() / 1000) - updatedAt);
}

export function isProxyChangeCoolingDown(result: ProxyIpChangeFeedback): boolean {
  const remaining = getProxyChangeCooldownRemainingSeconds(result);
  return remaining !== null && remaining > 0;
}
