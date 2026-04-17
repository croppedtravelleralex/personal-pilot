import type { DesktopTaskItem } from "../../types/desktop";
import { createStore } from "../../store/createStore";
import type {
  AutomationContractGap,
  AutomationLauncherDraft,
  AutomationLaunchOutcome,
  AutomationLaunchStatus,
  AutomationManualGateStatus,
  AutomationNoticeTone,
  AutomationRunDetail,
  AutomationRunDetailStatus,
  AutomationTaskWriteAction,
  PreparedLaunchCompilePreview,
  LauncherMode,
  LauncherTargetScope,
  PreparedLaunchPlan,
} from "./model";

interface AutomationState {
  selectedRun: DesktopTaskItem | null;
  launcherDraft: AutomationLauncherDraft;
  lastPreparedLaunch: PreparedLaunchPlan | null;
  launcherNotice: string | null;
  launcherNoticeTone: AutomationNoticeTone;
  contractGaps: AutomationContractGap[];
  isPreparingLaunch: boolean;
  launchStatus: AutomationLaunchStatus;
  launchFailureReason: string | null;
  launchedRun: AutomationLaunchOutcome | null;
  runDetail: AutomationRunDetail | null;
  runDetailStatus: AutomationRunDetailStatus;
  runDetailFailureReason: string | null;
  manualGateStatus: AutomationManualGateStatus;
  manualGateFailureReason: string | null;
  activeTaskWriteAction: AutomationTaskWriteAction | null;
}

const DEFAULT_LAUNCHER_DRAFT: AutomationLauncherDraft = {
  mode: "queue",
  targetScope: "template_default",
  launchNote: "",
};

const CONTRACT_GAPS: AutomationContractGap[] = [
  {
    contract: "compileTemplateRun",
    status: "Ready",
    detail:
      "Native compile is wired and writes launch manifests for accepted profiles.",
  },
  {
    contract: "launchTemplateRun",
    status: "Ready",
    detail:
      "Native launch is wired and dispatches prepared manifests into local runtime.",
  },
  {
    contract: "readRunDetail",
    status: "Ready",
    detail:
      "Per-run detail read is wired and returns timeline, artifacts, failure reason, and manual-gate state.",
  },
  {
    contract: "readRecorderSnapshot / startBehaviorRecording / stopBehaviorRecording",
    status: "Ready",
    detail:
      "Recorder uses native read/start/stop on main path, with local draft fallback only when needed.",
  },
];

function updateContractGap(
  current: AutomationContractGap[],
  contract: string,
  status: string,
  detail: string,
): AutomationContractGap[] {
  return current.map((item) =>
    item.contract === contract
      ? {
          ...item,
          status,
          detail,
        }
      : item,
  );
}

const automationStore = createStore<AutomationState>({
  selectedRun: null,
  launcherDraft: DEFAULT_LAUNCHER_DRAFT,
  lastPreparedLaunch: null,
  launcherNotice: null,
  launcherNoticeTone: "info",
  contractGaps: CONTRACT_GAPS,
  isPreparingLaunch: false,
  launchStatus: "idle",
  launchFailureReason: null,
  launchedRun: null,
  runDetail: null,
  runDetailStatus: "idle",
  runDetailFailureReason: null,
  manualGateStatus: "idle",
  manualGateFailureReason: null,
  activeTaskWriteAction: null,
});

