/**
 * UnifiedCard - 统一的卡片组件系统
 * 基于shadcn/ui Card组件，提供一致的卡片样式
 */

import React, { ReactNode } from 'react'
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { cardVariants } from '@/lib/cardStyles'

// 基础卡片组件接口
interface BaseCardProps {
  children: ReactNode
  className?: string
  variant?: keyof typeof cardVariants
  onClick?: () => void
}

// 标准卡片组件
export const UnifiedCard: React.FC<BaseCardProps> = ({ 
  children, 
  className, 
  variant = 'standard',
  onClick 
}) => {
  return (
    <Card 
      className={cn(cardVariants[variant], className)}
      onClick={onClick}
      role={onClick ? 'button' : undefined}
      tabIndex={onClick ? 0 : undefined}
    >
      {children}
    </Card>
  )
}

// 带标题的卡片组件
interface TitledCardProps extends BaseCardProps {
  title: string
  description?: string
  headerClassName?: string
  contentClassName?: string
}

export const TitledCard: React.FC<TitledCardProps> = ({
  children,
  className,
  variant = 'standard',
  title,
  description,
  headerClassName,
  contentClassName,
  onClick
}) => {
  return (
    <UnifiedCard variant={variant} className={className} onClick={onClick}>
      <CardHeader className={cn('pb-3', headerClassName)}>
        <CardTitle className="text-lg font-semibold text-neutral-800">
          {title}
        </CardTitle>
        {description && (
          <CardDescription className="text-sm text-neutral-600">
            {description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent className={cn('pt-0', contentClassName)}>
        {children}
      </CardContent>
    </UnifiedCard>
  )
}

// 统计卡片组件
interface StatCardProps {
  icon?: ReactNode
  value: string
  label: string
  delta?: string
  deltaType?: 'positive' | 'negative' | 'neutral'
  color?: string
  className?: string
  onClick?: () => void
}

export const UnifiedStatCard: React.FC<StatCardProps> = ({
  icon,
  value,
  label,
  delta,
  deltaType = 'neutral',
  color,
  className,
  onClick
}) => {
  const deltaColors = {
    positive: 'text-emerald-600',
    negative: 'text-red-600', 
    neutral: 'text-neutral-500'
  }

  return (
    <UnifiedCard variant="stat" className={className} onClick={onClick}>
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          {icon && (
            <div
              className={cn(
                "flex h-10 w-10 items-center justify-center rounded-xl text-white",
                !color && "bg-violet-600"
              )}
              style={color ? { backgroundColor: color } : undefined}
            >
              {icon}
            </div>
          )}
          <div className="min-w-0 flex-1">
            <div className="text-sm text-neutral-600 mb-1">{label}</div>
            <div className="flex items-baseline gap-2">
              <div className="text-2xl font-bold text-neutral-900 truncate">
                {value}
              </div>
              {delta && (
                <div className={cn('text-xs font-medium', deltaColors[deltaType])}>
                  {delta}
                </div>
              )}
            </div>
          </div>
        </div>
      </CardContent>
    </UnifiedCard>
  )
}

// 加载状态卡片
interface LoadingCardProps {
  variant?: keyof typeof cardVariants
  className?: string
  lines?: number
}

export const LoadingCard: React.FC<LoadingCardProps> = ({
  variant = 'standard',
  className,
  lines = 3
}) => {
  return (
    <UnifiedCard variant={variant} className={cn('animate-pulse', className)}>
      <CardContent className="p-6">
        <div className="space-y-3">
          {Array.from({ length: lines }, (_, i) => (
            <div
              key={i}
              className={cn(
                'h-4 bg-neutral-200 rounded',
                i === 0 && 'w-3/4',
                i === 1 && 'w-full',
                i === 2 && 'w-2/3',
                i > 2 && 'w-5/6'
              )}
            />
          ))}
        </div>
      </CardContent>
    </UnifiedCard>
  )
}

// 空状态卡片
interface EmptyCardProps {
  icon?: ReactNode
  title: string
  description?: string
  action?: ReactNode
  className?: string
}

export const EmptyCard: React.FC<EmptyCardProps> = ({
  icon,
  title,
  description,
  action,
  className
}) => {
  return (
    <UnifiedCard variant="standard" className={cn('text-center', className)}>
      <CardContent className="p-12">
        {icon && (
          <div className="flex justify-center mb-4 text-neutral-400">
            {icon}
          </div>
        )}
        <h3 className="text-lg font-medium text-neutral-900 mb-2">
          {title}
        </h3>
        {description && (
          <p className="text-sm text-neutral-600 mb-4">
            {description}
          </p>
        )}
        {action && action}
      </CardContent>
    </UnifiedCard>
  )
}