/**
 * KpiCard.tsx
 * 关键指标卡片：展示单个 KPI 的主值与同比/环比信息。
 */

import React from 'react'

/** KPI 卡片的属性定义 */
export interface KpiCardProps {
  /** 指标标题 */
  title: string
  /** 主数值（已格式化） */
  value: string
  /** 同比/环比变化（带符号百分比，如 "+8%" 或 "-3.2%"） */
  delta?: string
  /** 变化说明，如 "较昨日"、"vs 昨日" */
  hint?: string
  /** 底部补充说明，如 "成功/总数 98% (984/1004)" */
  subtext?: string
  /** 自定义右上角角标内容（可选） */
  badge?: React.ReactNode
}

/**
 * KpiCard
 * - 白色卡片、细边、轻阴影，延续现有风格
 * - delta 正向为绿色、负向为红色，自动判断
 */
const KpiCard: React.FC<KpiCardProps> = ({ title, value, delta, hint, subtext, badge }) => {
  /** 简单判断 delta 正负，空值不展示颜色 */
  const isPositive = typeof delta === 'string' ? delta.trim().startsWith('+') : undefined

  return (
    <div className="rounded-xl border border-neutral-200 bg-white p-4 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div className="text-sm text-neutral-500">{title}</div>
        {badge}
      </div>
      <div className="mt-1 flex items-baseline gap-2">
        <div className="text-2xl font-semibold text-neutral-900">{value}</div>
        {delta && (
          <div
            className={[
              'text-xs',
              isPositive === undefined
                ? 'text-neutral-500'
                : isPositive
                ? 'text-emerald-600'
                : 'text-rose-600',
            ].join(' ')}
            aria-label="变化百分比"
          >
            {delta} {hint ? hint : ''}
          </div>
        )}
      </div>
      {subtext && <div className="mt-1 text-xs text-neutral-400">{subtext}</div>}
    </div>
  )
}

export default KpiCard
