import type {
  FilterOption,
  ProfilePlatform,
  ProfileRuntimeStatus,
  ProxyHealth,
} from "../../features/profiles/model";

interface ProfilesFilterRailProps {
  totalCount: number;
  loadedCount: number;
  visibleCount: number;
  runningCount: number;
  healthyProxyCount: number;
  activeFilterCount: number;
  selectedGroupIds: string[];
  selectedTagValues: string[];
  selectedPlatformIds: ProfilePlatform[];
  selectedRuntimeStatuses: ProfileRuntimeStatus[];
  selectedProxyHealth: ProxyHealth[];
  groupOptions: FilterOption[];
  tagOptions: FilterOption[];
  platformOptions: FilterOption<ProfilePlatform>[];
  runtimeStatusOptions: FilterOption<ProfileRuntimeStatus>[];
  proxyHealthOptions: FilterOption<ProxyHealth>[];
  onToggleGroup: (groupId: string) => void;
  onToggleTag: (tagValue: string) => void;
  onTogglePlatform: (platformId: ProfilePlatform) => void;
  onToggleRuntimeStatus: (status: ProfileRuntimeStatus) => void;
  onToggleProxyHealth: (proxyHealth: ProxyHealth) => void;
  onClearFilters: () => void;
}

function FilterSection<T extends string>({
  title,
  options,
  selectedValues,
  onToggle,
}: {
  title: string;
  options: FilterOption<T>[];
  selectedValues: readonly T[];
  onToggle: (value: T) => void;
}) {
  return (
    <section className="profiles-filter-rail__section">
      <div className="profiles-filter-rail__section-head">
        <strong>{title}</strong>
        <span>{options.length}</span>
      </div>
      <div className="profiles-filter-rail__section-body">
        {options.map((option) => {
          const isSelected = selectedValues.includes(option.value);

          return (
            <button
              key={option.value}
              className={`profiles-filter-rail__option ${isSelected ? "is-active" : ""}`}
              type="button"
              onClick={() => onToggle(option.value)}
            >
              <span>{option.label}</span>
              <strong>{option.count}</strong>
            </button>
          );
        })}
      </div>
    </section>
  );
}

export function ProfilesFilterRail({
  totalCount,
  loadedCount,
  visibleCount,
  runningCount,
  healthyProxyCount,
  activeFilterCount,
  selectedGroupIds,
  selectedTagValues,
  selectedPlatformIds,
  selectedRuntimeStatuses,
  selectedProxyHealth,
  groupOptions,
  tagOptions,
  platformOptions,
  runtimeStatusOptions,
  proxyHealthOptions,
  onToggleGroup,
  onToggleTag,
  onTogglePlatform,
  onToggleRuntimeStatus,
  onToggleProxyHealth,
  onClearFilters,
}: ProfilesFilterRailProps) {
  return (
    <aside className="profiles-filter-rail">
      <div className="profiles-filter-rail__snapshot">
        <div>
          <span className="shell__eyebrow">Workbench Snapshot</span>
          <h2>Filter Rail</h2>
          <p>Keep group, runtime, platform, and proxy scopes outside the dense table path.</p>
        </div>
        <button
          className="button button--secondary profiles-filter-rail__clear"
          type="button"
          onClick={onClearFilters}
          disabled={activeFilterCount === 0}
        >
          Clear Filters
        </button>
      </div>

      <div className="profiles-filter-rail__metrics">
        <article className="profiles-filter-rail__metric">
          <span>Visible</span>
          <strong>{visibleCount}</strong>
          <small>
            loaded {loadedCount} / total {totalCount}
          </small>
        </article>
        <article className="profiles-filter-rail__metric">
          <span>Running</span>
          <strong>{runningCount}</strong>
          <small>active browsers</small>
        </article>
        <article className="profiles-filter-rail__metric">
          <span>Healthy Proxy</span>
          <strong>{healthyProxyCount}</strong>
          <small>ready lanes</small>
        </article>
        <article className="profiles-filter-rail__metric">
          <span>Filters</span>
          <strong>{activeFilterCount}</strong>
          <small>current scopes</small>
        </article>
      </div>

      <FilterSection
        title="Groups"
        options={groupOptions}
        selectedValues={selectedGroupIds}
        onToggle={onToggleGroup}
      />
      <FilterSection
        title="Tags"
        options={tagOptions}
        selectedValues={selectedTagValues}
        onToggle={onToggleTag}
      />
      <FilterSection
        title="Platforms"
        options={platformOptions}
        selectedValues={selectedPlatformIds}
        onToggle={onTogglePlatform}
      />
      <FilterSection
        title="Runtime"
        options={runtimeStatusOptions}
        selectedValues={selectedRuntimeStatuses}
        onToggle={onToggleRuntimeStatus}
      />
      <FilterSection
        title="Proxy Health"
        options={proxyHealthOptions}
        selectedValues={selectedProxyHealth}
        onToggle={onToggleProxyHealth}
      />
    </aside>
  );
}
