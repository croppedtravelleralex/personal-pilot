import { InlineContentPreview } from "../components/InlineContentPreview";
import { ProfilesBatchBar } from "../components/profiles/ProfilesBatchBar";
import { ProfilesFilterRail } from "../components/profiles/ProfilesFilterRail";
import { ProfilesTable } from "../components/profiles/ProfilesTable";
import { ProfilesToolbar } from "../components/profiles/ProfilesToolbar";
import { ProfileDrawer } from "../components/profiles/ProfileDrawer";
import { useProfilesWorkbenchViewModel } from "../features/profiles/hooks";

export function ProfilesPage() {
  const { state, filteredProfiles, visibleColumns, selection, summary, filterOptions, actions } =
    useProfilesWorkbenchViewModel();

  return (
    <div className="profiles-workbench">
      <ProfilesFilterRail
        totalCount={summary.totalCount}
        loadedCount={summary.loadedCount}
        visibleCount={summary.visibleCount}
        runningCount={summary.runningCount}
        healthyProxyCount={summary.healthyProxyCount}
        activeFilterCount={summary.activeFilterCount}
        selectedGroupIds={state.groupIds}
        selectedTagValues={state.tagValues}
        selectedPlatformIds={state.platformIds}
        selectedRuntimeStatuses={state.runtimeStatuses}
        selectedProxyHealth={state.proxyHealth}
        groupOptions={filterOptions.groups}
        tagOptions={filterOptions.tags}
        platformOptions={filterOptions.platforms}
        runtimeStatusOptions={filterOptions.runtimeStatuses}
        proxyHealthOptions={filterOptions.proxyHealth}
        onToggleGroup={actions.toggleGroup}
        onToggleTag={actions.toggleTag}
        onTogglePlatform={actions.togglePlatform}
        onToggleRuntimeStatus={actions.toggleRuntimeStatus}
        onToggleProxyHealth={actions.toggleProxyHealth}
        onClearFilters={actions.clearFilters}
      />

      <div className="profiles-workbench__main">
        {state.list.error ? (
          <div className="banner banner--error">
            <InlineContentPreview value={state.list.error} collapseAt={260} inlineLimit={4000} />
          </div>
        ) : null}

        {state.actionNotice ? (
          <div className="toolbar-card toolbar-card--subtle profiles-workbench__hint">
            <div>
              <strong>Workbench notice</strong>
              <InlineContentPreview value={state.actionNotice} collapseAt={220} inlineLimit={4000} />
            </div>
            <button
              className="button button--secondary"
              type="button"
              onClick={actions.dismissNotice}
            >
              Dismiss
            </button>
          </div>
        ) : null}

        <ProfilesToolbar
          searchInput={state.searchInput}
          sortBy={state.sortBy}
          density={state.density}
          visibleCount={summary.visibleCount}
          totalCount={summary.totalCount}
          activeFilterCount={summary.activeFilterCount}
          visibleColumnCount={visibleColumns.length}
          isFiltering={state.isFiltering || state.list.status === "loading"}
          columnVisibility={state.columnVisibility}
          onSearchInputChange={actions.setSearchInput}
          onSortChange={actions.setSortBy}
          onDensityChange={actions.setDensity}
          onToggleColumn={actions.toggleColumn}
          onResetView={actions.resetView}
          onCreateProfile={actions.requestCreateProfile}
        />

        <ProfilesBatchBar
          selectedCount={summary.selectedCount}
          selectedVisibleCount={selection.selectedVisibleCount}
          status={state.batch.status}
          activeAction={state.batch.activeAction}
          feedbackMessage={state.batch.message}
          feedbackTone={state.batch.tone}
          feedbackUpdatedAt={state.batch.updatedAt}
          onRunAction={actions.runBatchAction}
          onClearSelection={actions.clearSelection}
          onDismissFeedback={actions.dismissBatchFeedback}
        />

        <div className="profiles-workbench__content">
          <ProfilesTable
            rows={filteredProfiles}
            visibleColumns={visibleColumns}
            density={state.density}
            activeProfileId={state.drawer.openedProfileId}
            selectedIds={state.selectedIds}
            allVisibleSelected={selection.allVisibleSelected}
            partiallySelected={selection.partiallySelected}
            onToggleRowSelection={actions.toggleRowSelection}
            onToggleVisibleSelection={actions.toggleVisibleSelection}
            onOpenProfile={actions.requestProfileDrawer}
            onInspectProfile={actions.requestProfileDrawer}
            onEditProfile={actions.requestEditProfile}
          />

          <ProfileDrawer
            openedProfileId={state.drawer.openedProfileId}
            status={state.drawer.status}
            source={state.drawer.source}
            detail={state.drawer.detail}
            error={state.drawer.error}
            activeTab={state.drawer.activeTab}
            onClose={actions.closeDrawer}
            onRetry={actions.retryDrawer}
            onTabChange={actions.setDrawerTab}
          />
        </div>
      </div>
    </div>
  );
}
