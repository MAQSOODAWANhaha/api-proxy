/**
 * 时区工具函数
 *
 * 提供时区转换和格式化相关的工具函数
 */

import dayjs from 'dayjs'
import utc from 'dayjs/plugin/utc'
import timezone from 'dayjs/plugin/timezone'

// 扩展 dayjs 插件
dayjs.extend(utc)
dayjs.extend(timezone)

/**
 * 将 UTC 时间转换为用户本地时间
 * @param utcTime UTC 时间字符串 (ISO 8601 格式)
 * @param userTimezone 用户时区 (如: 'Asia/Shanghai')
 * @returns 格式化的本地时间字符串
 */
export const formatUTCtoLocal = (utcTime: string, userTimezone: string): string => {
  try {
    return dayjs.utc(utcTime).tz(userTimezone).format('YYYY-MM-DD HH:mm:ss')
  } catch (error) {
    console.warn('Failed to format UTC time to local time:', error)
    return dayjs.utc(utcTime).format('YYYY-MM-DD HH:mm:ss')
  }
}

/**
 * 将本地时间转换为 ISO 8601 格式字符串（不带时区信息）
 * @param localTime 本地时间字符串
 * @returns ISO 格式的时间字符串
 */
export const formatLocalToISOString = (localTime: string): string => {
  try {
    return dayjs(localTime).format('YYYY-MM-DD HH:mm:ss')
  } catch (error) {
    console.warn('Failed to format local time to ISO string:', error)
    return localTime
  }
}

/**
 * 将本地时间字符串转换为 Date 对象
 * @param localTime 本地时间字符串
 * @returns Date 对象
 */
export const parseLocalTimeToDate = (localTime: string): Date => {
  try {
    return dayjs(localTime).toDate()
  } catch (error) {
    console.warn('Failed to parse local time:', error)
    return new Date()
  }
}

/**
 * 获取时间的相对显示（如: "2小时前"）
 * @param utcTime UTC 时间字符串
 * @param userTimezone 用户时区
 * @returns 相对时间字符串
 */
export const formatRelativeTime = (utcTime: string, userTimezone: string): string => {
  try {
    const now = dayjs().tz(userTimezone)
    const targetTime = dayjs.utc(utcTime).tz(userTimezone)
    const diffMinutes = now.diff(targetTime, 'minute')

    if (diffMinutes < 1) {
      return '刚刚'
    } else if (diffMinutes < 60) {
      return `${diffMinutes}分钟前`
    } else if (diffMinutes < 1440) {
      const diffHours = Math.floor(diffMinutes / 60)
      return `${diffHours}小时前`
    } else if (diffMinutes < 10080) {
      const diffDays = Math.floor(diffMinutes / 1440)
      return `${diffDays}天前`
    } else {
      return formatUTCtoLocal(utcTime, userTimezone)
    }
  } catch (error) {
    console.warn('Failed to format relative time:', error)
    return formatUTCtoLocal(utcTime, userTimezone)
  }
}

/**
 * 格式化日期（不包含时间）
 * @param utcTime UTC 时间字符串
 * @param userTimezone 用户时区
 * @returns 格式化的日期字符串
 */
export const formatDate = (utcTime: string, userTimezone: string): string => {
  try {
    return dayjs.utc(utcTime).tz(userTimezone).format('YYYY-MM-DD')
  } catch (error) {
    console.warn('Failed to format date:', error)
    return dayjs.utc(utcTime).format('YYYY-MM-DD')
  }
}

/**
 * 格式化时间（不包含日期）
 * @param utcTime UTC 时间字符串
 * @param userTimezone 用户时区
 * @returns 格式化的时间字符串
 */
export const formatTime = (utcTime: string, userTimezone: string): string => {
  try {
    return dayjs.utc(utcTime).tz(userTimezone).format('HH:mm:ss')
  } catch (error) {
    console.warn('Failed to format time:', error)
    return dayjs.utc(utcTime).format('HH:mm:ss')
  }
}

/**
 * 获取时区的当前时间
 * @param timezone 时区标识符
 * @returns 当前时间的字符串表示
 */
export const getCurrentTimeInTimezone = (timezone: string): string => {
  try {
    return dayjs().tz(timezone).format('YYYY-MM-DD HH:mm:ss')
  } catch (error) {
    console.warn('Failed to get current time in timezone:', error)
    return dayjs().format('YYYY-MM-DD HH:mm:ss')
  }
}

/**
 * 检查时区是否有效
 * @param timezone 时区标识符
 * @returns 是否有效
 */
export const isValidTimezone = (timezone: string): boolean => {
  try {
    // 尝试获取当前时区的时间，如果失败则时区无效
    dayjs().tz(timezone).format()
    return true
  } catch {
    return false
  }
}

/**
 * 获取时区的偏移量（分钟）
 * @param timezone 时区标识符
 * @returns 偏移量（相对于 UTC）
 */
export const getTimezoneOffset = (timezone: string): number => {
  try {
    return dayjs().tz(timezone).utcOffset()
  } catch {
    return 0
  }
}

/**
 * 格式化时区偏移量为字符串
 * @param offsetMinutes 偏移量（分钟）
 * @returns 格式化的偏移量字符串 (如: "+08:00")
 */
export const formatTimezoneOffset = (offsetMinutes: number): string => {
  const hours = Math.floor(Math.abs(offsetMinutes) / 60)
  const minutes = Math.abs(offsetMinutes) % 60
  const sign = offsetMinutes >= 0 ? '+' : '-'
  return `${sign}${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`
}

/**
 * 时间相关的常量
 */
export const TIME_CONSTANTS = {
  // 时间格式
  DATETIME_FORMAT: 'YYYY-MM-DD HH:mm:ss',
  DATE_FORMAT: 'YYYY-MM-DD',
  TIME_FORMAT: 'HH:mm:ss',

  // 相对时间阈值（分钟）
  JUST_NOW_THRESHOLD: 1,
  MINUTES_THRESHOLD: 60,
  HOURS_THRESHOLD: 1440, // 24 * 60
  DAYS_THRESHOLD: 10080, // 7 * 24 * 60

  // 常用时区
  COMMON_TIMEZONES: [
    'UTC',
    'America/New_York',
    'America/Los_Angeles',
    'Europe/London',
    'Europe/Paris',
    'Asia/Shanghai',
    'Asia/Tokyo',
    'Asia/Hong_Kong',
    'Asia/Singapore',
    'Australia/Sydney',
  ] as const,
}