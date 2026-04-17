import { RecorderTimeline } from "../components/automation/RecorderTimeline";
import { RunDetailPanel } from "../components/automation/RunDetailPanel";
import { RunLauncher } from "../components/automation/RunLauncher";
import { RunsBoard } from "../components/automation/RunsBoard";
import { TemplatesBoard } from "../components/automation/TemplatesBoard";
import { StatCard } from "../components/StatCard";
import { useAutomationCenterViewModel } from "../features/automation/hooks";
import { AutomationTasksSection } from "./TasksPage";
import { formatCount } from "../utils/format";

export function AutomationPage() {
  const viewModel = useAutomationCenterViewModel();
  const dispatchReady = Boolean(viewModel.automation.lastPreparedLaunch?.ready);
  const dispatchStatus =
    viewModel.lastLaunchResult?.status ??
    (viewModel.isLaunchingRun ? "launching" : dispatchReady ? "ready" : "staged");
  const manualGateIds = [
    viewModel.selectedRun?.manualGateRequestId,
    viewModel.manualGate?.requestId,
  ].filter((item): item is string => Boolean(item));
  const manualGateCount = new Set(manualGateIds).size;
  const launchNotice =
    viewModel.actionFeedback &&
    viewModel.actionFeedback.message !== viewModel.automation.launcherNotice
      ? viewModel.actionFeedback.message
      : null;
  const launchNoticeTone = viewModel.actionFeedback?.tone ?? "info";
  const launchResult = viewModel.lastLaunchResult;

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
          value={dispatchStatus}
          hint={
            launchResult?.headline ??
            (viewModel.isLaunchingRun
              ? "Launch request is being written into local runtime."
              : dispatchReady
                ? "Prepared launch can be dispatched from this page."
                : "Prepare, review, then dispatch from this console.")
          }
          tone={
            launchResult?.status === "failed" || launchResult?.status === "blocked"
              ? "warning"
              : dispatchReady || viewModel.isLaunchingRun || Boolean(launchResult)
                ? "success"
                : "warning"
          }
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
            viewModel.manualGate?.headline ??
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
            {viewModel.isLaunchingRun
              ? "dispatch in progress"
              : dispatchReady
                ? "dispatch ready in this page"
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
              {launchResult?.detail ??
                (viewModel.automation.lastPreparedLaunch?.compilePreview.kind ??
                  "prepare launch to write manifest")}
            </small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Run detail</span>
            <strong>{viewModel.runDetail?.status ?? "live row"}</strong>
            <small>
              {viewModel.runDetail?.headline ??
                viewModel.runDetailNotice ??
                "Refresh run detail after dispatch to load timeline, artifacts, and gate state."}
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
            launch, detail, and manual-gate loop. It is not an AdsPower-grade orchestration
            control plane beyond the desktop commands already connected in this repo.
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
          isLaunching={viewModel.isLaunchingRun}
          launchNotice={launchNotice}
          launchNoticeTone={launchNoticeTone}
          lastLaunchResult={launchResult}
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
          onLaunch={viewModel.actions.launchRun}
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
        runDetail={viewModel.runDetail}
        isRunDetailLoading={viewModel.isRunDetailLoading}
        runDetailNotice={viewModel.runDetailNotice}
        manualGate={viewModel.manualGate}
        actionFeedback={viewModel.actionFeedback}
        actionState={viewModel.actionState}
        onRefreshRunDetail={viewModel.actions.refreshRunDetail}
        onRetryRun={viewModel.actions.retryRun}
        onCancelRun={viewModel.actions.cancelRun}
        onApproveGate={viewModel.actions.approveManualGate}
        onRejectGate={viewModel.actions.rejectManualGate}
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
