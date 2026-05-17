import { EmptyState } from "../EmptyState";
import { Panel } from "../Panel";
import type {
  AutomationContractGap,
  AutomationNoticeTone,
  AutomationRunDetail,
  PreparedLaunchPlan,
} from "../../features/automation/model";
import type {
  AutomationChainSummary,
  AutomationTemplateRecommendation,
} from "../../features/automation/derived";
import type { RecorderSessionModel } from "../../features/recorder/model";
import type {
  TemplateCompileRequestDraft,
  TemplateSummary,
} from "../../features/templates/model";
import type { DesktopTaskItem } from "../../types/desktop";
import { formatRelativeTimestamp, formatStatusLabel } from "../../utils/format";

export interface RunManualGateState {
  requestId: string;
  status: string;
  headline: string;
  detail: string;
  decisionOptions?: string[];
  failureReason?: string | null;
}

export interface RunActionFeedback {
  tone: AutomationNoticeTone;
  message: string;
  updatedAtLabel?: string | null;
}

export interface RunActionState {
  isRefreshing?: boolean;
  isRetrying?: boolean;
  isCancelling?: boolean;
  isApprovingGate?: boolean;
  isRejectingGate?: boolean;
}

interface RunDetailPanelProps {
  selectedRun: DesktopTaskItem | null;
  selectedTemplate: TemplateSummary | null;
  compileDraft: TemplateCompileRequestDraft | null;
  recorderSnapshot: RecorderSessionModel | null;
  lastPreparedLaunch: PreparedLaunchPlan | null;
  contractGaps: AutomationContractGap[];
  recommendation: AutomationTemplateRecommendation | null;
  chainSummary: AutomationChainSummary;
  runDetail?: AutomationRunDetail | null;
  isRunDetailLoading?: boolean;
  runDetailNotice?: string | null;
  manualGate?: RunManualGateState | null;
  actionFeedback?: RunActionFeedback | null;
  actionState?: RunActionState | null;
  onRefreshRunDetail?: () => void;
  onRetryRun?: () => void;
  onCancelRun?: () => void;
  onApproveGate?: () => void;
  onRejectGate?: () => void;
}

function getSummaryBannerTone(tone: AutomationChainSummary["tone"]): AutomationNoticeTone {
  switch (tone) {
    case "danger":
      return "error";
    case "success":
      return "success";
    case "warning":
      return "warning";
    default:
      return "info";
  }
}

function getNoticeClassName(tone: AutomationNoticeTone) {
  if (tone === "error") {
    return "banner banner--error";
  }

  if (tone === "success" || tone === "info") {
    return "banner banner--info";
  }

  return "banner banner--warning";
}

function formatManualGateStatus(status: string | null | undefined): string {
  return status ? formatStatusLabel(status) : "CLEAR";
}

