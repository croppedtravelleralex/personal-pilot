import type { DesktopSyncWindowState } from "../../types/desktop";
import type { SynchronizerWindowGroup } from "../../features/synchronizer/store";
import { EmptyState } from "../EmptyState";
import { WindowCard } from "./WindowCard";

interface WindowMatrixProps {
  windows: DesktopSyncWindowState[];
  groups: SynchronizerWindowGroup[];
  selectedWindowId: string | null;
  activeAction: "layout" | "setMain" | "focus" | null;
  onSelect: (windowId: string) => void;
  onSetMain: (windowId: string) => void;
  onFocus: (windowId: string) => void;
}

export function WindowMatrix({
  windows,
  groups,
  selectedWindowId,
  activeAction,
  onSelect,
  onSetMain,
  onFocus,
}: WindowMatrixProps) {
  if (windows.length === 0) {
    return (
      <EmptyState
        title="No sync windows detected"
        detail="Open profile windows locally, then refresh the matrix to stage focus and layout actions."
      />
    );
  }

  return (
    <div className="page-stack">
      {groups.map((group) => (
        <section className="page-stack" key={group.id}>
          <div className="contract-card">
            <div className="contract-card__top">
              <strong>{group.label}</strong>
              <span className="badge badge--info">{group.windows.length} windows</span>
            </div>
            <p>{group.detail}</p>
          </div>

          <div className="window-matrix">
            {group.windows.map((window) => (
              <WindowCard
                key={window.windowId}
                window={window}
                isSelected={selectedWindowId === window.windowId}
                activeAction={activeAction}
                onSelect={onSelect}
                onSetMain={onSetMain}
                onFocus={onFocus}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
