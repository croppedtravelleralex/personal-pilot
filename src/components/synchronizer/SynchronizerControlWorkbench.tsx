import type {
  SynchronizerBroadcastPlanTemplate,
  SynchronizerCommandCapability,
} from "../../features/synchronizer/model";
import { formatRelativeTimestamp } from "../../utils/format";

interface SynchronizerControlWorkbenchProps {
  capabilities: SynchronizerCommandCapability[];
  plans: SynchronizerBroadcastPlanTemplate[];
  stagedPlanId: string | null;
  controllerLabel: string;
  targetCount: number;
  onStagePlan: (plan: SynchronizerBroadcastPlanTemplate) => void;
}

function getCapabilityTone(status: SynchronizerCommandCapability["status"]) {
  if (status === "native_live") {
    return "info";
  }

  if (status === "local_staged") {
    return "warning";
  }

  return "error";
}

export function SynchronizerControlWorkbench({
  capabilities,
  plans,
  stagedPlanId,
  controllerLabel,
  targetCount,
  onStagePlan,
}: SynchronizerControlWorkbenchProps) {
  return (
    <div className="page-stack">
      <div className="automation-metric-strip">
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Controller lane</span>
          <strong>{controllerLabel}</strong>
          <small>Current lead window for sync intent</small>
        </article>
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Broadcast scope</span>
          <strong>{targetCount} windows</strong>
          <small>Current filtered operator scope</small>
        </article>
      </div>

      <div className="contract-list">
        {capabilities.map((capability) => (
          <article className="contract-card" key={capability.key}>
            <div className="contract-card__top">
              <strong>{capability.label}</strong>
              <span className={`badge badge--${getCapabilityTone(capability.status)}`}>
                {capability.status.replaceAll("_", " ")}
              </span>
            </div>
            <p>{capability.detail}</p>
            <small>
              {capability.lastUpdatedAt
                ? `Updated ${formatRelativeTimestamp(capability.lastUpdatedAt)}`
                : "Not exercised in this session yet"}
            </small>
          </article>
        ))}
      </div>

      <div className="contract-list">
        {plans.map((plan) => (
          <article className="contract-card" key={plan.id}>
            <div className="contract-card__top">
              <strong>{plan.title}</strong>
              <div className="toolbar-actions">
                <span className="badge badge--info">{plan.scopeLabel}</span>
                <span className={`badge badge--${plan.id === stagedPlanId ? "warning" : "info"}`}>
                  {plan.id === stagedPlanId ? "staged" : "ready"}
                </span>
              </div>
            </div>
            <p>{plan.detail}</p>
            <p className="record-card__subline">
              Requires {plan.requiredFlags.length > 0 ? plan.requiredFlags.join(" / ") : "layout only"} -
              intensity {plan.intensity}
            </p>
            <button className="button" type="button" onClick={() => onStagePlan(plan)}>
              {plan.id === stagedPlanId ? "Restage plan" : "Stage plan"}
            </button>
          </article>
        ))}
      </div>
    </div>
  );
}
