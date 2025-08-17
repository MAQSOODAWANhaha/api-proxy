/**
 * StatCard - 统一的统计卡片组件，保持与仪表盘设计一致
 */

import { ReactNode } from 'react'

interface StatCardProps {
  icon: ReactNode
  value: string
  label: string
  delta?: string
  color: string
}

export function StatCard({ icon, value, label, delta, color }: StatCardProps) {
  return (
    <div className="group relative overflow-hidden rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm transition hover:shadow-md">
      {/* 顶部色条 */}
      <div className="absolute inset-x-0 top-0 h-1" style={{ backgroundColor: color }} />
      <div className="flex items-center gap-3">
        <div
          className="flex h-10 w-10 items-center justify-center rounded-xl text-white"
          style={{ backgroundColor: color }}
          aria-hidden
        >
          {icon}
        </div>
        <div className="min-w-0">
          <div className="text-sm text-neutral-500">{label}</div>
          <div className="flex items-baseline gap-2">
            <div className="truncate text-xl font-semibold text-neutral-900">{value}</div>
            {delta && (
              <div className="text-xs text-emerald-600">{delta}</div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}