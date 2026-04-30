import { useEffect, useMemo, useState } from 'react'
import { Check, Clipboard, Copy, Minimize2 } from 'lucide-react'
import clsx from 'clsx'

const DEFAULT_COLLAPSE_AT = 220
const DEFAULT_INLINE_LIMIT = 12000

export function truncateInlineContent(value: string | null | undefined, maxChars = DEFAULT_COLLAPSE_AT) {
  const normalized = normalizeInlineContent(value)
  if (normalized.length <= maxChars) {
    return normalized
  }

  return `${normalized.slice(0, Math.max(0, maxChars - 1)).trimEnd()}...`
}

function normalizeInlineContent(value: string | null | undefined) {
  return typeof value === 'string' ? value.replace(/\r\n/g, '\n') : ''
}

type CopyState = 'idle' | 'success' | 'error'

interface InlineContentPreviewProps {
  value: string | null | undefined
  emptyText?: string
  collapseAt?: number
  inlineLimit?: number
  expandable?: boolean
  copyable?: boolean
  mono?: boolean
  muted?: boolean
  className?: string
  bodyClassName?: string
}

export function InlineContentPreview({
  value,
  emptyText = 'Empty',
  collapseAt = DEFAULT_COLLAPSE_AT,
  inlineLimit = DEFAULT_INLINE_LIMIT,
  expandable = true,
  copyable = true,
  mono = false,
  muted = false,
  className,
  bodyClassName,
}: InlineContentPreviewProps) {
  const normalized = normalizeInlineContent(value)
  const hasContent = normalized.trim().length > 0
  const previewLimit = Math.max(24, Math.min(collapseAt, inlineLimit))
  const [expanded, setExpanded] = useState(false)
  const [copyState, setCopyState] = useState<CopyState>('idle')

  useEffect(() => {
    setExpanded(false)
    setCopyState('idle')
  }, [normalized, previewLimit, inlineLimit])

  useEffect(() => {
    if (copyState === 'idle') {
      return undefined
    }

    const timer = window.setTimeout(() => setCopyState('idle'), 1600)
    return () => window.clearTimeout(timer)
  }, [copyState])

  const canExpand = hasContent && expandable && normalized.length > previewLimit
  const needsInlineCap = hasContent && normalized.length > inlineLimit

  const displayValue = useMemo(() => {
    if (!hasContent) {
      return emptyText
    }

    if (!expanded) {
      return truncateInlineContent(normalized, previewLimit)
    }

    if (needsInlineCap) {
      return truncateInlineContent(normalized, inlineLimit)
    }

    return normalized
  }, [emptyText, expanded, hasContent, inlineLimit, needsInlineCap, normalized, previewLimit])

  async function handleCopy() {
    if (!hasContent || !copyable || !navigator.clipboard) {
      return
    }

    try {
      await navigator.clipboard.writeText(normalized)
      setCopyState('success')
    } catch {
      setCopyState('error')
    }
  }

  return (
    <div className={clsx('space-y-2', className)}>
      <div
        className={clsx(
          'whitespace-pre-wrap break-words rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-surface)] px-3 py-2 text-sm leading-6',
          mono && 'font-mono text-xs',
          muted ? 'text-[var(--color-text-muted)]' : 'text-[var(--color-text-secondary)]',
          bodyClassName
        )}
      >
        {displayValue}
      </div>
      {hasContent && (canExpand || copyable) ? (
        <div className="flex flex-wrap items-center gap-2">
          {canExpand ? (
            <button
              type="button"
              className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-[var(--color-border-default)] px-2 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-muted)]"
              onClick={() => setExpanded((current) => !current)}
            >
              {expanded ? <Minimize2 className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
              {expanded ? 'Collapse' : 'Expand'}
            </button>
          ) : null}
          {copyable ? (
            <button
              type="button"
              className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-[var(--color-border-default)] px-2 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-muted)]"
              onClick={() => void handleCopy()}
            >
              {copyState === 'success' ? <Check className="h-3.5 w-3.5" /> : <Clipboard className="h-3.5 w-3.5" />}
              {copyState === 'success' ? 'Copied' : copyState === 'error' ? 'Copy failed' : 'Copy'}
            </button>
          ) : null}
          {expanded && needsInlineCap ? (
            <span className="text-xs text-[var(--color-text-muted)]">
              Inline rendering is capped at {inlineLimit.toLocaleString()} characters.
            </span>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}
