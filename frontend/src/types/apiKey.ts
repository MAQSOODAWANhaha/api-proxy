// API密钥相关类型定义

// 调度策略类型 - 从后端动态获取
export type SchedulingStrategy = string

// 调度策略定义接口
export interface SchedulingStrategyOption {
  key: SchedulingStrategy
  name: string
  description: string
}

// 服务商类型定义
export interface ProviderType {
  id: string
  name: string
  display_name: string
  base_url: string
  default_model?: string
  supported_features: string[]
}

// 用户内部API密钥（号池）
export interface UserProviderKey {
  id: number
  user_id: number
  provider_type: string
  provider_name: string
  api_key: string
  name: string
  weight: number
  max_requests_per_minute?: number
  max_tokens_per_day?: number
  used_tokens_today: number
  last_used?: string
  is_active: boolean
  health_status: string
  created_at: string
  updated_at: string
}

// 用户对外API服务
export interface UserServiceApi {
  id: number
  user_id: number
  provider_type: string
  provider_name: string
  api_key: string // 对外API密钥
  api_secret: string
  name?: string
  description?: string
  scheduling_strategy: SchedulingStrategy
  retry_count: number
  timeout_seconds: number
  rate_limit: number
  max_tokens_per_day: number
  used_tokens_today: number
  total_requests: number
  successful_requests: number
  last_used?: string
  expires_at?: string
  is_active: boolean
  created_at: string
  updated_at: string
}

// 创建内部API密钥请求
export interface CreateProviderKeyRequest {
  id?: number // 用于编辑时传递ID
  provider_type: string
  api_key: string
  name: string
  weight?: number
  max_requests_per_minute?: number
  max_tokens_per_day?: number
  is_active?: boolean
}

// 创建对外API服务请求
export interface CreateServiceApiRequest {
  provider_type: string
  name?: string
  description?: string
  scheduling_strategy?: SchedulingStrategy
  retry_count?: number
  timeout_seconds?: number
  rate_limit?: number
  max_tokens_per_day?: number
  expires_in_days?: number
}

// API密钥列表查询参数
export interface ApiKeyListParams {
  page?: number
  limit?: number
  user_id?: number
  provider_type?: string
  status?: 'active' | 'inactive'
}

// API密钥列表响应
export interface ApiKeyListResponse {
  api_keys: UserServiceApi[]
  apis?: UserServiceApi[]  // 兼容service APIs返回格式
  pagination: {
    page: number
    limit: number
    total: number
    pages: number
  }
}

// 健康状态
export interface ApiHealthStatus {
  id: number
  user_provider_key_id: number
  is_healthy: boolean
  response_time_ms: number
  success_rate: number
  last_success?: string
  last_failure?: string
  consecutive_failures: number
  total_checks: number
  successful_checks: number
  last_error_message?: string
  created_at: string
  updated_at: string
}