export const automationActions = {
  selectRun(selectedRun: DesktopTaskItem) {
    automationStore.setState((current) => ({
      ...current,
      selectedRun,
    }));
  },
  refreshSelectedRun(selectedRun: DesktopTaskItem) {
    automationStore.setState((current) => {
      if (!current.selectedRun || current.selectedRun.id !== selectedRun.id) {
        return current;
      }

      if (current.selectedRun === selectedRun) {
        return current;
      }

      return {
        ...current,
        selectedRun,
      };
    });
  },
  setLaunchMode(mode: LauncherMode) {
    automationStore.setState((current) => ({
      ...current,
      launcherDraft: {
        ...current.launcherDraft,
        mode,
      },
    }));
  },
  setTargetScope(targetScope: LauncherTargetScope) {
    automationStore.setState((current) => ({
      ...current,
      launcherDraft: {
        ...current.launcherDraft,
        targetScope,
      },
    }));
  },
  setLaunchNote(launchNote: string) {
    automationStore.setState((current) => ({
      ...current,
      launcherDraft: {
        ...current.launcherDraft,
        launchNote,
      },
    }));
  },
  prepareLaunchStarted() {
    automationStore.setState((current) => ({
      ...current,
      isPreparingLaunch: true,
      launchStatus: current.launchStatus === "launched" ? "launched" : "idle",
      launchFailureReason: null,
      launcherNotice: "Preparing launch context and writing compile manifest preview...",
      launcherNoticeTone: "info",
    }));
  },
  prepareLaunch(plan: {
    templateId: string;
    templateName: string;
    stepCount: number;
    recorderStepCount: number;
    variableCount: number;
    boundProfileIds: string[];
    recorderSessionId: string | null;
    missingRequiredKeys: string[];
    warnings: string[];
    note: string;
    ready: boolean;
    compilePreview: PreparedLaunchCompilePreview;
  }) {
    automationStore.setState((current) => ({
      ...current,
      isPreparingLaunch: false,
      launchStatus:
        plan.compilePreview.status === "ready"
          ? "prepared"
          : plan.compilePreview.status === "failed"
            ? "failed"
            : "blocked",
      launchFailureReason:
        plan.compilePreview.status === "failed" ? plan.compilePreview.message : null,
      launchedRun:
        plan.compilePreview.status === "ready" ? current.launchedRun : null,
      runDetail:
        plan.compilePreview.status === "ready" ? current.runDetail : null,
      runDetailStatus:
        plan.compilePreview.status === "ready" ? current.runDetailStatus : "idle",
      runDetailFailureReason:
        plan.compilePreview.status === "ready" ? current.runDetailFailureReason : null,
      manualGateStatus:
        plan.compilePreview.status === "ready" ? current.manualGateStatus : "idle",
      manualGateFailureReason:
        plan.compilePreview.status === "ready" ? current.manualGateFailureReason : null,
      activeTaskWriteAction: null,
      lastPreparedLaunch: {
        templateId: plan.templateId,
        templateName: plan.templateName,
        stepCount: plan.stepCount,
        recorderStepCount: plan.recorderStepCount,
        variableCount: plan.variableCount,
        boundProfileIds: plan.boundProfileIds,
        mode: current.launcherDraft.mode,
        targetScope: current.launcherDraft.targetScope,
        sourceRunId: current.selectedRun?.id ?? null,
        recorderSessionId: plan.recorderSessionId,
        missingRequiredKeys: plan.missingRequiredKeys,
        warnings: plan.warnings,
        note: current.launcherDraft.launchNote || plan.note,
        ready: plan.ready && plan.compilePreview.status === "ready",
        preparedAtLabel: new Intl.DateTimeFormat("zh-CN", {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
          hour12: false,
        }).format(new Date()),
        compilePreview: plan.compilePreview,
      },
      launcherNotice:
        plan.compilePreview.status === "ready"
          ? `${plan.compilePreview.message} Launch is ready for dispatch in this page.`
          : plan.compilePreview.message,
      launcherNoticeTone:
        plan.compilePreview.status === "ready"
          ? "success"
          : plan.compilePreview.status === "failed"
            ? "error"
            : "warning",
    }));
  },
  prepareLaunchFailed(message: string) {
    automationStore.setState((current) => ({
      ...current,
      isPreparingLaunch: false,
      launchStatus: "failed",
      launchFailureReason: message,
      launcherNotice: message,
      launcherNoticeTone: "error",
    }));
  },
  launchStarted() {
    automationStore.setState((current) => ({
      ...current,
      launchStatus: "launching",
      launchFailureReason: null,
      activeTaskWriteAction: "launch",
      launcherNotice: "Dispatching prepared launch to local runtime...",
      launcherNoticeTone: "info",
    }));
  },
  launchSucceeded(outcome: AutomationLaunchOutcome) {
    automationStore.setState((current) => ({
      ...current,
      launchStatus: "launched",
      launchFailureReason: null,
      launchedRun: outcome,
      manualGateStatus: outcome.manualGateRequestId ? "pending" : "idle",
      manualGateFailureReason: null,
      activeTaskWriteAction: null,
      contractGaps: updateContractGap(
        current.contractGaps,
        "launchTemplateRun",
        "Ready",
        "Native launch is wired and dispatches prepared manifests into local runtime.",
      ),
      launcherNotice: outcome.message,
      launcherNoticeTone: "success",
    }));
  },
  launchFailed(message: string, blocked: boolean) {
    automationStore.setState((current) => ({
      ...current,
      launchStatus: blocked ? "blocked" : "failed",
      launchFailureReason: message,
      activeTaskWriteAction: null,
      launcherNotice: message,
      launcherNoticeTone: blocked ? "warning" : "error",
    }));
  },
  runDetailStarted() {
    automationStore.setState((current) => ({
      ...current,
      runDetailStatus: "loading",
      runDetailFailureReason: null,
      activeTaskWriteAction:
        current.activeTaskWriteAction === null
          ? "refresh_detail"
          : current.activeTaskWriteAction,
    }));
  },
  runDetailSucceeded(detail: AutomationRunDetail) {
    automationStore.setState((current) => ({
      ...current,
      runDetail: detail,
      runDetailStatus: "ready",
      runDetailFailureReason: null,
      manualGateStatus:
        detail.manualGateRequestId && detail.manualGateStatus !== "confirmed" && detail.manualGateStatus !== "rejected"
          ? "pending"
          : detail.manualGateStatus === "confirmed"
            ? "confirmed"
            : detail.manualGateStatus === "rejected"
              ? "rejected"
              : current.manualGateStatus,
      manualGateFailureReason: null,
      activeTaskWriteAction:
        current.activeTaskWriteAction === "refresh_detail" ||
        current.activeTaskWriteAction === "launch"
          ? null
          : current.activeTaskWriteAction,
      contractGaps: updateContractGap(
        current.contractGaps,
        "readRunDetail",
        "Ready",
        "Per-run detail read is wired and returns timeline, artifacts, failure reason, and manual-gate state.",
      ),
    }));
  },
  runDetailFailed(message: string, blocked: boolean) {
    automationStore.setState((current) => ({
      ...current,
      runDetailStatus: blocked ? "blocked" : "failed",
      runDetailFailureReason: message,
      activeTaskWriteAction:
        current.activeTaskWriteAction === "refresh_detail" ||
        current.activeTaskWriteAction === "launch"
          ? null
          : current.activeTaskWriteAction,
      launcherNotice: message,
      launcherNoticeTone: blocked ? "warning" : "error",
    }));
  },
  taskWriteStarted(action: AutomationTaskWriteAction) {
    automationStore.setState((current) => ({
      ...current,
      activeTaskWriteAction: action,
      manualGateStatus:
        action === "confirm_manual_gate"
          ? "confirming"
          : action === "reject_manual_gate"
            ? "rejecting"
            : current.manualGateStatus,
      manualGateFailureReason: null,
    }));
  },
  taskWriteFinished() {
    automationStore.setState((current) => ({
      ...current,
      activeTaskWriteAction: null,
    }));
  },
  taskWriteFailed(message: string) {
    automationStore.setState((current) => ({
      ...current,
      activeTaskWriteAction: null,
      manualGateStatus:
        current.manualGateStatus === "confirming" || current.manualGateStatus === "rejecting"
          ? "failed"
          : current.manualGateStatus,
      manualGateFailureReason: message,
      launcherNotice: message,
      launcherNoticeTone: "error",
    }));
  },
  resetLauncherDraft() {
    automationStore.setState((current) => ({
      ...current,
      launcherDraft: DEFAULT_LAUNCHER_DRAFT,
      lastPreparedLaunch: null,
      launcherNotice: null,
      launcherNoticeTone: "info",
      isPreparingLaunch: false,
      launchStatus: "idle",
      launchFailureReason: null,
      launchedRun: null,
      runDetail: null,
      runDetailStatus: "idle",
      runDetailFailureReason: null,
      manualGateStatus: "idle",
      manualGateFailureReason: null,
      activeTaskWriteAction: null,
    }));
  },
};

export { automationStore };
