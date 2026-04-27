import { Star } from 'lucide-react'

interface MainWindowBadgeProps {
  isMain: boolean
}

export function MainWindowBadge({ isMain }: MainWindowBadgeProps) {
  if (!isMain) return null
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-amber-100 px-1.5 py-0.5 text-[10px] font-medium text-amber-700">
      <Star className="w-2.5 h-2.5" /> 主窗口
    </span>
  )
}
