import { EmptyState } from "../EmptyState";
import {
  InlineContentPreview,
  truncateInlineContent,
} from "../InlineContentPreview";
import { Panel } from "../Panel";
import { SearchInput } from "../SearchInput";
import { VirtualList } from "../VirtualList";
import type { DesktopTaskItem } from "../../types/desktop";
import {
  formatCount,
  formatRelativeTimestamp,
  formatStatusLabel,
} from "../../utils/format";

const TASK_STATUS_OPTIONS = [
  { value: "all", label: "All statuses" },
  { value: "queued", label: "Queued" },
  { value: "running", label: "Running" },
  { value: "succeeded", label: "Succeeded" },
  { value: "failed", label: "Failed" },
  { value: "timed_out", label: "Timed out" },
  { value: "cancelled", label: "Cancelled" },
];

const PAGE_SIZE_OPTIONS = [25, 50, 100];

interface RunsBoardState {
  items: DesktopTaskItem[];
  total: number;
  page: number;
  pageSize: number;
  statusFilter: string;
  searchInput: string;
  isLoading: boolean;
  error: string | null;
}

interface RunsBoardProps {
  state: RunsBoardState;
  totalPages: number;
  selectedRunId?: string | null;
  title?: string;
  subtitle?: string;
  height?: number;
  onSearchInputChange: (value: string) => void;
  onStatusFilterChange: (value: string) => void;
  onPageSizeChange: (value: number) => void;
  onPageChange: (page: number) => void;
  onRefresh: () => Promise<void> | void;
  onSelectRun?: (run: DesktopTaskItem) => void;
}

