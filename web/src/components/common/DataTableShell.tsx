import * as React from 'react'

import { cn } from '@/lib/utils'

export interface DataTableShellProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode
}

const DataTableShell = React.forwardRef<HTMLDivElement, DataTableShellProps>(
  ({ className, children, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        'overflow-hidden rounded-2xl border border-slate-200 bg-white',
        className
      )}
      {...props}
    >
      {children}
    </div>
  )
)

DataTableShell.displayName = 'DataTableShell'

export default DataTableShell
