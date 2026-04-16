import type {
  DesktopSyncLayoutState,
  DesktopSyncWindowState,
} from "../../types/desktop";
import { formatRelativeTimestamp } from "../../utils/format";

interface MainWindowBadgeProps {
  layout: DesktopSyncLayoutState;
  mainWindow: DesktopSyncWindowState | null;
  focusedWindow: DesktopSyncWindowState | null;
  selectedWindow: DesktopSyncWindowState | null;
  controlledCount: number;
  stagedBroadcastPlanTitle: string | null;
}

function renderWindowLabel(window: DesktopSyncWindowState | null, fallback: string) {
  if (!window) {
    return fallback;
  }

  return window.profileLabel ?? window.title ?? window.windowId;
}

export function MainWindowBadge({
  layout,
  mainWindow,
  focusedWindow,
  selectedWindow,
  controlledCount,
  stagedBroadcastPlanTitle,
}: MainWindowBadgeProps) {
  const hasMainFocusDrift =
    mainWindow && focusedWindow && mainWindow.windowId !== focusedWindow.windowId;
  const hasSelectionDrift =
    selectedWindow &&
    focusedWindow &&
    selectedWindow.windowId !== focusedWindow.windowId;
  const driftMessage = hasMainFocusDrift
    ? `Main window and focused window differ: ${renderWindowLabel(mainWindow, "main")} vs ${renderWindowLabel(focusedWindow, "focus")}.`
    : hasSelectionDrift
      ? `Console selection is different from the focused native window: ${renderWindowLabel(selectedWindow, "selected")}.`
      : "Selection, focus, and main-window state are aligned.";
  const driftTone = hasMainFocusDrift ? "warning" : hasSelectionDrift ? "info" : "info";

  return (
    <div className="main-window-badge">
      <div className="main-window-badge__hero">
        <div>
          <span className="shell__eyebrow">Main Window State</span>
          <h3>{renderWindowLabel(mainWindow, "No main window selected")}</h3>
          <p>
            {mainWindow
              ? `${mainWindow.platformId ?? "unknown platform"} | ${mainWindow.storeId ?? "no store"}`
              : "Pick a window in the matrix to stage a primary sync target."}
          </p>
        </div>
        <span className={`badge badge--${mainWindow ? "succeeded" : "warning"}`}>
          {mainWindow ? "Primary ready" : "Selection needed"}
        </span>
      </div>

      <div className={`banner banner--${driftTone}`}>{driftMessage}</div>

      <div className="details-grid details-grid--stacked">
        <article className="details-grid__item">
          <dt>Main driver</dt>
          <dd>{renderWindowLabel(mainWindow, "No main window")}</dd>
        </article>
        <article className="details-grid__item">
          <dt>Focused window</dt>
          <dd>{renderWindowLabel(focusedWindow, "No focused window")}</dd>
        </article>
        <article className="details-grid__item">
          <dt>Selected card</dt>
          <dd>{renderWindowLabel(selectedWindow, "No card selected")}</dd>
        </article>
        <article className="details-grid__item">
          <dt>Layout preset</dt>
          <dd>{layout.mode.replaceAll("_", " ")}</dd>
        </article>
        <article className="details-grid__item">
          <dt>Controlled windows</dt>
          <dd>{controlledCount}</dd>
        </article>
        <article className="details-grid__item">
          <dt>Sync flags</dt>
          <dd>
            Scroll {layout.syncScroll ? "on" : "off"} | Navigation{" "}
            {layout.syncNavigation ? "on" : "off"} | Input {layout.syncInput ? "on" : "off"}
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Broadcast plan</dt>
          <dd>{stagedBroadcastPlanTitle ?? "No prepared plan"}</dd>
        </article>
        <article className="details-grid__item">
          <dt>Operator note</dt>
          <dd>
            {hasMainFocusDrift
              ? "Decide whether to restore focus to the main window or intentionally keep the current focus target."
              : hasSelectionDrift
                ? "The console is watching a different card than the native focus lane."
                : "The primary sync lane is clear enough for quick layout and focus actions."}
          </dd>
        </article>
        <article className="details-grid__item">
          <dt>Last layout update</dt>
          <dd>{formatRelativeTimestamp(layout.updatedAt)}</dd>
        </article>
      </div>
    </div>
  );
}