export function RunsBoard({
  state,
  totalPages,
  selectedRunId = null,
  title = "Runs Board",
  subtitle = "Paged local run inventory with debounced search, filters, and large-list virtualization.",
  height = 620,
  onSearchInputChange,
  onStatusFilterChange,
  onPageSizeChange,
  onPageChange,
  onRefresh,
  onSelectRun,
}: RunsBoardProps) {
  const pageStart = state.total === 0 ? 0 : (state.page - 1) * state.pageSize + 1;
  const pageEnd = Math.min(state.total, state.page * state.pageSize);
  const interactive = Boolean(onSelectRun);

  return (
    <div className="page-stack">
      {state.error ? (
        <div className="banner banner--error">
          <InlineContentPreview value={state.error} collapseAt={240} inlineLimit={4000} />
        </div>
      ) : null}

      <Panel
        title={title}
        subtitle={subtitle}
        actions={
          <span className={`badge ${state.isLoading ? "badge--warning" : "badge--info"}`}>
            {state.isLoading ? "Refreshing" : "Live feed"}
          </span>
        }
      >
        <div className="page-stack">
          <div className="automation-metric-strip">
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Visible</span>
              <strong>{formatCount(state.items.length)}</strong>
              <small>Current page slice</small>
            </article>
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Total</span>
              <strong>{formatCount(state.total)}</strong>
              <small>Local run records</small>
            </article>
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Page</span>
              <strong>
                {formatCount(state.page)} / {formatCount(totalPages)}
              </strong>
              <small>{formatCount(state.pageSize)} rows per page</small>
            </article>
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Selection</span>
              <strong>{selectedRunId ? "Bound" : "Open"}</strong>
              <small>{selectedRunId ? `Run ${selectedRunId}` : "Board-only mode"}</small>
            </article>
          </div>

          <div className="toolbar-card toolbar-card--subtle">
            <div className="toolbar-grid">
              <SearchInput
                label="Search runs"
                value={state.searchInput}
                placeholder="Run id, kind, profile, platform"
                onChange={onSearchInputChange}
              />
              <label className="field">
                <span className="field__label">Status</span>
                <select
                  className="field__input"
                  value={state.statusFilter}
                  onChange={(event) => onStatusFilterChange(event.target.value)}
                >
                  {TASK_STATUS_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span className="field__label">Page size</span>
                <select
                  className="field__input"
                  value={state.pageSize}
                  onChange={(event) => onPageSizeChange(Number(event.target.value))}
                >
                  {PAGE_SIZE_OPTIONS.map((pageSize) => (
                    <option key={pageSize} value={pageSize}>
                      {pageSize} rows
                    </option>
                  ))}
                </select>
              </label>
              <div className="toolbar-actions">
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => onPageChange(state.page - 1)}
                  disabled={state.page <= 1}
                >
                  Previous
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => onPageChange(state.page + 1)}
                  disabled={state.page >= totalPages}
                >
                  Next
                </button>
                <button className="button" type="button" onClick={() => void onRefresh()}>
                  {state.isLoading ? "Refreshing..." : "Refresh"}
                </button>
              </div>
            </div>
            <div className="toolbar-summary">
              Showing {formatCount(pageStart)}-{formatCount(pageEnd)} of{" "}
              {formatCount(state.total)} runs
            </div>
          </div>

          {state.items.length === 0 ? (
            <EmptyState
              title="No runs found"
              detail="Adjust filters or wait for the local runtime to emit execution records."
            />
          ) : (
            <VirtualList
              items={state.items}
              height={height}
              itemHeight={188}
              getKey={(item) => item.id}
              renderItem={(item) => {
                const selected = item.id === selectedRunId;
                const timingSummary =
                  item.finishedAt != null
                    ? `Finished ${formatRelativeTimestamp(item.finishedAt)}`
                    : item.startedAt != null
                      ? `Started ${formatRelativeTimestamp(item.startedAt)}`
                      : `Created ${formatRelativeTimestamp(item.createdAt)}`;

                return (
                  <article
                    className={[
                      "record-card",
                      interactive ? "record-card--interactive" : "",
                      selected ? "record-card--selected" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    onClick={() => onSelectRun?.(item)}
                    onKeyDown={(event) => {
                      if ((event.key === "Enter" || event.key === " ") && onSelectRun) {
                        event.preventDefault();
                        onSelectRun(item);
                      }
                    }}
                    tabIndex={interactive ? 0 : -1}
                  >
                    <div className="record-card__top">
                      <div>
                        <strong>{item.title ?? item.kind}</strong>
                        <p className="record-card__subline">
                          {item.kind} | Profile {item.personaId ?? "N/A"} | Platform {item.platformId ?? "N/A"}
                        </p>
                      </div>
                      <div className="panel__actions">
                        {item.manualGateRequestId ? (
                          <span className="badge badge--warning">manual gate</span>
                        ) : null}
                        {item.isBrowserTask ? (
                          <span className="badge badge--info">browser</span>
                        ) : null}
                        <span className={`badge badge--${item.status}`}>
                          {formatStatusLabel(item.status)}
                        </span>
                      </div>
                    </div>
                    <div className="record-card__meta">
                      <span>ID {item.id}</span>
                      <span>Priority {item.priority}</span>
                      <span>{timingSummary}</span>
                      <span>Content {item.contentReady === true ? "ready" : item.contentReady === false ? "pending" : "n/a"}</span>
                    </div>
                    <InlineContentPreview
                      className="record-card__content"
                      value={item.contentPreview ?? "No preview recorded for this run."}
                      collapseAt={200}
                      expandable={false}
                      copyable={false}
                      muted={!item.contentPreview}
                    />
                    <div className="record-card__footer">
                      <span>Started {formatRelativeTimestamp(item.startedAt)}</span>
                      <span>Finished {formatRelativeTimestamp(item.finishedAt)}</span>
                      <span>Gate {item.manualGateRequestId ?? "none"}</span>
                      <span>Error: {truncateInlineContent(item.errorMessage ?? "None", 120)}</span>
                    </div>
                  </article>
                );
              }}
            />
          )}
        </div>
      </Panel>
    </div>
  );
}
