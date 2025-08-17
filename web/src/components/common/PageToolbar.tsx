/**
 * PageToolbar.tsx
 * 通用页面工具栏组件：用于统一“左主操作 + 右搜索/筛选”的布局、高度与间距。
 */

import React from 'react'

/**
 * PageToolbarProps
 * - left: 左侧主操作区（新增按钮、批量操作等）
 * - right: 右侧搜索/筛选区
 * - className: 自定义容器样式扩展
 */
export interface PageToolbarProps {
  /** 左侧主操作区 */
  left?: React.ReactNode
  /** 右侧搜索/筛选区 */
  right?: React.ReactNode
  /** 自定义样式 */
  className?: string
}

/**
 * PageToolbar
 * 统一工具栏结构，保持视觉与交互一致性。
 */
const PageToolbar: React.FC<PageToolbarProps> = ({ left, right, className }) => {
  return (
    <div
      className={[
        'flex flex-wrap items-center justify-between gap-2',
        className || '',
      ].join(' ')}
      aria-label="页面工具栏"
    >
      {/* 左侧主操作区 */}
      <div className="flex items-center gap-2">{left}</div>

      {/* 右侧搜索/筛选区 */}
      {right ? <div className="flex items-center gap-2">{right}</div> : <div />}
    </div>
  )
}

export default PageToolbar
