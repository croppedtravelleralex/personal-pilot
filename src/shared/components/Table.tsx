import {
  ReactNode,
  UIEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import clsx from 'clsx'
import { ArrowDown, ArrowUp } from 'lucide-react'

const DEFAULT_VIRTUALIZATION_THRESHOLD = 200
const DEFAULT_VIRTUAL_ROW_HEIGHT = 64
const DEFAULT_VIRTUAL_OVERSCAN = 6

export type SortOrder = 'asc' | 'desc' | undefined

export interface SorterResult {
  column: string
  order: SortOrder
}

export interface TableColumn<T> {
  key: string
  title: ReactNode
  width?: string | number
  align?: 'left' | 'center' | 'right'
  render?: (value: any, record: T, index: number) => ReactNode
  sortable?: boolean
}

interface TableProps<T> {
  columns: TableColumn<T>[]
  data: T[]
  rowKey: string | ((record: T) => string)
  loading?: boolean
  emptyText?: string
  onRowClick?: (record: T) => void
  className?: string
  maxHeight?: string
  stickyHeader?: boolean
  onSort?: (sorterResult: SorterResult) => void
  sortColumn?: string
  sortOrder?: SortOrder
  virtualized?: boolean
  virtualizationThreshold?: number
  virtualRowHeight?: number
  virtualOverscan?: number
}

export function Table<T extends object>({
  columns,
  data,
  rowKey,
  loading = false,
  emptyText = '暂无数据',
  onRowClick,
  className,
  maxHeight = 'calc(100vh - 320px)',
  stickyHeader = true,
  onSort,
  sortColumn,
  sortOrder,
  virtualized,
  virtualizationThreshold = DEFAULT_VIRTUALIZATION_THRESHOLD,
  virtualRowHeight = DEFAULT_VIRTUAL_ROW_HEIGHT,
  virtualOverscan = DEFAULT_VIRTUAL_OVERSCAN,
}: TableProps<T>) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(0)
  const rowHeight = Math.max(1, virtualRowHeight)
  const overscan = Math.max(0, virtualOverscan)
  const shouldVirtualize =
    (virtualized ?? data.length > virtualizationThreshold) && data.length > 0

  const syncViewportHeight = useCallback(() => {
    setViewportHeight(containerRef.current?.clientHeight ?? 0)
  }, [])

  useEffect(() => {
    syncViewportHeight()

    if (!shouldVirtualize || !containerRef.current || typeof ResizeObserver === 'undefined') {
      return
    }

    const resizeObserver = new ResizeObserver(syncViewportHeight)
    resizeObserver.observe(containerRef.current)

    return () => resizeObserver.disconnect()
  }, [shouldVirtualize, syncViewportHeight])

  useEffect(() => {
    if (!shouldVirtualize) {
      setScrollTop(0)
      return
    }

    const maxScrollTop = Math.max(0, data.length * rowHeight - viewportHeight)
    if (scrollTop <= maxScrollTop) return

    setScrollTop(maxScrollTop)
    if (containerRef.current) {
      containerRef.current.scrollTop = maxScrollTop
    }
  }, [data.length, rowHeight, scrollTop, shouldVirtualize, viewportHeight])

  const handleScroll = useCallback(
    (event: UIEvent<HTMLDivElement>) => {
      if (shouldVirtualize) {
        setScrollTop(event.currentTarget.scrollTop)
      }
    },
    [shouldVirtualize]
  )

  const virtualRange = useMemo(() => {
    if (!shouldVirtualize) {
      return {
        startIndex: 0,
        endIndex: data.length,
        topSpacerHeight: 0,
        bottomSpacerHeight: 0,
      }
    }

    const visibleCount = Math.max(
      1,
      Math.ceil((viewportHeight || rowHeight * 12) / rowHeight)
    )
    const firstVisibleIndex = Math.floor(scrollTop / rowHeight)
    const startIndex = Math.max(0, firstVisibleIndex - overscan)
    const endIndex = Math.min(data.length, firstVisibleIndex + visibleCount + overscan)

    return {
      startIndex,
      endIndex,
      topSpacerHeight: startIndex * rowHeight,
      bottomSpacerHeight: Math.max(0, (data.length - endIndex) * rowHeight),
    }
  }, [data.length, overscan, rowHeight, scrollTop, shouldVirtualize, viewportHeight])

  const visibleRows = useMemo(() => {
    if (!shouldVirtualize) return data
    return data.slice(virtualRange.startIndex, virtualRange.endIndex)
  }, [data, shouldVirtualize, virtualRange.endIndex, virtualRange.startIndex])

  const getRowKey = (record: T, index: number): string => {
    if (typeof rowKey === 'function') {
      return rowKey(record)
    }
    const value = (record as Record<string, unknown>)[rowKey]
    return value == null ? index.toString() : String(value)
  }

  const handleSortClick = (column: TableColumn<T>) => {
    if (!column.sortable || !onSort) return

    let newOrder: SortOrder
    if (sortColumn !== column.key) {
      newOrder = 'asc'
    } else {
      newOrder = sortOrder === 'asc' ? 'desc' : sortOrder === 'desc' ? undefined : 'asc'
    }

    onSort({ column: column.key, order: newOrder })
  }

  const renderSortIcon = (column: TableColumn<T>) => {
    if (!column.sortable) return null

    if (sortColumn === column.key) {
      if (sortOrder === 'asc') {
        return <ArrowUp className="w-3.5 h-3.5 ml-1 text-[var(--color-accent)]" />
      }
      if (sortOrder === 'desc') {
        return <ArrowDown className="w-3.5 h-3.5 ml-1 text-[var(--color-accent)]" />
      }
    }

    return (
      <span className="text-[var(--color-text-muted)] ml-1 opacity-40 group-hover:opacity-70">
        <ArrowUp className="w-3 h-3" />
      </span>
    )
  }

  const renderCell = (column: TableColumn<T>, record: T, index: number) => {
    const value = (record as Record<string, unknown>)[column.key]
    return column.render ? column.render(value, record, index) : (value as ReactNode)
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16" style={{ maxHeight }}>
        <div className="flex flex-col items-center gap-3">
          <div className="w-6 h-6 border-2 border-[var(--color-border-default)] border-t-[var(--color-accent)] rounded-full animate-spin" />
          <span className="text-sm text-[var(--color-text-muted)]">加载中...</span>
        </div>
      </div>
    )
  }

  return (
    <div
      ref={containerRef}
      className={clsx('overflow-auto', className)}
      style={{ maxHeight }}
      onScroll={handleScroll}
    >
      <table className="min-w-full">
        <thead className={clsx(stickyHeader && 'sticky top-0 z-10')}>
          <tr>
            {columns.map((col) => (
              <th
                key={col.key}
                className={clsx(
                  'px-4 py-3 text-xs font-semibold text-[var(--color-text-muted)] uppercase tracking-wider bg-[var(--color-bg-muted)]',
                  col.align === 'center' && 'text-center',
                  col.align === 'right' && 'text-right',
                  !col.align && 'text-left',
                  col.sortable && 'cursor-pointer group hover:text-[var(--color-text-primary)]'
                )}
                style={{ width: col.width }}
                onClick={() => col.sortable && handleSortClick(col)}
              >
                <span className="flex items-center">
                  {col.title}
                  {renderSortIcon(col)}
                </span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-[var(--color-border-muted)] bg-[var(--color-bg-surface)]">
          {data.length === 0 ? (
            <tr>
              <td colSpan={columns.length} className="px-4 py-16 text-center">
                <div className="flex flex-col items-center gap-2">
                  <div className="w-12 h-12 rounded-full bg-[var(--color-bg-muted)] flex items-center justify-center">
                    <svg className="w-6 h-6 text-[var(--color-text-muted)]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
                    </svg>
                  </div>
                  <span className="text-sm text-[var(--color-text-muted)]">{emptyText}</span>
                </div>
              </td>
            </tr>
          ) : (
            <>
              {shouldVirtualize && virtualRange.topSpacerHeight > 0 && (
                <tr aria-hidden="true" style={{ height: virtualRange.topSpacerHeight }}>
                  <td colSpan={columns.length} className="p-0" />
                </tr>
              )}

              {visibleRows.map((record, offset) => {
                const index = virtualRange.startIndex + offset

                return (
                  <tr
                    key={getRowKey(record, index)}
                    className={clsx(
                      'hover:bg-[var(--color-bg-muted)]/50 transition-colors duration-150',
                      onRowClick && 'cursor-pointer'
                    )}
                    style={shouldVirtualize ? { height: rowHeight } : undefined}
                    onClick={() => onRowClick?.(record)}
                  >
                    {columns.map((col) => {
                      const content = renderCell(col, record, index)

                      return (
                        <td
                          key={col.key}
                          className={clsx(
                            'px-4 text-sm text-[var(--color-text-secondary)]',
                            shouldVirtualize ? 'py-2' : 'py-3.5',
                            col.align === 'center' && 'text-center',
                            col.align === 'right' && 'text-right'
                          )}
                        >
                          {shouldVirtualize ? (
                            <div
                              className="overflow-hidden"
                              style={{ maxHeight: Math.max(24, rowHeight - 16) }}
                            >
                              {content}
                            </div>
                          ) : (
                            content
                          )}
                        </td>
                      )
                    })}
                  </tr>
                )
              })}

              {shouldVirtualize && virtualRange.bottomSpacerHeight > 0 && (
                <tr aria-hidden="true" style={{ height: virtualRange.bottomSpacerHeight }}>
                  <td colSpan={columns.length} className="p-0" />
                </tr>
              )}
            </>
          )}
        </tbody>
      </table>
    </div>
  )
}
