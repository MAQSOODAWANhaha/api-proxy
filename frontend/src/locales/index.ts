/**
 * 国际化配置
 */

import { createI18n } from 'vue-i18n'
import zh from './zh'
import en from './en'

// 支持的语言列表
export const supportedLocales = [
  {
    code: 'zh',
    name: '中文',
    flag: '🇨🇳'
  },
  {
    code: 'en',
    name: 'English',
    flag: '🇺🇸'
  }
] as const

export type SupportedLocale = typeof supportedLocales[number]['code']

// 默认语言
export const defaultLocale: SupportedLocale = 'zh'

// 获取浏览器语言
export function getBrowserLocale(): SupportedLocale {
  const browserLang = navigator.language.split('-')[0]
  return supportedLocales.find(locale => locale.code === browserLang)?.code ?? defaultLocale
}

// 获取存储的语言设置
export function getStoredLocale(): SupportedLocale {
  const stored = localStorage.getItem('locale')
  return supportedLocales.find(locale => locale.code === stored)?.code ?? getBrowserLocale()
}

// 保存语言设置
export function setStoredLocale(locale: SupportedLocale): void {
  localStorage.setItem('locale', locale)
}

// 获取语言显示名称
export function getLocaleName(code: SupportedLocale): string {
  return supportedLocales.find(locale => locale.code === code)?.name ?? code
}

// 获取语言旗帜图标
export function getLocaleFlag(code: SupportedLocale): string {
  return supportedLocales.find(locale => locale.code === code)?.flag ?? '🌐'
}

// 创建 i18n 实例
export const i18n = createI18n({
  legacy: false,
  locale: getStoredLocale(),
  fallbackLocale: defaultLocale,
  globalInjection: true,
  messages: {
    zh,
    en
  }
})

// 切换语言
export function switchLocale(locale: SupportedLocale): void {
  if (!supportedLocales.find(l => l.code === locale)) {
    console.warn(`Unsupported locale: ${locale}`)
    return
  }
  
  i18n.global.locale.value = locale
  setStoredLocale(locale)
  
  // 更新 HTML lang 属性
  document.documentElement.lang = locale
  
  // 更新页面标题（如果需要）
  updatePageTitle()
}

// 更新页面标题
function updatePageTitle(): void {
  const { t } = i18n.global
  
  // 根据当前路由更新标题
  const routeTitle = getRouteTitleKey()
  if (routeTitle) {
    document.title = `${t(routeTitle)} - AI服务代理管理平台`
  }
}

// 获取路由标题键
function getRouteTitleKey(): string | null {
  const path = window.location.pathname
  
  const routeTitleMap: Record<string, string> = {
    '/': 'nav.dashboard',
    '/dashboard': 'nav.dashboard',
    '/api-keys/provider': 'nav.providerKeys',
    '/api-keys/service': 'nav.serviceKeys',
    '/statistics/requests': 'nav.requestLogs',
    '/statistics/daily': 'nav.dailyStats',
    '/health': 'nav.health',
    '/user-center': 'nav.userCenter',
    '/settings': 'nav.settings'
  }
  
  return routeTitleMap[path] || null
}

// 组合式函数：使用国际化
export function useI18n() {
  return {
    ...i18n.global,
    supportedLocales,
    switchLocale,
    getLocaleName,
    getLocaleFlag,
    currentLocale: i18n.global.locale
  }
}

// 类型定义
export type I18nKey = keyof typeof zh
export type MessageKey<T = any> = T extends Record<string, any>
  ? {
      [K in keyof T]: T[K] extends Record<string, any>
        ? `${K & string}.${MessageKey<T[K]> & string}`
        : K & string
    }[keyof T]
  : never

export type AllMessageKeys = MessageKey<typeof zh>

// 类型安全的翻译函数
export function $t(key: AllMessageKeys, values?: Record<string, any>): string {
  return i18n.global.t(key, values)
}

// 导出默认实例
export default i18n