export function RunDetailPanel({
  selectedRun,
  selectedTemplate,
  compileDraft,
  recorderSnapshot,
  lastPreparedLaunch,
  contractGaps,
  recommendation,
  chainSummary,
  runDetail = null,
  isRunDetailLoading = false,
  runDetailNotice = null,
  manualGate = null,
  actionFeedback = null,
  actionState = null,
  onRefreshRunDetail,
  onRetryRun,
  onCancelRun,
  onApproveGate,
  onRejectGate,
}: RunDetailPanelProps) {
  const summaryTone = getSummaryBannerTone(chainSummary.tone);
  const summaryClassName = getNoticeClassName(summaryTone);
  const effectiveManualGate =
    manualGate ??
    (selectedRun?.manualGateRequestId
      ? {
          requestId: selectedRun.manualGateRequestId,
          status: "pending",
          headline: "Manual review is required before the run can continue.",
          detail:
            "A gate id exists on the selected run row, but richer request detail still depends on the connected desktop run-detail read path.",
          decisionOptions: ["Approve", "Reject"],
        }
      : null);
  const detailStatus = runDetail?.status ?? selectedRun?.status ?? "idle";
  const selectedTimelinePreview = recorderSnapshot?.steps.slice(0, 4) ?? [];
  const truthBoundaryCards = [
    {
      label: "Run detail source",
      detail: runDetail
        ? "Per-run detail is connected from the desktop read path and rendered directly."
        : "The panel falls back to the live run row plus recorder/template context when per-run detail is absent.",
    },
    {
      label: "Artifacts",
      detail:
        (runDetail?.artifacts.length ?? 0) > 0
          ? "Artifacts shown below come from the current run detail payload."
          : "Only returned artifacts, final URL, and content preview are shown. No hidden vendor artifact registry is implied.",
    },
    {
      label: "Operator decisions",
      detail: effectiveManualGate
        ? "Approve / reject only represent currently wired local manual-gate actions."
        : "No manual gate is active for the selected run right now.",
    },
  ];

  return (
    <Panel
      title="Run Operations"
      subtitle="Inspect runtime posture, evidence, and operator decisions from one local execution workbench without overstating the current desktop control surface."
      actions={
        <span className={`badge ${selectedRun ? `badge--${selectedRun.status}` : "badge--warning"}`}>
          {selectedRun ? formatStatusLabel(selectedRun.status) : "Awaiting selection"}
        </span>
      }
    >
      <div className="page-stack">
        <div className={summaryClassName}>
          <strong>{chainSummary.headline}</strong>
          <div>{chainSummary.detail}</div>
        </div>

        {selectedRun ? (
          <>
            <div className="automation-metric-strip automation-metric-strip--compact">
              <article className="automation-metric-strip__item">
                <span className="automation-metric-strip__label">Selected run</span>
                <strong>{selectedRun.title ?? selectedRun.kind}</strong>
                <small>{selectedRun.id}</small>
              </article>
              <article className="automation-metric-strip__item">
                <span className="automation-metric-strip__label">Runtime status</span>
                <strong>{formatStatusLabel(detailStatus)}</strong>
                <small>{runDetail?.headline ?? "Live row fallback"}</small>
              </article>
              <article className="automation-metric-strip__item">
                <span className="automation-metric-strip__label">Manual gate</span>
                <strong>{formatManualGateStatus(effectiveManualGate?.status)}</strong>
                <small>{effectiveManualGate?.requestId ?? "No gate on this run"}</small>
              </article>
              <article className="automation-metric-strip__item">
                <span className="automation-metric-strip__label">Prepared posture</span>
                <strong>{lastPreparedLaunch?.compilePreview.status ?? "not prepared"}</strong>
                <small>{lastPreparedLaunch?.compilePreview.kind ?? "waiting for prepare"}</small>
              </article>
            </div>

            <div className="details-grid details-grid--two">
              <article className="details-grid__item">
                <dt>Run kind</dt>
                <dd>{selectedRun.kind}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Profile / platform</dt>
                <dd>
                  {selectedRun.personaId ?? "N/A"}
                  <br />
                  {selectedRun.platformId ?? "N/A"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Started / finished</dt>
                <dd>
                  {formatRelativeTimestamp(selectedRun.startedAt)}
                  <br />
                  {formatRelativeTimestamp(selectedRun.finishedAt)}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Created / manual gate</dt>
                <dd>
                  {formatRelativeTimestamp(selectedRun.createdAt)}
                  <br />
                  {selectedRun.manualGateRequestId ?? "None"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Final URL</dt>
                <dd>{selectedRun.finalUrl ?? "N/A"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Error</dt>
                <dd>{selectedRun.errorMessage ?? "None"}</dd>
              </article>
            </div>

            <article className="record-card record-card--compact">
              <div className="record-card__top">
                <strong>Execution snapshot</strong>
                <span className={`badge badge--${detailStatus}`}>
                  {isRunDetailLoading ? "refreshing" : detailStatus}
                </span>
              </div>
              <p className="record-card__content">
                {runDetail?.message ??
                  selectedRun.contentPreview ??
                  "No detailed execution narrative is available yet for this run."}
              </p>
              <div className="details-grid details-grid--two">
                <article className="details-grid__item">
                  <dt>Run detail headline</dt>
                  <dd>{runDetail?.headline ?? "Waiting for run detail"}</dd>
                </article>
                <article className="details-grid__item">
                  <dt>Run detail id / task</dt>
                  <dd>
                    {runDetail?.runId ?? selectedRun.id}
                    <br />
                    {runDetail?.taskId ?? "Task id unavailable"}
                  </dd>
                </article>
                <article className="details-grid__item">
                  <dt>Created / updated</dt>
                  <dd>
                    {runDetail?.createdAtLabel ?? "Not returned"}
                    <br />
                    {runDetail?.updatedAtLabel ?? "Not returned"}
                  </dd>
                </article>
                <article className="details-grid__item">
                  <dt>Failure reason</dt>
                  <dd>{runDetail?.failureReason ?? selectedRun.errorMessage ?? "None"}</dd>
                </article>
              </div>
            </article>
          </>
        ) : (
          <EmptyState
            title="No run selected"
            detail="Select a run on the board to populate runtime detail, action controls, and launch alignment."
          />
        )}

        <div className="details-grid details-grid--two">
          <article className="details-grid__item">
            <dt>Launcher template</dt>
            <dd>{selectedTemplate?.name ?? "Not selected"}</dd>
          </article>
          <article className="details-grid__item">
            <dt>Prepared plan</dt>
            <dd>{lastPreparedLaunch?.templateName ?? "Not prepared"}</dd>
          </article>
          <article className="details-grid__item">
            <dt>Compile draft</dt>
            <dd>
              {compileDraft
                ? `${compileDraft.bindings.length} bindings / ${compileDraft.stepCount} steps / ${compileDraft.targetProfileIds.length} targets`
                : "No compile draft"}
            </dd>
          </article>
          <article className="details-grid__item">
            <dt>Recorder session</dt>
            <dd>{recorderSnapshot?.sessionId ?? "No snapshot"}</dd>
          </article>
          <article className="details-grid__item">
            <dt>Template recommendation</dt>
            <dd>
              {recommendation
                ? `${recommendation.templateName} / ${recommendation.confidence}`
                : "No recommendation"}
            </dd>
          </article>
          <article className="details-grid__item">
            <dt>Recorder source</dt>
            <dd>{recorderSnapshot?.source ?? "No recorder source"}</dd>
          </article>
        </div>

        <article className="record-card record-card--compact">
          <div className="record-card__top">
            <strong>Operator decision surface</strong>
            <span className="badge badge--info">
              {effectiveManualGate ? "manual gate live" : "runtime controls"}
            </span>
          </div>
          <p className="record-card__content">
            Keep retry, cancel, refresh, and manual gate decisions in one surface so operators can
            move from observation to action without leaving the page.
          </p>
          <div className="inline-actions">
            <button
              className="button button--secondary"
              type="button"
              disabled={!onRefreshRunDetail || actionState?.isRefreshing}
              onClick={onRefreshRunDetail}
            >
              {actionState?.isRefreshing ? "Refreshing..." : "Refresh detail"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={!onRetryRun || actionState?.isRetrying}
              onClick={onRetryRun}
            >
              {actionState?.isRetrying ? "Retrying..." : "Retry run"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={!onCancelRun || actionState?.isCancelling}
              onClick={onCancelRun}
            >
              {actionState?.isCancelling ? "Cancelling..." : "Cancel run"}
            </button>
            <button
              className="button"
              type="button"
              disabled={!effectiveManualGate || !onApproveGate || actionState?.isApprovingGate}
              onClick={onApproveGate}
            >
              {actionState?.isApprovingGate ? "Approving..." : "Approve gate"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={!effectiveManualGate || !onRejectGate || actionState?.isRejectingGate}
              onClick={onRejectGate}
            >
              {actionState?.isRejectingGate ? "Rejecting..." : "Reject gate"}
            </button>
          </div>
        </article>

        {actionFeedback ? (
          <div className={getNoticeClassName(actionFeedback.tone)}>
            <strong>{actionFeedback.message}</strong>
            {actionFeedback.updatedAtLabel ? <div>{actionFeedback.updatedAtLabel}</div> : null}
          </div>
        ) : null}

        {runDetailNotice ? (
          <div className="banner banner--warning">
            <strong>Run detail sync</strong>
            <div>{runDetailNotice}</div>
          </div>
        ) : null}

        {effectiveManualGate ? (
          <article className="record-card record-card--compact">
            <div className="record-card__top">
              <strong>Manual gate</strong>
              <span className={`badge badge--${effectiveManualGate.status}`}>
                {effectiveManualGate.status}
              </span>
            </div>
            <p className="record-card__content">{effectiveManualGate.headline}</p>
            <p className="record-card__content record-card__content--muted">
              {effectiveManualGate.detail}
            </p>
            {effectiveManualGate.failureReason ? (
              <p className="record-card__content">
                Failure reason: {effectiveManualGate.failureReason}
              </p>
            ) : null}
            <div className="record-card__footer">
              <span>Request {effectiveManualGate.requestId}</span>
              <span>
                {effectiveManualGate.decisionOptions?.join(" / ") ?? "Decision options pending"}
              </span>
            </div>
          </article>
        ) : null}

        {runDetail?.timeline.length ? (
          <div className="record-list">
            {runDetail.timeline.map((item, index) => (
              <article className="record-card record-card--compact" key={item.id}>
                <div className="record-card__top">
                  <strong>
                    {index + 1}. {item.label}
                  </strong>
                  <span className={`badge badge--${item.status}`}>{item.status}</span>
                </div>
                <p className="record-card__content">
                  {item.detail ?? "No event detail returned for this timeline entry."}
                </p>
                <div className="record-card__footer">
                  <span>{item.createdAt ? formatRelativeTimestamp(item.createdAt) : "No timestamp"}</span>
                  <span>{runDetail.runId}</span>
                </div>
              </article>
            ))}
          </div>
        ) : null}

        {(runDetail?.artifacts.length ?? 0) > 0 || selectedRun?.finalUrl || selectedRun?.contentPreview ? (
          <div className="record-list">
            {runDetail?.artifacts.map((artifact) => (
              <article className="record-card record-card--compact" key={artifact.id}>
                <div className="record-card__top">
                  <strong>{artifact.label}</strong>
                  <span className={`badge ${artifact.status ? `badge--${artifact.status}` : "badge--info"}`}>
                    {artifact.status ?? "captured"}
                  </span>
                </div>
                <p className="record-card__content">{artifact.path ?? "Artifact path not returned."}</p>
                <div className="record-card__footer">
                  <span>{artifact.id}</span>
                  <span>Run artifact</span>
                </div>
              </article>
            ))}
            {selectedRun?.finalUrl ? (
              <article className="record-card record-card--compact" key="final-url">
                <div className="record-card__top">
                  <strong>Final URL</strong>
                  <span className="badge badge--info">result</span>
                </div>
                <p className="record-card__content">{selectedRun.finalUrl}</p>
              </article>
            ) : null}
            {selectedRun?.contentPreview ? (
              <article className="record-card record-card--compact" key="content-preview">
                <div className="record-card__top">
                  <strong>Content preview</strong>
                  <span className="badge badge--info">snapshot</span>
                </div>
                <p className="record-card__content">{selectedRun.contentPreview}</p>
              </article>
            ) : null}
          </div>
        ) : null}

        {chainSummary.blockers.length > 0 ? (
          <div className="contract-list">
            {chainSummary.blockers.map((item) => (
              <article className="contract-card" key={item}>
                <div className="contract-card__top">
                  <strong>Current blocker</strong>
                  <span className="badge badge--warning">attention</span>
                </div>
                <p>{item}</p>
              </article>
            ))}
          </div>
        ) : null}

        {chainSummary.warnings.length > 0 ? (
          <div className="contract-list">
            {chainSummary.warnings.map((item) => (
              <article className="contract-card" key={item}>
                <div className="contract-card__top">
                  <strong>Operational warning</strong>
                  <span className="badge badge--info">review</span>
                </div>
                <p>{item}</p>
              </article>
            ))}
          </div>
        ) : null}

        {selectedTimelinePreview.length > 0 ? (
          <div className="record-list">
            {selectedTimelinePreview.map((step) => (
              <article className="record-card record-card--compact" key={step.id}>
                <div className="record-card__top">
                  <strong>
                    {step.index + 1}. {step.label}
                  </strong>
                  <span className="badge badge--info">{step.actionType}</span>
                </div>
                <p className="record-card__content">{step.detail}</p>
                <div className="record-card__footer">
                  <span>{step.tabLabel}</span>
                  <span>{formatRelativeTimestamp(step.capturedAt)}</span>
                </div>
              </article>
            ))}
          </div>
        ) : null}

        <div className="details-grid details-grid--two">
          {truthBoundaryCards.map((item) => (
            <article className="contract-card" key={item.label}>
              <div className="contract-card__top">
                <strong>{item.label}</strong>
                <span className="badge badge--warning">boundary</span>
              </div>
              <p>{item.detail}</p>
            </article>
          ))}
        </div>

        <div className="contract-list">
          {contractGaps.map((gap) => (
            <article className="contract-card" key={gap.contract}>
              <div className="contract-card__top">
                <strong>{gap.contract}</strong>
                <span className={`badge ${gap.status === "Ready" ? "badge--info" : "badge--warning"}`}>
                  {gap.status}
                </span>
              </div>
              <p>{gap.detail}</p>
            </article>
          ))}
        </div>
      </div>
    </Panel>
  );
}
