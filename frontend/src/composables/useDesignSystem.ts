/**
 * 设计系统组合式函数 - 提供统一的设计令牌访问和工具函数
 */

import { computed, ref, readonly, type ComputedRef } from 'vue'
import { 
  colors, 
  typography, 
  spacing, 
  borderRadius, 
  boxShadow, 
  animation, 
  breakpoints,
  components
} from '../styles/design-tokens'
import { useTheme, type Theme } from '../styles/theme'

// 设计系统接口
export interface DesignSystem {
  // 颜色系统
  colors: typeof colors
  
  // 字体系统
  typography: typeof typography
  
  // 间距系统
  spacing: typeof spacing
  
  // 圆角系统
  borderRadius: typeof borderRadius
  
  // 阴影系统
  boxShadow: typeof boxShadow
  
  // 动画系统
  animation: typeof animation
  
  // 断点系统
  breakpoints: typeof breakpoints
  
  // 组件令牌
  components: typeof components
  
  // 主题相关
  theme: ComputedRef<Theme>
  isDark: ComputedRef<boolean>
  
  // 工具函数
  utils: {
    // 间距工具
    spacing: (value: keyof typeof spacing) => string
    
    // 颜色工具
    color: (path: string, opacity?: number) => string
    
    // 字体工具
    fontSize: (size: keyof typeof typography.fontSize) => string
    fontWeight: (weight: keyof typeof typography.fontWeight) => number
    
    // 圆角工具
    rounded: (radius: keyof typeof borderRadius) => string
    
    // 阴影工具
    shadow: (shadow: keyof typeof boxShadow) => string
    
    // 断点工具
    breakpoint: (bp: keyof typeof breakpoints) => string
    
    // 媒体查询工具
    mediaQuery: (bp: keyof typeof breakpoints, type?: 'min' | 'max') => string
    
    // CSS变量工具
    cssVar: (name: string, fallback?: string) => string
    
    // 主题色工具
    themeColor: (path: string) => string
    
    // 动画工具
    transition: (properties: string[], duration?: keyof typeof animation.duration, easing?: keyof typeof animation.easing) => string
    
    // 响应式值工具
    responsive: <T>(values: Partial<Record<keyof typeof breakpoints, T>>) => T | undefined
  }
}

/**
 * 使用设计系统
 */
