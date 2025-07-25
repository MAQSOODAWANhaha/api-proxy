import request from './index'
import type { AxiosPromise } from 'axios'

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

// Get daily statistics for dashboard
export function getDailyStats(): AxiosPromise<{ stats: DailyStat[], distribution: ProviderDistribution[] }> {
  return new Promise(async (resolve, reject) => {
    try {
      // Fetch both overview and request stats in parallel
      const [overviewRes, requestStatsRes] = await Promise.all([
        request({ url: '/statistics/overview', method: 'get', params: { hours: 168 } }), // 7 days
        request({ url: '/statistics/requests', method: 'get', params: { hours: 168, group_by: 'day' } })
      ])

      const overview: StatisticsOverview = overviewRes.data
      const requestStats: RequestStatsResponse = requestStatsRes.data

      // Transform the data to match frontend expectations
      const stats: DailyStat[] = requestStats.data.map(item => ({
        date: new Date(item.timestamp).toISOString().split('T')[0],
        totalRequests: item.requests,
        successfulRequests: item.successful,
        totalTokens: Math.floor(item.requests * 1000) // Estimate tokens
      }))

      const distribution: ProviderDistribution[] = Object.entries(overview.by_provider).map(([provider, data]) => ({
        provider,
        count: data.requests
      }))

      resolve({ data: { stats, distribution } } as any)
    } catch (error) {
      console.error('Failed to fetch statistics:', error)
      reject(error)
    }
  })
}
