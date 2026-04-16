import { useEffect, useMemo } from "react";

import * as desktopServices from "../../services/desktop";
import { useStore } from "../../store/createStore";
import type {
  DesktopLaunchTemplateRunRequest,
  DesktopLaunchTemplateRunResult,
  DesktopManualGateActionRequest,
  DesktopReadRunDetailQuery,
  DesktopRunArtifact,
  DesktopRunDetail,
  DesktopRunTimelineEntry,
} from "../../types/desktop";
import { useRecorderViewModel } from "../recorder/hooks";
import { buildTemplateCompileRequestDraft } from "../templates/model";
import { useTemplatesViewModel } from "../templates/hooks";
import { templateActions } from "../templates/store";
import { useTasksViewModel } from "../tasks/hooks";
import type {
  AutomationNoticeTone,
  AutomationLaunchOutcome,
  AutomationRunArtifact,
  AutomationRunDetail,
  AutomationRunTimelineEntry,
  PreparedLaunchCompilePreview,
} from "./model";
import {
  buildAutomationChainSummary,
  getRecommendedTemplate,
  getRunContextBindingValue,
} from "./derived";
import { automationActions, automationStore } from "./store";

const automationDesktop = desktopServices;

function isCommandNotReady(error: unknown): boolean {
  return (
    Boolean(error) &&
    typeof error === "object" &&
    "code" in error &&
    (error as { code?: string }).code === "desktop_command_not_ready"
  );
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Failed to prepare launch draft";
}

function hasDesktopCommand<T extends keyof typeof automationDesktop>(
  command: T,
): boolean {
  return typeof automationDesktop[command] === "function";
}

