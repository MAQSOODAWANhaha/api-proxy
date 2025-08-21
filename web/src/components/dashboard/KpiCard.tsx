/**
 * KpiCard.tsx
 * 关键指标卡片：展示单个 KPI 的主值与同比/环比信息。
 */

import React from 'react'
import { UnifiedStatCard } from '@/components/common/UnifiedCard'

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
 * - 使用统一的UnifiedStatCard组件
 * - delta 正向为绿色、负向为红色，自动判断
 */
const KpiCard: React.FC<KpiCardProps> = ({ title, value, delta, hint, subtext, badge }) => {
  /** 简单判断 delta 正负，空值不展示颜色 */
  const isPositive = typeof delta === 'string' ? delta.trim().startsWith('+') : undefined
  
  // 确定 deltaType
  const deltaType = isPositive === undefined 
    ? 'neutral' 
    : isPositive 
      ? 'positive' 
      : 'negative'
  
  // 组合 delta 显示文本
  const deltaText = delta && hint ? `${delta} ${hint}` : delta

  return (
    <div className="relative">
      <UnifiedStatCard
        label={title}
        value={value}
        delta={deltaText}
        deltaType={deltaType}
      />
      
      {/* 右上角角标 */}
      {badge && (
        <div className="absolute top-4 right-4">
          {badge}
        </div>
      )}
      
      {/* 底部补充说明 */}
      {subtext && (
        <div className="mt-2 text-xs text-neutral-400 text-center">
          {subtext}
        </div>
      )}
    </div>
  )
}

export default KpiCard
