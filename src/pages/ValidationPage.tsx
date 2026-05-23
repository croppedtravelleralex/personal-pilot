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

export function ValidationPage() {
  const {
    collectObservedEvidence,
    error,
    isCollecting,
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
          <button
            className="button"
            disabled={isCollecting}
            type="button"
            onClick={() => void collectObservedEvidence()}
          >
            {isCollecting ? "Collecting" : "Collect evidence"}
          </button>
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
            DNS and transport observed evidence will appear after collection.
          </p>
        )}
      </Panel>
    </div>
  );
}
