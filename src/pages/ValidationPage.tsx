import { Panel } from "../components/Panel";
import { StatCard } from "../components/StatCard";
import { useValidationBoardViewModel } from "../features/validation/hooks";
import type {
  ValidationEvidenceLayer,
  ValidationEvidenceStatus,
} from "../features/validation/model";

const LAYER_LABELS: Record<ValidationEvidenceLayer, string> = {
  declared: "Declared",
  applied: "Applied",
  observed: "Observed",
};

const STATUS_LABELS: Record<ValidationEvidenceStatus, string> = {
  ready: "Ready",
  partial: "Partial",
  missing: "Missing",
};

function statusTone(status: ValidationEvidenceStatus) {
  if (status === "ready") return "succeeded";
  if (status === "partial") return "warning";
  return "failed";
}

function formatEpochSeconds(value: string) {
  const millis = Number(value) * 1000;
  if (!Number.isFinite(millis) || millis <= 0) return value;
  return new Date(millis).toLocaleString();
}

export function ValidationPage() {
  const {
    collectObservedEvidence,
    error,
    exportProfileEvidence,
    fingerprintAudit,
    history,
    isCollecting,
    isExporting,
    isLoadingHistory,
    lastExport,
    refreshHistory,
    report,
    snapshot,
    summary,
  } = useValidationBoardViewModel();

  return (
    <div className="page-stack">
      <div className="stat-grid">
        <StatCard
          label="Evidence categories"
          value={String(summary.categoryCount)}
          hint="Detector through transport"
          tone="neutral"
        />
        <StatCard
          label="Declared"
          value={String(summary.declaredCount)}
          hint="Board surface exists"
          tone="success"
        />
        <StatCard
          label="Applied"
          value={String(summary.appliedCount)}
          hint="Runtime boundary present"
          tone="warning"
        />
        <StatCard
          label="Observed"
          value={String(summary.observedCount)}
          hint={report ? "Latest report loaded" : "Repeatable evidence pending"}
          tone={report ? "success" : "danger"}
        />
      </div>

      <Panel
        title="Validation Board"
        subtitle={`Generated ${new Date(snapshot.generatedAt).toLocaleString()}`}
        actions={
          <div className="panel__actions">
            <button
              className="button button--secondary"
              disabled={isLoadingHistory}
              type="button"
              onClick={() => void refreshHistory()}
            >
              {isLoadingHistory ? "Loading" : "Load history"}
            </button>
            <button
              className="button"
              disabled={isCollecting}
              type="button"
              onClick={() => void collectObservedEvidence()}
            >
              {isCollecting ? "Collecting" : "Collect evidence"}
            </button>
          </div>
        }
      >
        {error ? <p className="validation-error">{error}</p> : null}
        <div className="validation-board">
          {snapshot.categories.map((category) => (
            <article className="validation-card" key={category.id}>
              <header className="validation-card__header">
                <div>
                  <h3>{category.label}</h3>
                  <p>{category.description}</p>
                </div>
                <span className="badge badge--info">
                  {category.items.reduce((sum, item) => sum + item.signalCount, 0)} signals
                </span>
              </header>

              <div className="validation-layers">
                {category.items.map((item) => (
                  <div className="validation-layer" key={item.id}>
                    <div className="validation-layer__top">
                      <span>{LAYER_LABELS[item.layer]}</span>
                      <span className={`badge badge--${statusTone(item.status)}`}>
                        {STATUS_LABELS[item.status]}
                      </span>
                    </div>
                    <strong>{item.signalCount}</strong>
                    <p>{item.summary}</p>
                  </div>
                ))}
              </div>
            </article>
          ))}
        </div>
      </Panel>

      <Panel
        title="Latest Observed Report"
        subtitle={report ? report.reportId : "No report collected in this session"}
      >
        {report ? (
          <div className="validation-report">
            <div className="details-grid">
              <div>
                <span>Collector</span>
                <strong>{report.collectorVersion}</strong>
              </div>
              <div>
                <span>Categories</span>
                <strong>{report.categories.join(", ")}</strong>
              </div>
              <div>
                <span>Report path</span>
                <strong>{report.reportPath}</strong>
              </div>
            </div>
            <p className="validation-report__summary">{report.summary}</p>
            <div className="validation-signal-list">
              {report.signals.map((signal) => (
                <article className="record-card record-card--compact" key={signal.id}>
                  <div className="record-card__top">
                    <div>
                      <strong>{signal.label}</strong>
                      <p className="record-card__subline">{signal.summary}</p>
                    </div>
                    <span className={`badge badge--${signal.status}`}>{signal.status}</span>
                  </div>
                  <div className="record-card__footer">
                    <span>{signal.category}</span>
                    <span>
                      {signal.durationMs == null ? "duration n/a" : `${signal.durationMs}ms`}
                    </span>
                  </div>
                  {signal.detail ? <p className="record-card__content">{signal.detail}</p> : null}
                </article>
              ))}
            </div>
          </div>
        ) : (
          <p className="record-card__content--muted">
            DNS, transport, desktop WebView, and profile browser runtime evidence will appear after collection.
          </p>
        )}
      </Panel>

      <Panel
        title="Fingerprint Observation Audit"
        subtitle="Declared, projected, and observed counts are separated"
      >
        <div className="details-grid">
          <div>
            <span>Declared controls</span>
            <strong>{fingerprintAudit.declaredControlCount}</strong>
          </div>
          <div>
            <span>Runtime projected</span>
            <strong>{fingerprintAudit.runtimeProjectedFieldCount}</strong>
          </div>
          <div>
            <span>Target signals</span>
            <strong>{fingerprintAudit.targetSignalCountLabel}</strong>
          </div>
          <div>
            <span>Observed proof</span>
            <strong>{fingerprintAudit.observedSignalCount}</strong>
          </div>
        </div>
        <p className="validation-report__summary">{fingerprintAudit.summary}</p>
        <div className="validation-audit-grid">
          <article className="validation-layer">
            <div className="validation-layer__top">
              <span>Profile Browser</span>
              <span className={`badge badge--${statusTone(fingerprintAudit.status)}`}>
                {STATUS_LABELS[fingerprintAudit.status]}
              </span>
            </div>
            <strong>{fingerprintAudit.profileBrowserObservedCount}</strong>
            <p>
              Profile runtime signals. Success {fingerprintAudit.readyObservedCount}, warning {fingerprintAudit.warningObservedCount}, failed {fingerprintAudit.failedObservedCount}.
            </p>
          </article>
          <article className="validation-layer">
            <div className="validation-layer__top">
              <span>Covered</span>
              <span className="badge badge--info">
                {fingerprintAudit.observedCategoryCount}/5
              </span>
            </div>
            <strong>{fingerprintAudit.coveredCategories.join(", ") || "none"}</strong>
            <p>Fingerprint-related categories with real observed signals in the loaded report.</p>
          </article>
          <article className="validation-layer">
            <div className="validation-layer__top">
              <span>Gaps</span>
              <span className="badge badge--warning">
                {fingerprintAudit.missingCategories.length}
              </span>
            </div>
            <strong>{fingerprintAudit.missingCategories.join(", ") || "none"}</strong>
            <p>Missing categories remain gaps, not declared or projected evidence.</p>
          </article>
        </div>
      </Panel>

      <Panel
        title="Report History"
        subtitle={`${history.length} local validation report${history.length === 1 ? "" : "s"}`}
        actions={
          <button
            className="button button--secondary"
            disabled={isExporting}
            type="button"
            onClick={() => void exportProfileEvidence(null)}
          >
            {isExporting ? "Exporting" : "Export evidence"}
          </button>
        }
      >
        {lastExport ? (
          <p className="validation-report__summary">
            {lastExport.summary} {lastExport.exportPath}
          </p>
        ) : null}
        {history.length > 0 ? (
          <div className="validation-history-list">
            {history.slice(0, 8).map((item) => (
              <article className="record-card record-card--compact" key={item.reportId}>
                <div className="record-card__top">
                  <div>
                    <strong>{item.reportId}</strong>
                    <p className="record-card__subline">
                      {formatEpochSeconds(item.generatedAt)}
                    </p>
                  </div>
                  <span className={item.failedCount > 0 ? "badge badge--failed" : "badge badge--succeeded"}>
                    {item.failedCount > 0 ? "issues" : "clean"}
                  </span>
                </div>
                <div className="record-card__footer">
                  <span>{item.categories.join(", ")}</span>
                  <span>{item.signalCount} signals</span>
                </div>
                <p className="record-card__content">{item.summary}</p>
                <p className="record-card__subline">{item.reportPath}</p>
              </article>
            ))}
          </div>
        ) : (
          <p className="record-card__content--muted">
            Load history or collect evidence to list local validation reports.
          </p>
        )}
      </Panel>
    </div>
  );
}
