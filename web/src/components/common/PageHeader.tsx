/**
 * PageHeader.tsx
 * 统一的页面头部：标题 + 可选操作区 + 描述文案。
 * 用于保证各业务页面在标题层级与间距上的一致性。
 */

import React from 'react'

/**
 * PageHeaderProps
 * - title: 标题文本（必填）
 * - description: 描述文本（可选）
 * - actions: 右侧操作区（可选）
 * - className: 自定义样式（可选）
 */
export interface PageHeaderProps {
  /** 页面主标题 */
  title: string
  /** 标题下方的描述文案 */
  description?: string
  /** 标题行右侧的操作区（按钮等） */
  actions?: React.ReactNode
  /** 容器额外样式 */
  className?: string
}

/**
 * PageHeader
 * 标准化页面头部：上方为标题行（左标题右操作），下方为描述。
 */
const PageHeader: React.FC<PageHeaderProps> = ({ title, description, actions, className }) => {
  return (
    <header className={['space-y-2', className || ''].join(' ')}>
      {/* 标题 + 右侧操作 */}
      <div className="flex items-center justify-between gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
      </div>

      {/* 描述 */}
      {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
    </header>
  )
}

export default PageHeader
