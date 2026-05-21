import { useMemo, useState, type ReactNode } from "react";

interface VirtualListProps<T> {
  items: T[];
  height: number;
  itemHeight: number;
  overscan?: number;
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => ReactNode;
}

export function VirtualList<T>({
  items,
  height,
  itemHeight,
  overscan = 4,
  getKey,
  renderItem,
}: VirtualListProps<T>) {
  const [scrollTop, setScrollTop] = useState(0);
  const totalHeight = items.length * itemHeight;

  const windowed = useMemo(() => {
    const start = Math.max(0, Math.floor(scrollTop / itemHeight) - overscan);
    const end = Math.min(
      items.length,
      Math.ceil((scrollTop + height) / itemHeight) + overscan,
    );

    return {
      start,
      end,
      offsetY: start * itemHeight,
      items: items.slice(start, end),
    };
  }, [height, itemHeight, items, overscan, scrollTop]);

  return (
    <div
      className="virtual-list"
      style={{ height }}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div className="virtual-list__canvas" style={{ height: totalHeight }}>
        <div
          className="virtual-list__window"
          style={{ transform: `translateY(${windowed.offsetY}px)` }}
        >
          {windowed.items.map((item, index) => {
            const absoluteIndex = windowed.start + index;

            return (
              <div
                key={getKey(item, absoluteIndex)}
                style={{ minHeight: itemHeight, maxHeight: itemHeight }}
              >
                {renderItem(item, absoluteIndex)}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
