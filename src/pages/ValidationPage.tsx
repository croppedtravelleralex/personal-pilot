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
  const { snapshot, summary } = useValidationBoardViewModel();

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
          hint="Repeatable evidence pending"
          tone="danger"
        />
      </div>

      <Panel
        title="Validation Board"
        subtitle={`Generated ${new Date(snapshot.generatedAt).toLocaleString()}`}
      >
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
    </div>
  );
}
