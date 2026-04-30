import type { ReactNode } from 'react'
import clsx from 'clsx'

export type WorkbenchActionTone = 'primary' | 'secondary' | 'ghost' | 'danger'

export interface WorkbenchActionItem {
  id: string
  label: string
  description?: string
  icon?: ReactNode
  disabled?: boolean
  tone?: WorkbenchActionTone
  onClick: () => void
}

interface WorkbenchActionStripProps {
  title?: string
  detail?: string
  items: readonly WorkbenchActionItem[]
  compact?: boolean
  className?: string
}

const toneClassNames: Record<WorkbenchActionTone, string> = {
  primary: 'bg-[var(--color-accent)] text-[var(--color-text-inverse)] hover:opacity-90',
  secondary:
    'border border-[var(--color-border-default)] bg-[var(--color-bg-surface)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-muted)]',
  ghost: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-accent-muted)]',
  danger: 'bg-[var(--color-error)] text-white hover:opacity-90',
}

export function WorkbenchActionStrip({
  title,
  detail,
  items,
  compact = false,
  className,
}: WorkbenchActionStripProps) {
  if (items.length === 0) {
    return null
  }

  return (
    <section
      className={clsx(
        'rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-surface)]',
        compact ? 'p-3' : 'p-4',
        className
      )}
    >
      {title || detail ? (
        <header className="mb-3">
          {title ? <h2 className="text-sm font-semibold text-[var(--color-text-primary)]">{title}</h2> : null}
          {detail ? <p className="mt-1 text-xs text-[var(--color-text-muted)]">{detail}</p> : null}
        </header>
      ) : null}
      <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            disabled={item.disabled}
            className={clsx(
              'flex min-h-[44px] items-center gap-2 rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors',
              toneClassNames[item.tone ?? 'secondary'],
              item.disabled && 'cursor-not-allowed opacity-50'
            )}
            onClick={item.onClick}
          >
            {item.icon ? <span className="flex h-5 w-5 shrink-0 items-center justify-center">{item.icon}</span> : null}
            <span className="min-w-0">
              <span className="block truncate">{item.label}</span>
              {item.description ? (
                <span className="block truncate text-xs font-normal opacity-75">{item.description}</span>
              ) : null}
            </span>
          </button>
        ))}
      </div>
    </section>
  )
}
