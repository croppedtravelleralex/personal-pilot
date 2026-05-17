import { EmptyState } from "./EmptyState";
import { Panel } from "./Panel";
import { StatCard } from "./StatCard";

interface ModuleMetric {
  label: string;
  value: string;
  hint: string;
  tone?: "neutral" | "success" | "warning" | "danger";
}

interface ModuleArea {
  title: string;
  detail: string;
  status: string;
}

interface ModuleLane {
  title: string;
  detail: string;
  owner: string;
}

interface ParallelModulePageProps {
  metrics: ModuleMetric[];
  areas: ModuleArea[];
  lanes: ModuleLane[];
  currentStageTitle: string;
  currentStageDetail: string;
}

export function ParallelModulePage({
  metrics,
  areas,
  lanes,
  currentStageTitle,
  currentStageDetail,
}: ParallelModulePageProps) {
  return (
    <div className="page-stack">
      <div className="stat-grid">
        {metrics.map((metric) => (
          <StatCard
            key={metric.label}
            label={metric.label}
            value={metric.value}
            hint={metric.hint}
            tone={metric.tone}
          />
        ))}
      </div>

      <div className="page-grid page-grid--two">
        <Panel
          title="Module Scope"
          subtitle="The shell is split first so multiple module slices can move without collisions."
        >
          <div className="details-grid details-grid--stacked">
            {areas.map((area) => (
              <article className="details-grid__item" key={area.title}>
                <dt>{area.title}</dt>
                <dd className="details-grid__value">{area.detail}</dd>
                <div className="details-grid__actions">
                  <span className="badge">{area.status}</span>
                </div>
              </article>
            ))}
          </div>
        </Panel>

        <Panel
          title="Parallel Lanes"
          subtitle="These workstreams can move independently once the route and shell boundary are stable."
        >
          <div className="record-list">
            {lanes.map((lane) => (
              <article className="record-card record-card--compact" key={lane.title}>
                <div className="record-card__top">
                  <strong>{lane.title}</strong>
                  <span className="badge">{lane.owner}</span>
                </div>
                <p className="record-card__content">{lane.detail}</p>
              </article>
            ))}
          </div>
        </Panel>
      </div>

      <Panel title="Current Stage" subtitle="What this split unlocks right now">
        <EmptyState title={currentStageTitle} detail={currentStageDetail} />
      </Panel>
    </div>
  );
}
