import { http } from '@/utils/request'
import type { AxiosResponse } from 'axios'
import type { ApiResponse } from '@/utils/request'

export interface DailyStat {
  date: string
  totalRequests: number
  successfulRequests: number
  totalTokens: number
}

export interface ProviderDistribution {
  provider: string
  count: number
}

export interface StatisticsOverview {
  time_range: {
    hours: number
    start_time: string
    end_time: string
  }
  requests: {
    total: number
    successful: number
    failed: number
    success_rate: number
  }
  response_times: {
    avg_ms: number
    p50_ms: number
    p95_ms: number
    p99_ms: number
  }
  traffic: {
    requests_per_second: number
    bytes_sent: number
    bytes_received: number
  }
  by_provider: Record<string, {
    requests: number
    success_rate: number
    avg_response_ms: number
  }>
  top_endpoints: Array<{
    path: string
    requests: number
    percentage: number
  }>
}

export interface RequestStatsResponse {
  time_range: {
    hours: number
    group_by: string
    start_time: string
    end_time: string
    points: number
  }
  data: Array<{
    timestamp: string
    requests: number
    successful: number
    failed: number
    avg_response_ms: number
    success_rate: number
  }>
  aggregated: {
    total_requests: number
    total_successful: number
    total_failed: number
    avg_response_ms: number
  }
}

/**
 * 获取统计概览
 */
export function getStatisticsOverview(hours = 24): Promise<AxiosResponse<ApiResponse<StatisticsOverview>>> {
  return http.get('/statistics/overview', {
    params: { hours },
    showLoading: true
  })
}

/**
 * 获取请求统计数据
 */
export function getRequestStats(
  hours = 168, 
  groupBy = 'day'
): Promise<AxiosResponse<ApiResponse<RequestStatsResponse>>> {
  return http.get('/statistics/requests', {
    params: { hours, group_by: groupBy },
    showLoading: true
  })
}

/**
 * 获取日统计数据（Dashboard使用）
 */
export async function getDailyStats(): Promise<{ stats: DailyStat[], distribution: ProviderDistribution[] }> {
  try {
    // 并行获取概览和请求统计数据
    const [overviewRes, requestStatsRes] = await Promise.all([
      getStatisticsOverview(168), // 7天
      getRequestStats(168, 'day')
    ])

    const overview = overviewRes.data.data
    const requestStats = requestStatsRes.data.data

    // 转换数据格式以匹配前端期望
    const stats: DailyStat[] = requestStats.data.map((item: any) => ({
      date: new Date(item.timestamp).toISOString().split('T')[0],
      totalRequests: item.requests,
      successfulRequests: item.successful,
      totalTokens: Math.floor(item.requests * 1000) // 估算令牌数
    }))

    const distribution: ProviderDistribution[] = Object.entries(overview.by_provider).map(([provider, data]: [string, any]) => ({
      provider,
      count: data.requests
    }))

    return { stats, distribution }
  } catch (error) {
    console.error('获取统计数据失败:', error)
    throw error
  }
}

/**
 * 获取实时统计数据
 */
export function getRealTimeStats(): Promise<AxiosResponse<ApiResponse<{
  current_requests_per_minute: number
  current_active_connections: number
  current_response_time_ms: number
  provider_status: Record<string, 'healthy' | 'warning' | 'error'>
}>>> {
  return http.get('/statistics/realtime', {
    skipErrorHandler: false,
    retryable: true
  })
}

/**
 * 获取错误统计
 */
export function getErrorStats(hours = 24): Promise<AxiosResponse<ApiResponse<{
  total_errors: number
  error_rate: number
  by_error_code: Record<string, number>
  by_provider: Record<string, number>
  recent_errors: Array<{
    timestamp: string
    error_code: number
    provider: string
    message: string
  }>
}>>> {
  return http.get('/statistics/errors', {
    params: { hours },
    showLoading: true
  })
}
