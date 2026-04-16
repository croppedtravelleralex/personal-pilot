import type { DesktopTaskItem } from "../../types/desktop";
import type { RecorderSessionModel } from "../recorder/model";
import type {
  TemplateCompileRequestDraft,
  TemplateSummary,
  TemplateVariable,
} from "../templates/model";
import type {
  AutomationLaunchOutcome,
  AutomationRunDetail,
  PreparedLaunchPlan,
} from "./model";

export interface AutomationTemplateRecommendation {
  templateId: string;
  templateName: string;
  reason: string;
  confidence: "high" | "medium" | "low";
}

export interface AutomationChainSummary {
  tone: "success" | "warning" | "danger" | "neutral";
  headline: string;
  detail: string;
  blockers: string[];
  warnings: string[];
}

function tokenize(value: string | null | undefined): string[] {
  if (!value) {
    return [];
  }

  return value
    .toLowerCase()
    .split(/[^a-z0-9]+/i)
    .map((item) => item.trim())
    .filter((item) => item.length >= 3);
}

function countKeywordOverlap(left: string[], right: string[]): number {
  if (left.length === 0 || right.length === 0) {
    return 0;
  }

  const rightSet = new Set(right);
  return left.filter((item) => rightSet.has(item)).length;
}

export function getRecommendedTemplate(
  selectedRun: DesktopTaskItem | null,
  templates: TemplateSummary[],
): AutomationTemplateRecommendation | null {
  if (!selectedRun || templates.length === 0) {
    return null;
  }

  const runKeywords = [
    ...tokenize(selectedRun.kind),
    ...tokenize(selectedRun.title),
    ...tokenize(selectedRun.contentPreview),
    ...tokenize(selectedRun.platformId),
  ];

  let bestMatch:
    | {
        template: TemplateSummary;
        score: number;
        platformMatch: boolean;
        keywordOverlap: number;
      }
    | null = null;

  templates.forEach((template) => {
    const templateKeywords = [
      ...tokenize(template.name),
      ...tokenize(template.category),
      ...tokenize(template.summary),
      ...tokenize(template.platformId),
      ...tokenize(template.profileScope),
    ];
    const platformMatch =
      Boolean(selectedRun.platformId) && selectedRun.platformId === template.platformId;
    const keywordOverlap = countKeywordOverlap(runKeywords, templateKeywords);
    const score =
      (platformMatch ? 120 : 0) +
      keywordOverlap * 18 +
      (template.status === "ready" ? 12 : 0) +
      (template.readinessLevel === "ready" ? 8 : 0);

    if (!bestMatch || score > bestMatch.score) {
      bestMatch = {
        template,
        score,
        platformMatch,
        keywordOverlap,
      };
    }
  });

  if (!bestMatch || bestMatch.score <= 0) {
    return null;
  }

  const reason = bestMatch.platformMatch
    ? bestMatch.keywordOverlap > 0
      ? "模板平台与当前 run 的关键语义都较贴近。"
      : "模板平台与当前选中的 run 完全对齐。"
    : "模板摘要与当前 run 的标题或内容更接近。";

  return {
    templateId: bestMatch.template.id,
    templateName: bestMatch.template.name,
    reason,
    confidence:
      bestMatch.score >= 130 ? "high" : bestMatch.score >= 70 ? "medium" : "low",
  };
}

export function getRunContextBindingValue(
  variable: TemplateVariable,
  selectedRun: DesktopTaskItem | null,
): string | null {
  if (!selectedRun) {
    return null;
  }

  const hint = `${variable.key} ${variable.label}`.toLowerCase();

  if (variable.kind === "url" && selectedRun.finalUrl) {
    return selectedRun.finalUrl;
  }

  if ((hint.includes("title") || hint.includes("subject")) && selectedRun.title) {
    return selectedRun.title;
  }

  if (
    (variable.kind === "textarea" ||
      hint.includes("text") ||
      hint.includes("body") ||
      hint.includes("reply") ||
      hint.includes("comment")) &&
    selectedRun.contentPreview
  ) {
    return selectedRun.contentPreview;
  }

  if ((hint.includes("surface") || hint.includes("platform")) && selectedRun.platformId) {
    return selectedRun.platformId;
  }

  return null;
}

function buildLaunchStateWarnings(
  selectedRun: DesktopTaskItem | null,
  lastPreparedLaunch: PreparedLaunchPlan | null,
  launchedRun: AutomationLaunchOutcome | null,
  runDetail: AutomationRunDetail | null,
): string[] {
  const warnings: string[] = [];

  if (
    lastPreparedLaunch &&
    selectedRun &&
    lastPreparedLaunch.sourceRunId &&
    lastPreparedLaunch.sourceRunId !== selectedRun.id
  ) {
    warnings.push("Prepared launch belongs to another run. Re-prepare before dispatching.");
  }

  if (
    launchedRun &&
    selectedRun?.manualGateRequestId &&
    launchedRun.manualGateRequestId !== selectedRun.manualGateRequestId
  ) {
    warnings.push("The selected run and the dispatched run expose different manual gate ids.");
  }

  if (runDetail?.manualGateRequestId && runDetail.manualGateStatus !== "confirmed") {
    warnings.push(
      runDetail.manualGateStatus === "rejected"
        ? "Manual gate was rejected. Retry or re-dispatch may be required."
        : "Manual gate is still pending operator confirmation.",
    );
  }

  if (runDetail?.failureReason) {
    warnings.push(`Latest run detail reports a failure reason: ${runDetail.failureReason}`);
  }

  return warnings;
}

