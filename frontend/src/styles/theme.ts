/**
 * 主题系统 - 统一的主题配置和切换功能
 */

import { reactive, ref, computed, readonly } from 'vue'
import { colors, typography, spacing, borderRadius, boxShadow } from './design-tokens'

// 主题类型定义
export type ThemeMode = 'light' | 'dark' | 'auto'

export interface Theme {
  mode: ThemeMode
  colors: {
    // 背景色
    background: {
      primary: string
      secondary: string
      tertiary: string
      elevated: string
    }
    
    // 文本色
    text: {
      primary: string
      secondary: string
      tertiary: string
      disabled: string
      inverse: string
    }
    
    // 边框色
    border: {
      primary: string
      secondary: string
      tertiary: string
    }
    
    // 品牌色
    brand: {
      primary: string
      secondary: string
      accent: string
    }
    
    // 状态色
    status: {
      success: string
      warning: string
      error: string
      info: string
    }
    
    // 交互色
    interactive: {
      hover: string
      pressed: string
      focus: string
      disabled: string
    }
  }
}

// 浅色主题
export const lightTheme: Theme = {
  mode: 'light',
  colors: {
    background: {
      primary: colors.white,
      secondary: colors.neutral[50],
      tertiary: colors.neutral[100],
      elevated: colors.white,
    },
    
    text: {
      primary: colors.neutral[900],
      secondary: colors.neutral[700],
      tertiary: colors.neutral[500],
      disabled: colors.neutral[400],
      inverse: colors.white,
    },
    
    border: {
      primary: colors.border.light,
      secondary: colors.border.default,
      tertiary: colors.border.dark,
    },
    
    brand: {
      primary: colors.primary[500],
      secondary: colors.primary[600],
      accent: colors.primary[400],
    },
    
    status: {
      success: colors.success[500],
      warning: colors.warning[500],
      error: colors.error[500],
      info: colors.info[500],
    },
    
    interactive: {
      hover: colors.neutral[100],
      pressed: colors.neutral[200],
      focus: colors.primary[100],
      disabled: colors.neutral[200],
    },
  },
}

// 深色主题
export const darkTheme: Theme = {
  mode: 'dark',
  colors: {
    background: {
      primary: colors.neutral[900],
      secondary: colors.neutral[800],
      tertiary: colors.neutral[700],
      elevated: colors.neutral[800],
    },
    
    text: {
      primary: colors.neutral[100],
      secondary: colors.neutral[300],
      tertiary: colors.neutral[400],
      disabled: colors.neutral[600],
      inverse: colors.neutral[900],
    },
    
    border: {
      primary: colors.neutral[700],
      secondary: colors.neutral[600],
      tertiary: colors.neutral[500],
    },
    
    brand: {
      primary: colors.primary[400],
      secondary: colors.primary[500],
      accent: colors.primary[300],
    },
    
    status: {
      success: colors.success[400],
      warning: colors.warning[400],
      error: colors.error[400],
      info: colors.info[400],
    },
    
    interactive: {
      hover: colors.neutral[700],
      pressed: colors.neutral[600],
      focus: colors.primary[800],
      disabled: colors.neutral[700],
    },
  },
}

// 主题状态管理
export const themeState = reactive({
  currentMode: 'light' as ThemeMode,
  systemPreference: 'light' as 'light' | 'dark',
})

// 当前主题计算属性
export const currentTheme = computed<Theme>(() => {
  const mode = themeState.currentMode === 'auto' 
    ? themeState.systemPreference 
    : themeState.currentMode
    
  return mode === 'dark' ? darkTheme : lightTheme
})

// 主题管理类
export class ThemeManager {
  private static instance: ThemeManager
  private storageKey = 'app-theme-mode'
  
  static getInstance(): ThemeManager {
    if (!ThemeManager.instance) {
      ThemeManager.instance = new ThemeManager()
    }
    return ThemeManager.instance
  }
  
  constructor() {
    this.initializeTheme()
    this.setupSystemThemeListener()
  }
  
  // 初始化主题
  private initializeTheme() {
    // 从本地存储读取主题设置
    const savedTheme = localStorage.getItem(this.storageKey) as ThemeMode
    if (savedTheme && ['light', 'dark', 'auto'].includes(savedTheme)) {
      themeState.currentMode = savedTheme
    }
    
    // 检测系统主题偏好
    this.updateSystemPreference()
    
    // 应用主题到DOM
    this.applyThemeToDOM()
  }
  
  // 监听系统主题变化
  private setupSystemThemeListener() {
    if (typeof window !== 'undefined' && window.matchMedia) {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      
      mediaQuery.addEventListener('change', (e) => {
        themeState.systemPreference = e.matches ? 'dark' : 'light'
        this.applyThemeToDOM()
      })
    }
  }
  
  // 更新系统偏好
  private updateSystemPreference() {
    if (typeof window !== 'undefined' && window.matchMedia) {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      themeState.systemPreference = mediaQuery.matches ? 'dark' : 'light'
    }
  }
  
  // 设置主题模式
  setThemeMode(mode: ThemeMode) {
    themeState.currentMode = mode
    localStorage.setItem(this.storageKey, mode)
    this.applyThemeToDOM()
  }
  