function formatTimeLabel(value: string | null): string | null {
  if (!value) {
    return null;
  }

  const numericValue = Number(value);
  const date = Number.isFinite(numericValue) ? new Date(numericValue * 1000) : new Date(value);

  if (Number.isNaN(date.getTime())) {
    return null;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function normalizeArtifact(item: DesktopRunArtifact): AutomationRunArtifact {
  return {
    id: item.id,
    label: item.label,
    path: item.path,
    status: item.status,
  };
}

function normalizeTimelineEntry(
  item: DesktopRunTimelineEntry,
): AutomationRunTimelineEntry {
  return {
    id: item.id,
    label: item.label,
    status: item.status,
    detail: item.detail,
    createdAt: item.createdAt,
  };
}

function normalizeLaunchOutcome(
  payload: DesktopLaunchTemplateRunResult,
  fallbackStatus: string,
): AutomationLaunchOutcome {
  const launchedAt = payload.launchedAt ?? payload.launchSummary.launchedAt;

  return {
    runId: payload.runId || payload.taskId || "unknown-run",
    taskId: payload.taskId ?? null,
    status: payload.status ?? fallbackStatus,
    message:
      payload.message ??
      (payload.manualGateRequestId
        ? "Run dispatched and is waiting on a manual gate."
        : "Run dispatched into the local runtime."),
    manualGateRequestId: payload.manualGateRequestId ?? null,
    launchedAtLabel: formatTimeLabel(launchedAt ?? null),
    raw: payload,
  };
}

function getManualGateHeadline(status: string): string {
  switch (status) {
    case "confirmed":
      return "Operator approved the gate and the run can continue.";
    case "rejected":
      return "Operator rejected the gate and the run is held for rework.";
    case "confirming":
      return "Approval is being sent to the local runtime.";
    case "rejecting":
      return "Rejection is being sent to the local runtime.";
    case "failed":
      return "The last manual gate write failed and needs operator review.";
    default:
      return "Manual review is required before the run can continue.";
  }
}

function getManualGateDetail(requestId: string, status: string, runId: string | null): string {
  if (status === "confirmed") {
    return `Request ${requestId} was confirmed from this workbench. Refresh detail to confirm downstream runtime progress.`;
  }

  if (status === "rejected") {
    return `Request ${requestId} was rejected from this workbench. Retry or a new dispatch may be required for run ${runId ?? "unknown"}.`;
  }

  if (status === "failed") {
    return `Request ${requestId} is still the active operator checkpoint, but the latest decision write did not complete.`;
  }

  return `Request ${requestId} is the current operator checkpoint${runId ? ` for run ${runId}` : ""}. Keep review and decision in this local panel.`;
}

function normalizeRunDetail(payload: DesktopRunDetail): AutomationRunDetail {
  const status = payload.status ?? "unknown";
  const timeline = payload.timeline.map(normalizeTimelineEntry);
  const artifacts = payload.artifacts.map(normalizeArtifact);
  const headline =
    payload.headline ??
    (payload.manualGateRequestId
      ? "Manual gate is holding this run."
      : `Run ${status}`);

  return {
    runId: payload.runId || payload.taskId || "unknown-run",
    taskId: payload.taskId ?? null,
    status,
    headline,
    message: payload.message ?? null,
    failureReason: payload.failureReason ?? null,
    manualGateRequestId: payload.manualGateRequestId ?? null,
    manualGateStatus: payload.manualGateStatus ?? null,
    updatedAtLabel: payload.updatedAtLabel ?? null,
    createdAtLabel: payload.createdAtLabel ?? null,
    artifacts,
    timeline,
    raw: payload,
  };
}

export function useAutomationCenterViewModel() {
  const runs = useTasksViewModel();
  const automation = useStore(automationStore, (current) => current);

  useEffect(() => {
    if (!automation.selectedRun && runs.state.items[0]) {
      automationActions.selectRun(runs.state.items[0]);
      return;
    }

    if (!automation.selectedRun) {
      return;
    }

    const currentPageRun = runs.state.items.find(
      (item) => item.id === automation.selectedRun?.id,
    );

    if (currentPageRun) {
      automationActions.refreshSelectedRun(currentPageRun);
    }
  }, [automation.selectedRun, runs.state.items]);

  const selectedRun = automation.selectedRun;
  const templates = useTemplatesViewModel();
  const recorder = useRecorderViewModel(templates.selectedTemplate, {
    profileId: selectedRun?.personaId ?? null,
  });
  const recommendation = useMemo(
    () => getRecommendedTemplate(selectedRun, templates.state.items),
    [selectedRun, templates.state.items],
  );

  useEffect(() => {
    if (!recommendation) {
      return;
    }

    if (templates.selectedTemplate?.id === recommendation.templateId) {
      return;
    }

    if (
      templates.selectedTemplate &&
      selectedRun?.platformId &&
      templates.selectedTemplate.platformId === selectedRun.platformId
    ) {
      return;
    }

    templateActions.selectTemplate(recommendation.templateId);
  }, [
    recommendation,
    selectedRun?.platformId,
    templates.selectedTemplate,
  ]);

  useEffect(() => {
    const selectedTemplate = templates.selectedTemplate;
    const bindingDraft = templates.selectedBindingDraft;

    if (!selectedTemplate || !bindingDraft || !selectedRun) {
      return;
    }

    selectedTemplate.variables.forEach((variable) => {
      const binding = bindingDraft.values[variable.key];
      if (!binding || binding.value.trim().length > 0) {
        return;
      }

      const hydratedValue = getRunContextBindingValue(variable, selectedRun);
      if (!hydratedValue) {
        return;
      }

      templateActions.hydrateBindingValueFromRunContext(
        selectedTemplate.id,
        variable.key,
        hydratedValue,
      );
    });
  }, [selectedRun, templates.selectedBindingDraft, templates.selectedTemplate]);

  useEffect(() => {
    const selectedTemplate = templates.selectedTemplate;
    const bindingDraft = templates.selectedBindingDraft;
    const variableCandidates = recorder.state.snapshot?.variableCandidates;

    if (!selectedTemplate || !bindingDraft || !variableCandidates) {
      return;
    }

    variableCandidates.forEach((candidate) => {
      const binding = bindingDraft.values[candidate.key];
      if (!binding) {
        return;
      }

      if (
        (binding.value.trim().length > 0 && binding.source !== "run_context") ||
        candidate.previewValue.trim().length === 0
      ) {
        return;
      }

      templateActions.hydrateBindingValueFromRecorder(
        selectedTemplate.id,
        candidate.key,
        candidate.previewValue,
      );
    });
  }, [
    recorder.state.snapshot?.sessionId,
    recorder.state.snapshot?.stepCount,
    templates.selectedBindingDraft,
    templates.selectedTemplate,
  ]);

  const compileDraft = useMemo(() => {
    if (!templates.selectedTemplate || !templates.selectedBindingDraft) {
      return null;
    }

    return buildTemplateCompileRequestDraft(
      templates.selectedTemplate,
      templates.selectedBindingDraft,
      {
        selectedRunProfileId: selectedRun?.personaId ?? null,
        recorderSessionId: recorder.state.snapshot?.sessionId ?? null,
        recorderStepCount: recorder.state.snapshot?.stepCount ?? 0,
        recorderSource: recorder.state.snapshot?.source ?? "none",
      },
    );
  }, [
    recorder.state.snapshot?.source,
    recorder.state.snapshot?.sessionId,
    recorder.state.snapshot?.stepCount,
    selectedRun?.personaId,
    templates.selectedBindingDraft,
    templates.selectedTemplate,
  ]);

  const chainSummary = useMemo(
    () =>
      buildAutomationChainSummary({
        selectedRun,
        selectedTemplate: templates.selectedTemplate,
        compileDraft,
        recorderSnapshot: recorder.state.snapshot,
        lastPreparedLaunch: automation.lastPreparedLaunch,
        recommendation,
        launchedRun: automation.launchedRun,
        runDetail: automation.runDetail,
        launchStatus: automation.launchStatus,
        runDetailStatus: automation.runDetailStatus,
        launchFailureReason: automation.launchFailureReason,
        runDetailFailureReason: automation.runDetailFailureReason,
      }),
    [
      automation.lastPreparedLaunch,
      automation.launchFailureReason,
      automation.launchStatus,
      automation.launchedRun,
      automation.runDetail,
      automation.runDetailFailureReason,
      automation.runDetailStatus,
      compileDraft,
      recommendation,
      recorder.state.snapshot,
      selectedRun,
      templates.selectedTemplate,
    ],
  );

  const activeRunCount = runs.state.items.filter(
    (item) => item.status === "queued" || item.status === "running",
  ).length;
  const runDetailNotice =
    automation.runDetailFailureReason ??
    automation.runDetail?.message ??
    automation.launcherNotice;
  const manualGate = automation.runDetail?.manualGateRequestId
    ? {
        requestId: automation.runDetail.manualGateRequestId,
        status: automation.runDetail.manualGateStatus ?? automation.manualGateStatus,
        headline: getManualGateHeadline(
          automation.runDetail.manualGateStatus ?? automation.manualGateStatus,
        ),
        detail: getManualGateDetail(
          automation.runDetail.manualGateRequestId,
          automation.runDetail.manualGateStatus ?? automation.manualGateStatus,
          automation.runDetail.runId,
        ),
        decisionOptions: ["Approve", "Reject"],
        failureReason: automation.manualGateFailureReason,
      }
    : automation.launchedRun?.manualGateRequestId
      ? {
          requestId: automation.launchedRun.manualGateRequestId,
          status: automation.manualGateStatus,
          headline: getManualGateHeadline(automation.manualGateStatus),
          detail: getManualGateDetail(
            automation.launchedRun.manualGateRequestId,
            automation.manualGateStatus,
            automation.launchedRun.runId,
          ),
          decisionOptions: ["Approve", "Reject"],
          failureReason: automation.manualGateFailureReason,
        }
      : null;
  const lastLaunchResult = automation.launchedRun
    ? {
        status: automation.launchedRun.status,
        headline: automation.launchedRun.manualGateRequestId
          ? "Run dispatched and paused on a manual gate."
          : "Run dispatched into the local runtime.",
        detail: automation.launchedRun.message,
        launchedAtLabel: automation.launchedRun.launchedAtLabel,
        queueLabel:
          automation.launchedRun.status === "queued"
            ? "Queued in local runtime"
            : automation.launchedRun.status === "running"
              ? "Running in local runtime"
              : null,
        acceptedProfileCount: automation.lastPreparedLaunch?.compilePreview.acceptedProfileCount ?? null,
        runId: automation.launchedRun.runId,
        warnings:
          automation.launchedRun.manualGateRequestId
            ? ["Manual confirmation is still required before the run can fully continue."]
            : [],
      }
    : null;
  const isLaunchingRun =
    automation.launchStatus === "launching" ||
    automation.activeTaskWriteAction === "launch";
  const isRunDetailLoading = automation.runDetailStatus === "loading";
  const actionFeedback =
    automation.manualGateFailureReason || automation.launcherNotice
      ? {
          tone: (automation.manualGateFailureReason
            ? "error"
            : automation.launcherNoticeTone) as AutomationNoticeTone,
          message: automation.manualGateFailureReason ?? automation.launcherNotice ?? "",
          updatedAtLabel:
            automation.runDetail?.updatedAtLabel ??
            automation.launchedRun?.launchedAtLabel ??
            automation.lastPreparedLaunch?.preparedAtLabel ??
            null,
        }
      : null;
  const actionState = {
    isRefreshing:
      automation.activeTaskWriteAction === "refresh_detail" ||
      automation.runDetailStatus === "loading",
    isRetrying: automation.activeTaskWriteAction === "retry",
    isCancelling: automation.activeTaskWriteAction === "cancel",
    isApprovingGate: automation.activeTaskWriteAction === "confirm_manual_gate",
    isRejectingGate: automation.activeTaskWriteAction === "reject_manual_gate",
  };

  async function refreshRunDetailInternal() {
    const runId =
      automation.launchedRun?.runId && automation.launchedRun.runId !== "unknown-run"
        ? automation.launchedRun.runId
        : automation.runDetail?.runId ?? null;
    const taskId =
      automation.runDetail?.taskId ??
      automation.launchedRun?.taskId ??
      selectedRun?.id ??
      null;
    const query: DesktopReadRunDetailQuery = {
      runId,
      taskId: runId ? undefined : taskId,
    };

    if (!query.runId && !query.taskId) {
      automationActions.runDetailFailed(
        "No launched run is available yet. Dispatch a run first before reading detail.",
        true,
      );
      return;
    }

    if (!hasDesktopCommand("readRunDetail")) {
      automationActions.runDetailFailed(
        "This desktop build does not expose readRunDetail yet.",
        true,
      );
      return;
    }

    automationActions.runDetailStarted();
    try {
      const detail = await automationDesktop.readRunDetail(query);
      automationActions.runDetailSucceeded(normalizeRunDetail(detail));
    } catch (error) {
      automationActions.runDetailFailed(
        toErrorMessage(error),
        isCommandNotReady(error),
      );
    }
  }

  async function launchPreparedRunInternal() {
    if (!templates.selectedTemplate || !compileDraft || !automation.lastPreparedLaunch?.ready) {
      automationActions.launchFailed(
        "Launch is still blocked. Prepare a ready manifest before dispatching.",
        true,
      );
      return;
    }

    if (!hasDesktopCommand("launchTemplateRun")) {
      automationActions.launchFailed(
        "This desktop build does not expose launchTemplateRun yet. The prepared manifest is kept locally.",
        true,
      );
      return;
    }

    automationActions.launchStarted();

    try {
      const request: DesktopLaunchTemplateRunRequest = {
        templateId: compileDraft.templateId,
        storeId: templates.selectedTemplate.storeId,
        profileIds: compileDraft.targetProfileIds,
        variableBindings: Object.fromEntries(
          compileDraft.bindings.map((binding) => [binding.key, binding.value]),
        ),
        dryRun: automation.launcherDraft.mode !== "queue",
        mode: automation.launcherDraft.mode,
        targetScope: automation.launcherDraft.targetScope,
        launchNote: automation.launcherDraft.launchNote || compileDraft.note,
        sourceRunId: selectedRun?.id ?? null,
        recorderSessionId: compileDraft.recorderSessionId,
      };
      const result = await automationDesktop.launchTemplateRun(request);

      const outcome = normalizeLaunchOutcome(result, "queued");
      automationActions.launchSucceeded(outcome);

      if (outcome.runId !== "unknown-run" || outcome.taskId) {
        if (hasDesktopCommand("readRunDetail")) {
          automationActions.runDetailStarted();
          try {
            const query: DesktopReadRunDetailQuery =
              outcome.runId !== "unknown-run"
                ? { runId: outcome.runId }
                : { taskId: outcome.taskId };
            const detail = await automationDesktop.readRunDetail(query);
            automationActions.runDetailSucceeded(normalizeRunDetail(detail));
          } catch (error) {
            const message = isCommandNotReady(error)
              ? "Run dispatched successfully, but this desktop build cannot read per-run detail yet."
              : toErrorMessage(error);
            automationActions.runDetailFailed(message, isCommandNotReady(error));
          }
        } else {
          automationActions.runDetailFailed(
            "Run dispatched successfully, but this desktop build does not expose readRunDetail yet.",
            true,
          );
        }
      }
    } catch (error) {
      automationActions.launchFailed(
        isCommandNotReady(error)
          ? "This desktop build does not expose launchTemplateRun yet. The prepared manifest remains staged locally."
          : toErrorMessage(error),
        isCommandNotReady(error),
      );
    }
  }

  async function retryRunTaskInternal() {
    const taskId = automation.runDetail?.taskId ?? automation.launchedRun?.taskId;
    if (!taskId) {
      automationActions.taskWriteFailed("No task id is available for retry.");
      return;
    }

    if (!hasDesktopCommand("retryTask")) {
      automationActions.taskWriteFailed("This desktop build does not expose retryTask yet.");
      return;
    }

    automationActions.taskWriteStarted("retry");
    try {
      await automationDesktop.retryTask(taskId);
      automationActions.taskWriteFinished();
      await refreshRunDetailInternal();
    } catch (error) {
      automationActions.taskWriteFailed(toErrorMessage(error));
    }
  }

  async function cancelRunTaskInternal() {
    const taskId = automation.runDetail?.taskId ?? automation.launchedRun?.taskId;
    if (!taskId) {
      automationActions.taskWriteFailed("No task id is available for cancellation.");
      return;
    }

    if (!hasDesktopCommand("cancelTask")) {
      automationActions.taskWriteFailed("This desktop build does not expose cancelTask yet.");
      return;
    }

    automationActions.taskWriteStarted("cancel");
    try {
      await automationDesktop.cancelTask(taskId);
      automationActions.taskWriteFinished();
      await refreshRunDetailInternal();
    } catch (error) {
      automationActions.taskWriteFailed(toErrorMessage(error));
    }
  }

  async function confirmRunManualGateInternal() {
    const requestId =
      automation.runDetail?.manualGateRequestId ??
      automation.launchedRun?.manualGateRequestId ??
      selectedRun?.manualGateRequestId;
    if (!requestId) {
      automationActions.taskWriteFailed("No manual gate request is available to confirm.");
      return;
    }

    if (!hasDesktopCommand("confirmManualGate")) {
      automationActions.taskWriteFailed(
        "This desktop build does not expose confirmManualGate yet.",
      );
      return;
    }

    automationActions.taskWriteStarted("confirm_manual_gate");
    try {
      const request: DesktopManualGateActionRequest = {
        manualGateRequestId: requestId,
      };
      await automationDesktop.confirmManualGate(request);
      automationActions.taskWriteFinished();
      await refreshRunDetailInternal();
    } catch (error) {
      automationActions.taskWriteFailed(toErrorMessage(error));
    }
  }

  async function rejectRunManualGateInternal() {
    const requestId =
      automation.runDetail?.manualGateRequestId ??
      automation.launchedRun?.manualGateRequestId ??
      selectedRun?.manualGateRequestId;
    if (!requestId) {
      automationActions.taskWriteFailed("No manual gate request is available to reject.");
      return;
    }

    if (!hasDesktopCommand("rejectManualGate")) {
      automationActions.taskWriteFailed(
        "This desktop build does not expose rejectManualGate yet.",
      );
      return;
    }

    automationActions.taskWriteStarted("reject_manual_gate");
    try {
      const request: DesktopManualGateActionRequest = {
        manualGateRequestId: requestId,
      };
      await automationDesktop.rejectManualGate(request);
      automationActions.taskWriteFinished();
      await refreshRunDetailInternal();
    } catch (error) {
      automationActions.taskWriteFailed(toErrorMessage(error));
    }
  }

  return {
    runs,
    templates,
    recorder,
    automation,
    selectedRun,
    selectedTemplate: templates.selectedTemplate,
    recommendation,
    chainSummary,
    compileDraft,
    runDetail: automation.runDetail,
    isRunDetailLoading,
    runDetailNotice,
    manualGate,
    isLaunchingRun,
    lastLaunchResult,
    actionFeedback,
    actionState,
    metrics: {
      activeRunCount,
      templateCount: templates.state.items.length,
      readyTemplateCount: templates.readyCount,
      contractGapCount: automation.contractGaps.filter((gap) => gap.status !== "Ready").length,
      blockerCount: chainSummary.blockers.length,
      warningCount: chainSummary.warnings.length,
      recorderStepCount: recorder.state.snapshot?.stepCount ?? 0,
    },
    actions: {
      selectRun: automationActions.selectRun,
      setTemplateSearchInput: templates.actions.setSearchInput,
      selectTemplate: templates.actions.selectTemplate,
      refreshTemplates: templates.actions.refresh,
      setBindingValue(variableKey: string, value: string) {
        if (!templates.selectedTemplate) {
          return;
        }

        templates.actions.setBindingValue(templates.selectedTemplate.id, variableKey, value);
      },
      setBindingNote(value: string) {
        if (!templates.selectedTemplate) {
          return;
        }

        templates.actions.setBindingNote(templates.selectedTemplate.id, value);
      },
      setBindingProfileIdsInput(value: string) {
        if (!templates.selectedTemplate) {
          return;
        }

        templates.actions.setBindingProfileIdsInput(templates.selectedTemplate.id, value);
      },
      resetBindingDraft() {
        if (!templates.selectedTemplate) {
          return;
        }

        templates.actions.resetBindingDraft(templates.selectedTemplate.id);
      },
      setLaunchMode: automationActions.setLaunchMode,
      setTargetScope: automationActions.setTargetScope,
      setLaunchNote: automationActions.setLaunchNote,
      async prepareLaunch() {
        if (!templates.selectedTemplate || !compileDraft) {
          return;
        }

        automationActions.prepareLaunchStarted();

        let compilePreview: PreparedLaunchCompilePreview;
        if (compileDraft.missingRequiredKeys.length > 0) {
          const blockers = compileDraft.missingRequiredKeys.map(
            (key) => `Missing required binding: ${key}`,
          );
          compilePreview = {
            status: "blocked",
            kind: "preflight_blocked",
            message: `Blocked before compile: ${compileDraft.missingRequiredKeys.join(", ")}`,
            acceptedProfileCount: 0,
            compiledAtLabel: null,
            blockers,
            warnings: [...compileDraft.warnings],
          };
        } else if (compileDraft.targetProfileIds.length === 0) {
          compilePreview = {
            status: "blocked",
            kind: "preflight_blocked",
            message: "Blocked before compile: no target profile is bound yet.",
            acceptedProfileCount: 0,
            compiledAtLabel: null,
            blockers: ["Bind explicit profile ids or select a run that already has persona context."],
            warnings: [...compileDraft.warnings],
          };
        } else {
          try {
            const result = await desktopServices.compileTemplateRun({
              templateId: compileDraft.templateId,
              storeId: templates.selectedTemplate.storeId,
              profileIds: compileDraft.targetProfileIds,
              variableBindings: Object.fromEntries(
                compileDraft.bindings.map((binding) => [binding.key, binding.value]),
              ),
              dryRun: automation.launcherDraft.mode !== "queue",
            });

            compilePreview = {
              status: "ready",
              kind: "native_success",
              message:
                result.message ||
                `Compile preview accepted ${result.acceptedProfileCount} target profiles.`,
              acceptedProfileCount: result.acceptedProfileCount,
              compiledAtLabel: formatTimeLabel(result.compiledAt),
              blockers: [],
              warnings: [...compileDraft.warnings],
            };
          } catch (error) {
            compilePreview = isCommandNotReady(error)
              ? {
                  status: "blocked",
                  kind: "native_blocked",
                  message:
                    "This desktop build does not expose the compile manifest contract yet. The launch draft is still staged locally.",
                  acceptedProfileCount: 0,
                  compiledAtLabel: null,
                  blockers: [
                    "Upgrade the desktop shared base to a build that includes compileTemplateRun.",
                    "Launch queue command is also still missing, so this request can only stay in prepared state.",
                  ],
                  warnings: [...compileDraft.warnings],
                }
              : {
                  status: "failed",
                  kind: "probe_failed",
                  message: toErrorMessage(error),
                  acceptedProfileCount: 0,
                  compiledAtLabel: null,
                  blockers: ["Compile manifest write failed before launch preparation completed."],
                  warnings: [...compileDraft.warnings],
                };
          }
        }

        automationActions.prepareLaunch({
          templateId: templates.selectedTemplate.id,
          templateName: templates.selectedTemplate.name,
          stepCount: compileDraft.stepCount,
          recorderStepCount: compileDraft.recorderStepCount,
          variableCount: compileDraft.bindings.length,
          boundProfileIds: compileDraft.targetProfileIds,
          recorderSessionId: compileDraft.recorderSessionId,
          missingRequiredKeys: compileDraft.missingRequiredKeys,
          warnings: compileDraft.warnings,
          note: compileDraft.note,
          ready: compileDraft.ready && compilePreview.status === "ready",
          compilePreview,
        });
      },
      async launchPreparedRun() {
        await launchPreparedRunInternal();
      },
      async refreshRunDetail() {
        await refreshRunDetailInternal();
      },
      async launchRun() {
        await launchPreparedRunInternal();
      },
      async retryRunTask() {
        await retryRunTaskInternal();
      },
      async cancelRunTask() {
        await cancelRunTaskInternal();
      },
      async confirmRunManualGate() {
        await confirmRunManualGateInternal();
      },
      async rejectRunManualGate() {
        await rejectRunManualGateInternal();
      },
      async retryRun() {
        await retryRunTaskInternal();
      },
      async cancelRun() {
        await cancelRunTaskInternal();
      },
      async approveManualGate() {
        await confirmRunManualGateInternal();
      },
      async rejectManualGate() {
        await rejectRunManualGateInternal();
      },
      resetLaunch: automationActions.resetLauncherDraft,
      refreshRecorder() {
        void recorder.actions.refresh(templates.selectedTemplate, {
          profileId: selectedRun?.personaId ?? null,
        });
      },
      startRecorder() {
        if (!templates.selectedTemplate) {
          return;
        }

        void recorder.actions.startSession(templates.selectedTemplate, {
          profileId: selectedRun?.personaId ?? null,
        });
      },
      pauseRecorder: recorder.actions.pauseDraftSession,
      stopRecorder: () => void recorder.actions.stopSession(),
      captureNextRecorderStep() {
        if (!templates.selectedTemplate) {
          return;
        }

        void recorder.actions.captureNextStep(templates.selectedTemplate);
      },
      selectRecorderStep: recorder.actions.selectStep,
    },
  };
}
