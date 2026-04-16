import { RecorderTimeline } from "../components/automation/RecorderTimeline";
import {
  RunDetailPanel,
  type RunActionFeedback,
  type RunActionState,
  type RunManualGateState,
} from "../components/automation/RunDetailPanel";
import {
  RunLauncher,
  type RunLauncherLaunchResult,
} from "../components/automation/RunLauncher";
import { RunsBoard } from "../components/automation/RunsBoard";
import { TemplatesBoard } from "../components/automation/TemplatesBoard";
import { StatCard } from "../components/StatCard";
import { useAutomationCenterViewModel } from "../features/automation/hooks";
import type {
  AutomationNoticeTone,
  AutomationRunDetail,
} from "../features/automation/model";
import { AutomationTasksSection } from "./TasksPage";
import { formatCount } from "../utils/format";

type OptionalActionBag = {
  launchRun?: () => void;
  refreshRunDetail?: () => void;
  retryRun?: () => void;
  cancelRun?: () => void;
  approveManualGate?: () => void;
  rejectManualGate?: () => void;
};

type OptionalAutomationBag = {
  isLaunchingRun?: boolean;
  launchNotice?: string | null;
  launchNoticeTone?: AutomationNoticeTone;
  lastLaunchResult?: RunLauncherLaunchResult | null;
  runDetail?: AutomationRunDetail | null;
  isRunDetailLoading?: boolean;
  runDetailNotice?: string | null;
  manualGate?: RunManualGateState | null;
  actionFeedback?: RunActionFeedback | null;
  actionState?: RunActionState | null;
};

function readOptionalActionBag(value: unknown): OptionalActionBag {
  if (!value || typeof value !== "object") {
    return {};
  }

  const bag = value as Record<string, unknown>;

  return {
    launchRun: typeof bag.launchRun === "function" ? (bag.launchRun as () => void) : undefined,
    refreshRunDetail:
      typeof bag.refreshRunDetail === "function"
        ? (bag.refreshRunDetail as () => void)
        : undefined,
    retryRun: typeof bag.retryRun === "function" ? (bag.retryRun as () => void) : undefined,
    cancelRun: typeof bag.cancelRun === "function" ? (bag.cancelRun as () => void) : undefined,
    approveManualGate:
      typeof bag.approveManualGate === "function"
        ? (bag.approveManualGate as () => void)
        : undefined,
    rejectManualGate:
      typeof bag.rejectManualGate === "function"
        ? (bag.rejectManualGate as () => void)
        : undefined,
  };
}

function readOptionalAutomationBag(value: unknown): OptionalAutomationBag {
  if (!value || typeof value !== "object") {
    return {};
  }

  const bag = value as Record<string, unknown>;

  return {
    isLaunchingRun:
      typeof bag.isLaunchingRun === "boolean" ? (bag.isLaunchingRun as boolean) : undefined,
    launchNotice: typeof bag.launchNotice === "string" ? bag.launchNotice : null,
    launchNoticeTone:
      typeof bag.launchNoticeTone === "string"
        ? (bag.launchNoticeTone as AutomationNoticeTone)
        : undefined,
    lastLaunchResult: (bag.lastLaunchResult as RunLauncherLaunchResult | null | undefined) ?? null,
    runDetail: (bag.runDetail as AutomationRunDetail | null | undefined) ?? null,
    isRunDetailLoading:
      typeof bag.isRunDetailLoading === "boolean"
        ? (bag.isRunDetailLoading as boolean)
        : undefined,
    runDetailNotice: typeof bag.runDetailNotice === "string" ? bag.runDetailNotice : null,
    manualGate: (bag.manualGate as RunManualGateState | null | undefined) ?? null,
    actionFeedback: (bag.actionFeedback as RunActionFeedback | null | undefined) ?? null,
    actionState: (bag.actionState as RunActionState | null | undefined) ?? null,
  };
}

