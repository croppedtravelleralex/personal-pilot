import { EmptyState } from "../EmptyState";
import { InlineContentPreview } from "../InlineContentPreview";
import { Panel } from "../Panel";
import { SearchInput } from "../SearchInput";
import type { TemplateSummary } from "../../features/templates/model";

interface TemplatesBoardProps {
  items: TemplateSummary[];
  selectedTemplateId: string | null;
  totalCount: number;
  readyCount: number;
  sourceMessage: string;
  searchInput: string;
  isLoading: boolean;
  selectedRunPlatformId: string | null;
  recommendedTemplateId: string | null;
  recommendedReason: string | null;
  onRefresh: () => void;
  onSearchInputChange: (value: string) => void;
  onSelectTemplate: (templateId: string) => void;
}

export function TemplatesBoard({
  items,
  selectedTemplateId,
  totalCount,
  readyCount,
  sourceMessage,
  searchInput,
  isLoading,
  selectedRunPlatformId,
  recommendedTemplateId,
  recommendedReason,
  onRefresh,
  onSearchInputChange,
  onSelectTemplate,
}: TemplatesBoardProps) {
  return (
    <Panel
      title="Templates Board"
      subtitle="Template metadata now flows through feature state, the desktop read model, variable definitions, and manifest preparation for this local execution workbench."
      actions={
        <div className="panel__actions">
          <span className={`badge ${isLoading ? "badge--warning" : "badge--info"}`}>
            {isLoading ? "Loading" : "Catalog"}
          </span>
          <button className="button button--secondary" type="button" onClick={onRefresh}>
            Refresh templates
          </button>
        </div>
      }
    >
      <div className="page-stack">
        <div className="automation-metric-strip automation-metric-strip--compact">
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Visible</span>
            <strong>{items.length}</strong>
            <small>Filtered in board</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Total</span>
            <strong>{totalCount}</strong>
            <small>Desktop read model + adapter detail</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Ready</span>
            <strong>{readyCount}</strong>
            <small>Launch-draft capable</small>
          </article>
        </div>

        <div className="toolbar-card toolbar-card--subtle">
          <SearchInput
            label="Search templates"
            value={searchInput}
            placeholder="Name, category, scope, variable, step"
            onChange={onSearchInputChange}
          />
          <div className="toolbar-summary">{sourceMessage}</div>
        </div>

        {items.length === 0 ? (
          <EmptyState
            title="No templates match this filter"
            detail="The board is live, but the current search narrowed the current template catalog to zero rows."
          />
        ) : (
          <div className="automation-scroll-stack">
            <div className="record-list">
              {items.map((item) => {
                const selected = item.id === selectedTemplateId;

                return (
                  <article
                    className={[
                      "record-card",
                      "record-card--compact",
                      "record-card--interactive",
                      selected ? "record-card--selected" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    key={item.id}
                    onClick={() => onSelectTemplate(item.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onSelectTemplate(item.id);
                      }
                    }}
                    tabIndex={0}
                  >
                    <div className="record-card__top">
                      <div>
                        <strong>{item.name}</strong>
                        <p className="record-card__subline">
                          {item.category} | {item.profileScope}
                        </p>
                      </div>
                      <div className="panel__actions">
                        {item.id === recommendedTemplateId ? (
                          <span className="badge badge--info">recommended</span>
                        ) : null}
                        {selectedRunPlatformId && item.platformId === selectedRunPlatformId ? (
                          <span className="badge badge--warning">run-aligned</span>
                        ) : null}
                        <span className={`badge badge--${item.status}`}>{item.status}</span>
                      </div>
                    </div>
                    <InlineContentPreview
                      className="record-card__content"
                      value={item.summary}
                      collapseAt={220}
                      expandable={false}
                      copyable={false}
                    />
                    <div className="record-card__meta">
                      <span>{item.platformId}</span>
                      <span>{item.stepCount} steps</span>
                      <span>{item.variableCount} variables</span>
                      <span>{item.updatedLabel}</span>
                    </div>
                    <div className="record-card__meta">
                      <span>{item.coverageLabel}</span>
                      <span>
                        {item.allowedRegions.length > 0
                          ? `Regions ${item.allowedRegions.join(", ")}`
                          : "Regions open"}
                      </span>
                    </div>
                    <div className="automation-pill-list">
                      {item.variables.map((variable) => (
                        <span className="automation-pill" key={variable.key}>
                          {variable.label}
                          {variable.required ? " *" : ""}
                        </span>
                      ))}
                    </div>
                    {item.id === recommendedTemplateId && recommendedReason ? (
                      <InlineContentPreview
                        className="record-card__content"
                        bodyClassName="record-card__content--muted"
                        value={recommendedReason}
                        collapseAt={200}
                        expandable={false}
                        copyable={false}
                        muted
                      />
                    ) : null}
                    <div className="record-card__footer">
                      <span>{item.compilerState}</span>
                      <span>{item.sourceLabel}</span>
                    </div>
                  </article>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </Panel>
  );
}
