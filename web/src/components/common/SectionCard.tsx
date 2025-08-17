/**
 * SectionCard.tsx
 * 统一的内容承载卡片容器：圆角 + 边框 + 阴影，并提供一致的内边距。
 * 外部用于包裹表格、图表或表单，确保全站视觉风格一致。
 */

import React from 'react'

/**
 * SectionCardProps
 * - children: 内容
 * - className: 自定义样式
 * - bodyClassName: 内层内容容器样式
 */
export interface SectionCardProps {
  /** 卡片内容 */
  children: React.ReactNode
  /** 外层容器样式 */
  className?: string
  /** 内层内容样式（默认 p-4 md:p-6） */
  bodyClassName?: string
}

/**
 * SectionCard
 * 标准卡片容器
 */
const SectionCard: React.FC<SectionCardProps> = ({ children, className, bodyClassName }) => {
  return (
    <section className={['rounded-xl border bg-card shadow-sm', className || ''].join(' ')}>
      <div className={['p-4 md:p-6', bodyClassName || ''].join(' ')}>{children}</div>
    </section>
  )
}

export default SectionCard
