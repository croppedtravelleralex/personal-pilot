import { StatCard } from "../components/StatCard";
import { BatchCheckToolbar } from "../components/proxies/BatchCheckToolbar";
import { ProxyChangeToolbar } from "../components/proxies/ProxyChangeToolbar";
import { ProxyFilterBar } from "../components/proxies/ProxyFilterBar";
import { ProxyOperationsSummary } from "../components/proxies/ProxyOperationsSummary";
import { ProxyTable } from "../components/proxies/ProxyTable";
import { UsagePanel } from "../components/proxies/UsagePanel";
import { useProxiesViewModel } from "../features/proxies/hooks";
import { formatCount } from "../utils/format";

function getAttentionTone(attentionCount: number) {
  if (attentionCount === 0) {
    return "success" as const;
  }
  if (attentionCount < 3) {
    return "warning" as const;
  }
  return "danger" as const;
}

export function ProxiesPage() {
  const {
    state,
    rows,
    selectedProxy,
    selectedProxyHidden,
    selectedProxyChangeIpFeedback,
    summary,
    batchTargetCount,
    allVisibleSelected,
    changeIpTargetLabel,
    recentChangeResults,
    healthOptions,
    usageOptions,
    sortOptions,
    sourceOptions,
    regionOptions,
    tagOptions,
    actions,
  } = useProxiesViewModel();

  return (
    <div className="page-stack">
      <div className="stat-grid">
        <StatCard
          label="Visible inventory"
          value={formatCount(summary.visible)}
          hint={`Loaded ${formatCount(summary.loaded)} / inventory ${formatCount(summary.total)}`}
          tone="neutral"
        />
        <StatCard
          label="Ready healthy"
          value={formatCount(summary.healthy)}
          hint={`${formatCount(summary.ready)} healthy and currently unused`}
          tone="success"
        />
        <StatCard
          label="Risk watch"
          value={formatCount(summary.highRisk)}
          hint={`${formatCount(summary.attention)} warning/failed rows need operator review`}
          tone={getAttentionTone(summary.highRisk)}
        />
        <StatCard
          label="Live usage"
          value={formatCount(summary.used)}
          hint={`${formatCount(summary.activeUsage)} active profile links`}
          tone="warning"
        />
        <StatCard
          label="Provider mix"
          value={formatCount(summary.providers)}
          hint={`${formatCount(summary.sources)} sources currently represented`}
          tone="neutral"
        />
        <StatCard
          label="Local rotation watch"
          value={formatCount(summary.localRotationTracked)}
          hint={`${formatCount(summary.coolingDown)} cooling down, ${formatCount(summary.localRotationFailures)} failed`}
          tone={getAttentionTone(summary.localRotationFailures)}
        />
      </div>

      <ProxyOperationsSummary
        dataSource={state.dataSource}
        summary={summary}
        recentResults={recentChangeResults}
      />

      <BatchCheckToolbar
        scope={state.batchCheck.scope}
        phase={state.batchCheck.phase}
        feedbackTone={state.batchCheck.feedbackTone}
        selectedCount={state.selectedIds.length}
        filteredCount={rows.length}
        targetCount={batchTargetCount}
        completedCount={state.batchCheck.completedCount}
        message={state.batchCheck.lastMessage}
        lastStartedAt={state.batchCheck.lastStartedAt}
        lastFinishedAt={state.batchCheck.lastFinishedAt}
        onScopeChange={actions.setBatchScope}
        onStart={actions.startBatchCheck}
        onSelectVisible={() => actions.setSelection(rows.map((row) => row.id))}
        onClearSelection={actions.clearSelection}
        onDismiss={actions.dismissBatchFeedback}
      />

      <ProxyChangeToolbar
        phase={state.changeIp.phase}
        feedbackTone={state.changeIp.feedbackTone}
        targetLabel={changeIpTargetLabel}
        selectedCount={state.selectedIds.length}
        completedCount={state.changeIp.completedCount}
        succeededCount={state.changeIp.succeededCount}
        failedCount={state.changeIp.failedCount}
        coolingDownCount={summary.coolingDown}
        message={state.changeIp.lastMessage}
        activeProxyName={
          state.changeIp.activeProxyId
            ? state.rows.find((row) => row.id === state.changeIp.activeProxyId)?.name ??
              state.changeIp.activeProxyId
            : null
        }
        lastStartedAt={state.changeIp.lastStartedAt}
        lastFinishedAt={state.changeIp.lastFinishedAt}
        recentResults={recentChangeResults.slice(0, 4)}
        onStart={actions.startChangeIp}
        onDismiss={actions.dismissChangeIpFeedback}
      />

      <ProxyFilterBar
        searchValue={state.filters.searchInput}
        healthFilter={state.filters.healthFilter}
        sourceFilter={state.filters.sourceFilter}
        regionFilter={state.filters.regionFilter}
        usageFilter={state.filters.usageFilter}
        tagFilter={state.filters.tagFilter}
        resultCount={rows.length}
        loadedCount={state.rows.length}
        totalCount={state.totalCount}
        healthOptions={healthOptions}
        sourceOptions={sourceOptions}
        regionOptions={regionOptions}
        usageOptions={usageOptions}
        tagOptions={tagOptions}
        onSearchChange={actions.setSearchInput}
        onHealthFilterChange={(value) =>
          actions.setHealthFilter(value as typeof state.filters.healthFilter)
        }
        onSourceFilterChange={(value) =>
          actions.setSourceFilter(value as typeof state.filters.sourceFilter)
        }
        onRegionFilterChange={actions.setRegionFilter}
        onUsageFilterChange={(value) =>
          actions.setUsageFilter(value as typeof state.filters.usageFilter)
        }
        onTagFilterChange={actions.setTagFilter}
        onClearFilters={actions.clearFilters}
      />

      {state.listError ? <div className="banner banner--error">{state.listError}</div> : null}

      <div className="proxies-layout">
        <ProxyTable
          rows={rows}
          totalCount={state.totalCount || state.rows.length}
          selectedProxyId={state.selectedProxyId}
          selectedIds={state.selectedIds}
          changeIpResults={state.changeIp.results}
          allVisibleSelected={allVisibleSelected}
          sortField={state.table.sortField}
          sortOptions={sortOptions}
          onSortFieldChange={actions.setSortField}
          onOpenRow={actions.selectProxy}
          onToggleSelection={actions.toggleSelection}
          onSetVisibleSelection={actions.setSelection}
        />

        <UsagePanel
          proxy={selectedProxy}
          detail={state.detail}
          detailSource={state.detailSource}
          isLoading={state.isLoadingDetail}
          error={state.detailError}
          hiddenByFilters={selectedProxyHidden}
          changeIpFeedback={selectedProxyChangeIpFeedback}
          isChangingIp={Boolean(
            selectedProxy &&
              state.changeIp.phase === "running" &&
              state.changeIp.activeProxyId === selectedProxy.id,
          )}
          changeIpActionLabel={
            state.selectedIds.length > 0
              ? `Change ${state.selectedIds.length} selected IP`
              : "Change current IP"
          }
          onChangeIp={actions.startChangeIp}
          onRetry={actions.reloadSelectedProxy}
        />
      </div>
    </div>
  );
}
