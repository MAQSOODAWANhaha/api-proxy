/**
 * 数据验证和格式化工具函数
 */

/**
 * 安全获取数值，提供默认值和类型验证
 */
export const safeNumber = (value: any, defaultValue: number = 0): number => {
  if (value === null || value === undefined || value === '') {
    return defaultValue
  }
  const num = Number(value)
  return isNaN(num) ? defaultValue : num
}

/**
 * 安全获取百分比，确保在 0-100 范围内
 */
export const safePercentage = (value: any, defaultValue: number = 0): number => {
  const num = safeNumber(value, defaultValue)
  return Math.max(0, Math.min(100, num))
}

/**
 * 安全格式化货币
 */
export const safeCurrency = (value: any, defaultValue: number = 0, precision: number = 2): string => {
  const num = safeNumber(value, defaultValue)
  return `$${num.toFixed(precision)}`
}

/**
 * 安全格式化响应时间
 */
export const safeResponseTime = (value: any, defaultValue: number = 0): string => {
  const num = Math.round(safeNumber(value, defaultValue))
  return `${num}ms`
}

/**
 * 安全格式化日期时间
 */
export const safeDateTime = (value: any, fallback: string = '从未使用'): string => {
  if (!value) return fallback
  try {
    return new Date(value).toLocaleString()
  } catch {
    return fallback
  }
}

/**
 * 安全格式化大数字
 */
export const safeLargeNumber = (value: any, defaultValue: number = 0): string => {
  const num = safeNumber(value, defaultValue)
  return num.toLocaleString()
}

/**
 * 计算成功率（基于成功和失败请求数）
 */
export const calculateSuccessRate = (success: number, failure: number): number => {
  const total = success + failure
  if (total === 0) return 0
  return Math.round((success / total) * 100 * 100) / 100 // 保留两位小数
}

/**
 * 创建统计数据的降级处理对象
 */
export const createSafeStats = (usage: any = {}) => {
  const successfulRequests = safeNumber(usage.successful_requests, 0)
  const failedRequests = safeNumber(usage.failed_requests, 0)

  return {
    totalRequests: safeNumber(usage.total_requests, successfulRequests + failedRequests),
    successfulRequests,
    failedRequests,
    successRate: safePercentage(usage.success_rate, calculateSuccessRate(successfulRequests, failedRequests)),
    avgResponseTime: safeNumber(usage.avg_response_time, 0),
    totalCost: safeNumber(usage.total_cost, 0),
    totalTokens: safeNumber(usage.total_tokens, 0),
    lastUsedAt: usage.last_used_at || null,
  }
}

/**
 * 验证和清理趋势数据
 */
export const safeTrendData = (data: any[] = [], defaultValue: number = 0): number[] => {
  if (!Array.isArray(data)) {
    return Array(7).fill(defaultValue)
  }

  return data.slice(0, 7).map(item => safeNumber(item, defaultValue))
}