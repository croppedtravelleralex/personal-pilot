import type {
  SynchronizerBroadcastPlanTemplate,
  SynchronizerCommandCapability,
} from "../../features/synchronizer/model";
import { formatRelativeTimestamp } from "../../utils/format";

interface SynchronizerControlWorkbenchProps {
  capabilities: SynchronizerCommandCapability[];
  plans: SynchronizerBroadcastPlanTemplate[];
  stagedPlanId: string | null;
  runningPlanId: string | null;
  controllerLabel: string;
  targetCount: number;
  onStagePlan: (plan: SynchronizerBroadcastPlanTemplate) => void;
  onRunPlan: (planId?: string) => void;
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
  runningPlanId,
  controllerLabel,
  targetCount,
  onStagePlan,
  onRunPlan,
}: SynchronizerControlWorkbenchProps) {
  const broadcastCapability =
    capabilities.find((capability) => capability.key === "broadcastPlan") ?? null;
  const isBroadcastNativeReady = broadcastCapability?.status === "native_live";
  const isBroadcastRunning = runningPlanId !== null;

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
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Broadcast path</span>
          <strong>
            {isBroadcastNativeReady ? "Native execute" : "Awaiting native confirmation"}
          </strong>
          <small>
            {isBroadcastNativeReady
              ? "Native broadcast has already executed in this session."
              : "If the native contract is available, execution will use it. Otherwise the plan remains prepared only and is not replayed locally."}
          </small>
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
                  {plan.id === stagedPlanId ? "prepared" : "available"}
                </span>
                <span className={`badge badge--${plan.id === runningPlanId ? "warning" : "info"}`}>
                  {plan.id === runningPlanId ? "executing" : "idle"}
                </span>
              </div>
            </div>
            <p>{plan.detail}</p>
            <p className="record-card__subline">
              Requires {plan.requiredFlags.length > 0 ? plan.requiredFlags.join(" / ") : "layout only"} -
              intensity {plan.intensity}
            </p>
            <div className="toolbar-actions">
              <button className="button button--secondary" type="button" onClick={() => onStagePlan(plan)}>
                {plan.id === stagedPlanId ? "Refresh prepared plan" : "Prepare plan"}
              </button>
              <button
                className="button"
                type="button"
                onClick={() => onRunPlan(plan.id)}
                disabled={isBroadcastRunning}
              >
                {plan.id === runningPlanId
                  ? "Executing..."
                  : isBroadcastNativeReady
                    ? "Execute (native path)"
                    : "Attempt execute (native required)"}
              </button>
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}
