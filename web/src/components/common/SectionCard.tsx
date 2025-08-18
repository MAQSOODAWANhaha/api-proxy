/**
 * SectionCard.tsx
 * 统一的内容承载卡片容器：基于 shadcn/ui Card 组件，提供一致的内边距。
 * 外部用于包裹表格、图表或表单，确保全站视觉风格一致。
 */

import React from 'react'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

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
 * 基于 shadcn/ui Card 组件的标准卡片容器
 */
const SectionCard: React.FC<SectionCardProps> = ({ children, className, bodyClassName }) => {
  return (
    <Card className={cn('shadow-sm', className)}>
      <CardContent className={cn('p-4 md:p-6', bodyClassName)}>
        {children}
      </CardContent>
    </Card>
  )
}

export default SectionCard
