/**
 * EmptyState.tsx
 * 空数据占位组件：统一列表/卡片的空态展示（图标 + 文案）。
 */

import React from 'react'
import { cn } from '@/lib/utils' // 若不存在该工具，请忽略；className 合并不会出错

/**
 * EmptyStateProps
 * - title: 主文案
 * - icon: 可选图标
 * - className: 额外样式
 */
export interface EmptyStateProps {
  /** 主文案 */
  title: string
  /** 可选图标 */
  icon?: React.ReactNode
  /** 容器样式扩展 */
  className?: string
}

/**
 * EmptyState
 * 简洁的空数据提示，默认用于表格内或卡片内。
 */
const EmptyState: React.FC<EmptyStateProps> = ({ title, icon, className }) => {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center gap-2 text-muted-foreground',
        className || '',
      )}
      aria-label="空数据"
    >
      {icon ? <div className="text-neutral-400">{icon}</div> : null}
      <div className="text-sm">{title}</div>
    </div>
  )
}

export default EmptyState
