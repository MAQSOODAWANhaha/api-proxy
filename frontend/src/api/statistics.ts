// 统计数据相关API

import { HttpClient } from '@/utils/http'
import { MockDataService, useMockData } from '@/utils/mockData'
import type {
  StatisticsOverview,
  RequestStatistics,
  RequestLog,
  RequestLogParams,
  RequestLogListResponse,
  StatisticsParams
} from '@/types'

export class StatisticsAPI {
  // ===== 统计概览 =====

  // 获取统计概览
  static async getOverview(params: StatisticsParams = {}): Promise<StatisticsOverview> {
    return HttpClient.get('/statistics/overview', params)
  }

  // 获取请求统计数据
  static async getRequestStatistics(params: StatisticsParams = {}): Promise<RequestStatistics> {
    return HttpClient.get('/statistics/requests', params)
  }

  // ===== 请求日志 =====

  // 获取请求日志列表
  static async getRequestLogs(params: RequestLogParams = {}): Promise<RequestLogListResponse> {
    if (useMockData) {
      return MockDataService.getRequestLogs(params)
    }
    return HttpClient.get('/statistics/logs', params)
  }

  // 获取单个请求日志详情
  static async getRequestLog(id: number): Promise<RequestLog> {
    return HttpClient.get(`/statistics/logs/${id}`)
  }

  // 导出请求日志
  static async exportRequestLogs(params: RequestLogParams & {
    format: 'csv' | 'xlsx'
  }): Promise<void> {
    const filename = `request_logs_${new Date().toISOString().split('T')[0]}.${params.format}`
    return HttpClient.download('/statistics/logs/export', params, filename)
  }

  // ===== 仪表盘数据 =====

  // 获取仪表盘卡片数据
  static async getDashboardCards(): Promise<{
    total_requests_today: number
    success_rate_today: number
    total_tokens_today: number
    active_api_services: number
    healthy_keys: number
    total_keys: number
    avg_response_time: number
    requests_per_minute: number
  }> {
    return HttpClient.get('/statistics/dashboard/cards')
  }

  // 获取请求趋势数据（过去7天）
  static async getRequestTrend(days: number = 7): Promise<Array<{
    date: string
    requests: number
    successful: number
    failed: number
    tokens: number
  }>> {
    return HttpClient.get('/statistics/dashboard/trend', { days })
  }

  // 获取服务商使用分布
  static async getProviderDistribution(): Promise<Array<{
    provider: string
    requests: number
    percentage: number
    tokens: number
  }>> {
    return HttpClient.get('/statistics/dashboard/provider-distribution')
  }

  // 获取实时统计数据
  static async getRealTimeStats(): Promise<{
    current_requests: number
    requests_per_second: number
    active_connections: number
    avg_response_time: number
    error_rate: number
    timestamp: string
  }> {
    return HttpClient.get('/statistics/realtime')
  }

  // ===== 响应时间分析 =====

  // 获取响应时间分析
  static async getResponseTimeAnalysis(params: {
    hours?: number
    group_by?: 'hour' | 'day'
    provider_type?: string
  } = {}): Promise<{
    data: Array<{
      timestamp: string
      avg_response_time: number
      p50_response_time: number
      p95_response_time: number
      p99_response_time: number
    }>
    summary: {
      overall_avg: number
      best_performance: number
      worst_performance: number
      trend: 'improving' | 'stable' | 'degrading'
    }
  }> {
    return HttpClient.get('/statistics/response-time', params)
  }

  // ===== 错误分析 =====

  // 获取错误统计
  static async getErrorStatistics(params: {
    hours?: number
    group_by?: 'hour' | 'day'
  } = {}): Promise<{
    data: Array<{
      timestamp: string
      error_count: number
      error_rate: number
      error_types: Record<string, number>
    }>
    top_errors: Array<{
      error_type: string
      count: number
      percentage: number
      last_occurrence: string
    }>
    summary: {
      total_errors: number
      overall_error_rate: number
      most_common_error: string
    }
  }> {
    return HttpClient.get('/statistics/errors', params)
  }

  // ===== Token使用分析 =====

  // 获取Token使用统计
  static async getTokenUsage(params: {
    start_date?: string
    end_date?: string
    group_by?: 'hour' | 'day'
    provider_type?: string
  } = {}): Promise<{
    data: Array<{
      timestamp: string
      total_tokens: number
      prompt_tokens: number
      completion_tokens: number
      cost_estimate: number
    }>
    summary: {
      total_tokens: number
      avg_tokens_per_request: number
      total_cost_estimate: number
      trend: 'increasing' | 'stable' | 'decreasing'
    }
    by_provider: Record<string, {
      tokens: number
      percentage: number
      cost_estimate: number
    }>
  }> {
    return HttpClient.get('/statistics/tokens', params)
  }

  // ===== 用户使用分析 =====

  // 获取用户使用排行
  static async getUserUsageRanking(params: {
    period?: 'today' | 'week' | 'month'
    limit?: number
  } = {}): Promise<Array<{
    user_id: number
    username: string
    total_requests: number
    total_tokens: number
    success_rate: number
    last_active: string
  }>> {
    return HttpClient.get('/statistics/users/ranking', params)
  }

  // ===== 自定义报表 =====

  // 生成自定义报表
  static async generateCustomReport(params: {
    start_date: string
    end_date: string
    metrics: string[]
    group_by: 'hour' | 'day'
    filters?: Record<string, any>
  }): Promise<{
    report_id: string
    status: 'generating' | 'completed' | 'failed'
    download_url?: string
    created_at: string
  }> {
    return HttpClient.post('/statistics/reports/custom', params)
  }

  // 获取报表状态
  static async getReportStatus(reportId: string): Promise<{
    report_id: string
    status: 'generating' | 'completed' | 'failed'
    progress: number
    download_url?: string
    error_message?: string
  }> {
    return HttpClient.get(`/statistics/reports/${reportId}/status`)
  }

  // 下载报表
  static async downloadReport(reportId: string): Promise<void> {
    return HttpClient.download(`/statistics/reports/${reportId}/download`)
  }
}