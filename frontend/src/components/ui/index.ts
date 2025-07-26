/**
 * UI 组件库索引文件
 * 统一导出所有UI组件，便于在应用中使用
 */

// 基础UI组件
export { default as Card } from './Card.vue'
export { default as Button } from './Button.vue'
export { default as Badge } from './Badge.vue'
export { default as Tag } from './Tag.vue'
export { default as Loading } from './Loading.vue'
export { default as Skeleton } from './Skeleton.vue'
export { default as ThemeToggle } from './ThemeToggle.vue'
export { default as LanguageSelector } from './LanguageSelector.vue'
export { default as ErrorBoundary } from './ErrorBoundary.vue'
export { default as ErrorPage } from './ErrorPage.vue'

// 布局组件
export { default as PageContainer } from '../layout/PageContainer.vue'
export { default as Grid } from '../layout/Grid.vue'
export { default as GridItem } from '../layout/GridItem.vue'

// 组件类型定义
export interface BreadcrumbItem {
  title: string
  path?: string
}

export interface ButtonProps {
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
  variant?: 'filled' | 'outlined' | 'text' | 'ghost'
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  disabled?: boolean
  loading?: boolean
  block?: boolean
  circle?: boolean
  round?: boolean
}

export interface CardProps {
  title?: string
  subtitle?: string
  variant?: 'default' | 'outlined' | 'elevated' | 'filled'
  size?: 'sm' | 'md' | 'lg'
  hoverable?: boolean
  clickable?: boolean
  bordered?: boolean
  shadow?: boolean
  padding?: 'none' | 'sm' | 'md' | 'lg'
  loading?: boolean
  disabled?: boolean
}

export interface BadgeProps {
  count?: number
  max?: number
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
  size?: 'xs' | 'sm' | 'md' | 'lg'
  dot?: boolean
  bordered?: boolean
  hidden?: boolean
  color?: string
  textColor?: string
}

export interface TagProps {
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
  variant?: 'filled' | 'outlined' | 'light' | 'ghost'
  size?: 'xs' | 'sm' | 'md' | 'lg'
  closable?: boolean
  disabled?: boolean
  round?: boolean
  color?: string
  textColor?: string
  clickable?: boolean
}

export interface LoadingProps {
  visible?: boolean
  text?: string
  spinner?: 'default' | 'dots' | 'pulse' | 'bounce' | 'wave'
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  color?: string
  overlay?: boolean
  fullscreen?: boolean
  centered?: boolean
  background?: string
  delay?: number
}

export interface PageContainerProps {
  title?: string
  description?: string
  breadcrumb?: BreadcrumbItem[]
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
  fluid?: boolean
  padded?: boolean
  centered?: boolean
  background?: 'default' | 'secondary' | 'transparent'
  bordered?: boolean
  minHeight?: string
}

// 响应式类型
export type ResponsiveValue<T> = T | {
  xs?: T
  sm?: T
  md?: T
  lg?: T
  xl?: T
  '2xl'?: T
}

export interface GridProps {
  cols?: ResponsiveValue<number>
  gap?: ResponsiveValue<number | string>
  rowGap?: ResponsiveValue<number | string>
  colGap?: ResponsiveValue<number | string>
  autoFit?: boolean
  minItemWidth?: string
  justify?: 'start' | 'end' | 'center' | 'stretch' | 'space-around' | 'space-between' | 'space-evenly'
  align?: 'start' | 'end' | 'center' | 'stretch'
  dense?: boolean
}

export interface GridItemProps {
  span?: ResponsiveValue<number>
  offset?: ResponsiveValue<number>
  rowStart?: ResponsiveValue<number>
  rowEnd?: ResponsiveValue<number>
  colStart?: ResponsiveValue<number>
  colEnd?: ResponsiveValue<number>
  rowSpan?: ResponsiveValue<number>
  justify?: 'start' | 'end' | 'center' | 'stretch'
  align?: 'start' | 'end' | 'center' | 'stretch'
  area?: string
  order?: ResponsiveValue<number>
}

// 组件事件类型
export interface ButtonEvents {
  click: [event: MouseEvent]
}

export interface CardEvents {
  click: [event: MouseEvent]
}

export interface TagEvents {
  close: []
  click: [event: MouseEvent]
}

// 工具函数
export function createResponsiveValue<T>(
  value: T,
  overrides?: Partial<Record<'xs' | 'sm' | 'md' | 'lg' | 'xl' | '2xl', T>>
): ResponsiveValue<T> {
  if (!overrides) return value
  return { ...overrides, xs: value }
}

// 组件预设配置
export const componentPresets = {
  button: {
    primary: { type: 'primary', variant: 'filled' } as ButtonProps,
    secondary: { type: 'default', variant: 'outlined' } as ButtonProps,
    ghost: { type: 'default', variant: 'ghost' } as ButtonProps,
    danger: { type: 'danger', variant: 'filled' } as ButtonProps,
    text: { type: 'primary', variant: 'text' } as ButtonProps,
  },
  card: {
    default: { variant: 'default', shadow: true, bordered: true } as CardProps,
    elevated: { variant: 'elevated', shadow: false, bordered: false } as CardProps,
    outlined: { variant: 'outlined', shadow: false, bordered: true } as CardProps,
    interactive: { hoverable: true, clickable: true, shadow: true } as CardProps,
  },
  badge: {
    notification: { type: 'danger', size: 'sm' } as BadgeProps,
    status: { dot: true, size: 'md' } as BadgeProps,
    count: { type: 'primary', max: 99 } as BadgeProps,
  },
  tag: {
    default: { variant: 'light' } as TagProps,
    status: { variant: 'filled', size: 'sm' } as TagProps,
    interactive: { variant: 'outlined', clickable: true } as TagProps,
    removable: { variant: 'light', closable: true } as TagProps,
  },
  loading: {
    inline: { size: 'sm', centered: false } as LoadingProps,
    overlay: { overlay: true, centered: true } as LoadingProps,
    fullscreen: { fullscreen: true, overlay: true } as LoadingProps,
    minimal: { spinner: 'dots', size: 'md' } as LoadingProps,
  },
  grid: {
    responsive: {
      cols: { xs: 1, sm: 2, md: 3, lg: 4 },
      gap: 4
    } as GridProps,
    autoFit: {
      autoFit: true,
      minItemWidth: '250px',
      gap: 4
    } as GridProps,
    dense: {
      cols: 4,
      gap: 2,
      dense: true
    } as GridProps,
  }
} as const