  // 应用主题到DOM
  private applyThemeToDOM() {
    if (typeof document === 'undefined') return
    
    const theme = currentTheme.value
    const root = document.documentElement
    
    // 设置CSS自定义属性
    root.style.setProperty('--color-bg-primary', theme.colors.background.primary)
    root.style.setProperty('--color-bg-secondary', theme.colors.background.secondary)
    root.style.setProperty('--color-bg-tertiary', theme.colors.background.tertiary)
    root.style.setProperty('--color-bg-elevated', theme.colors.background.elevated)
    
    root.style.setProperty('--color-text-primary', theme.colors.text.primary)
    root.style.setProperty('--color-text-secondary', theme.colors.text.secondary)
    root.style.setProperty('--color-text-tertiary', theme.colors.text.tertiary)
    root.style.setProperty('--color-text-disabled', theme.colors.text.disabled)
    root.style.setProperty('--color-text-inverse', theme.colors.text.inverse)
    
    root.style.setProperty('--color-border-primary', theme.colors.border.primary)
    root.style.setProperty('--color-border-secondary', theme.colors.border.secondary)
    root.style.setProperty('--color-border-tertiary', theme.colors.border.tertiary)
    
    root.style.setProperty('--color-brand-primary', theme.colors.brand.primary)
    root.style.setProperty('--color-brand-secondary', theme.colors.brand.secondary)
    root.style.setProperty('--color-brand-accent', theme.colors.brand.accent)
    
    root.style.setProperty('--color-success', theme.colors.status.success)
    root.style.setProperty('--color-warning', theme.colors.status.warning)
    root.style.setProperty('--color-error', theme.colors.status.error)
    root.style.setProperty('--color-info', theme.colors.status.info)
    
    root.style.setProperty('--color-interactive-hover', theme.colors.interactive.hover)
    root.style.setProperty('--color-interactive-pressed', theme.colors.interactive.pressed)
    root.style.setProperty('--color-interactive-focus', theme.colors.interactive.focus)
    root.style.setProperty('--color-interactive-disabled', theme.colors.interactive.disabled)
    
    // 设置主题类名
    root.classList.remove('theme-light', 'theme-dark')
    root.classList.add(`theme-${theme.mode}`)
    
    // 设置Element Plus主题
    this.applyElementPlusTheme(theme)
  }
  
  // 应用Element Plus主题
  private applyElementPlusTheme(theme: Theme) {
    const root = document.documentElement
    
    // Element Plus 主要颜色变量
    root.style.setProperty('--el-color-primary', theme.colors.brand.primary)
    root.style.setProperty('--el-color-success', theme.colors.status.success)
    root.style.setProperty('--el-color-warning', theme.colors.status.warning)
    root.style.setProperty('--el-color-danger', theme.colors.status.error)
    root.style.setProperty('--el-color-info', theme.colors.status.info)
    
    // Element Plus 背景色
    root.style.setProperty('--el-bg-color', theme.colors.background.primary)
    root.style.setProperty('--el-bg-color-page', theme.colors.background.secondary)
    root.style.setProperty('--el-bg-color-overlay', theme.colors.background.elevated)
    
    // Element Plus 文本色
    root.style.setProperty('--el-text-color-primary', theme.colors.text.primary)
    root.style.setProperty('--el-text-color-regular', theme.colors.text.secondary)
    root.style.setProperty('--el-text-color-secondary', theme.colors.text.tertiary)
    root.style.setProperty('--el-text-color-placeholder', theme.colors.text.disabled)
    root.style.setProperty('--el-text-color-disabled', theme.colors.text.disabled)
    
    // Element Plus 边框色
    root.style.setProperty('--el-border-color', theme.colors.border.primary)
    root.style.setProperty('--el-border-color-light', theme.colors.border.secondary)
    root.style.setProperty('--el-border-color-lighter', theme.colors.border.tertiary)
  }
  
  // 切换主题
  toggleTheme() {
    const currentMode = themeState.currentMode
    const newMode: ThemeMode = currentMode === 'light' ? 'dark' : 'light'
    this.setThemeMode(newMode)
  }
  
  // 获取当前主题
  getCurrentTheme(): Theme {
    return currentTheme.value
  }
  
  // 获取当前模式
  getCurrentMode(): ThemeMode {
    return themeState.currentMode
  }
  
  // 判断是否为深色主题
  isDarkMode(): boolean {
    return currentTheme.value.mode === 'dark'
  }
}

// 导出单例实例
export const themeManager = ThemeManager.getInstance()

// 导出组合式函数
export function useTheme() {
  const theme = currentTheme
  const mode = computed(() => themeState.currentMode)
  const isDark = computed(() => theme.value.mode === 'dark')
  
  const setTheme = (newMode: ThemeMode) => {
    themeManager.setThemeMode(newMode)
  }
  
  const toggleTheme = () => {
    themeManager.toggleTheme()
  }
  
  return {
    theme: readonly(theme),
    mode: readonly(mode),
    isDark: readonly(isDark),
    setTheme,
    toggleTheme,
  }
}

// 辅助函数：生成CSS变量
export function cssVar(name: string, fallback?: string): string {
  return fallback ? `var(--${name}, ${fallback})` : `var(--${name})`
}

// 辅助函数：获取主题色值
export function getThemeColor(path: string): string {
  const theme = currentTheme.value
  const keys = path.split('.')
  let value: any = theme.colors
  
  for (const key of keys) {
    value = value?.[key]
  }
  
  return value || ''
}