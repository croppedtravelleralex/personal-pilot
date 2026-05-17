import type { SynchronizerActionFeedItem } from "../../features/synchronizer/model";
import { EmptyState } from "../EmptyState";
import { formatRelativeTimestamp } from "../../utils/format";

interface SynchronizerActionFeedProps {
  items: SynchronizerActionFeedItem[];
}

export function SynchronizerActionFeed({ items }: SynchronizerActionFeedProps) {
  if (items.length === 0) {
    return (
      <EmptyState
        title="No sync actions yet"
        detail="Layout changes, refresh feedback, main-window updates, and focus attempts will accumulate here."
      />
    );
  }

  return (
    <div className="action-feed">
      {items.map((item) => (
        <article className="action-feed__item" key={item.id}>
          <div className="action-feed__top">
            <div>
              <strong>{item.title}</strong>
              <p className="record-card__subline">
                {item.kind.replaceAll("_", " ")} - {item.executionLabel}
              </p>
            </div>
            <div className="toolbar-actions">
              <span
                className={`badge badge--${
                  item.tone === "success"
                    ? "succeeded"
                    : item.tone === "error"
                      ? "error"
                      : item.tone
                }`}
              >
                {item.tone}
              </span>
              <span
                className={`badge badge--${
                  item.executionMode === "native_live"
                    ? "info"
                    : item.executionMode === "local_staged"
                      ? "warning"
                      : "error"
                }`}
              >
                {item.executionLabel}
              </span>
            </div>
          </div>
          <p>{item.detail}</p>
          <span>{formatRelativeTimestamp(item.createdAt)}</span>
        </article>
      ))}
    </div>
  );
}
