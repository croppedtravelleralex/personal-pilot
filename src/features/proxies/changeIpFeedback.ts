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

  return includesAny(collectFeedbackText(result), ROLLBACK_TOKENS);
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

  if (hasProxyRollbackSignal(result)) {
    return "rollback_flagged";
  }

  if (result.phase === "error" || includesAny(normalized, FAILED_TOKENS)) {
    return "failed";
  }

  if (includesAny(normalized, BLOCKED_TOKENS)) {
    return "blocked";
  }

  if (result.phase === "success") {
    if (result.trackingTaskId || includesAny(normalized, ACCEPTED_TOKENS)) {
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

  switch (state) {
    case "submitting":
      return "Submitting changeProxyIp request through desktop contract.";
    case "accepted":
      return result.trackingTaskId
        ? `Provider write queued with tracking task ${result.trackingTaskId}.`
        : "Provider write was accepted and queued; tracking id is pending.";
    case "verify_pending":
      return "Write completed locally, but exit-IP drift still needs a later health/detail refresh.";
    case "rollback_flagged":
      return "Rollback/revert signal detected in status or note; verify latest detail before reuse.";
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
