import type { DesktopSyncWindowState } from "../../types/desktop";
import { formatRelativeTimestamp } from "../../utils/format";

interface WindowCardProps {
  window: DesktopSyncWindowState;
  isSelected: boolean;
  activeAction: "layout" | "setMain" | "focus" | null;
  onSelect: (windowId: string) => void;
  onSetMain: (windowId: string) => void;
  onFocus: (windowId: string) => void;
}

function getWindowTone(window: DesktopSyncWindowState) {
  if (window.status === "missing") {
    return "danger";
  }
  if (window.status === "busy" || window.status === "minimized") {
    return "warning";
  }
  if (window.status === "focused") {
    return "info";
  }
  return "success";
}

export function WindowCard({
  window,
  isSelected,
  activeAction,
  onSelect,
  onSetMain,
  onFocus,
}: WindowCardProps) {
  const tone = getWindowTone(window);
  const bounds = window.bounds
    ? `${window.bounds.width}x${window.bounds.height} @ ${window.bounds.x},${window.bounds.y}`
    : "No bounds captured";
  const activityLabel =
    window.status === "missing"
      ? "Needs recovery"
      : window.status === "busy"
        ? "Busy lane"
        : window.status === "minimized"
          ? "Hidden from operator view"
          : window.isMainWindow
            ? "Primary sync lane"
            : "Ready for operator control";

  return (
    <article
      className={`window-card${isSelected ? " window-card--selected" : ""}`}
      onClick={() => onSelect(window.windowId)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect(window.windowId);
        }
      }}
      role="button"
      tabIndex={0}
    >
      <div className="window-card__top">
        <div>
          <strong>{window.profileLabel ?? window.title ?? window.windowId}</strong>
          <p>{window.title ?? "Untitled local browser window"}</p>
        </div>
        <div className="window-card__badges">
          <span className={`badge badge--${tone}`}>{window.status.replaceAll("_", " ")}</span>
          {window.isMainWindow ? <span className="badge badge--succeeded">Main</span> : null}
          {window.isFocused ? <span className="badge badge--info">Focused</span> : null}
          {window.isMinimized ? <span className="badge badge--warning">Minimized</span> : null}
        </div>
      </div>

      <div className="window-card__meta">
        <span>{window.platformId ?? "unknown platform"}</span>
        <span>{window.storeId ?? "no store bound"}</span>
        <span>{window.nativeHandle ?? "native handle pending"}</span>
      </div>

      <p className="record-card__content record-card__content--muted">{activityLabel}</p>

      <div className="toolbar-actions">
        <span className={`badge badge--${window.isVisible && !window.isMinimized ? "succeeded" : "warning"}`}>
          {window.isVisible && !window.isMinimized ? "Visible" : "Hidden"}
        </span>
        <span className="badge badge--info">
          {window.isMainWindow ? "Controller" : "Controlled"}
        </span>
      </div>

      <div className="window-card__stats">
        <div>
          <small>Bounds</small>
          <strong>{bounds}</strong>
        </div>
        <div>
          <small>Last seen</small>
          <strong>{formatRelativeTimestamp(window.lastSeenAt)}</strong>
        </div>
        <div>
          <small>Last action</small>
          <strong>{formatRelativeTimestamp(window.lastActionAt)}</strong>
        </div>
      </div>

      <div className="window-card__footer">
        <button
          className="button button--secondary"
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onFocus(window.windowId);
          }}
          disabled={activeAction !== null || window.status === "missing"}
        >
          {activeAction === "focus" ? "Focusing..." : "Bring focus"}
        </button>
        <button
          className="button"
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onSetMain(window.windowId);
          }}
          disabled={activeAction !== null || window.status === "missing"}
        >
          {window.isMainWindow
            ? "Controller"
            : activeAction === "setMain"
              ? "Applying..."
              : "Make controller"}
        </button>
      </div>
    </article>
  );
}
