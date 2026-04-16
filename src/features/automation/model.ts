export type LauncherMode = "queue" | "dry_run" | "batch_prepare";
export type LauncherTargetScope =
  | "template_default"
  | "selected_profile_group"
  | "last_active_profile";

export type PreparedLaunchCompileStatus = "ready" | "blocked" | "failed";
export type PreparedLaunchCompileKind =
  | "native_success"
  | "native_blocked"
  | "preflight_blocked"
  | "probe_failed";
export type AutomationNoticeTone = "info" | "success" | "warning" | "error";
export type AutomationLaunchStatus =
  | "idle"
  | "prepared"
  | "launching"
  | "launched"
  | "blocked"
  | "failed";
export type AutomationRunDetailStatus =
  | "idle"
  | "loading"
  | "ready"
  | "blocked"
  | "failed";
export type AutomationManualGateStatus =
  | "idle"
  | "pending"
  | "confirming"
  | "rejecting"
  | "confirmed"
  | "rejected"
  | "failed";
export type AutomationTaskWriteAction =
  | "launch"
  | "refresh_detail"
  | "retry"
  | "cancel"
  | "confirm_manual_gate"
  | "reject_manual_gate";

export interface AutomationLauncherDraft {
  mode: LauncherMode;
  targetScope: LauncherTargetScope;
  launchNote: string;
}

export interface PreparedLaunchCompilePreview {
  status: PreparedLaunchCompileStatus;
  kind: PreparedLaunchCompileKind;
  message: string;
  acceptedProfileCount: number;
  compiledAtLabel: string | null;
  blockers: string[];
  warnings: string[];
}

export interface PreparedLaunchPlan {
  templateId: string;
  templateName: string;
  stepCount: number;
  recorderStepCount: number;
  variableCount: number;
  boundProfileIds: string[];
  mode: LauncherMode;
  targetScope: LauncherTargetScope;
  sourceRunId: string | null;
  recorderSessionId: string | null;
  missingRequiredKeys: string[];
  warnings: string[];
  note: string;
  ready: boolean;
  preparedAtLabel: string;
  compilePreview: PreparedLaunchCompilePreview;
}

export interface AutomationRunArtifact {
  id: string;
  label: string;
  path: string | null;
  status: string | null;
}

export interface AutomationRunTimelineEntry {
  id: string;
  label: string;
  status: string;
  detail: string | null;
  createdAt: string | null;
}

export interface AutomationRunDetail {
  runId: string;
  taskId: string | null;
  status: string;
  headline: string;
  message: string | null;
  failureReason: string | null;
  manualGateRequestId: string | null;
  manualGateStatus: string | null;
  updatedAtLabel: string | null;
  createdAtLabel: string | null;
  artifacts: AutomationRunArtifact[];
  timeline: AutomationRunTimelineEntry[];
  raw: unknown;
}

export interface AutomationLaunchOutcome {
  runId: string;
  taskId: string | null;
  status: string;
  message: string;
  manualGateRequestId: string | null;
  launchedAtLabel: string | null;
  raw: unknown;
}

export interface AutomationContractGap {
  contract: string;
  status: string;
  detail: string;
}
