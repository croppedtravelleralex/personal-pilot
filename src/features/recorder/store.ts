import * as desktop from "../../services/desktop";
import type { DesktopServiceError } from "../../services/desktop";
import { createStore } from "../../store/createStore";
import type { TemplateSummary } from "../templates/model";
import {
  appendNextFallbackRecorderStep,
  buildDesktopRecorderStepRequest,
  createFallbackRecorderSession,
  mapDesktopRecorderSnapshot,
  type RecorderSessionModel,
} from "./model";

interface RecorderState {
  snapshot: RecorderSessionModel | null;
  selectedStepId: string | null;
  isLoading: boolean;
  error: string | null;
  requestId: number;
  sourceMessage: string;
}

const recorderStore = createStore<RecorderState>({
  snapshot: null,
  selectedStepId: null,
  isLoading: false,
  error: null,
  requestId: 0,
  sourceMessage:
    "Recorder desktop read/start/stop contracts are primary; local draft capture only fills missing step depth.",
});

function isCommandNotReady(error: unknown): error is DesktopServiceError {
  return (
    Boolean(error) &&
    typeof error === "object" &&
    "code" in error &&
    (error as { code?: string }).code === "desktop_command_not_ready"
  );
}

function toErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

function buildFallbackSnapshot(
  currentSnapshot: RecorderSessionModel | null,
  template: TemplateSummary,
  context?: { profileId?: string | null },
): RecorderSessionModel {
  if (currentSnapshot && currentSnapshot.templateId === template.id) {
    return currentSnapshot;
  }

  return createFallbackRecorderSession(template, {
    profileId: context?.profileId,
    platformId: template.platformId,
  });
}

function startFallbackSnapshot(
  currentSnapshot: RecorderSessionModel | null,
  template: TemplateSummary,
  context?: { profileId?: string | null },
): RecorderSessionModel {
  const baseSnapshot = buildFallbackSnapshot(currentSnapshot, template, context);

  return {
    ...baseSnapshot,
    status: "recording",
    stoppedAt: null,
    updatedAt: String(Math.floor(Date.now() / 1000)),
  };
}

function stopFallbackSnapshot(snapshot: RecorderSessionModel): RecorderSessionModel {
  const now = String(Math.floor(Date.now() / 1000));

  return {
    ...snapshot,
    status: "stopped",
    stoppedAt: now,
    updatedAt: now,
  };
}

