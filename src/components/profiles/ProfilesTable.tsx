import { useEffect, useRef } from "react";

import { EmptyState } from "../EmptyState";
import { VirtualList } from "../VirtualList";
import type {
  ProfileColumnDefinition,
  ProfileColumnId,
  ProfileRow,
  ProfilesDensity,
  ProfileRuntimeStatus,
} from "../../features/profiles/model";
import { formatRelativeTimestamp } from "../../utils/format";

const RUNTIME_LABELS: Record<ProfileRuntimeStatus, string> = {
  running: "Running",
  warming: "Warming",
  idle: "Idle",
  error: "Needs Attention",
};

interface ProfilesTableProps {
  rows: ProfileRow[];
  visibleColumns: ProfileColumnDefinition[];
  density: ProfilesDensity;
  activeProfileId: string | null;
  selectedIds: string[];
  allVisibleSelected: boolean;
  partiallySelected: boolean;
  onToggleRowSelection: (profileId: string) => void;
  onToggleVisibleSelection: () => void;
  onOpenProfile: (profileId: string) => void;
  onInspectProfile: (profileId: string) => void;
  onEditProfile: (profileId: string, profileName: string) => void;
}

function SelectionCheckbox({
  checked,
  indeterminate,
  label,
  onChange,
}: {
  checked: boolean;
  indeterminate: boolean;
  label: string;
  onChange: () => void;
}) {
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.indeterminate = indeterminate;
    }
  }, [indeterminate]);

  return (
    <label className="profiles-table__checkbox">
      <input
        ref={inputRef}
        type="checkbox"
        checked={checked}
        onChange={onChange}
        onClick={(event) => event.stopPropagation()}
      />
      {label ? <span>{label}</span> : null}
    </label>
  );
}

function renderCell(
  columnId: ProfileColumnId,
  row: ProfileRow,
  onInspect: () => void,
  onEdit: () => void,
) {
  switch (columnId) {
    case "profile":
      return (
        <div className="profiles-table__profile">
          <div>
            <strong>
              {row.name}
              <span className="profiles-table__profile-code">{row.code}</span>
            </strong>
            <p>{row.platformLabel}</p>
          </div>
          <div className="profiles-table__profile-actions">
            <button
              className="profiles-table__text-action"
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onInspect();
              }}
            >
              Inspect
            </button>
            <button
              className="profiles-table__text-action"
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onEdit();
              }}
            >
              Edit
            </button>
          </div>
        </div>
      );
    case "group":
      return (
        <div>
          <strong>{row.groupLabel}</strong>
          <p>{row.platformLabel}</p>
        </div>
      );
    case "tags":
      return (
        <div className="profiles-table__tags">
          {row.tags.map((tag) => (
            <span key={tag} className="profiles-table__tag">
              {tag}
            </span>
          ))}
        </div>
      );
    case "browser":
      return (
        <div>
          <strong>{row.browserLabel}</strong>
          <p>Local browser shell</p>
        </div>
      );
    case "proxy":
      return (
        <div>
          <strong>{row.proxyLabel}</strong>
          <p className={`profiles-table__proxy profiles-table__proxy--${row.proxyHealth}`}>
            {row.proxyHealth}
          </p>
        </div>
      );
    case "region":
      return (
        <div>
          <strong>{row.regionLabel}</strong>
          <p>Locale pinned</p>
        </div>
      );
    case "fingerprint":
      return (
        <div>
          <strong>{row.fingerprintLabel}</strong>
          <p>Template placeholder</p>
        </div>
      );
    case "runtime":
      return (
        <div>
          <span className={`badge badge--${row.runtimeStatus}`}>{RUNTIME_LABELS[row.runtimeStatus]}</span>
          <p>{formatRelativeTimestamp(row.lastActiveAt)}</p>
        </div>
      );
    default:
      return null;
  }
}

export function ProfilesTable({
  rows,
  visibleColumns,
  density,
  activeProfileId,
  selectedIds,
  allVisibleSelected,
  partiallySelected,
  onToggleRowSelection,
  onToggleVisibleSelection,
  onOpenProfile,
  onInspectProfile,
  onEditProfile,
}: ProfilesTableProps) {
  const rowHeight = density === "compact" ? 76 : 92;
  const listHeight = Math.min(640, Math.max(280, rows.length * rowHeight));
  const gridTemplateColumns = `52px ${visibleColumns.map((column) => column.width).join(" ")}`;

  return (
    <section className="panel profiles-table-shell">
      <header className="panel__header profiles-table-shell__header">
        <div>
          <h2 className="panel__title">ProfilesTable</h2>
          <p className="panel__subtitle">
            Dense workbench grid with stable column tracks and virtualization-ready body window.
          </p>
        </div>
      </header>

      {rows.length === 0 ? (
        <EmptyState
          title="No profiles match the current filters"
          detail="Adjust the filter rail or clear search to bring rows back into the workbench."
        />
      ) : (
        <div className="profiles-table">
          <div className="profiles-table__header-row" style={{ gridTemplateColumns }}>
            <SelectionCheckbox
              checked={allVisibleSelected}
              indeterminate={partiallySelected}
              label="All"
              onChange={onToggleVisibleSelection}
            />
            {visibleColumns.map((column) => (
              <div key={column.id} className="profiles-table__header-cell">
                {column.label}
              </div>
            ))}
          </div>

          <VirtualList
            items={rows}
            height={listHeight}
            itemHeight={rowHeight}
            getKey={(row) => row.id}
            renderItem={(row) => {
              const isSelected = selectedIds.includes(row.id);
              const isActive = activeProfileId === row.id;

              return (
                <div
                  className={`profiles-table__row profiles-table__row--${density} ${
                    isSelected ? "is-selected" : ""
                  } ${isActive ? "is-active" : ""}`}
                  style={{ gridTemplateColumns }}
                  onClick={() => onOpenProfile(row.id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onOpenProfile(row.id);
                    }
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <SelectionCheckbox
                    checked={isSelected}
                    indeterminate={false}
                    label=""
                    onChange={() => onToggleRowSelection(row.id)}
                  />
                  {visibleColumns.map((column) => (
                    <div key={column.id} className="profiles-table__cell">
                      {renderCell(
                        column.id,
                        row,
                        () => onInspectProfile(row.id),
                        () => onEditProfile(row.id, row.name),
                      )}
                    </div>
                  ))}
                </div>
              );
            }}
          />
        </div>
      )}
    </section>
  );
}
