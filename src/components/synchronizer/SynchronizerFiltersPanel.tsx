import type {
  SynchronizerWindowGroupBy,
  SynchronizerWindowRoleFilter,
  SynchronizerVisibilityFilter,
} from "../../features/synchronizer/model";

interface SelectOption {
  value: string;
  label: string;
}

interface SynchronizerFiltersPanelProps {
  searchText: string;
  platformFilter: string;
  statusFilter: string;
  visibilityFilter: SynchronizerVisibilityFilter;
  roleFilter: SynchronizerWindowRoleFilter;
  groupBy: SynchronizerWindowGroupBy;
  platformOptions: SelectOption[];
  statusOptions: SelectOption[];
  visibilityOptions: SelectOption[];
  roleOptions: SelectOption[];
  groupByOptions: SelectOption[];
  filteredCount: number;
  totalCount: number;
  onSearchTextChange: (value: string) => void;
  onPlatformFilterChange: (value: string) => void;
  onStatusFilterChange: (value: string) => void;
  onVisibilityFilterChange: (value: SynchronizerVisibilityFilter) => void;
  onRoleFilterChange: (value: SynchronizerWindowRoleFilter) => void;
  onGroupByChange: (value: SynchronizerWindowGroupBy) => void;
  onReset: () => void;
}

export function SynchronizerFiltersPanel({
  searchText,
  platformFilter,
  statusFilter,
  visibilityFilter,
  roleFilter,
  groupBy,
  platformOptions,
  statusOptions,
  visibilityOptions,
  roleOptions,
  groupByOptions,
  filteredCount,
  totalCount,
  onSearchTextChange,
  onPlatformFilterChange,
  onStatusFilterChange,
  onVisibilityFilterChange,
  onRoleFilterChange,
  onGroupByChange,
  onReset,
}: SynchronizerFiltersPanelProps) {
  return (
    <div className="page-stack">
      <div className="toolbar-actions">
        <span className="badge badge--info">
          {filteredCount}/{totalCount} in scope
        </span>
        <button className="button button--secondary" type="button" onClick={onReset}>
          Reset filters
        </button>
      </div>

      <label className="field">
        <span className="field__label">Search window / profile / store</span>
        <input
          className="field__input"
          type="search"
          value={searchText}
          onChange={(event) => onSearchTextChange(event.target.value)}
          placeholder="Search current synchronizer scope"
        />
      </label>

      <div className="details-grid details-grid--stacked">
        <article className="details-grid__item">
          <dt>Platform</dt>
          <dd>
            <select
              className="field__input"
              value={platformFilter}
              onChange={(event) => onPlatformFilterChange(event.target.value)}
            >
              {platformOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Status</dt>
          <dd>
            <select
              className="field__input"
              value={statusFilter}
              onChange={(event) => onStatusFilterChange(event.target.value)}
            >
              {statusOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Visibility</dt>
          <dd>
            <select
              className="field__input"
              value={visibilityFilter}
              onChange={(event) =>
                onVisibilityFilterChange(event.target.value as SynchronizerVisibilityFilter)
              }
            >
              {visibilityOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Role</dt>
          <dd>
            <select
              className="field__input"
              value={roleFilter}
              onChange={(event) =>
                onRoleFilterChange(event.target.value as SynchronizerWindowRoleFilter)
              }
            >
              {roleOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Group matrix</dt>
          <dd>
            <select
              className="field__input"
              value={groupBy}
              onChange={(event) => onGroupByChange(event.target.value as SynchronizerWindowGroupBy)}
            >
              {groupByOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </dd>
        </article>
      </div>
    </div>
  );
}
