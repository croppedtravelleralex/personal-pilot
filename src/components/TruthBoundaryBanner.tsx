import { AlertTriangle, Info } from 'lucide-react'
import clsx from 'clsx'

type TruthBoundaryTone = 'info' | 'warning'

interface TruthBoundaryBannerProps {
  title?: string
  detail: string
  tone?: TruthBoundaryTone
  compact?: boolean
  className?: string
}

const toneClassNames: Record<TruthBoundaryTone, string> = {
  info: 'border-sky-200 bg-sky-50 text-sky-900',
  warning: 'border-amber-200 bg-amber-50 text-amber-950',
}

export function TruthBoundaryBanner({
  title = 'Truth boundary',
  detail,
  tone = 'info',
  compact = false,
  className,
}: TruthBoundaryBannerProps) {
  const Icon = tone === 'warning' ? AlertTriangle : Info

  return (
    <div
      className={clsx(
        'flex items-start gap-3 rounded-lg border',
        compact ? 'px-3 py-2' : 'px-4 py-3',
        toneClassNames[tone],
        className
      )}
    >
      <Icon className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
      <div className="min-w-0">
        <div className="text-sm font-semibold">{title}</div>
        <div className="mt-1 text-sm leading-5 opacity-90">{detail}</div>
      </div>
    </div>
  )
}
