/**
 * 时区状态管理
 *
 * 管理用户的时区信息，并提供时区相关的状态和方法
 */

import { useEffect } from 'react'
import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export interface TimezoneState {
  /** 当前用户时区 */
  timezone: string
  /** 时区偏移量（分钟） */
  offset: number
  /** 是否正在检测时区 */
  isDetecting: boolean
  /** 时区是否已初始化 */
  isInitialized: boolean
}

export interface TimezoneActions {
  /** 设置时区 */
  setTimezone: (timezone: string) => void
  /** 自动检测并设置时区 */
  detectTimezone: () => void
  /** 获取时区偏移量 */
  getOffset: () => number
  /** 重置时区为UTC */
  resetTimezone: () => void
}

export type TimezoneStore = TimezoneState & TimezoneActions

/**
 * 时区状态管理器
 */
export const useTimezoneStore = create<TimezoneStore>()(
  persist(
    (set, get) => ({
      // 初始状态
      timezone: 'UTC',
      offset: 0,
      isDetecting: false,
      isInitialized: false,

      // 设置时区
      setTimezone: (timezone: string) => {
        try {
          // 验证时区是否有效
          const now = new Date()
          const formatter = new Intl.DateTimeFormat('en-US', {
            timeZone: timezone,
            timeZoneName: 'short'
          })

          // 尝试格式化，如果时区无效会抛出错误
          formatter.format(now)

          // 计算时区偏移量
          const utcDate = new Date(now.getTime() + now.getTimezoneOffset() * 60000)
          const localDate = new Date(utcDate.toLocaleString('en-US', { timeZone: timezone }))
          const offset = Math.round((localDate.getTime() - utcDate.getTime()) / 60000)

          set({
            timezone,
            offset,
            isInitialized: true
          })
        } catch (error) {
          console.warn('Invalid timezone provided, falling back to UTC:', error)
          set({
            timezone: 'UTC',
            offset: 0,
            isInitialized: true
          })
        }
      },

      // 自动检测时区
      detectTimezone: () => {
        set({ isDetecting: true })

        try {
          // 使用 Intl API 获取用户时区
          const detectedTimezone = Intl.DateTimeFormat().resolvedOptions().timeZone

          if (detectedTimezone && detectedTimezone !== 'UTC') {
            get().setTimezone(detectedTimezone)
          } else {
            // 如果检测失败，设置为UTC
            set({
              timezone: 'UTC',
              offset: 0,
              isInitialized: true,
              isDetecting: false
            })
          }
        } catch (error) {
          console.warn('Failed to detect timezone:', error)
          set({
            timezone: 'UTC',
            offset: 0,
            isInitialized: true,
            isDetecting: false
          })
        } finally {
          set({ isDetecting: false })
        }
      },

      // 获取时区偏移量
      getOffset: () => {
        return get().offset
      },

      // 重置时区
      resetTimezone: () => {
        set({
          timezone: 'UTC',
          offset: 0,
          isInitialized: true
        })
      }
    }),
    {
      name: 'timezone-storage',
      // 只持久化必要的状态
      partialize: (state) => ({
        timezone: state.timezone,
        offset: state.offset,
        isInitialized: state.isInitialized
      })
    }
  )
)

/**
 * 时区相关的工具函数
 */
export const timezoneUtils = {
  /**
   * 检查给定时区是否有效
   */
  isValidTimezone(timezone: string): boolean {
    try {
      const formatter = new Intl.DateTimeFormat('en-US', {
        timeZone: timezone,
        timeZoneName: 'short'
      })
      formatter.format(new Date())
      return true
    } catch {
      return false
    }
  },

  /**
   * 获取时区的当前时间
   */
  getTimeInTimezone(timezone: string, date: Date = new Date()): string {
    try {
      return new Date(date.toLocaleString('en-US', { timeZone: timezone }))
        .toISOString()
    } catch {
      return date.toISOString()
    }
  },

  /**
   * 格式化时间为指定时区的本地时间
   */
  formatToLocalTime(date: string | Date, timezone: string): string {
    const dateObj = typeof date === 'string' ? new Date(date) : date
    try {
      return dateObj.toLocaleString('zh-CN', {
        timeZone: timezone,
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
      })
    } catch {
      return dateObj.toLocaleString('zh-CN')
    }
  },

  /**
   * 获取常见的时区列表
   */
  getCommonTimezones(): Array<{ value: string; label: string; offset: string }> {
    return [
      { value: 'UTC', label: 'UTC (Coordinated Universal Time)', offset: '+00:00' },
      { value: 'America/New_York', label: 'New York (Eastern Time)', offset: '-05:00' },
      { value: 'America/Los_Angeles', label: 'Los Angeles (Pacific Time)', offset: '-08:00' },
      { value: 'Europe/London', label: 'London (Greenwich Mean Time)', offset: '+00:00' },
      { value: 'Europe/Paris', label: 'Paris (Central European Time)', offset: '+01:00' },
      { value: 'Asia/Shanghai', label: 'Shanghai (China Standard Time)', offset: '+08:00' },
      { value: 'Asia/Tokyo', label: 'Tokyo (Japan Standard Time)', offset: '+09:00' },
      { value: 'Asia/Hong_Kong', label: 'Hong Kong (Hong Kong Time)', offset: '+08:00' },
      { value: 'Asia/Singapore', label: 'Singapore (Singapore Time)', offset: '+08:00' },
      { value: 'Australia/Sydney', label: 'Sydney (Australian Eastern Time)', offset: '+10:00' },
    ]
  }
}

/**
 * 初始化时区状态的 Hook
 */
export const useTimezoneInit = () => {
  const { isInitialized, detectTimezone } = useTimezoneStore()

  // 在组件挂载时自动检测时区
  useEffect(() => {
    if (!isInitialized) {
      detectTimezone()
    }
  }, [isInitialized, detectTimezone])

  return {
    isInitialized,
    detectTimezone
  }
}