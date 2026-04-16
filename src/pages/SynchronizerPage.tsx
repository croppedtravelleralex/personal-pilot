import { Panel } from "../components/Panel";
import { StatCard } from "../components/StatCard";
import { LayoutToolbar } from "../components/synchronizer/LayoutToolbar";
import { MainWindowBadge } from "../components/synchronizer/MainWindowBadge";
import { SynchronizerActionFeed } from "../components/synchronizer/SynchronizerActionFeed";
import { SynchronizerControlWorkbench } from "../components/synchronizer/SynchronizerControlWorkbench";
import { SynchronizerFiltersPanel } from "../components/synchronizer/SynchronizerFiltersPanel";
import { WindowMatrix } from "../components/synchronizer/WindowMatrix";
import { useSynchronizerViewModel } from "../features/synchronizer/hooks";
import { formatRelativeTimestamp } from "../utils/format";

function getBadgeTone(tone: "neutral" | "success" | "warning" | "danger") {
  if (tone === "danger") {
    return "error";
  }

  if (tone === "success") {
    return "succeeded";
  }

  if (tone === "neutral") {
    return "info";
  }

  return tone;
}

export function SynchronizerPage() {
  const {
    state,
    summary,
    consoleSummary,
    layoutOptions,
    refreshIntervalOptions,
    groupByOptions,
    roleFilterOptions,
    visibilityFilterOptions,
    targetScreenOptions,
    broadcastPlanTemplates,
    actions,
  } = useSynchronizerViewModel();
  const selectedWindow = summary.selectedWindow;
  const latestFeedItem = state.actionFeed[0] ?? null;
  const stagedPlan =
    broadcastPlanTemplates.find((plan) => plan.id === state.stagedBroadcastPlanId) ?? null;
  const isBroadcastNativeReady = state.capabilities.broadcastPlan.status === "native_live";
  const isBroadcastExecuting = state.runningBroadcastPlanId !== null;
  const capabilityList = [
    state.capabilities.readSnapshot,
    state.capabilities.layout,
    state.capabilities.setMain,
    state.capabilities.focus,
    state.capabilities.broadcastPlan,
  ];

  return (
    <div className="page-stack">
      {state.error ? <div className="banner banner--error">{state.error}</div> : null}
      {state.info ? <div className="banner banner--info">{state.info}</div> : null}

      <div className="toolbar-card">
        <div className="automation-center__hero">
          <div className="automation-center__hero-copy">
            <span className="shell__eyebrow">Synchronizer Desk</span>
            <h2>Local Sync Control Console</h2>
            <p>
              AdsPower-style controller/member expression, grouped matrix review, and
              capability-gated broadcast execution now live in one sync workbench.
            </p>
          </div>
          <div className="automation-center__hero-aside">
            <span className={`badge badge--${getBadgeTone(consoleSummary.postureTone)}`}>
              {consoleSummary.postureLabel}
            </span>
            <span className={`badge badge--${state.dataSource === "native" ? "info" : "warning"}`}>
              {state.dataSource === "native" ? "Live desktop snapshot" : "Local prepared/fallback"}
            </span>
            <span className="badge badge--info">{consoleSummary.cadenceLabel}</span>
            <span className="badge badge--info">
              Updated {formatRelativeTimestamp(state.snapshot.updatedAt)}
            </span>
          </div>
        </div>

        <div className="automation-metric-strip">
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Controller / members</span>
            <strong>{summary.mainWindow?.profileLabel ?? "Controller not set"}</strong>
            <small>{summary.controlledCount} controlled windows in current snapshot</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Current scope</span>
            <strong>{consoleSummary.coverageLabel}</strong>
            <small>{consoleSummary.coverageDetail}</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Broadcast plan</span>
            <strong>{stagedPlan?.title ?? "No prepared plan"}</strong>
            <small>
              {isBroadcastExecuting
                ? "Broadcast execution is in progress."
                : stagedPlan
                  ? isBroadcastNativeReady
                    ? `${stagedPlan.scopeLabel} - execute goes through native path when ready`
                    : `${stagedPlan.scopeLabel} - execute is capability-gated with prepared fallback`
                  : "Prepare a plan, then execute it through native path or explicit fallback."}
            </small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Latest feedback</span>
            <strong>{latestFeedItem?.title ?? "No recent actions"}</strong>
            <small>
              {latestFeedItem
                ? `${latestFeedItem.executionLabel} - ${formatRelativeTimestamp(
                    latestFeedItem.createdAt,
                  )}`
                : "Sync actions, prepared plans, and fallback notes will accumulate here."}
            </small>
          </article>
        </div>

        {consoleSummary.attentionItems.length > 0 ? (
          <div className="contract-list">
            {consoleSummary.attentionItems.map((item) => (
              <article className="contract-card" key={item.id}>
                <div className="contract-card__top">
                  <strong>{item.title}</strong>
                  <span className={`badge badge--${item.tone}`}>{item.tone}</span>
                </div>
                <p>{item.detail}</p>
              </article>
            ))}
          </div>
        ) : null}
      </div>

      <div className="stat-grid">
        <StatCard
          label="Windows Detected"
          value={String(summary.windows.length)}
          hint={`${summary.filterCount} currently in operator scope`}
          tone={summary.visibleCount > 0 ? "success" : "warning"}
        />
        <StatCard
          label="Controller"
          value={summary.mainWindow?.profileLabel ?? "Not set"}
          hint={summary.mainWindow ? consoleSummary.alignmentDetail : "Primary sync target is still unassigned"}
          tone={summary.mainWindow ? "success" : "warning"}
        />
        <StatCard
          label="Focused Lane"
          value={summary.focusedWindow?.profileLabel ?? "No focus"}
          hint={
            state.activeAction
              ? `Applying ${state.activeAction}`
              : selectedWindow && !selectedWindow.isFocused
                ? "Selected card is not the native focus target"
                : "Current native focus target"
          }
          tone={
            summary.focusedWindow && (!selectedWindow || selectedWindow.isFocused)
              ? "success"
              : "warning"
          }
        />
        <StatCard
          label="Write Surface"
          value={state.capabilities.broadcastPlan.status.replaceAll("_", " ")}
          hint="Broadcast execution is capability-gated: native when available, prepared fallback otherwise."
          tone={
            state.capabilities.broadcastPlan.status === "native_live"
              ? "success"
              : state.capabilities.broadcastPlan.status === "local_staged"
                ? "warning"
                : "danger"
          }
        />
      </div>

      <div className="synchronizer-layout">
        <div className="synchronizer-layout__main">
          <Panel
            title="Sync Command Workbench"
            subtitle="Prepare and execute sync plans with explicit native capability vs fallback feedback."
          >
            <SynchronizerControlWorkbench
              capabilities={capabilityList}
              plans={broadcastPlanTemplates}
              stagedPlanId={state.stagedBroadcastPlanId}
              runningPlanId={state.runningBroadcastPlanId}
              controllerLabel={summary.mainWindow?.profileLabel ?? "Controller not pinned"}
              targetCount={summary.filterCount}
              onStagePlan={actions.stageBroadcastPlan}
              onRunPlan={(planId) => void actions.runBroadcastPlan(planId)}
            />
          </Panel>

          <Panel
            title="Window Matrix"
            subtitle="Grouped matrix with role-aware filtering, then apply focus or controller changes from the same surface."
            actions={
              <div className="toolbar-actions">
                <span className={`badge badge--${summary.mainWindow ? "succeeded" : "warning"}`}>
                  {summary.mainWindow ? "Controller pinned" : "Controller missing"}
                </span>
                <span className={`badge badge--${summary.focusedWindow ? "info" : "warning"}`}>
                  {summary.focusedWindow ? "Focus captured" : "No focus target"}
                </span>
                <span className="badge badge--info">{summary.groupedWindows.length} groups</span>
              </div>
            }
          >
            <WindowMatrix
              windows={summary.filteredWindows}
              groups={summary.groupedWindows}
              selectedWindowId={state.selectedWindowId}
              activeAction={state.activeAction}
              onSelect={actions.selectWindow}
              onSetMain={(windowId) => void actions.setMainWindow(windowId)}
              onFocus={(windowId) => void actions.focusWindow(windowId)}
            />
          </Panel>
        </div>

        <div className="synchronizer-layout__side">
          <Panel
            title="Filters & Grouping"
            subtitle="Narrow the operator scope before preparing or executing broadcast/controller actions."
          >
            <SynchronizerFiltersPanel
              searchText={state.filters.searchText}
              platformFilter={state.filters.platformFilter}
              statusFilter={state.filters.statusFilter}
              visibilityFilter={state.filters.visibilityFilter}
              roleFilter={state.filters.roleFilter}
              groupBy={state.filters.groupBy}
              platformOptions={summary.platformOptions}
              statusOptions={summary.statusOptions}
              visibilityOptions={visibilityFilterOptions}
              roleOptions={roleFilterOptions}
              groupByOptions={groupByOptions}
              filteredCount={summary.filterCount}
              totalCount={summary.windows.length}
              onSearchTextChange={actions.setSearchText}
              onPlatformFilterChange={actions.setPlatformFilter}
              onStatusFilterChange={actions.setStatusFilter}
              onVisibilityFilterChange={actions.setVisibilityFilter}
              onRoleFilterChange={actions.setRoleFilter}
              onGroupByChange={actions.setGroupBy}
              onReset={actions.resetFilters}
            />
          </Panel>

          <Panel
            title="Layout & Sync Settings"
            subtitle="Use synchronizer state-write settings and execution safeguards with explicit capability feedback."
          >
            <LayoutToolbar
              layout={state.snapshot.layout}
              dataSource={state.dataSource}
              windowCount={summary.windows.length}
              visibleCount={summary.visibleCount}
              busyCount={summary.busyCount}
              missingCount={summary.missingCount}
              autoRefreshEnabled={state.autoRefreshEnabled}
              refreshIntervalMs={state.refreshIntervalMs}
              isLoading={state.isLoading}
              activeAction={state.activeAction}
              updatedAt={state.snapshot.updatedAt}
              layoutOptions={layoutOptions}
              refreshIntervalOptions={refreshIntervalOptions}
              operatorSettings={state.operatorSettings}
              targetScreenOptions={targetScreenOptions}
              onRefresh={() => void actions.refresh()}
              onToggleAutoRefresh={actions.setAutoRefreshEnabled}
              onRefreshIntervalChange={actions.setRefreshIntervalMs}
              onSetLayoutMode={(value) => void actions.setLayoutMode(value)}
              onSetSyncFlag={(flag, value) => void actions.setLayoutSyncFlag(flag, value)}
              onSetOperatorSetting={actions.setOperatorSetting}
            />
          </Panel>

          <Panel
            title="Controller State"
            subtitle="Track controller ownership, focus drift, and the current prepared/executing broadcast plan."
          >
            <MainWindowBadge
              layout={state.snapshot.layout}
              mainWindow={summary.mainWindow}
              focusedWindow={summary.focusedWindow}
              selectedWindow={summary.selectedWindow}
              controlledCount={summary.controlledCount}
              stagedBroadcastPlanTitle={stagedPlan?.title ?? null}
            />
          </Panel>

          <Panel
            title="Selected Window"
            subtitle="Fast diagnostic snapshot plus the next recommended operator move."
            actions={
              selectedWindow ? (
                <div className="toolbar-actions">
                  <button
                    className="button button--secondary"
                    type="button"
                    onClick={() => void actions.focusWindow(selectedWindow.windowId)}
                    disabled={state.activeAction !== null || selectedWindow.status === "missing"}
                  >
                    {state.activeAction === "focus" ? "Focusing..." : "Bring focus"}
                  </button>
                  <button
                    className="button"
                    type="button"
                    onClick={() => void actions.setMainWindow(selectedWindow.windowId)}
                    disabled={state.activeAction !== null || selectedWindow.status === "missing"}
                  >
                    {selectedWindow.isMainWindow
                      ? "Controller"
                      : state.activeAction === "setMain"
                        ? "Applying..."
                        : "Make controller"}
                  </button>
                </div>
              ) : undefined
            }
          >
            <div
              className={`banner banner--${
                consoleSummary.nextActionTone === "danger"
                  ? "error"
                  : consoleSummary.nextActionTone === "neutral"
                    ? "info"
                    : consoleSummary.nextActionTone
              }`}
            >
              <strong>{consoleSummary.nextActionLabel}</strong>
              <br />
              {consoleSummary.nextActionDetail}
            </div>

            <div className="details-grid details-grid--stacked">
              <article className="details-grid__item">
                <dt>Profile</dt>
                <dd>{selectedWindow?.profileLabel ?? "No selection"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Window state</dt>
                <dd>{selectedWindow?.status.replaceAll("_", " ") ?? "N/A"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Role</dt>
                <dd>
                  {selectedWindow?.isMainWindow ? "Controller window" : "Controlled member"}
                  <br />
                  {selectedWindow?.isFocused ? "Focused now" : "Not focused"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Window id</dt>
                <dd>{selectedWindow?.windowId ?? "N/A"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Last seen</dt>
                <dd>{formatRelativeTimestamp(selectedWindow?.lastSeenAt ?? null)}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Visibility</dt>
                <dd>
                  {selectedWindow?.isVisible ? "Visible" : "Hidden"}
                  <br />
                  {selectedWindow?.isMinimized ? "Minimized" : "Not minimized"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Store / platform</dt>
                <dd>
                  {selectedWindow?.storeId ?? "N/A"}
                  <br />
                  {selectedWindow?.platformId ?? "N/A"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Native handle</dt>
                <dd>
                  {selectedWindow?.nativeHandle ?? "Not attached"}
                  <br />
                  {selectedWindow?.bounds
                    ? `${selectedWindow.bounds.width}x${selectedWindow.bounds.height} @ ${selectedWindow.bounds.x},${selectedWindow.bounds.y}`
                    : "Bounds not captured"}
                </dd>
              </article>
            </div>
          </Panel>

          <Panel
            title="Action Feed"
            subtitle="Dense operator feedback with live native vs local prepared/fallback labeling."
            actions={<span className="badge badge--info">{state.actionFeed.length} recent items</span>}
          >
            <SynchronizerActionFeed items={state.actionFeed} />
          </Panel>
        </div>
      </div>
    </div>
  );
}
