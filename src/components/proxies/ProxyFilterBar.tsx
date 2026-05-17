import { SearchInput } from "../SearchInput";

interface FilterOption {
  value: string;
  label: string;
}

interface ProxyFilterBarProps {
  searchValue: string;
  healthFilter: string;
  sourceFilter: string;
  regionFilter: string;
  usageFilter: string;
  tagFilter: string;
  resultCount: number;
  loadedCount: number;
  totalCount: number;
  healthOptions: FilterOption[];
  sourceOptions: FilterOption[];
  regionOptions: FilterOption[];
  usageOptions: FilterOption[];
  tagOptions: FilterOption[];
  onSearchChange: (value: string) => void;
  onHealthFilterChange: (value: string) => void;
  onSourceFilterChange: (value: string) => void;
  onRegionFilterChange: (value: string) => void;
  onUsageFilterChange: (value: string) => void;
  onTagFilterChange: (value: string) => void;
  onClearFilters: () => void;
}

export function ProxyFilterBar({
  searchValue,
  healthFilter,
  sourceFilter,
  regionFilter,
  usageFilter,
  tagFilter,
  resultCount,
  loadedCount,
  totalCount,
  healthOptions,
  sourceOptions,
  regionOptions,
  usageOptions,
  tagOptions,
  onSearchChange,
  onHealthFilterChange,
  onSourceFilterChange,
  onRegionFilterChange,
  onUsageFilterChange,
  onTagFilterChange,
  onClearFilters,
}: ProxyFilterBarProps) {
  return (
    <section className="toolbar-card proxy-filterbar">
      <div className="batch-toolbar__header">
        <div>
          <span className="shell__eyebrow">Filtering</span>
          <h2 className="panel__title">Inventory Filters</h2>
          <p className="panel__subtitle">
            Slice the workbench by health, provider source, region, usage posture, and operator tags
            without leaving the list-driven state model.
          </p>
        </div>
        <button className="button button--secondary" type="button" onClick={onClearFilters}>
          Reset filters
        </button>
      </div>

      <div className="proxy-filterbar__grid">
        <SearchInput
          label="Search proxy"
          value={searchValue}
          placeholder="Search name, endpoint, tag, region, or provider"
          onChange={onSearchChange}
        />

        <label className="field">
          <span className="field__label">Health</span>
          <select
            className="field__input"
            value={healthFilter}
            onChange={(event) => onHealthFilterChange(event.target.value)}
          >
            {healthOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field__label">Source</span>
          <select
            className="field__input"
            value={sourceFilter}
            onChange={(event) => onSourceFilterChange(event.target.value)}
          >
            {sourceOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field__label">Usage</span>
          <select
            className="field__input"
            value={usageFilter}
            onChange={(event) => onUsageFilterChange(event.target.value)}
          >
            {usageOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field__label">Region</span>
          <select
            className="field__input"
            value={regionFilter}
            onChange={(event) => onRegionFilterChange(event.target.value)}
          >
            {regionOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="proxy-filterbar__footer">
        <div className="proxy-filterbar__summary">
          Showing <strong>{resultCount}</strong> of <strong>{loadedCount}</strong> loaded proxies
          <span> / total inventory {totalCount}</span>
        </div>

        <div className="proxy-filterbar__tag-group">
          {tagOptions.map((option) => (
            <button
              key={option.value}
              className={`filter-chip ${tagFilter === option.value ? "filter-chip--active" : ""}`}
              type="button"
              onClick={() => onTagFilterChange(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}