export function buildAutomationChainSummary(input: {
  selectedRun: DesktopTaskItem | null;
  selectedTemplate: TemplateSummary | null;
  compileDraft: TemplateCompileRequestDraft | null;
  recorderSnapshot: RecorderSessionModel | null;
  lastPreparedLaunch: PreparedLaunchPlan | null;
  recommendation: AutomationTemplateRecommendation | null;
  launchedRun: AutomationLaunchOutcome | null;
  runDetail: AutomationRunDetail | null;
  launchStatus: string;
  runDetailStatus: string;
  launchFailureReason: string | null;
  runDetailFailureReason: string | null;
}): AutomationChainSummary {
  const {
    selectedRun,
    selectedTemplate,
    compileDraft,
    recorderSnapshot,
    lastPreparedLaunch,
    recommendation,
    launchedRun,
    runDetail,
    launchStatus,
    runDetailStatus,
    launchFailureReason,
    runDetailFailureReason,
  } = input;

  const blockers: string[] = [];
  const warnings: string[] = [];

  if (!selectedRun) {
    blockers.push("Select a source run first so bindings and profile context can be resolved.");
  }

  if (!selectedTemplate) {
    blockers.push("Select a template before preparing recorder bindings and launch context.");
  }

  if (compileDraft?.missingRequiredKeys.length) {
    blockers.push(
      `Fill the required bindings first: ${compileDraft.missingRequiredKeys.join(", ")}.`,
    );
  }

  if (compileDraft && compileDraft.targetSource === "unbound") {
    blockers.push("Bind target profile ids or pick a run that already carries persona context.");
  }

  if (recorderSnapshot && recorderSnapshot.source !== "desktop") {
    warnings.push(
      "Recorder is still using an adapter-fallback session. Review the draft before dispatching.",
    );
  }

  if (
    selectedRun &&
    selectedTemplate &&
    selectedRun.platformId &&
    selectedRun.platformId !== selectedTemplate.platformId
  ) {
    warnings.push("The selected template platform does not match the current run platform.");
  }

  if (
    recommendation &&
    selectedTemplate &&
    recommendation.templateId !== selectedTemplate.id
  ) {
    warnings.push(
      `A closer template match exists: ${recommendation.templateName}. ${recommendation.reason}`,
    );
  }

  if (
    recorderSnapshot &&
    selectedTemplate &&
    recorderSnapshot.templateId &&
    recorderSnapshot.templateId !== selectedTemplate.id
  ) {
    warnings.push("Recorder session is bound to another template and may hydrate the wrong values.");
  }

  if (
    recorderSnapshot &&
    selectedRun?.personaId &&
    recorderSnapshot.profileId &&
    recorderSnapshot.profileId !== selectedRun.personaId
  ) {
    warnings.push("Recorder session is bound to another profile than the selected run.");
  }

  warnings.push(
    ...buildLaunchStateWarnings(selectedRun, lastPreparedLaunch, launchedRun, runDetail),
  );

  if (lastPreparedLaunch?.compilePreview.status === "blocked") {
    blockers.push(lastPreparedLaunch.compilePreview.message);
  }

  if (lastPreparedLaunch?.compilePreview.status === "failed") {
    blockers.push(`Compile manifest failed: ${lastPreparedLaunch.compilePreview.message}`);
  }

  if (launchStatus === "blocked" && launchFailureReason) {
    blockers.push(launchFailureReason);
  }

  if (launchStatus === "failed" && launchFailureReason) {
    blockers.push(`Launch failed: ${launchFailureReason}`);
  }

  if (runDetailStatus === "blocked" && runDetailFailureReason) {
    warnings.push(runDetailFailureReason);
  }

  if (runDetailStatus === "failed" && runDetailFailureReason) {
    warnings.push(`Run detail read failed: ${runDetailFailureReason}`);
  }

  if (runDetail?.status === "failed" && runDetail.failureReason) {
    blockers.push(`Runtime reports failure: ${runDetail.failureReason}`);
  }

  if (blockers.length === 0 && runDetail) {
    return {
      tone:
        runDetail.manualGateRequestId && runDetail.manualGateStatus !== "confirmed"
          ? "warning"
          : "success",
      headline:
        runDetail.manualGateRequestId && runDetail.manualGateStatus !== "confirmed"
          ? "Launch completed, but the run is waiting on a manual gate."
          : "Automation has been dispatched and run detail is live.",
      detail:
        runDetail.message ??
        `Run ${runDetail.runId} is now readable in the feature layer with status ${runDetail.status}.`,
      blockers,
      warnings,
    };
  }

  if (blockers.length === 0 && launchedRun) {
    return {
      tone: runDetailStatus === "blocked" ? "warning" : "success",
      headline: "Automation has been dispatched into the local runtime.",
      detail:
        runDetailStatus === "blocked"
          ? "Launch succeeded, but per-run detail is not available from the current desktop build yet."
          : launchedRun.message,
      blockers,
      warnings,
    };
  }

  if (blockers.length === 0 && lastPreparedLaunch?.compilePreview.status === "ready") {
    return {
      tone: "success",
      headline: "The chain is prepared and ready for runtime dispatch.",
      detail:
        "Template, bindings, recorder state, and compile manifest are aligned. The next step is launch dispatch.",
      blockers,
      warnings,
    };
  }

  if (blockers.length === 0) {
    return {
      tone: warnings.length > 0 ? "warning" : "neutral",
      headline: "The chain can keep moving, but runtime execution is not ready yet.",
      detail:
        lastPreparedLaunch?.compilePreview.message ??
        "Template, recorder, and draft state are connected. Keep moving toward prepare and launch.",
      blockers,
      warnings,
    };
  }

  return {
    tone: "danger",
    headline: "The automation chain still has blocking issues.",
    detail: blockers[0],
    blockers,
    warnings,
  };
}