export function useDesignSystem(): DesignSystem {
  const { theme, isDark } = useTheme()
  
  // 工具函数实现
  const utils = {
    // 获取间距值
    spacing: (value: keyof typeof spacing): string => {
      return spacing[value]
    },
    
    // 获取颜色值（支持透明度）
    color: (path: string, opacity?: number): string => {
      const keys = path.split('.')
      let value: any = colors
      
      for (const key of keys) {
        value = value?.[key]
      }
      
      if (typeof value !== 'string') {
        console.warn(`Color path "${path}" not found`)
        return '#000000'
      }
      
      if (opacity !== undefined) {
        // 如果有透明度，转换为rgba格式
        const hex = value.replace('#', '')
        const r = parseInt(hex.substring(0, 2), 16)
        const g = parseInt(hex.substring(2, 4), 16)
        const b = parseInt(hex.substring(4, 6), 16)
        return `rgba(${r}, ${g}, ${b}, ${opacity})`
      }
      
      return value
    },
    
    // 获取字体大小
    fontSize: (size: keyof typeof typography.fontSize): string => {
      return typography.fontSize[size]
    },
    
    // 获取字体权重
    fontWeight: (weight: keyof typeof typography.fontWeight): number => {
      return typography.fontWeight[weight]
    },
    
    // 获取圆角值
    rounded: (radius: keyof typeof borderRadius): string => {
      return borderRadius[radius]
    },
    
    // 获取阴影值
    shadow: (shadow: keyof typeof boxShadow): string => {
      return boxShadow[shadow]
    },
    
    // 获取断点值
    breakpoint: (bp: keyof typeof breakpoints): string => {
      return breakpoints[bp]
    },
    
    // 生成媒体查询
    mediaQuery: (bp: keyof typeof breakpoints, type: 'min' | 'max' = 'min'): string => {
      const value = breakpoints[bp]
      return `@media (${type}-width: ${value})`
    },
    
    // 生成CSS变量引用
    cssVar: (name: string, fallback?: string): string => {
      return fallback ? `var(--${name}, ${fallback})` : `var(--${name})`
    },
    
    // 获取主题色值
    themeColor: (path: string): string => {
      const keys = path.split('.')
      let value: any = theme.value.colors
      
      for (const key of keys) {
        value = value?.[key]
      }
      
      return value || ''
    },
    
    // 生成过渡动画
    transition: (
      properties: string[], 
      duration: keyof typeof animation.duration = 'normal',
      easing: keyof typeof animation.easing = 'ease'
    ): string => {
      const durationValue = animation.duration[duration]
      const easingValue = animation.easing[easing]
      
      return properties.map(prop => `${prop} ${durationValue} ${easingValue}`).join(', ')
    },
    
    // 响应式值选择器
    responsive: <T>(values: Partial<Record<keyof typeof breakpoints, T>>): T | undefined => {
      // 这里简化实现，实际项目中可能需要更复杂的逻辑
      const breakpointOrder: (keyof typeof breakpoints)[] = ['xs', 'sm', 'md', 'lg', 'xl', '2xl']
      
      // 获取当前屏幕宽度对应的断点
      if (typeof window === 'undefined') {
        return values.md || values.sm || values.xs
      }
      
      const width = window.innerWidth
      let currentBreakpoint: keyof typeof breakpoints = 'xs'
      
      for (const bp of breakpointOrder) {
        const bpValue = parseInt(breakpoints[bp])
        if (width >= bpValue) {
          currentBreakpoint = bp
        }
      }
      
      // 返回当前断点或最接近的较小断点的值
      for (let i = breakpointOrder.indexOf(currentBreakpoint); i >= 0; i--) {
        const bp = breakpointOrder[i]
        if (values[bp] !== undefined) {
          return values[bp]
        }
      }
      
      return undefined
    }
  }
  
  return {
    colors,
    typography,
    spacing,
    borderRadius,
    boxShadow,
    animation,
    breakpoints,
    components,
    theme,
    isDark,
    utils
  }
}

/**
 * 颜色工具函数 - 单独导出常用功能
 */
export function useColors() {
  const { utils } = useDesignSystem()
  
  return {
    // 获取颜色
    get: utils.color,
    
    // 获取主题色
    theme: utils.themeColor,
    
    // 预设的语义化颜色
    semantic: {
      primary: computed(() => utils.themeColor('brand.primary')),
      success: computed(() => utils.themeColor('status.success')),
      warning: computed(() => utils.themeColor('status.warning')),
      error: computed(() => utils.themeColor('status.error')),
      info: computed(() => utils.themeColor('status.info')),
    },
    
    // 背景色
    background: {
      primary: computed(() => utils.themeColor('background.primary')),
      secondary: computed(() => utils.themeColor('background.secondary')),
      tertiary: computed(() => utils.themeColor('background.tertiary')),
      elevated: computed(() => utils.themeColor('background.elevated')),
    },
    
    // 文本色
    text: {
      primary: computed(() => utils.themeColor('text.primary')),
      secondary: computed(() => utils.themeColor('text.secondary')),
      tertiary: computed(() => utils.themeColor('text.tertiary')),
      disabled: computed(() => utils.themeColor('text.disabled')),
      inverse: computed(() => utils.themeColor('text.inverse')),
    },
    
    // 边框色
    border: {
      primary: computed(() => utils.themeColor('border.primary')),
      secondary: computed(() => utils.themeColor('border.secondary')),
      tertiary: computed(() => utils.themeColor('border.tertiary')),
    }
  }
}

/**
 * 间距工具函数
 */
