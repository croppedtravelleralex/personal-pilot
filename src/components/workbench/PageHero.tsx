import type { ReactNode } from 'react'
import clsx from 'clsx'

interface PageHeroProps {
  eyebrow?: string
  title: string
  detail?: ReactNode
  actions?: ReactNode
  meta?: ReactNode
  compact?: boolean
  className?: string
}

export function PageHero({
  eyebrow,
  title,
  detail,
  actions,
  meta,
  compact = false,
  className,
}: PageHeroProps) {
  return (
    <section
      className={clsx(
        'flex flex-col gap-4 border-b border-[var(--color-border-muted)]',
        compact ? 'pb-4' : 'pb-5',
        className
      )}
    >
      <div className="flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
        <div className="min-w-0">
          {eyebrow ? (
            <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
              {eyebrow}
            </div>
          ) : null}
          <h1 className={clsx('font-semibold text-[var(--color-text-primary)]', compact ? 'text-xl' : 'text-2xl')}>
            {title}
          </h1>
          {detail ? (
            <div className="mt-2 max-w-3xl text-sm leading-6 text-[var(--color-text-secondary)]">
              {detail}
            </div>
          ) : null}
        </div>
        {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
      {meta ? <div className="flex flex-wrap items-center gap-2">{meta}</div> : null}
    </section>
  )
}
