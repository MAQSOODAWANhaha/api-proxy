export type SummaryMetricId = 'requests' | 'tokens' | 'cost'

export interface SummaryMetric {
  id: SummaryMetricId
  label: string
  icon: string
  unit: 'count' | 'token' | 'usd'
  today: number
  total: number
  delta: number
}

export interface TrendPoint {
  timestamp: string
  requests: number
  tokens: number
  cost: number
  success_rate: number
}

export interface ModelShareItem {
  model: string
  scope: 'today' | 'total'
  requests: number
  tokens: number
  cost: number
  percentage: number
}

export interface LogsPage {
  items: LogItem[]
  page: number
  page_size: number
  total: number
}

export interface LogItem {
  id: number
  timestamp: string
  method: string
  path?: string | null
  status_code?: number | null
  is_success: boolean
  duration_ms?: number | null
  model?: string | null
  tokens_prompt: number
  tokens_completion: number
  tokens_total: number
  cost?: number | null
  cost_currency?: string | null
  request_id: string
  operation?: string | null
  error_type?: string | null
  error_message?: string | null
  provider_type_id?: number | null
  retry_count: number
  client_ip?: string | null
  user_agent?: string | null
}

export interface StatsOverviewResponse {
  summary: SummaryMetric[]
}

export interface StatsTrendResponse {
  trend: TrendPoint[]
}

export interface StatsModelShareResponse {
  today: ModelShareItem[]
  total: ModelShareItem[]
}

export interface StatsLogsResponse {
  logs: LogsPage
}

export interface StatsOverviewQuery {
  user_service_key: string
  from?: string
  to?: string
  aggregate?: 'Single' | 'Aggregate'
}

export interface StatsTrendQuery extends StatsOverviewQuery {
  timeframe?: '1d' | '7d' | '30d' | '90d'
}

export interface StatsModelShareQuery extends StatsOverviewQuery {
  include_today?: boolean
}

export interface StatsLogsQuery extends StatsOverviewQuery {
  page?: number
  page_size?: number
  search?: string
}
