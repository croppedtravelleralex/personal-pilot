import { SearchInput } from "../SearchInput";
import {
  PROFILE_COLUMN_DEFINITIONS,
  type ProfileColumnId,
  type ProfilesDensity,
  type ProfilesSortKey,
} from "../../features/profiles/model";

const SORT_OPTIONS: Array<{ value: ProfilesSortKey; label: string }> = [
  { value: "lastActive", label: "Last Active" },
  { value: "status", label: "Runtime Status" },
  { value: "name", label: "Profile Name" },
];

const DENSITY_OPTIONS: Array<{ value: ProfilesDensity; label: string }> = [
  { value: "compact", label: "Compact" },
  { value: "comfortable", label: "Comfortable" },
];

interface ProfilesToolbarProps {
  searchInput: string;
  sortBy: ProfilesSortKey;
  density: ProfilesDensity;
  visibleCount: number;
  totalCount: number;
  activeFilterCount: number;
  visibleColumnCount: number;
  isFiltering: boolean;
  columnVisibility: Record<ProfileColumnId, boolean>;
  onSearchInputChange: (value: string) => void;
  onSortChange: (value: ProfilesSortKey) => void;
  onDensityChange: (value: ProfilesDensity) => void;
  onToggleColumn: (columnId: ProfileColumnId) => void;
  onResetView: () => void;
  onCreateProfile: () => void;
}

export function ProfilesToolbar({
  searchInput,
  sortBy,
  density,
  visibleCount,
  totalCount,
  activeFilterCount,
  visibleColumnCount,
  isFiltering,
  columnVisibility,
  onSearchInputChange,
  onSortChange,
  onDensityChange,
  onToggleColumn,
  onResetView,
  onCreateProfile,
}: ProfilesToolbarProps) {
  return (
    <section className="toolbar-card profiles-toolbar">
      <div className="profiles-toolbar__grid">
        <SearchInput
          label="Profile Search"
          value={searchInput}
          placeholder="Search profile, code, tag, proxy, region"
          onChange={onSearchInputChange}
        />
        <label className="field">
          <span className="field__label">Sort</span>
          <select
            className="field__input"
            value={sortBy}
            onChange={(event) => onSortChange(event.target.value as ProfilesSortKey)}
          >
            {SORT_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field__label">Density</span>
          <select
            className="field__input"
            value={density}
            onChange={(event) => onDensityChange(event.target.value as ProfilesDensity)}
          >
            {DENSITY_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <div className="profiles-toolbar__actions">
          <button className="button button--secondary" type="button" onClick={onResetView}>
            Reset View
          </button>
          <button className="button" type="button" onClick={onCreateProfile}>
            New Profile
          </button>
        </div>
      </div>

      <div className="profiles-toolbar__meta">
        <div className="profiles-toolbar__summary">
          <span className="badge">Workbench</span>
          <strong>
            {visibleCount} / {totalCount}
          </strong>
          <span>{isFiltering ? "Refreshing filtered result..." : "profiles in current result"}</span>
          <span>{activeFilterCount} active filters</span>
          <span>{visibleColumnCount} columns visible</span>
        </div>

        <div className="profiles-toolbar__columns">
          {PROFILE_COLUMN_DEFINITIONS.map((column) => (
            <button
              key={column.id}
              className={`profiles-toolbar__column-chip ${
                columnVisibility[column.id] ? "is-active" : ""
              }`}
              type="button"
              onClick={() => onToggleColumn(column.id)}
              disabled={!column.optional}
              title={column.optional ? `Toggle ${column.label}` : `${column.label} is always visible`}
            >
              {column.label}
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}