export function AutomationPage() {
  const viewModel = useAutomationCenterViewModel();
  const optionalActions = readOptionalActionBag(viewModel.actions);
  const optionalAutomation = readOptionalAutomationBag(viewModel.automation);
  const dispatchReady = Boolean(
    viewModel.automation.lastPreparedLaunch?.ready && optionalActions.launchRun,
  );
  const manualGateCount =
    (viewModel.selectedRun?.manualGateRequestId ? 1 : 0) + (optionalAutomation.manualGate ? 1 : 0);

  return (
    <div className="page-stack automation-center">
      <div className="stat-grid">
        <StatCard
          label="Runs"
          value={formatCount(viewModel.runs.state.total)}
          hint="Paged local run inventory is live and still drives the operator view."
          tone="success"
        />
        <StatCard
          label="Dispatch"
          value={
            optionalAutomation.lastLaunchResult?.status ??
            (dispatchReady ? "ready" : "staged")
          }
          hint={
            optionalAutomation.lastLaunchResult?.headline ??
            (dispatchReady
              ? "Prepared launch can move into execution."
              : "Prepare, review, then dispatch from the same console.")
          }
          tone={dispatchReady ? "success" : "warning"}
        />
        <StatCard
          label="Recorder"
          value={formatCount(viewModel.metrics.recorderStepCount)}
          hint={
            viewModel.recorder.state.snapshot
              ? `${viewModel.recorder.state.snapshot.source} / ${viewModel.recorder.state.snapshot.status}`
              : "Recorder snapshot and native session state will appear here."
          }
          tone={
            viewModel.recorder.state.snapshot?.source === "desktop" ? "success" : "warning"
          }
        />
        <StatCard
          label="Manual Gates"
          value={formatCount(manualGateCount)}
          hint={
            optionalAutomation.manualGate?.headline ??
            (viewModel.selectedRun?.manualGateRequestId
              ? "Selected run is waiting for operator review."
              : "No manual gate is active on the current selection.")
          }
          tone={manualGateCount > 0 ? "warning" : "success"}
        />
      </div>

      <div className="toolbar-card automation-center__hero">
        <div className="automation-center__hero-copy">
          <span className="shell__eyebrow">Automation Center</span>
          <h2>Build, launch, track, and intervene from one local execution workbench.</h2>
          <p>
            Runs, templates, recorder context, launch staging, run detail, artifacts, and
            operator decisions now sit in one desktop work surface so operators can move from
            preflight to dispatch to review without leaving the page.
          </p>
        </div>
        <div className="automation-center__hero-aside">
          <span className="badge badge--info">
            {formatCount(viewModel.metrics.activeRunCount)} active runs on page
          </span>
          <span className={`badge ${dispatchReady ? "badge--info" : "badge--warning"}`}>
            {dispatchReady
              ? "launch path connected"
              : viewModel.automation.lastPreparedLaunch?.ready
                ? "prepared, awaiting dispatch wiring"
                : `${formatCount(viewModel.metrics.readyTemplateCount)} ready templates`}
          </span>
          <span
            className={`badge ${
              viewModel.recorder.state.snapshot?.source === "desktop"
                ? "badge--info"
                : "badge--warning"
            }`}
          >
            {viewModel.recorder.state.snapshot
              ? `${viewModel.recorder.state.snapshot.source} / ${viewModel.recorder.state.snapshot.status}`
              : "recorder pending"}
          </span>
        </div>
      </div>

      <div className="toolbar-card toolbar-card--subtle">
        <div className="automation-metric-strip automation-metric-strip--compact">
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Selected run</span>
            <strong>{viewModel.selectedRun?.title ?? viewModel.selectedRun?.kind ?? "No run"}</strong>
            <small>{viewModel.selectedRun?.personaId ?? "Waiting for persona context"}</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Selected template</span>
            <strong>{viewModel.selectedTemplate?.name ?? "No template"}</strong>
            <small>{viewModel.selectedTemplate?.platformId ?? "Waiting for template context"}</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Prepared launch</span>
            <strong>
              {viewModel.automation.lastPreparedLaunch?.compilePreview.status ?? "not prepared"}
            </strong>
            <small>
              {optionalAutomation.lastLaunchResult?.detail ??
                (viewModel.automation.lastPreparedLaunch?.compilePreview.kind ??
                  "prepare launch to write manifest")}
            </small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Run detail</span>
            <strong>{optionalAutomation.runDetail?.status ?? "live row"}</strong>
            <small>
              {optionalAutomation.runDetail?.headline ??
                optionalAutomation.runDetailNotice ??
                "Detailed timeline will appear once the run detail bridge is connected."}
            </small>
          </article>
        </div>
        <div
          className={`banner ${
            viewModel.chainSummary.tone === "danger"
              ? "banner--error"
              : viewModel.chainSummary.tone === "success"
                ? "banner--info"
                : "banner--warning"
          }`}
        >
          <strong>{viewModel.chainSummary.headline}</strong>
          <div>{viewModel.chainSummary.detail}</div>
        </div>
        <div className="banner banner--warning">
          <strong>Truth boundary</strong>
          <div>
            This page is a denser local execution workbench around the existing compile,
            launch, detail, and manual-gate loop. It does not imply vendor-grade orchestration
            features beyond the desktop commands already connected in this repo.
          </div>
        </div>
      </div>

      <div className="automation-center__grid">
        <RunsBoard
          state={viewModel.runs.state}
          totalPages={viewModel.runs.totalPages}
          selectedRunId={viewModel.selectedRun?.id ?? null}
          title="Runs Board"
          subtitle="Execution inventory remains live and now feeds launch staging, runtime inspection, and operator decisions."
          height={560}
          onSearchInputChange={viewModel.runs.actions.setSearchInput}
          onStatusFilterChange={viewModel.runs.actions.setStatusFilter}
          onPageSizeChange={viewModel.runs.actions.setPageSize}
          onPageChange={viewModel.runs.actions.setPage}
          onRefresh={viewModel.runs.actions.refresh}
          onSelectRun={viewModel.actions.selectRun}
        />

        <TemplatesBoard
          items={viewModel.templates.filteredItems}
          selectedTemplateId={viewModel.selectedTemplate?.id ?? null}
          totalCount={viewModel.templates.state.items.length}
          readyCount={viewModel.templates.readyCount}
          sourceMessage={viewModel.templates.state.sourceMessage}
          searchInput={viewModel.templates.state.searchInput}
          isLoading={viewModel.templates.state.isLoading}
          selectedRunPlatformId={viewModel.selectedRun?.platformId ?? null}
          recommendedTemplateId={viewModel.recommendation?.templateId ?? null}
          recommendedReason={viewModel.recommendation?.reason ?? null}
          onRefresh={viewModel.actions.refreshTemplates}
          onSearchInputChange={viewModel.actions.setTemplateSearchInput}
          onSelectTemplate={viewModel.actions.selectTemplate}
        />
      </div>

      <div className="automation-center__grid">
        <RunLauncher
          templates={viewModel.templates.state.items}
          selectedTemplateId={viewModel.selectedTemplate?.id ?? null}
          selectedTemplate={viewModel.selectedTemplate}
          bindingDraft={viewModel.templates.selectedBindingDraft}
          compileDraft={viewModel.compileDraft}
          draft={viewModel.automation.launcherDraft}
          launcherNotice={viewModel.automation.launcherNotice}
          launcherNoticeTone={viewModel.automation.launcherNoticeTone}
          lastPreparedLaunch={viewModel.automation.lastPreparedLaunch}
          isPreparingLaunch={viewModel.automation.isPreparingLaunch}
          selectedRun={viewModel.selectedRun}
          recorderSnapshot={viewModel.recorder.state.snapshot}
          recommendation={viewModel.recommendation}
          isLaunching={optionalAutomation.isLaunchingRun}
          launchNotice={optionalAutomation.launchNotice}
          launchNoticeTone={optionalAutomation.launchNoticeTone}
          lastLaunchResult={optionalAutomation.lastLaunchResult}
          onSelectTemplate={viewModel.actions.selectTemplate}
          onSetMode={viewModel.actions.setLaunchMode}
          onSetTargetScope={viewModel.actions.setTargetScope}
          onSetLaunchNote={viewModel.actions.setLaunchNote}
          onSetBindingValue={viewModel.actions.setBindingValue}
          onSetBindingNote={viewModel.actions.setBindingNote}
          onSetBindingProfileIdsInput={viewModel.actions.setBindingProfileIdsInput}
          onResetBindings={viewModel.actions.resetBindingDraft}
          onPrepareLaunch={viewModel.actions.prepareLaunch}
          onResetLaunch={viewModel.actions.resetLaunch}
          onLaunch={optionalActions.launchRun}
        />

        <RecorderTimeline
          snapshot={viewModel.recorder.state.snapshot}
          selectedStepId={viewModel.recorder.state.selectedStepId}
          sourceMessage={viewModel.recorder.state.sourceMessage}
          isLoading={viewModel.recorder.state.isLoading}
          selectedTemplateId={viewModel.selectedTemplate?.id ?? null}
          selectedProfileId={viewModel.selectedRun?.personaId ?? null}
          onRefresh={viewModel.actions.refreshRecorder}
          onStart={viewModel.actions.startRecorder}
          onPause={viewModel.actions.pauseRecorder}
          onCaptureNext={viewModel.actions.captureNextRecorderStep}
          onStop={viewModel.actions.stopRecorder}
          onSelectStep={viewModel.actions.selectRecorderStep}
        />
      </div>

      <RunDetailPanel
        selectedRun={viewModel.selectedRun}
        selectedTemplate={viewModel.selectedTemplate}
        compileDraft={viewModel.compileDraft}
        recorderSnapshot={viewModel.recorder.state.snapshot}
        lastPreparedLaunch={viewModel.automation.lastPreparedLaunch}
        contractGaps={viewModel.automation.contractGaps}
        recommendation={viewModel.recommendation}
        chainSummary={viewModel.chainSummary}
        runDetail={optionalAutomation.runDetail}
        isRunDetailLoading={optionalAutomation.isRunDetailLoading}
        runDetailNotice={optionalAutomation.runDetailNotice}
        manualGate={optionalAutomation.manualGate}
        actionFeedback={optionalAutomation.actionFeedback}
        actionState={optionalAutomation.actionState}
        onRefreshRunDetail={optionalActions.refreshRunDetail}
        onRetryRun={optionalActions.retryRun}
        onCancelRun={optionalActions.cancelRun}
        onApproveGate={optionalActions.approveManualGate}
        onRejectGate={optionalActions.rejectManualGate}
      />

      <div className="toolbar-card toolbar-card--subtle">
        <div className="automation-center__hero-copy">
          <span className="shell__eyebrow">Tasks Console Entry</span>
          <h2>Task queue control is now embedded in Automation Center.</h2>
          <p>
            Legacy <strong>#tasks</strong> links still resolve to this page, and queue-lane
            selection, batch retry/cancel, plus manual-gate actions now stay in the same
            operator surface.
          </p>
        </div>
      </div>

      <AutomationTasksSection viewModel={viewModel.runs} />
    </div>
  );
}
