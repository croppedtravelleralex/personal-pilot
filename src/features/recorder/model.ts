import type {
  DesktopAppendBehaviorRecordingStepRequest,
  DesktopRecorderSnapshot,
  DesktopRecorderStep,
} from "../../types/desktop";
import type { TemplateSummary } from "../templates/model";

export type RecorderSnapshotSource = "desktop" | "adapter_fallback";

export interface RecorderVariableCandidate {
  key: string;
  label: string;
  stepId: string;
  actionType: string;
  previewValue: string;
  sensitive: boolean;
}

export interface RecorderStepTimelineItem {
  id: string;
  index: number;
  actionType: string;
  label: string;
  tabLabel: string;
  url: string | null;
  selector: string | null;
  detail: string;
  valuePreview: string | null;
  sensitive: boolean;
  capturedAt: string;
}

export interface RecorderSessionModel {
  sessionId: string;
  status: string;
  source: RecorderSnapshotSource;
  profileId: string | null;
  platformId: string | null;
  templateId: string | null;
  currentUrl: string | null;
  currentTabId: string | null;
  isDirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  stepCount: number;
  variableCount: number;
  startedAt: string | null;
  stoppedAt: string | null;
  updatedAt: string;
  note: string;
  steps: RecorderStepTimelineItem[];
  variableCandidates: RecorderVariableCandidate[];
}

export interface RecorderSeedContext {
  profileId?: string | null;
  platformId?: string | null;
}

function toEpochSeconds(value: string): string {
  return String(Math.floor(new Date(value).getTime() / 1000));
}

function buildVariableCandidatesFromTemplate(
  template: TemplateSummary,
  steps: RecorderStepTimelineItem[],
): RecorderVariableCandidate[] {
  return template.variables
    .map((variable) => {
      const matchedStep = steps.find((step) =>
        template.steps.find(
          (item) => item.id === step.id && item.variableKeys.includes(variable.key),
        ),
      );

      return {
        key: variable.key,
        label: variable.label,
        stepId: matchedStep?.id ?? template.steps[0]?.id ?? variable.key,
        actionType: matchedStep?.actionType ?? "input",
        previewValue:
          matchedStep?.valuePreview ??
          (variable.defaultValue || variable.example),
        sensitive: variable.sensitive,
      };
    })
    .filter((candidate) => candidate.previewValue.trim().length > 0);
}

function mapDesktopStep(step: DesktopRecorderStep): RecorderStepTimelineItem {
  const detailParts = [
    step.selector ? `Selector ${step.selector}` : null,
    step.url ? `URL ${step.url}` : null,
    step.waitMs ? `Wait ${step.waitMs}ms` : null,
  ].filter(Boolean);

  return {
    id: step.id,
    index: step.index,
    actionType: step.actionType,
    label: step.label,
    tabLabel: step.tabId ?? "Current",
    url: step.url,
    selector: step.selector,
    detail: detailParts.join(" | ") || "Desktop recorder step",
    valuePreview: step.valuePreview,
    sensitive: step.sensitive,
    capturedAt: step.capturedAt,
  };
}

function buildVariableCandidatesFromDesktop(
  steps: DesktopRecorderStep[],
): RecorderVariableCandidate[] {
  return steps
    .filter((step) => Boolean(step.inputKey) || Boolean(step.valuePreview))
    .map((step) => ({
      key: step.inputKey ?? step.id,
      label: step.inputKey ?? step.label,
      stepId: step.id,
      actionType: step.actionType,
      previewValue: step.valuePreview ?? "",
      sensitive: step.sensitive,
    }))
    .filter((candidate) => candidate.previewValue.trim().length > 0);
}

export function mapDesktopRecorderSnapshot(
  snapshot: DesktopRecorderSnapshot,
): RecorderSessionModel {
  const steps = snapshot.steps.map(mapDesktopStep);

  return {
    sessionId: snapshot.sessionId,
    status: snapshot.status,
    source: "desktop",
    profileId: snapshot.profileId,
    platformId: snapshot.platformId,
    templateId: snapshot.templateId,
    currentUrl: snapshot.currentUrl,
    currentTabId: snapshot.currentTabId,
    isDirty: snapshot.isDirty,
    canUndo: snapshot.canUndo,
    canRedo: snapshot.canRedo,
    stepCount: snapshot.stepCount,
    variableCount: snapshot.variableCount,
    startedAt: snapshot.startedAt,
    stoppedAt: snapshot.stoppedAt,
    updatedAt: snapshot.updatedAt,
    note: "Recorder snapshot loaded from desktop read model.",
    steps,
    variableCandidates: buildVariableCandidatesFromDesktop(snapshot.steps),
  };
}

function findVariableValue(template: TemplateSummary, variableKey: string | undefined): string | null {
  if (!variableKey) {
    return null;
  }

  const variable = template.variables.find((item) => item.key === variableKey);
  if (!variable) {
    return null;
  }

  return variable.defaultValue || variable.example || null;
}

function findTemplateVariable(template: TemplateSummary, variableKey: string | undefined) {
  if (!variableKey) {
    return null;
  }

  return template.variables.find((item) => item.key === variableKey) ?? null;
}

