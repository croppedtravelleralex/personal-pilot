import type { ReactNode } from 'react'
import clsx from 'clsx'

export type MetricStripTone = 'neutral' | 'info' | 'success' | 'warning' | 'danger'

export interface MetricStripItem {
  id?: string
  label: string
  value: ReactNode
  detail?: ReactNode
  tone?: MetricStripTone
}

interface MetricStripProps {
  items: readonly MetricStripItem[]
  columns?: 1 | 2 | 3 | 4
  compact?: boolean
  emptyText?: string
  className?: string
}

const toneClassNames: Record<MetricStripTone, string> = {
  neutral: 'border-[var(--color-border-default)]',
  info: 'border-sky-300 bg-sky-50/70 text-sky-900',
  success: 'border-emerald-300 bg-emerald-50/70 text-emerald-900',
  warning: 'border-amber-300 bg-amber-50/70 text-amber-900',
  danger: 'border-red-300 bg-red-50/70 text-red-900',
}

const columnClassNames: Record<NonNullable<MetricStripProps['columns']>, string> = {
  1: 'grid-cols-1',
  2: 'sm:grid-cols-2',
  3: 'sm:grid-cols-2 xl:grid-cols-3',
  4: 'sm:grid-cols-2 xl:grid-cols-4',
}

export function MetricStrip({
  items,
  columns = 4,
  compact = false,
  emptyText = 'No metrics available.',
  className,
}: MetricStripProps) {
  if (items.length === 0) {
    return (
      <div
        className={clsx(
          'rounded-lg border border-dashed border-[var(--color-border-default)] bg-[var(--color-bg-surface)] px-4 py-3 text-sm text-[var(--color-text-muted)]',
          className
        )}
      >
        {emptyText}
      </div>
    )
  }

  return (
    <section className={clsx('grid gap-3', columnClassNames[columns], className)}>
      {items.map((item) => (
        <article
          key={item.id ?? item.label}
          className={clsx(
            'rounded-lg border bg-[var(--color-bg-surface)]',
            compact ? 'px-3 py-2' : 'px-4 py-3',
            toneClassNames[item.tone ?? 'neutral']
          )}
        >
          <div className="text-xs font-medium text-[var(--color-text-muted)]">{item.label}</div>
          <div className="mt-1 text-lg font-semibold text-[var(--color-text-primary)]">{item.value}</div>
          {item.detail ? (
            <div className="mt-1 text-xs text-[var(--color-text-muted)]">{item.detail}</div>
          ) : null}
        </article>
      ))}
    </section>
  )
}
