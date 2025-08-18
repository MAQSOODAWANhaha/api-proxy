/**
 * StatCard - 统一的统计卡片组件，基于 shadcn/ui Card 组件
 */

import { ReactNode } from 'react'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

interface StatCardProps {
  icon: ReactNode
  value: string
  label: string
  delta?: string
  color: string
  className?: string
}

export function StatCard({ icon, value, label, delta, color, className }: StatCardProps) {
  return (
    <Card className={cn(
      'group relative overflow-hidden border-neutral-200 shadow-sm transition-shadow hover:shadow-md',
      className
    )}>
      {/* 顶部色条 */}
      <div className="absolute inset-x-0 top-0 h-1" style={{ backgroundColor: color }} />
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          <div
            className="flex h-10 w-10 items-center justify-center rounded-xl text-white"
            style={{ backgroundColor: color }}
            aria-hidden
          >
            {icon}
          </div>
          <div className="min-w-0">
            <div className="text-sm text-muted-foreground">{label}</div>
            <div className="flex items-baseline gap-2">
              <div className="truncate text-xl font-semibold text-foreground">{value}</div>
              {delta && (
                <div className="text-xs text-emerald-600">{delta}</div>
              )}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}