import { useEffect, useId, useRef, useState } from 'react'
import { Search, X } from 'lucide-react'
import clsx from 'clsx'

interface SearchInputProps {
  value: string
  onChange?: (value: string) => void
  onDebouncedChange?: (value: string) => void
  label?: string
  placeholder?: string
  debounceMs?: number
  clearable?: boolean
  className?: string
}

export function SearchInput({
  value,
  onChange,
  onDebouncedChange,
  label = 'Search',
  placeholder = 'Search',
  debounceMs = 300,
  clearable = true,
  className,
}: SearchInputProps) {
  const inputId = useId()
  const [draftValue, setDraftValue] = useState(value)
  const didMountRef = useRef(false)

  useEffect(() => {
    setDraftValue(value)
  }, [value])

  useEffect(() => {
    if (!onDebouncedChange) {
      return undefined
    }

    if (!didMountRef.current) {
      didMountRef.current = true
      return undefined
    }

    const timer = window.setTimeout(() => onDebouncedChange(draftValue), debounceMs)
    return () => window.clearTimeout(timer)
  }, [debounceMs, draftValue, onDebouncedChange])

  function setNextValue(nextValue: string) {
    setDraftValue(nextValue)
    onChange?.(nextValue)
  }

  return (
    <label className={clsx('block min-w-0', className)} htmlFor={inputId}>
      <span className="mb-1 block text-xs font-medium text-[var(--color-text-muted)]">{label}</span>
      <span className="flex h-9 items-center gap-2 rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-surface)] px-3 focus-within:border-[var(--color-border-strong)] focus-within:ring-1 focus-within:ring-[var(--color-border-strong)]">
        <Search className="h-4 w-4 shrink-0 text-[var(--color-text-muted)]" aria-hidden="true" />
        <input
          id={inputId}
          className="min-w-0 flex-1 bg-transparent text-sm text-[var(--color-text-primary)] outline-none placeholder:text-[var(--color-text-muted)]"
          type="search"
          value={draftValue}
          placeholder={placeholder}
          onChange={(event) => setNextValue(event.target.value)}
        />
        {clearable && draftValue ? (
          <button
            type="button"
            className="rounded p-0.5 text-[var(--color-text-muted)] hover:bg-[var(--color-bg-muted)] hover:text-[var(--color-text-primary)]"
            aria-label="Clear search"
            onClick={() => setNextValue('')}
          >
            <X className="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        ) : null}
      </span>
    </label>
  )
}
