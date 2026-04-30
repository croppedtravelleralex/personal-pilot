import { useMemo, useState, type ReactNode } from 'react'
import clsx from 'clsx'

interface VirtualListProps<TItem> {
  items: readonly TItem[]
  height: number
  itemHeight: number
  overscan?: number
  getKey: (item: TItem, index: number) => string
  renderItem: (item: TItem, index: number) => ReactNode
  className?: string
  itemClassName?: string
}

export function VirtualList<TItem>({
  items,
  height,
  itemHeight,
  overscan = 6,
  getKey,
  renderItem,
  className,
  itemClassName,
}: VirtualListProps<TItem>) {
  const [scrollTop, setScrollTop] = useState(0)
  const safeItemHeight = Math.max(1, itemHeight)
  const safeHeight = Math.max(1, height)
  const totalHeight = items.length * safeItemHeight

  const windowed = useMemo(() => {
    const firstVisibleIndex = Math.floor(scrollTop / safeItemHeight)
    const visibleCount = Math.ceil(safeHeight / safeItemHeight)
    const startIndex = Math.max(0, firstVisibleIndex - overscan)
    const endIndex = Math.min(items.length, firstVisibleIndex + visibleCount + overscan)

    return {
      startIndex,
      offsetY: startIndex * safeItemHeight,
      items: items.slice(startIndex, endIndex),
    }
  }, [items, overscan, safeHeight, safeItemHeight, scrollTop])

  return (
    <div
      className={clsx('overflow-auto', className)}
      style={{ height: safeHeight }}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div className="relative" style={{ height: totalHeight }}>
        <div
          className="absolute left-0 right-0 top-0"
          style={{ transform: `translateY(${windowed.offsetY}px)` }}
        >
          {windowed.items.map((item, index) => {
            const absoluteIndex = windowed.startIndex + index

            return (
              <div
                key={getKey(item, absoluteIndex)}
                className={itemClassName}
                style={{ height: safeItemHeight }}
              >
                {renderItem(item, absoluteIndex)}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