export function useSpacing() {
  const { utils } = useDesignSystem()
  
  return {
    // 获取间距值
    get: utils.spacing,
    
    // 预设的间距值
    xs: spacing[1],
    sm: spacing[2],
    md: spacing[4],
    lg: spacing[6],
    xl: spacing[8],
    '2xl': spacing[12],
    
    // 生成间距类
    padding: (value: keyof typeof spacing) => ({
      padding: utils.spacing(value)
    }),
    
    margin: (value: keyof typeof spacing) => ({
      margin: utils.spacing(value)
    }),
    
    gap: (value: keyof typeof spacing) => ({
      gap: utils.spacing(value)
    })
  }
}

/**
 * 字体工具函数
 */
export function useTypography() {
  const { utils } = useDesignSystem()
  
  return {
    // 字体大小
    fontSize: utils.fontSize,
    
    // 字体权重
    fontWeight: utils.fontWeight,
    
    // 预设的文本样式
    styles: {
      h1: {
        fontSize: typography.fontSize['4xl'],
        fontWeight: typography.fontWeight.semibold,
        lineHeight: typography.lineHeight.tight
      },
      h2: {
        fontSize: typography.fontSize['3xl'],
        fontWeight: typography.fontWeight.semibold,
        lineHeight: typography.lineHeight.tight
      },
      h3: {
        fontSize: typography.fontSize['2xl'],
        fontWeight: typography.fontWeight.semibold,
        lineHeight: typography.lineHeight.tight
      },
      h4: {
        fontSize: typography.fontSize.xl,
        fontWeight: typography.fontWeight.semibold,
        lineHeight: typography.lineHeight.tight
      },
      body: {
        fontSize: typography.fontSize.base,
        fontWeight: typography.fontWeight.normal,
        lineHeight: typography.lineHeight.normal
      },
      small: {
        fontSize: typography.fontSize.sm,
        fontWeight: typography.fontWeight.normal,
        lineHeight: typography.lineHeight.normal
      },
      caption: {
        fontSize: typography.fontSize.xs,
        fontWeight: typography.fontWeight.normal,
        lineHeight: typography.lineHeight.normal
      }
    }
  }
}

/**
 * 响应式工具函数
 */
export function useResponsive() {
  const { utils } = useDesignSystem()
  
  // 响应式状态
  const currentBreakpoint = ref<keyof typeof breakpoints>('md')
  
  // 监听窗口大小变化
  if (typeof window !== 'undefined') {
    const updateBreakpoint = () => {
      const width = window.innerWidth
      const breakpointOrder: (keyof typeof breakpoints)[] = ['xs', 'sm', 'md', 'lg', 'xl', '2xl']
      
      for (let i = breakpointOrder.length - 1; i >= 0; i--) {
        const bp = breakpointOrder[i]
        const bpValue = parseInt(breakpoints[bp])
        if (width >= bpValue) {
          currentBreakpoint.value = bp
          break
        }
      }
    }
    
    updateBreakpoint()
    window.addEventListener('resize', updateBreakpoint)
  }
  
  return {
    // 当前断点
    current: readonly(currentBreakpoint),
    
    // 断点检查
    isXs: computed(() => currentBreakpoint.value === 'xs'),
    isSm: computed(() => currentBreakpoint.value === 'sm'),
    isMd: computed(() => currentBreakpoint.value === 'md'),
    isLg: computed(() => currentBreakpoint.value === 'lg'),
    isXl: computed(() => currentBreakpoint.value === 'xl'),
    is2Xl: computed(() => currentBreakpoint.value === '2xl'),
    
    // 屏幕大小检查
    isMobile: computed(() => ['xs', 'sm'].includes(currentBreakpoint.value)),
    isTablet: computed(() => currentBreakpoint.value === 'md'),
    isDesktop: computed(() => ['lg', 'xl', '2xl'].includes(currentBreakpoint.value)),
    
    // 响应式值选择
    value: utils.responsive,
    
    // 媒体查询
    mediaQuery: utils.mediaQuery
  }
}