export const recorderActions = {
  selectStep(stepId: string) {
    recorderStore.setState((current) => ({
      ...current,
      selectedStepId: stepId,
    }));
  },
  startDraftSession(template: TemplateSummary, context?: { profileId?: string | null }) {
    recorderStore.setState((current) => {
      const snapshot = startFallbackSnapshot(current.snapshot, template, context);

      return {
        ...current,
        snapshot,
        selectedStepId: snapshot.steps[0]?.id ?? current.selectedStepId,
        error: null,
        sourceMessage:
          "Recorder stayed on local draft capture because no desktop start contract is available.",
      };
    });
  },
  async startSession(template: TemplateSummary, context?: { profileId?: string | null }) {
    const requestId = recorderStore.getState().requestId + 1;

    recorderStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
      sourceMessage: "Starting recorder session through the desktop service...",
    }));

    try {
      const currentSnapshot = recorderStore.getState().snapshot;
      const desktopSnapshot = await desktop.startBehaviorRecording({
        sessionId:
          currentSnapshot?.source === "desktop" && currentSnapshot.templateId === template.id
            ? currentSnapshot.sessionId
            : undefined,
        profileId: context?.profileId ?? null,
        platformId: template.platformId,
        templateId: template.id,
      });

      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      const mapped = mapDesktopRecorderSnapshot(desktopSnapshot);

      recorderStore.setState((current) => ({
        ...current,
        snapshot: mapped,
        selectedStepId: mapped.steps[0]?.id ?? current.selectedStepId,
        isLoading: false,
        error: null,
        sourceMessage:
          "Recorder desktop session is active. Local draft capture only fills missing step depth.",
      }));
    } catch (error) {
      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      if (isCommandNotReady(error)) {
        const fallback = startFallbackSnapshot(recorderStore.getState().snapshot, template, context);

        recorderStore.setState((current) => ({
          ...current,
          snapshot: fallback,
          selectedStepId: fallback.steps[0]?.id ?? current.selectedStepId,
          isLoading: false,
          error: null,
          sourceMessage:
            "This desktop build does not expose recorder start yet. The workbench stays on local draft capture.",
        }));
        return;
      }

      recorderStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: toErrorMessage(error, "Failed to start recorder session"),
        sourceMessage: "Recorder start failed before a native session could be established.",
      }));
    }
  },
  pauseDraftSession() {
    recorderStore.setState((current) => {
      if (!current.snapshot) {
        return current;
      }

      return {
        ...current,
        snapshot: {
          ...current.snapshot,
          status: "paused",
          updatedAt: String(Math.floor(Date.now() / 1000)),
        },
        sourceMessage:
          current.snapshot.source === "desktop"
            ? "Desktop recorder session stays primary. Pause is still a local marker until a native pause command lands."
            : current.sourceMessage,
      };
    });
  },
  stopDraftSession() {
    recorderStore.setState((current) => {
      if (!current.snapshot) {
        return current;
      }

      return {
        ...current,
        snapshot: stopFallbackSnapshot(current.snapshot),
        sourceMessage:
          current.snapshot.source === "desktop"
            ? "Recorder session closed locally after desktop stop fallback."
            : "Local draft recorder session stopped.",
      };
    });
  },
  async stopSession() {
    const snapshot = recorderStore.getState().snapshot;
    if (!snapshot) {
      return;
    }

    if (snapshot.source !== "desktop") {
      recorderStore.setState((current) => ({
        ...current,
        snapshot: current.snapshot ? stopFallbackSnapshot(current.snapshot) : null,
        sourceMessage: "Local draft recorder session stopped.",
      }));
      return;
    }

    const requestId = recorderStore.getState().requestId + 1;
    recorderStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
      sourceMessage: "Stopping recorder session through the desktop service...",
    }));

    try {
      const desktopSnapshot = await desktop.stopBehaviorRecording({
        sessionId: snapshot.sessionId,
      });

      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      const mapped = mapDesktopRecorderSnapshot(desktopSnapshot);

      recorderStore.setState((current) => ({
        ...current,
        snapshot: mapped,
        selectedStepId: mapped.steps[0]?.id ?? current.selectedStepId,
        isLoading: false,
        error: null,
        sourceMessage: "Recorder session stopped through the desktop service.",
      }));
    } catch (error) {
      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      if (isCommandNotReady(error)) {
        recorderStore.setState((current) => ({
          ...current,
          snapshot: current.snapshot ? stopFallbackSnapshot(current.snapshot) : null,
          isLoading: false,
          error: null,
          sourceMessage:
            "This desktop build does not expose recorder stop yet, so the current session was closed locally.",
        }));
        return;
      }

      recorderStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: toErrorMessage(error, "Failed to stop recorder session"),
        sourceMessage: "Recorder stop failed and the native session remains unchanged.",
      }));
    }
  },
  async captureNextStep(template: TemplateSummary) {
    const currentSnapshot = recorderStore.getState().snapshot;
    const request = buildDesktopRecorderStepRequest(currentSnapshot, template);
    if (!request) {
      return;
    }

    const requestId = recorderStore.getState().requestId + 1;
    recorderStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
      sourceMessage: "Appending the next recorder step through the desktop service...",
    }));

    try {
      const desktopSnapshot = await desktop.appendBehaviorRecordingStep(request);
      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      const mapped = mapDesktopRecorderSnapshot(desktopSnapshot);
      recorderStore.setState((current) => ({
        ...current,
        snapshot: mapped,
        selectedStepId: mapped.steps.at(-1)?.id ?? current.selectedStepId,
        isLoading: false,
        error: null,
        sourceMessage:
          "Recorder timeline now appends through the desktop contract. Step content still follows the current template outline until live browser capture lands.",
      }));
    } catch (error) {
      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      if (isCommandNotReady(error)) {
        recorderStore.setState((current) => {
          if (!current.snapshot) {
            const fallback = appendNextFallbackRecorderStep(
              createFallbackRecorderSession(template, {
                platformId: template.platformId,
              }),
              template,
            );

            return {
              ...current,
              snapshot: fallback,
              selectedStepId: fallback.steps.at(-1)?.id ?? null,
              isLoading: false,
              error: null,
              sourceMessage:
                "Recorder timeline is using adapter-assisted capture because no native step-write command exists yet.",
            };
          }

          const nextSnapshot = appendNextFallbackRecorderStep(current.snapshot, template);
          if (nextSnapshot === current.snapshot) {
            return {
              ...current,
              isLoading: false,
            };
          }

          return {
            ...current,
            snapshot: nextSnapshot,
            selectedStepId: nextSnapshot.steps.at(-1)?.id ?? current.selectedStepId,
            isLoading: false,
            error: null,
            sourceMessage:
              current.snapshot.source === "desktop"
                ? "Desktop recorder session stays primary. 'Capture next step' adds an adapter-assisted preview until a native step-write command lands."
                : current.sourceMessage,
          };
        });
        return;
      }

      recorderStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: toErrorMessage(error, "Failed to append recorder step"),
        sourceMessage:
          "Recorder step append failed before the desktop session timeline could be updated.",
      }));
    }
  },
  async refresh(template: TemplateSummary | null, context?: { profileId?: string | null }) {
    if (!template) {
      recorderStore.setState((current) => ({
        ...current,
        snapshot: null,
        selectedStepId: null,
        error: null,
      }));
      return;
    }

    const snapshot = recorderStore.getState();
    const requestId = snapshot.requestId + 1;
    recorderStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
      sourceMessage:
        current.snapshot?.source === "desktop"
          ? "Refreshing recorder snapshot from the desktop read model..."
          : current.sourceMessage,
    }));

    try {
      const desktopSnapshot = await desktop.readRecorderSnapshot({
        sessionId:
          snapshot.snapshot?.source === "desktop" &&
          snapshot.snapshot.templateId === template.id
            ? snapshot.snapshot.sessionId
            : undefined,
        templateId: template.id,
        profileId: context?.profileId ?? undefined,
        platformId: template.platformId,
      });

      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      const mapped = mapDesktopRecorderSnapshot(desktopSnapshot);

      recorderStore.setState((current) => ({
        ...current,
        snapshot: mapped,
        selectedStepId:
          mapped.steps.find((step) => step.id === current.selectedStepId)?.id ??
          mapped.steps[0]?.id ??
          null,
        isLoading: false,
        error: null,
        sourceMessage: "Recorder snapshot loaded from the desktop read model.",
      }));
    } catch (error) {
      if (recorderStore.getState().requestId !== requestId) {
        return;
      }

      const fallback = buildFallbackSnapshot(snapshot.snapshot, template, context);

      recorderStore.setState((current) => ({
        ...current,
        snapshot: fallback,
        selectedStepId:
          fallback.steps.find((step) => step.id === current.selectedStepId)?.id ??
          fallback.steps[0]?.id ??
          null,
        isLoading: false,
        error: isCommandNotReady(error)
          ? null
          : toErrorMessage(error, "Failed to load recorder snapshot"),
        sourceMessage: isCommandNotReady(error)
          ? "This desktop build cannot read recorder state for the current context, so the local draft session remains available."
          : "Recorder snapshot fell back to a local draft session after desktop loading failed.",
      }));
    }
  },
};

export { recorderStore };
