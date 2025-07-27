// UI组件统一导出

export { default as StatusIndicator } from './StatusIndicator.vue'
export { default as StatsCard } from './StatsCard.vue'
export { default as EmptyState } from './EmptyState.vue'
export { default as LoadingOverlay } from './LoadingOverlay.vue'

// 组件类型定义
export interface StatusIndicatorProps {
  status: 'active' | 'inactive' | 'warning' | 'error' | 'pending'
  text?: string
  size?: 'small' | 'medium' | 'large'
}

export interface StatsCardProps {
  value: number | string
  label: string
  icon?: any
  iconSize?: number
  trend?: number
  variant?: 'primary' | 'success' | 'warning' | 'danger' | 'info'
  format?: 'number' | 'percent' | 'currency' | 'duration'
  precision?: number
}

export interface EmptyStateProps {
  icon?: any
  title?: string
  description?: string
  size?: 'small' | 'large'
}

export interface LoadingOverlayProps {
  visible: boolean
  text?: string
  absolute?: boolean
  iconSize?: number
}