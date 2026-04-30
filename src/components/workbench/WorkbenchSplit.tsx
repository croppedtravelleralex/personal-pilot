import type { CSSProperties, ReactNode } from 'react'
import clsx from 'clsx'

interface WorkbenchSplitProps {
  main: ReactNode
  side: ReactNode
  sidePosition?: 'left' | 'right'
  sideWidth?: 'narrow' | 'default' | 'wide'
  stickySide?: boolean
  stickyTop?: number
  sideAriaLabel?: string
  className?: string
  mainClassName?: string
  sideClassName?: string
}

const rightSideWidthClassNames: Record<NonNullable<WorkbenchSplitProps['sideWidth']>, string> = {
  narrow: 'lg:grid-cols-[minmax(0,1fr)_280px]',
  default: 'lg:grid-cols-[minmax(0,1fr)_340px]',
  wide: 'lg:grid-cols-[minmax(0,1fr)_420px]',
}

const leftSideWidthClassNames: Record<NonNullable<WorkbenchSplitProps['sideWidth']>, string> = {
  narrow: 'lg:grid-cols-[280px_minmax(0,1fr)]',
  default: 'lg:grid-cols-[340px_minmax(0,1fr)]',
  wide: 'lg:grid-cols-[420px_minmax(0,1fr)]',
}

export function WorkbenchSplit({
  main,
  side,
  sidePosition = 'right',
  sideWidth = 'default',
  stickySide = true,
  stickyTop = 16,
  sideAriaLabel = 'Workbench side panel',
  className,
  mainClassName,
  sideClassName,
}: WorkbenchSplitProps) {
  const sideStyle: CSSProperties | undefined = stickySide
    ? {
        position: 'sticky',
        top: stickyTop,
        alignSelf: 'start',
        maxHeight: `calc(100vh - ${stickyTop * 2}px)`,
        overflowY: 'auto',
      }
    : undefined

  const mainNode = (
    <div className={clsx('min-w-0', mainClassName)}>
      {main}
    </div>
  )
  const sideNode = (
    <aside
      aria-label={sideAriaLabel}
      className={clsx('min-w-0', sideClassName)}
      style={sideStyle}
    >
      {side}
    </aside>
  )

  return (
    <div
      className={clsx(
        'grid gap-4',
        sidePosition === 'left' ? leftSideWidthClassNames[sideWidth] : rightSideWidthClassNames[sideWidth],
        className
      )}
    >
      {sidePosition === 'left' ? (
        <>
          {sideNode}
          {mainNode}
        </>
      ) : (
        <>
          {mainNode}
          {sideNode}
        </>
      )}
    </div>
  )
}
