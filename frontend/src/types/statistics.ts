// 统计数据相关类型定义

// 统计概览
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

// 请求统计
export interface RequestStatistics {
  time_range: {
    hours: number
    group_by: 'hour' | 'day'
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

// 请求日志
export interface RequestLog {
  id: number
  user_service_api_id?: number
  user_provider_key_id?: number
  request_id?: string
  method: string
  path?: string
  status_code?: number
  response_time_ms?: number
  request_size: number
  response_size: number
  tokens_prompt: number
  tokens_completion: number
  tokens_total: number
  model_used?: string
  client_ip?: string
  user_agent?: string
  error_type?: string
  error_message?: string
  retry_count: number
  created_at: string
}

// 每日统计
export interface DailyStatistics {
  id: number
  user_id: number
  user_service_api_id?: number
  provider_type_id: number
  date: string
  total_requests: number
  successful_requests: number
  failed_requests: number
  total_tokens: number
  avg_response_time: number
  max_response_time: number
  created_at: string
  updated_at: string
}

// 统计查询参数
export interface StatisticsParams {
  hours?: number
  group_by?: 'hour' | 'day'
  upstream_type?: string
}

// 请求日志查询参数
export interface RequestLogParams {
  page?: number
  limit?: number
  start_time?: string
  end_time?: string
  status_code?: number
  user_service_api_id?: number
  method?: string
}

// 请求日志列表响应
export interface RequestLogListResponse {
  logs: RequestLog[]
  pagination: {
    page: number
    limit: number
    total: number
    pages: number
  }
}