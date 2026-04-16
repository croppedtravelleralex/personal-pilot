import type {
  DesktopSyncLayoutMode,
  DesktopSyncLayoutState,
} from "../../types/desktop";
import type {
  SynchronizerDataSource,
  SynchronizerLayoutOption,
  SynchronizerOperatorSettings,
  SynchronizerRefreshIntervalOption,
  SynchronizerTargetScreen,
} from "../../features/synchronizer/model";
import { formatRelativeTimestamp } from "../../utils/format";

interface SelectOption {
  value: string;
  label: string;
}

interface LayoutToolbarProps {
  layout: DesktopSyncLayoutState;
  dataSource: SynchronizerDataSource;
  windowCount: number;
  visibleCount: number;
  busyCount: number;
  missingCount: number;
  autoRefreshEnabled: boolean;
  refreshIntervalMs: number;
  isLoading: boolean;
  activeAction: "layout" | "setMain" | "focus" | "broadcastPlan" | null;
  updatedAt: string;
  layoutOptions: SynchronizerLayoutOption[];
  refreshIntervalOptions: SynchronizerRefreshIntervalOption[];
  operatorSettings: SynchronizerOperatorSettings;
  targetScreenOptions: SelectOption[];
  onRefresh: () => void;
  onToggleAutoRefresh: (value: boolean) => void;
  onRefreshIntervalChange: (value: number) => void;
  onSetLayoutMode: (value: DesktopSyncLayoutMode) => void;
  onSetSyncFlag: (
    flag: "syncScroll" | "syncNavigation" | "syncInput",
    value: boolean,
  ) => void;
  onSetOperatorSetting: <K extends keyof SynchronizerOperatorSettings>(
    key: K,
    value: SynchronizerOperatorSettings[K],
  ) => void;
}

export function LayoutToolbar({
  layout,
  dataSource,
  windowCount,
  visibleCount,
  busyCount,
  missingCount,
  autoRefreshEnabled,
  refreshIntervalMs,
  isLoading,
  activeAction,
  updatedAt,
  layoutOptions,
  refreshIntervalOptions,
  operatorSettings,
  targetScreenOptions,
  onRefresh,
  onToggleAutoRefresh,
  onRefreshIntervalChange,
  onSetLayoutMode,
  onSetSyncFlag,
  onSetOperatorSetting,
}: LayoutToolbarProps) {
  const attentionCount = busyCount + missingCount;

  return (
    <div className="page-stack">
      <div className="toolbar-actions">
        <span className={`badge badge--${dataSource === "native" ? "info" : "warning"}`}>
          {dataSource === "native" ? "Live snapshot model" : "Local fallback model"}
        </span>
        <span className={`badge badge--${attentionCount > 0 ? "warning" : "succeeded"}`}>
          {attentionCount > 0 ? `${attentionCount} attention lanes` : "Matrix stable"}
        </span>
        {activeAction ? <span className="badge badge--warning">{activeAction}</span> : null}
        <button className="button" type="button" onClick={onRefresh}>
          {isLoading ? "Refreshing..." : "Refresh snapshot"}
        </button>
      </div>

      <div className="automation-metric-strip">
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Coverage</span>
          <strong>
            {visibleCount}/{windowCount}
          </strong>
          <small>Visible windows across current snapshot</small>
        </article>
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Layout recipe</span>
          <strong>{layout.mode.replaceAll("_", " ")}</strong>
          <small>Updated {formatRelativeTimestamp(updatedAt)}</small>
        </article>
      </div>

      <div className="segmented-control synchronizer-toolbar__layout-switch">
        {layoutOptions.map((option) => (
          <button
            key={option.value}
            className={`segmented-control__item${
              option.value === layout.mode ? " segmented-control__item--active" : ""
            }`}
            type="button"
            onClick={() => onSetLayoutMode(option.value)}
            disabled={activeAction !== null}
            title={option.detail}
          >
            {option.label}
          </button>
        ))}
      </div>

      <div className="details-grid details-grid--stacked">
        <article className="details-grid__item">
          <dt>Auto refresh</dt>
          <dd>
            <div className="page-stack">
              <label className="synchronizer-toggle">
                <input
                  checked={autoRefreshEnabled}
                  type="checkbox"
                  onChange={(event) => onToggleAutoRefresh(event.target.checked)}
                />
                <span>
                  {autoRefreshEnabled
                    ? "Keep polling the desktop snapshot in the background"
                    : "Hold the current snapshot until an operator refreshes"}
                </span>
              </label>
              <select
                className="field__input"
                value={refreshIntervalMs}
                onChange={(event) => onRefreshIntervalChange(Number(event.target.value))}
                disabled={!autoRefreshEnabled}
              >
                {refreshIntervalOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Sync flags</dt>
          <dd>
            <div className="page-stack">
              <label className="synchronizer-toggle">
                <input
                  checked={layout.syncScroll}
                  type="checkbox"
                  onChange={(event) => onSetSyncFlag("syncScroll", event.target.checked)}
                  disabled={activeAction !== null}
                />
                <span>Scroll sync</span>
              </label>
              <label className="synchronizer-toggle">
                <input
                  checked={layout.syncNavigation}
                  type="checkbox"
                  onChange={(event) => onSetSyncFlag("syncNavigation", event.target.checked)}
                  disabled={activeAction !== null}
                />
                <span>Navigation sync</span>
              </label>
              <label className="synchronizer-toggle">
                <input
                  checked={layout.syncInput}
                  type="checkbox"
                  onChange={(event) => onSetSyncFlag("syncInput", event.target.checked)}
                  disabled={activeAction !== null}
                />
                <span>Input sync</span>
              </label>
            </div>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Execution pacing</dt>
          <dd>
            <div className="page-stack">
              <label className="field">
                <span className="field__label">Click delay</span>
                <select
                  className="field__input"
                  value={operatorSettings.clickDelayMs}
                  onChange={(event) =>
                    onSetOperatorSetting("clickDelayMs", Number(event.target.value))
                  }
                >
                  {[180, 280, 420, 560].map((value) => (
                    <option key={value} value={value}>
                      {value} ms
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span className="field__label">Typing delay</span>
                <select
                  className="field__input"
                  value={operatorSettings.typingDelayMs}
                  onChange={(event) =>
                    onSetOperatorSetting("typingDelayMs", Number(event.target.value))
                  }
                >
                  {[80, 140, 220, 320].map((value) => (
                    <option key={value} value={value}>
                      {value} ms
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Execution safeguards</dt>
          <dd>
            <div className="page-stack">
              <label className="synchronizer-toggle">
                <input
                  checked={operatorSettings.stopOnHidden}
                  type="checkbox"
                  onChange={(event) =>
                    onSetOperatorSetting("stopOnHidden", event.target.checked)
                  }
                />
                <span>Pause broadcast execution when windows are hidden</span>
              </label>
              <label className="synchronizer-toggle">
                <input
                  checked={operatorSettings.respectBusy}
                  type="checkbox"
                  onChange={(event) =>
                    onSetOperatorSetting("respectBusy", event.target.checked)
                  }
                />
                <span>Skip busy windows during broadcast execution</span>
              </label>
              <label className="field">
                <span className="field__label">Target screen</span>
                <select
                  className="field__input"
                  value={operatorSettings.targetScreen}
                  onChange={(event) =>
                    onSetOperatorSetting(
                      "targetScreen",
                      event.target.value as SynchronizerTargetScreen,
                    )
                  }
                >
                  {targetScreenOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </dd>
        </article>
      </div>
    </div>
  );
}