export function buildDesktopRecorderStepRequest(
  session: RecorderSessionModel | null,
  template: TemplateSummary,
): DesktopAppendBehaviorRecordingStepRequest | null {
  const nextTemplateStep = template.steps[session?.steps.length ?? 0];
  if (!nextTemplateStep) {
    return null;
  }

  const primaryVariableKey = nextTemplateStep.variableKeys[0];
  const primaryVariable = findTemplateVariable(template, primaryVariableKey);
  const primaryValue = findVariableValue(template, primaryVariableKey);
  const selector =
    nextTemplateStep.actionType === "click" || nextTemplateStep.actionType === "input"
      ? `[data-recorder-step="${nextTemplateStep.id}"]`
      : null;
  const currentUrl =
    nextTemplateStep.actionType === "visit" && primaryVariable?.kind === "url"
      ? primaryValue
      : session?.currentUrl ?? null;

  return {
    sessionId: session?.source === "desktop" ? session.sessionId : undefined,
    profileId: session?.profileId ?? null,
    platformId: session?.platformId ?? template.platformId,
    templateId: template.id,
    stepId: nextTemplateStep.id,
    index: nextTemplateStep.index,
    actionType: nextTemplateStep.actionType,
    label: nextTemplateStep.label,
    tabId: nextTemplateStep.tabLabel,
    url: currentUrl,
    selector,
    selectorSource: selector ? "template_outline" : null,
    inputKey: primaryVariableKey ?? null,
    valuePreview: primaryValue,
    valueSource: primaryVariableKey ? "variable" : null,
    waitMs: nextTemplateStep.waitMs,
    sensitive: nextTemplateStep.sensitive,
    metadata: {
      detail: nextTemplateStep.detail,
      templateStepId: nextTemplateStep.id,
      variableKeys: nextTemplateStep.variableKeys,
      captureMode: "desktop_operator_append",
    },
  };
}

export function createFallbackRecorderSession(
  template: TemplateSummary,
  context: RecorderSeedContext = {},
): RecorderSessionModel {
  const capturedCount = Math.min(
    Math.max(2, Math.ceil(template.steps.length / 2)),
    template.steps.length,
  );

  const capturedSteps = template.steps.slice(0, capturedCount).map((step) => {
    const primaryValue = findVariableValue(template, step.variableKeys[0]);

    return {
      id: step.id,
      index: step.index,
      actionType: step.actionType,
      label: step.label,
      tabLabel: step.tabLabel,
      url: step.variableKeys[0] ? primaryValue : null,
      selector:
        step.actionType === "click" || step.actionType === "input"
          ? "[data-adapter]"
          : null,
      detail: step.detail,
      valuePreview: primaryValue,
      sensitive: step.sensitive,
      capturedAt: toEpochSeconds(
        `2026-04-15T21:${String(12 + step.index).padStart(2, "0")}:00+08:00`,
      ),
    };
  });

  const currentUrlVariable = template.variables.find((variable) => variable.kind === "url");

  return {
    sessionId: `${template.id}-adapter-session`,
    status: "paused",
    source: "adapter_fallback",
    profileId: context.profileId ?? null,
    platformId: context.platformId ?? template.platformId,
    templateId: template.id,
    currentUrl: currentUrlVariable?.defaultValue || currentUrlVariable?.example || null,
    currentTabId: capturedSteps[0]?.tabLabel ?? "Draft",
    isDirty: true,
    canUndo: capturedSteps.length > 0,
    canRedo: false,
    stepCount: capturedSteps.length,
    variableCount: template.variables.length,
    startedAt: toEpochSeconds("2026-04-15T21:10:00+08:00"),
    stoppedAt: null,
    updatedAt: String(Math.floor(Date.now() / 1000)),
    note:
      "Recorder desktop contract is not ready yet, so the adapter keeps a local draft session and timeline alive.",
    steps: capturedSteps,
    variableCandidates: buildVariableCandidatesFromTemplate(template, capturedSteps),
  };
}

export function appendNextFallbackRecorderStep(
  session: RecorderSessionModel,
  template: TemplateSummary,
): RecorderSessionModel {
  const nextTemplateStep = template.steps[session.steps.length];
  if (!nextTemplateStep) {
    return session;
  }

  const primaryValue = findVariableValue(template, nextTemplateStep.variableKeys[0]);

  const nextStep: RecorderStepTimelineItem = {
    id: nextTemplateStep.id,
    index: nextTemplateStep.index,
    actionType: nextTemplateStep.actionType,
    label: nextTemplateStep.label,
    tabLabel: nextTemplateStep.tabLabel,
    url:
      nextTemplateStep.variableKeys[0] &&
      template.variables.find((item) => item.key === nextTemplateStep.variableKeys[0])?.kind ===
        "url"
        ? primaryValue
        : null,
    selector:
      nextTemplateStep.actionType === "click" || nextTemplateStep.actionType === "input"
        ? "[data-local-recorder]"
        : null,
    detail: nextTemplateStep.detail,
    valuePreview: primaryValue,
    sensitive: nextTemplateStep.sensitive,
    capturedAt: String(Math.floor(Date.now() / 1000)),
  };

  const steps = [...session.steps, nextStep];

  return {
    ...session,
    status: "recording",
    isDirty: true,
    canUndo: true,
    stepCount: steps.length,
    updatedAt: String(Math.floor(Date.now() / 1000)),
    steps,
    variableCandidates: buildVariableCandidatesFromTemplate(template, steps),
  };
}
