import type { ReactNode } from 'react'
import clsx from 'clsx'

export interface SectionTabItem<TValue extends string = string> {
  value: TValue
  label: ReactNode
  disabled?: boolean
}

interface SectionTabsProps<TValue extends string = string> {
  items: readonly SectionTabItem<TValue>[]
  activeValue: TValue
  onChange: (value: TValue) => void
  ariaLabel?: string
  className?: string
}

export function SectionTabs<TValue extends string = string>({
  items,
  activeValue,
  onChange,
  ariaLabel = 'Section tabs',
  className,
}: SectionTabsProps<TValue>) {
  if (items.length === 0) {
    return null
  }

  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      className={clsx(
        'inline-flex max-w-full gap-1 overflow-x-auto rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-muted)] p-1',
        className
      )}
    >
      {items.map((item) => {
        const isActive = item.value === activeValue

        return (
          <button
            key={item.value}
            role="tab"
            type="button"
            aria-selected={isActive}
            disabled={item.disabled}
            className={clsx(
              'h-8 whitespace-nowrap rounded-md px-3 text-sm font-medium transition-colors',
              isActive
                ? 'bg-[var(--color-bg-surface)] text-[var(--color-text-primary)] shadow-sm'
                : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)]',
              item.disabled && 'cursor-not-allowed opacity-50'
            )}
            onClick={() => onChange(item.value)}
          >
            {item.label}
          </button>
        )
      })}
    </div>
  )
}
