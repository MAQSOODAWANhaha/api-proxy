/**
 * API客户端工具
 * 统一处理HTTP请求，包括认证、错误处理等
 */

// API基础配置
const API_BASE_URL = process.env.NODE_ENV === 'production' 
  ? '/api'  // 生产环境使用相对路径，由 Caddy 转发
  : 'http://192.168.153.140:9090/api'  // 开发环境直连后端，WSL环境指定IP地址

// 导入认证状态管理，用于401处理
import { useAuthStore } from '../store/auth'
// 导入时区状态管理
import { useTimezoneStore } from '../store/timezone'

// 全局401处理函数
const handle401Unauthorized = async () => {
  // 获取auth store实例并清除认证状态
  const authStore = useAuthStore.getState()
  await authStore.logout(false)
  
  // 跳转到登录页面
  if (typeof window !== 'undefined') {
    window.location.hash = '#/login'
  }
}

// 请求配置接口
export interface RequestConfig {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH'
  headers?: Record<string, string>
  body?: any
  params?: Record<string, string>
}

/**
 * 公开接口请求配置选项
 * 用于不需要认证的公开API端点
 */
export interface PublicRequestOptions extends Omit<RequestConfig, 'headers'> {
  /** 自定义请求头，不会自动添加Authorization */
  headers?: Record<string, string>
  /** 是否跳过认证，公开接口默认为true */
  skipAuth?: boolean
}

/**
 * 公开 API 客户端接口
 * 仅暴露不需要 JWT 的基础请求方法
 */
export interface PublicApiClient {
  request<T = any>(endpoint: string, config?: RequestConfig): Promise<ApiResponse<T>>
  get<T = any>(endpoint: string, params?: Record<string, string>): Promise<ApiResponse<T>>
  post<T = any>(endpoint: string, body?: any): Promise<ApiResponse<T>>
  put<T = any>(endpoint: string, body?: any): Promise<ApiResponse<T>>
  delete<T = any>(endpoint: string): Promise<ApiResponse<T>>
}

// 标准API响应格式（与后端 response.rs 保持一致）
export interface ApiResponse<T = any> {
  success: boolean
  message?: string
  data?: T
  error?: {
    code: string
    message: string
  }
  pagination?: {
    page: number
    limit: number
    total: number
    pages: number
  }
}

// 登录请求接口
export interface LoginRequest {
  username: string
  password: string
}

// 登录响应接口
export interface LoginResponse {
  token: string
  refresh_token: string
  user: {
    id: number
    username: string
    email: string
    is_admin: boolean
  }
}

// Token验证响应接口
export interface ValidateTokenResponse {
  valid: boolean
  user?: {
    id: number
    username: string
    email: string
    is_admin: boolean
  }
}

// 刷新token请求接口
export interface RefreshTokenRequest {
  refresh_token: string
}

// 刷新token响应接口
export interface RefreshTokenResponse {
  access_token: string
  token_type: string
  expires_in: number
}

// OAuth相关类型定义
export interface OAuthAuthorizeRequest {
  provider_name: string
  name: string
  description?: string
  extra_params?: Record<string, string>
}

export interface OAuthAuthorizeResponse {
  authorize_url: string
  session_id: string
  state: string
  expires_at: string
}

export interface OAuthCallbackResponse {
  access_token: string
  refresh_token: string
  token_type: string
  expires_in: number
  expires_at: string
  auth_type: string
  provider_type_id: number
  session_id: string
  auth_status: string
}

// OAuth轮询状态响应
export interface OAuthPollingStatusResponse {
  status: 'pending' | 'authorized' | 'error' | 'expired' | 'revoked'
  access_token?: string
  refresh_token?: string
  id_token?: string
  error?: string
  error_description?: string
  expires_in?: number
  polling_interval: number
}

export interface OAuthExchangeRequest {
  session_id: string
  authorization_code: string
}

export interface OAuthRefreshRequest {
  session_id: string
}

export interface OAuthRefreshResponse {
  access_token: string
  refresh_token?: string
  token_type: string
  expires_in?: number
  expires_at: string
}

// API Key 健康状态（与后端保持一致）
export type ApiKeyHealthStatus = 'healthy' | 'rate_limited' | 'unhealthy'

// Provider Types相关类型定义
export interface AuthConfig {
  authorize_url?: string
  token_url?: string
  client_id?: string
  scopes?: string
  pkce_required?: boolean
}

export interface ProviderType {
  id: number
  name: string
  display_name: string
  base_url?: string
  timeout_seconds?: number
  is_active: boolean
  supported_models?: string[]
  supported_auth_types: string[]
  auth_configs?: Record<string, AuthConfig>
  created_at: string
  updated_at?: string
}

export interface ProviderTypesResponse {
  provider_types: ProviderType[]
}

export interface SchedulingStrategy {
  value: string
  label: string
  description: string
  is_default: boolean
}

export interface SchedulingStrategiesResponse {
  scheduling_strategies: SchedulingStrategy[]
}

// 新的统计接口响应类型定义
export interface ModelUsage {
  model: string
  usage: number
  cost: number
  successful_requests: number
  failed_requests: number
  success_rate: number
}

export interface ModelsRateResponse {
  model_usage: ModelUsage[]
}

export interface ModelStatistics {
  model: string
  usage: number
  percentage: number
  cost: number
}

export interface ModelsStatisticsResponse {
  model_usage: ModelStatistics[]
}

export interface TokenTrendPoint {
  timestamp: string
  cache_create_tokens: number
  cache_read_tokens: number
  tokens_prompt: number
  tokens_completion: number
  cost: number
}

export interface TokensTrendResponse {
  token_usage: TokenTrendPoint[]
  current_token_usage: number
  average_token_usage: number
  max_token_usage: number
}

export interface UserApiKeysRequestTrendPoint {
  timestamp: string
  request: number
}

export interface UserApiKeysRequestTrendResponse {
  request_usage: UserApiKeysRequestTrendPoint[]
  current_request_usage: number
  average_request_usage: number
  max_request_usage: number
}

export interface UserApiKeysTokenTrendPoint {
  timestamp: string
  total_token: number
}

export interface UserApiKeysTokenTrendResponse {
  token_usage: UserApiKeysTokenTrendPoint[]
  current_token_usage: number
  average_token_usage: number
  max_token_usage: number
}

// Provider Keys 相关接口定义
export interface ProviderKey {
  id: number
  provider: string
  name: string
  api_key: string
  auth_type: string // "api_key", "oauth"
  weight: number
  max_requests_per_minute: number
  max_tokens_prompt_per_minute: number
  max_requests_per_day: number
  is_active: boolean
  project_id?: string
  usage: {
    total_requests: number
    successful_requests: number
    failed_requests: number
    success_rate: number
    total_tokens: number
    total_cost: number
    avg_response_time: number
    last_used_at?: string
  }
  limits: {
    max_requests_per_minute: number
    max_tokens_prompt_per_minute: number
    max_requests_per_day: number
  }
  status: {
    is_active: boolean
    health_status: ApiKeyHealthStatus
  }
  created_at: string
  updated_at: string
  health_status: ApiKeyHealthStatus
  health_status_detail?: string
}

export interface ProviderKeysListResponse {
  provider_keys: ProviderKey[]
  pagination: {
    page: number
    limit: number
    total: number
    pages: number
  }
}

export interface CreateProviderKeyRequest {
  provider_type_id: number
  name: string
  api_key?: string
  auth_type: string // "api_key", "oauth"
  weight?: number
  max_requests_per_minute?: number
  max_tokens_prompt_per_minute?: number
  max_requests_per_day?: number
  is_active?: boolean
  project_id?: string
}

export interface CreateProviderKeyResponse {
  id: string
  provider: string
  name: string
  created_at: string
}

export interface UpdateProviderKeyRequest {
  provider_type_id: number
  name: string
  api_key?: string
  auth_type: string // "api_key", "oauth"
  weight?: number
  max_requests_per_minute?: number
  max_tokens_prompt_per_minute?: number
  max_requests_per_day?: number
  is_active?: boolean
  project_id?: string
}

export interface UpdateProviderKeyResponse {
  id: string
  name: string
  updated_at: string
}

export interface DeleteProviderKeyResponse {
  id: string
  deleted_at: string
}

// 趋势数据接口定义
export interface TrendDataPoint {
  date: string
  requests: number
  successful_requests: number
  failed_requests: number
  success_rate: number
  avg_response_time: number
  tokens: number
  cost: number
}

export interface TrendDataResponse {
  trend_data: TrendDataPoint[]
  summary: {
    total_requests: number
    success_rate: number
    avg_response_time: number
    total_tokens: number
    total_cost: number
  }
}

export interface ProviderKeyStatsResponse {
  basic_info: {
    provider: string
    name: string
    weight: number
  }
  usage_stats: {
    total_usage: number
    monthly_cost: number
    success_rate: number
    avg_response_time: number
  }
  daily_trends: {
    usage: number[]
    cost: number[]
  }
  limits: {
    max_requests_per_minute: number
    max_tokens_prompt_per_minute: number
    max_requests_per_day: number
  }
}

export interface HealthCheckResponse {
  id: string
  health_status: ApiKeyHealthStatus
  check_time: string
  response_time: number
  details: {
    status_code: number
    latency: number
    error_message: string | null
  }
}

export interface ProviderKeysDashboardStatsResponse {
  total_keys: number
  active_keys: number
  total_usage: number
  total_cost: number
}

// 简单提供商密钥列表响应接口（用于下拉选择）
export interface SimpleProviderKeysListResponse {
  provider_keys: SimpleProviderKey[]
}

// 简单提供商密钥接口（用于下拉选择）
export interface SimpleProviderKey {
  id: number
  name: string
  display_name: string
  provider: string
  provider_type_id: number
  is_active: boolean
}

// Dashboard Cards响应接口（更新为新的后端API格式）
export interface DashboardCardsResponse {
  requests_today: number
  rate_requests_today: string
  successes_today: number
  rate_successes_today: string
  tokens_today: number
  rate_tokens_today: string
  avg_response_time_today: number
  rate_avg_response_time_today: string
}

// 用户档案相关接口定义
export interface UserProfile {
  name: string
  email: string
  avatar: string
  role: string
  created_at: string
  last_login?: string
  total_requests: number
  monthly_requests: number
}

export interface UpdateProfileRequest {
  email?: string
}

export interface ChangePasswordRequest {
  current_password: string
  new_password: string
}

// 用户服务API相关接口定义
export interface UserServiceCardsResponse {
  total_api_keys: number
  active_api_keys: number
  requests: number
}

export interface UserServiceApiKey {
  id: number
  name: string
  description: string
  provider: string
  provider_type_id: number
  api_key: string
  usage?: {
    successful_requests: number
    failed_requests: number
    total_requests: number
    success_rate: number
    avg_response_time: number
    total_cost: number
    total_tokens: number
    last_used_at: string | null
  }
  is_active: boolean
  log_mode: boolean
  last_used_at: string | null
  created_at: string
  expires_at?: string | null
}

export interface UserServiceApiKeysResponse {
  service_api_keys: UserServiceApiKey[]
  pagination: {
    page: number
    limit: number
    total: number
    pages: number
  }
}

export interface CreateUserServiceApiKeyRequest {
  name: string
  description?: string
  provider_type_id: number
  user_provider_keys_ids: number[]
  log_mode?: boolean
  scheduling_strategy?: string
  retry_count?: number
  timeout_seconds?: number
  max_request_per_min?: number
  max_requests_per_day?: number
  max_tokens_per_day?: number
  max_cost_per_day?: number
  expires_at?: string
  is_active?: boolean
}

export interface CreateUserServiceApiKeyResponse {
  id: number
  api_key: string
  name: string
  description: string
  provider_type_id: number
  is_active: boolean
  created_at: string
}

export interface UserServiceApiKeyDetail {
  id: number
  name: string
  description: string
  provider_type_id: number
  provider: string
  api_key: string
  user_provider_keys_ids: number[]
  log_mode: boolean
  scheduling_strategy: string
  retry_count: number
  timeout_seconds: number
  max_request_per_min: number
  max_requests_per_day: number
  max_tokens_per_day: number
  max_cost_per_day: number
  expires_at?: string
  is_active: boolean
  created_at: string
  updated_at: string
}

export interface UpdateUserServiceApiKeyRequest {
  name?: string
  description?: string
  user_provider_keys_ids?: number[]
  log_mode?: boolean
  scheduling_strategy?: string
  retry_count?: number
  timeout_seconds?: number
  max_request_per_min?: number
  max_requests_per_day?: number
  max_tokens_per_day?: number
  max_cost_per_day?: number
  expires_at?: string
}

export interface UpdateUserServiceApiKeyResponse {
  id: number
  name: string
  description: string
  updated_at: string
}

export interface UserServiceApiKeyUsageResponse {
  total_requests: number
  successful_requests: number
  failed_requests: number
  success_rate: number
  total_tokens: number
  tokens_prompt: number
  tokens_completion: number
  cache_create_tokens: number
  cache_read_tokens: number
  total_cost: number
  cost_currency: string
  avg_response_time: number
  last_used: string
  usage_trend: Array<{
    date: string
    requests: number
    successful_requests: number
    failed_requests: number
    tokens: number
    cost: number
  }>
}

export interface RegenerateUserServiceApiKeyResponse {
  id: number
  api_key: string
  regenerated_at: string
}

export interface UpdateUserServiceApiKeyStatusRequest {
  is_active: boolean
}

export interface UpdateUserServiceApiKeyStatusResponse {
  id: number
  is_active: boolean
  updated_at: string
}

// Logs 相关接口定义（基于 proxy_tracing 表）
interface ProxyTraceBase {
  id: number
  user_service_api_id: number
  user_provider_key_id?: number
  user_id?: number
  method: string
  path?: string
  status_code?: number
  tokens_prompt: number
  tokens_completion: number
  tokens_total: number
  token_efficiency_ratio?: number
  cache_create_tokens: number
  cache_read_tokens: number
  cost?: number
  cost_currency: string
  model_used?: string
  client_ip?: string
  user_agent?: string
  error_type?: string
  error_message?: string
  retry_count: number
  provider_type_id?: number
  start_time?: string
  end_time?: string
  duration_ms?: number
  is_success: boolean
  created_at: string
  provider_name?: string
  user_service_api_name?: string
  user_provider_key_name?: string
}

export interface ProxyTraceListEntry extends ProxyTraceBase {}

export interface ProxyTraceEntry extends ProxyTraceBase {
  request_id: string
}

export interface LogsDashboardStatsResponse {
  total_requests: number
  successful_requests: number
  failed_requests: number
  success_rate: number
  total_tokens: number
  total_cost: number
  avg_response_time: number
}

export interface LogsAnalyticsResponse {
  time_series: Array<{
    timestamp: string
    total_requests: number
    successful_requests: number
    failed_requests: number
    total_tokens: number
    total_cost: number
    avg_response_time: number
  }>
  model_distribution: Array<{
    model: string
    request_count: number
    token_count: number
    cost: number
    percentage: number
  }>
  provider_distribution: Array<{
    provider_name: string
    request_count: number
    success_rate: number
    avg_response_time: number
  }>
  status_distribution: Array<{
    status_code: number
    count: number
    percentage: number
  }>
}

export interface SystemMetrics {
  cpu_usage: number;
  memory: {
    total_mb: number;
    used_mb: number;
    usage_percentage: number;
  };
  disk: {
    total_gb: number;
    used_gb: number;
    usage_percentage: number;
  };
  uptime: string;
}

import { userApi } from './userApi';

/**
 * API客户端类
 */
class ApiClient {
  private baseURL: string
  private token: string | null = null

  constructor(baseURL: string) {
    this.baseURL = baseURL
    this.loadToken()
  }

  /**
   * 从localStorage加载token
   */
  private loadToken() {
    if (typeof window !== 'undefined') {
      this.token = localStorage.getItem('auth_token')
    }
  }

  /**
   * 设置认证token
   */
  setToken(token: string | null) {
    this.token = token
    if (typeof window !== 'undefined') {
      if (token) {
        localStorage.setItem('auth_token', token)
      } else {
        localStorage.removeItem('auth_token')
      }
    }
  }

  /**
   * 获取完整的请求URL
   */
  private getURL(endpoint: string, params?: Record<string, string>): string {
    let url = `${this.baseURL}${endpoint}`
    
    if (params) {
      const searchParams = new URLSearchParams(params)
      url += `?${searchParams.toString()}`
    }
    
    return url
  }

  /**
   * 构建请求头
   */
  private buildHeaders(customHeaders?: Record<string, string>, skipAuth: boolean = false): Record<string, string> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...customHeaders,
    }

    // 添加认证token（除非跳过认证）
    if (!skipAuth && this.token) {
      headers.Authorization = `Bearer ${this.token}`
    }

    // 添加时区信息
    if (typeof window !== 'undefined') {
      const timezoneStore = useTimezoneStore.getState()
      console.log('[API Debug] Timezone store state:', {
        isInitialized: timezoneStore.isInitialized,
        timezone: timezoneStore.timezone,
        offset: timezoneStore.offset
      })

      if (timezoneStore.isInitialized && timezoneStore.timezone) {
        headers['X-Timezone'] = timezoneStore.timezone
        console.log('[API Debug] X-Timezone header set:', timezoneStore.timezone)
      } else {
        console.log('[API Debug] X-Timezone header not set - store not initialized or no timezone')
      }
    } else {
      console.log('[API Debug] Not in browser environment, skipping timezone header')
    }

    return headers
  }

  /**
   * 内部请求处理方法
   *
   * @private
   */
  private async makeRequest<T = any>(
    endpoint: string,
    config: RequestConfig,
    skipAuth: boolean = false
  ): Promise<ApiResponse<T>> {
    const {
      method = 'GET',
      headers: customHeaders,
      body,
      params,
    } = config

    const url = this.getURL(endpoint, params)
    const headers = this.buildHeaders(customHeaders, skipAuth)

    const requestConfig: RequestInit = {
      method,
      headers,
    }

    if (body && method !== 'GET') {
      requestConfig.body = JSON.stringify(body)
    }

    try {
      const logPrefix = skipAuth ? '[API Public]' : '[API]'
      console.log(`${logPrefix} ${method} ${url}`, body ? { body } : '')

      const response = await fetch(url, requestConfig)

      if (!response.ok) {
        // 处理HTTP错误状态码
        const errorText = await response.text()
        let errorData
        try {
          errorData = JSON.parse(errorText)
        } catch {
          errorData = { message: errorText || `HTTP ${response.status}` }
        }

        console.error(`${logPrefix} Error] ${method} ${url}:`, errorData)

        // 仅对需要认证的接口处理401重定向
        if (!skipAuth && response.status === 401) {
          console.warn('401 Unauthorized detected, redirecting to login page')
          setTimeout(() => {
            handle401Unauthorized()
          }, 100)
        }

        return {
          success: false,
          error: {
            code: `HTTP_${response.status}`,
            message: errorData.message || `Request failed with status ${response.status}`,
          },
        }
      }

      const data: ApiResponse<T> = await response.json()
      console.log(`${logPrefix} Success] ${method} ${url}:`, data)

      return data
    } catch (error) {
      const logPrefix = skipAuth ? '[API Public]' : '[API]'
      console.error(`${logPrefix} Exception] ${method} ${url}:`, error)

      // 仅对需要认证的接口处理401异常
      if (!skipAuth && error instanceof Error && error.message.includes('401')) {
        console.warn('401 error detected in exception, redirecting to login page')
        setTimeout(() => {
          handle401Unauthorized()
        }, 100)
      }

      return {
        success: false,
        error: {
          code: 'NETWORK_ERROR',
          message: error instanceof Error ? error.message : 'Network request failed',
        },
      }
    }
  }

  /**
   * 通用请求方法（需要认证）
   */
  async request<T = any>(endpoint: string, config: RequestConfig = {}): Promise<ApiResponse<T>> {
    return this.makeRequest<T>(endpoint, config, false)
  }

  /**
   * 公开接口请求方法（不需要认证）
   *
   * 用于调用公开的API端点，如统计数据接口
   *
   * @param endpoint API端点
   * @param config 请求配置
   * @returns API响应
   */
  async publicRequest<T = any>(endpoint: string, config: RequestConfig = {}): Promise<ApiResponse<T>> {
    return this.makeRequest<T>(endpoint, config, true)
  }

  /**
   * GET请求
   */
  async get<T = any>(endpoint: string, params?: Record<string, string>): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'GET', params })
  }

  /**
   * 公开GET请求（不添加认证头）
   */
  async publicGet<T = any>(endpoint: string, params?: Record<string, string>): Promise<ApiResponse<T>> {
    return this.publicRequest<T>(endpoint, { method: 'GET', params })
  }

  /**
   * POST请求
   */
  async post<T = any>(endpoint: string, body?: any): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'POST', body })
  }

  /**
   * PUT请求
   */
  async put<T = any>(endpoint: string, body?: any): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'PUT', body })
  }

  /**
   * DELETE请求
   */
  async delete<T = any>(endpoint: string): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'DELETE' })
  }

  /**
   * 用户登录
   */
  async login(credentials: LoginRequest): Promise<ApiResponse<LoginResponse>> {
    const response = await this.post<LoginResponse>('/users/auth/login', credentials)
    
    // 登录成功后自动设置token
    if (response.success && response.data?.token) {
      this.setToken(response.data.token)
    }
    
    return response
  }

  /**
   * 用户登出
   */
  async logout(): Promise<ApiResponse<void>> {
    const response = await this.post<void>('/users/auth/logout')
    
    // 无论成功失败都清除本地token
    this.setToken(null)
    
    return response
  }

  /**
   * 验证token
   */
  async validateToken(): Promise<ApiResponse<ValidateTokenResponse>> {
    return this.get<ValidateTokenResponse>('/users/auth/validate')
  }

  /**
   * 刷新access token
   */
  async refreshToken(refreshToken: string): Promise<ApiResponse<RefreshTokenResponse>> {
    return this.post<RefreshTokenResponse>('/users/auth/refresh', { refresh_token: refreshToken })
  }
}

// 创建API客户端实例
export const apiClient = new ApiClient(API_BASE_URL)

/**
 * 创建专门的公开API客户端
 * 用于调用不需要JWT认证的公开接口
 *
 * @param baseURL API基础URL
 * @returns 配置为公开接口的API客户端实例
 */
export function createPublicApiClient(baseURL: string = API_BASE_URL): PublicApiClient {
  const client = new ApiClient(baseURL)
  return {
    ...client,
    request: <T = any>(endpoint: string, config: RequestConfig = {}) =>
      client.publicRequest<T>(endpoint, config),
    get: <T = any>(endpoint: string, params?: Record<string, string>) =>
      client.publicGet<T>(endpoint, params),
    post: <T = any>(endpoint: string, body?: any) =>
      client.publicRequest<T>(endpoint, { method: 'POST', body }),
    put: <T = any>(endpoint: string, body?: any) =>
      client.publicRequest<T>(endpoint, { method: 'PUT', body }),
    delete: <T = any>(endpoint: string) =>
      client.publicRequest<T>(endpoint, { method: 'DELETE' }),
  }
}

/**
 * 预配置的公开API客户端实例
 * 专门用于调用统计接口等公开端点
 */
export const publicApiClient = createPublicApiClient(API_BASE_URL)

/**
 * 安全的数值计算和格式化工具函数
 */
export const dashboardUtils = {
  /**
   * 安全地计算增长率，处理除零和异常值
   */
  calculateGrowthRate(current: number, previous: number): { rate: number; isPositive: boolean } {
    // 边界情况处理
    if (!Number.isFinite(current) || !Number.isFinite(previous)) {
      return { rate: 0, isPositive: false }
    }
    
    if (previous === 0) {
      return current > 0 ? { rate: 100, isPositive: true } : { rate: 0, isPositive: false }
    }
    
    const rate = ((current - previous) / Math.abs(previous)) * 100
    return {
      rate: Number.isFinite(rate) ? Math.round(rate * 10) / 10 : 0,
      isPositive: rate > 0
    }
  },

  /**
   * 格式化数值显示
   */
  formatValue(value: number, type: 'number' | 'percentage' | 'duration'): string {
    if (!Number.isFinite(value)) return '--'
    
    switch (type) {
      case 'number':
        return value >= 1000 ? `${(value / 1000).toFixed(1)}k` : value.toLocaleString()
      case 'percentage':
        return `${value.toFixed(1)}%`
      case 'duration':
        return `${Math.round(value)} ms`
      default:
        return value.toString()
    }
  },

  /**
   * 格式化增长率显示
   */
  formatDelta(rate: number, isPositive: boolean): string {
    if (rate === 0) return '0%'
    const sign = isPositive ? '+' : ''
    return `${sign}${rate.toFixed(1)}%`
  }
}

// 导出便捷方法
export const api = {
  login: (credentials: LoginRequest) => apiClient.login(credentials),
  logout: () => apiClient.logout(),
  validateToken: () => apiClient.validateToken(),
  refreshToken: (refreshToken: string) => apiClient.refreshToken(refreshToken),
  users: {
    ...userApi,

    /**
     * 获取用户档案信息
     */
    async getProfile(): Promise<ApiResponse<UserProfile>> {
      try {
        return await apiClient.get<UserProfile>('/users/profile')
      } catch (error) {
        console.error('[Users] Failed to fetch user profile:', error)
        return {
          success: false,
          error: {
            code: 'USER_PROFILE_ERROR',
            message: '获取用户档案失败'
          }
        }
      }
    },

    /**
     * 更新用户档案
     */
    async updateProfile(data: UpdateProfileRequest): Promise<ApiResponse<UserProfile>> {
      try {
        return await apiClient.put<UserProfile>('/users/profile', data)
      } catch (error) {
        console.error('[Users] Failed to update user profile:', error)
        return {
          success: false,
          error: {
            code: 'UPDATE_USER_PROFILE_ERROR',
            message: '更新用户档案失败'
          }
        }
      }
    },

    /**
     * 修改密码
     */
    async changePassword(data: ChangePasswordRequest): Promise<ApiResponse<void>> {
      try {
        return await apiClient.put<void>('/users/change-password', data)
      } catch (error) {
        console.error('[Users] Failed to change password:', error)
        return {
          success: false,
          error: {
            code: 'CHANGE_PASSWORD_ERROR',
            message: '修改密码失败'
          }
        }
      }
    },
  },
  
  // 新的统计接口调用
  statistics: {
    /**
     * 获取今日仪表盘卡片数据（含增长率）
     */
    async getDashboardCards(): Promise<ApiResponse<DashboardCardsResponse>> {
      try {
        return await apiClient.get<DashboardCardsResponse>('/statistics/today/cards')
      } catch (error) {
        console.error('[Statistics] Failed to fetch dashboard cards:', error)
        return {
          success: false,
          error: {
            code: 'DASHBOARD_CARDS_ERROR',
            message: '获取今日仪表盘数据失败'
          }
        }
      }
    },

    /**
     * 获取模型使用占比
     */
    async getModelsRate(range: string = '7days', start?: string, end?: string): Promise<ApiResponse<ModelsRateResponse>> {
      try {
        const params: Record<string, string> = { range }
        if (start) params.start = start
        if (end) params.end = end
        
        return await apiClient.get<ModelsRateResponse>('/statistics/models/rate', params)
      } catch (error) {
        console.error('[Statistics] Failed to fetch models rate:', error)
        return {
          success: false,
          error: {
            code: 'MODELS_RATE_ERROR',
            message: '获取模型使用占比失败'
          }
        }
      }
    },

    /**
     * 获取模型详细统计
     */
    async getModelsStatistics(range: string = '7days', start?: string, end?: string): Promise<ApiResponse<ModelsStatisticsResponse>> {
      try {
        const params: Record<string, string> = { range }
        if (start) params.start = start
        if (end) params.end = end
        
        return await apiClient.get<ModelsStatisticsResponse>('/statistics/models/statistics', params)
      } catch (error) {
        console.error('[Statistics] Failed to fetch models statistics:', error)
        return {
          success: false,
          error: {
            code: 'MODELS_STATISTICS_ERROR',
            message: '获取模型详细统计失败'
          }
        }
      }
    },

    /**
     * 获取Token使用趋势（固定30天）
     */
    async getTokensTrend(): Promise<ApiResponse<TokensTrendResponse>> {
      try {
        return await apiClient.get<TokensTrendResponse>('/statistics/tokens/trend')
      } catch (error) {
        console.error('[Statistics] Failed to fetch tokens trend:', error)
        return {
          success: false,
          error: {
            code: 'TOKENS_TREND_ERROR',
            message: '获取Token使用趋势失败'
          }
        }
      }
    },

    /**
     * 获取用户API Keys请求趋势（固定30天）
     */
    async getUserApiKeysRequestTrend(): Promise<ApiResponse<UserApiKeysRequestTrendResponse>> {
      try {
        return await apiClient.get<UserApiKeysRequestTrendResponse>('/statistics/user-service-api-keys/request')
      } catch (error) {
        console.error('[Statistics] Failed to fetch user API keys request trend:', error)
        return {
          success: false,
          error: {
            code: 'USER_API_KEYS_REQUEST_TREND_ERROR',
            message: '获取用户API Keys请求趋势失败'
          }
        }
      }
    },

    /**
     * 获取用户API Keys Token趋势（固定30天）
     */
    async getUserApiKeysTokenTrend(): Promise<ApiResponse<UserApiKeysTokenTrendResponse>> {
      try {
        return await apiClient.get<UserApiKeysTokenTrendResponse>('/statistics/user-service-api-keys/token')
      } catch (error) {
        console.error('[Statistics] Failed to fetch user API keys token trend:', error)
        return {
          success: false,
          error: {
            code: 'USER_API_KEYS_TOKEN_TREND_ERROR',
            message: '获取用户API Keys Token趋势失败'
          }
        }
      }
    }
  },

  // 用户服务API相关接口
  userService: {
    /**
     * 获取用户API Keys概览卡片数据
     */
    async getCards(): Promise<ApiResponse<UserServiceCardsResponse>> {
      try {
        return await apiClient.get<UserServiceCardsResponse>('/user-service/cards')
      } catch (error) {
        console.error('[UserService] Failed to fetch cards:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_CARDS_ERROR',
            message: '获取用户服务概览数据失败'
          }
        }
      }
    },

    /**
     * 获取用户API Keys列表
     */
    async getKeys(params?: {
      page?: number
      limit?: number
      name?: string
      description?: string
      provider_type_id?: number
      is_active?: boolean
    }): Promise<ApiResponse<UserServiceApiKeysResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.page) queryParams.page = params.page.toString()
        if (params?.limit) queryParams.limit = params.limit.toString()
        if (params?.name) queryParams.name = params.name
        if (params?.description) queryParams.description = params.description
        if (params?.provider_type_id) queryParams.provider_type_id = params.provider_type_id.toString()
        if (params?.is_active !== undefined) queryParams.is_active = params.is_active.toString()

        return await apiClient.get<UserServiceApiKeysResponse>('/user-service/keys', queryParams)
      } catch (error) {
        console.error('[UserService] Failed to fetch keys:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_KEYS_ERROR',
            message: '获取API Keys列表失败'
          }
        }
      }
    },

    /**
     * 创建新的API Key
     */
    async createKey(data: CreateUserServiceApiKeyRequest): Promise<ApiResponse<CreateUserServiceApiKeyResponse>> {
      try {
        return await apiClient.post<CreateUserServiceApiKeyResponse>('/user-service/keys', data)
      } catch (error) {
        console.error('[UserService] Failed to create key:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_CREATE_KEY_ERROR',
            message: '创建API Key失败'
          }
        }
      }
    },

    /**
     * 获取API Key详情
     */
    async getKeyDetail(id: number): Promise<ApiResponse<UserServiceApiKeyDetail>> {
      try {
        return await apiClient.get<UserServiceApiKeyDetail>(`/user-service/keys/${id}`)
      } catch (error) {
        console.error('[UserService] Failed to fetch key detail:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_KEY_DETAIL_ERROR',
            message: '获取API Key详情失败'
          }
        }
      }
    },

    /**
     * 更新API Key
     */
    async updateKey(id: number, data: UpdateUserServiceApiKeyRequest): Promise<ApiResponse<UpdateUserServiceApiKeyResponse>> {
      try {
        return await apiClient.put<UpdateUserServiceApiKeyResponse>(`/user-service/keys/${id}`, data)
      } catch (error) {
        console.error('[UserService] Failed to update key:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_UPDATE_KEY_ERROR',
            message: '更新API Key失败'
          }
        }
      }
    },

    /**
     * 删除API Key
     */
    async deleteKey(id: number): Promise<ApiResponse<null>> {
      try {
        return await apiClient.delete<null>(`/user-service/keys/${id}`)
      } catch (error) {
        console.error('[UserService] Failed to delete key:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_DELETE_KEY_ERROR',
            message: '删除API Key失败'
          }
        }
      }
    },

    /**
     * 获取API Key使用统计
     */
    async getKeyUsage(id: number, params?: {
      time_range?: 'today' | '7days' | '30days'
      start_date?: string
      end_date?: string
    }): Promise<ApiResponse<UserServiceApiKeyUsageResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.time_range) queryParams.time_range = params.time_range
        if (params?.start_date) queryParams.start_date = params.start_date
        if (params?.end_date) queryParams.end_date = params.end_date

        return await apiClient.get<UserServiceApiKeyUsageResponse>(`/user-service/keys/${id}/usage`, queryParams)
      } catch (error) {
        console.error('[UserService] Failed to fetch key usage:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_KEY_USAGE_ERROR',
            message: '获取API Key使用统计失败'
          }
        }
      }
    },

    /**
     * 重新生成API Key
     */
    async regenerateKey(id: number): Promise<ApiResponse<RegenerateUserServiceApiKeyResponse>> {
      try {
        return await apiClient.post<RegenerateUserServiceApiKeyResponse>(`/user-service/keys/${id}/regenerate`)
      } catch (error) {
        console.error('[UserService] Failed to regenerate key:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_REGENERATE_KEY_ERROR',
            message: '重新生成API Key失败'
          }
        }
      }
    },

    /**
     * 更新API Key状态
     */
    async updateKeyStatus(id: number, isActive: boolean): Promise<ApiResponse<UpdateUserServiceApiKeyStatusResponse>> {
      try {
        return await apiClient.put<UpdateUserServiceApiKeyStatusResponse>(`/user-service/keys/${id}/status`, {
          is_active: isActive
        })
      } catch (error) {
        console.error('[UserService] Failed to update key status:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_UPDATE_KEY_STATUS_ERROR',
            message: '更新API Key状态失败'
          }
        }
      }
    },

    /**
     * 获取API Key趋势数据
     */
    async getKeyTrends(id: number, params?: {
      days?: number
      start_date?: string
      end_date?: string
    }): Promise<ApiResponse<TrendDataResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.days) queryParams.days = params.days.toString()
        if (params?.start_date) queryParams.start_date = params.start_date
        if (params?.end_date) queryParams.end_date = params.end_date

        return await apiClient.get<TrendDataResponse>(`/user-service/keys/${id}/trends`, queryParams)
      } catch (error) {
        console.error('[UserService] Failed to fetch key trends:', error)
        return {
          success: false,
          error: {
            code: 'USER_SERVICE_KEY_TRENDS_ERROR',
            message: '获取API Key趋势数据失败'
          }
        }
      }
    }
  },

  // 认证相关接口
  auth: {
    /**
     * 获取服务商类型列表
     */
    async getProviderTypes(params?: {
      is_active?: boolean
      include_inactive?: boolean
    }): Promise<ApiResponse<ProviderTypesResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.is_active !== undefined) queryParams.is_active = params.is_active.toString()
        if (params?.include_inactive !== undefined) {
          queryParams.include_inactive = params.include_inactive.toString()
        }

        return await apiClient.get<ProviderTypesResponse>('/provider-types/providers', queryParams)
      } catch (error) {
        console.error('[Auth] Failed to fetch provider types:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_TYPES_ERROR',
            message: '获取服务商类型失败'
          }
        }
      }
    },

    /**
     * 获取调度策略列表
     */
    async getSchedulingStrategies(): Promise<ApiResponse<SchedulingStrategiesResponse>> {
      try {
        return await apiClient.get<SchedulingStrategiesResponse>('/provider-types/scheduling-strategies')
      } catch (error) {
        console.error('[Auth] Failed to fetch scheduling strategies:', error)
        return {
          success: false,
          error: {
            code: 'SCHEDULING_STRATEGIES_ERROR',
            message: '获取调度策略失败'
          }
        }
      }
    },

    /**
     * 获取用户提供商密钥列表（用于下拉选择）
     */
    async getUserProviderKeys(params?: {
      provider_type_id?: number
      is_active?: boolean
    }): Promise<ApiResponse<{ user_provider_keys: Array<{ id: number; name: string; display_name: string }> }>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.provider_type_id !== undefined) queryParams.provider_type_id = params.provider_type_id.toString()
        if (params?.is_active !== undefined) queryParams.is_active = params.is_active.toString()

        return await apiClient.get('/provider-keys/keys', queryParams)
      } catch (error) {
        console.error('[Auth] Failed to fetch user provider keys:', error)
        return {
          success: false,
          error: {
            code: 'USER_PROVIDER_KEYS_ERROR',
            message: '获取用户提供商密钥失败'
          }
        }
      }
    },

    // OAuth相关接口
    /**
     * 启动OAuth授权流程 (OAuth v2)
     */
    async initiateOAuth(request: OAuthAuthorizeRequest): Promise<ApiResponse<OAuthAuthorizeResponse>> {
      try {
        return await apiClient.post<OAuthAuthorizeResponse>('/oauth/authorize', request)
      } catch (error) {
        console.error('[OAuth] Failed to initiate OAuth flow:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_INITIATE_ERROR',
            message: 'OAuth授权启动失败'
          }
        }
      }
    },

    /**
     * 轮询OAuth状态 (OAuth v2)
     */
    async pollOAuthStatus(sessionId: string): Promise<ApiResponse<OAuthPollingStatusResponse>> {
      try {
        return await apiClient.get<OAuthPollingStatusResponse>(`/oauth/poll?session_id=${sessionId}`)
      } catch (error) {
        console.error('[OAuth] Failed to poll OAuth status:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_POLL_ERROR',
            message: '轮询OAuth状态失败'
          }
        }
      }
    },

    /**
     * 手动交换OAuth授权码获取Token (OAuth v2)
     */
    async exchangeOAuthToken(request: OAuthExchangeRequest): Promise<ApiResponse<OAuthCallbackResponse>> {
      try {
        return await apiClient.post<OAuthCallbackResponse>('/oauth/exchange', request)
      } catch (error) {
        console.error('[OAuth] Failed to exchange OAuth token:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_EXCHANGE_ERROR',
            message: 'OAuth令牌交换失败'
          }
        }
      }
    },

    /**
     * 查询OAuth状态 (旧版本，保持兼容性)
     */
    async getOAuthStatus(sessionId: string): Promise<ApiResponse<OAuthPollingStatusResponse>> {
      try {
        return await apiClient.get<OAuthPollingStatusResponse>(`/oauth/poll?session_id=${sessionId}`)
      } catch (error) {
        console.error('[OAuth] Failed to get OAuth status:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_STATUS_ERROR',
            message: '获取OAuth状态失败'
          }
        }
      }
    },

    /**
     * 刷新OAuth令牌
     */
    async refreshOAuthToken(request: OAuthRefreshRequest): Promise<ApiResponse<OAuthRefreshResponse>> {
      try {
        return await apiClient.post<OAuthRefreshResponse>('/oauth/refresh', request)
      } catch (error) {
        console.error('[OAuth] Failed to refresh OAuth token:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_REFRESH_ERROR',
            message: 'OAuth令牌刷新失败'
          }
        }
      }
    },

    /**
     * 删除OAuth会话
     */
    async deleteOAuthSession(sessionId: string): Promise<ApiResponse<void>> {
      try {
        return await apiClient.delete<void>(`/oauth/sessions/${sessionId}`)
      } catch (error) {
        console.error('[OAuth] Failed to delete OAuth session:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_DELETE_ERROR',
            message: 'OAuth会话删除失败'
          }
        }
      }
    },

    /**
     * 获取OAuth会话列表
     */
    async getOAuthSessions(): Promise<ApiResponse<{ sessions: any[] }>> {
      try {
        return await apiClient.get<{ sessions: any[] }>('/oauth/sessions')
      } catch (error) {
        console.error('[OAuth] Failed to get OAuth sessions:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_SESSIONS_ERROR',
            message: '获取OAuth会话列表失败'
          }
        }
      }
    },

    /**
     * 获取OAuth统计信息
     */
    async getOAuthStatistics(): Promise<ApiResponse<any>> {
      try {
        return await apiClient.get<any>('/oauth/statistics')
      } catch (error) {
        console.error('[OAuth] Failed to get OAuth statistics:', error)
        return {
          success: false,
          error: {
            code: 'OAUTH_STATS_ERROR',
            message: '获取OAuth统计信息失败'
          }
        }
      }
    }
  },

  // Provider Keys 相关接口
  providerKeys: {
    /**
     * 获取卡片统计数据
     */
    async getDashboardStats(): Promise<ApiResponse<ProviderKeysDashboardStatsResponse>> {
      try {
        return await apiClient.get<ProviderKeysDashboardStatsResponse>('/provider-keys/dashboard-stats')
      } catch (error) {
        console.error('[ProviderKeys] Failed to fetch dashboard stats:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_DASHBOARD_STATS_ERROR',
            message: '获取提供商密钥统计数据失败'
          }
        }
      }
    },

    /**
     * 获取提供商密钥列表
     */
    async getList(params?: {
      page?: number
      limit?: number
      search?: string
      provider?: string
      status?: string
    }): Promise<ApiResponse<ProviderKeysListResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.page !== undefined) queryParams.page = params.page.toString()
        if (params?.limit !== undefined) queryParams.limit = params.limit.toString()
        if (params?.search) queryParams.search = params.search
        if (params?.provider) queryParams.provider = params.provider
        if (params?.status) queryParams.status = params.status

        return await apiClient.get<ProviderKeysListResponse>('/provider-keys/keys', queryParams)
      } catch (error) {
        console.error('[ProviderKeys] Failed to fetch keys list:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_LIST_ERROR',
            message: '获取提供商密钥列表失败'
          }
        }
      }
    },

    /**
     * 创建提供商密钥
     */
    async create(data: CreateProviderKeyRequest): Promise<ApiResponse<CreateProviderKeyResponse>> {
      try {
        return await apiClient.post<CreateProviderKeyResponse>('/provider-keys/keys', data)
      } catch (error) {
        console.error('[ProviderKeys] Failed to create key:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_CREATE_ERROR',
            message: '创建提供商密钥失败'
          }
        }
      }
    },

    /**
     * 获取提供商密钥详情
     */
    async getDetail(id: string): Promise<ApiResponse<ProviderKey>> {
      try {
        return await apiClient.get<ProviderKey>(`/provider-keys/keys/${id}`)
      } catch (error) {
        console.error('[ProviderKeys] Failed to fetch key detail:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_DETAIL_ERROR',
            message: '获取提供商密钥详情失败'
          }
        }
      }
    },

    /**
     * 更新提供商密钥
     */
    async update(id: string, data: UpdateProviderKeyRequest): Promise<ApiResponse<UpdateProviderKeyResponse>> {
      try {
        return await apiClient.put<UpdateProviderKeyResponse>(`/provider-keys/keys/${id}`, data)
      } catch (error) {
        console.error('[ProviderKeys] Failed to update key:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_UPDATE_ERROR',
            message: '更新提供商密钥失败'
          }
        }
      }
    },

    /**
     * 删除提供商密钥
     */
    async delete(id: string): Promise<ApiResponse<DeleteProviderKeyResponse>> {
      try {
        return await apiClient.delete<DeleteProviderKeyResponse>(`/provider-keys/keys/${id}`)
      } catch (error) {
        console.error('[ProviderKeys] Failed to delete key:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_DELETE_ERROR',
            message: '删除提供商密钥失败'
          }
        }
      }
    },

    /**
     * 获取密钥统计信息
     */
    async getStats(id: string): Promise<ApiResponse<ProviderKeyStatsResponse>> {
      try {
        return await apiClient.get<ProviderKeyStatsResponse>(`/provider-keys/keys/${id}/stats`)
      } catch (error) {
        console.error('[ProviderKeys] Failed to fetch key stats:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_STATS_ERROR',
            message: '获取密钥统计信息失败'
          }
        }
      }
    },

    /**
     * 获取简单提供商密钥列表（用于下拉选择）
     */
    async getSimpleList(params?: {
      provider_type_id?: number
      is_active?: boolean
    }): Promise<ApiResponse<SimpleProviderKeysListResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.provider_type_id !== undefined) queryParams.provider_type_id = params.provider_type_id.toString()
        if (params?.is_active !== undefined) queryParams.is_active = params.is_active.toString()

        return await apiClient.get<SimpleProviderKeysListResponse>('/provider-keys/simple', queryParams)
      } catch (error) {
        console.error('[ProviderKeys] Failed to fetch simple keys list:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_SIMPLE_LIST_ERROR',
            message: '获取简单提供商密钥列表失败'
          }
        }
      }
    },

    /**
     * 执行健康检查
     */
    async healthCheck(id: string): Promise<ApiResponse<HealthCheckResponse>> {
      try {
        return await apiClient.post<HealthCheckResponse>(`/provider-keys/keys/${id}/health-check`)
      } catch (error) {
        console.error('[ProviderKeys] Failed to perform health check:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_HEALTH_CHECK_ERROR',
            message: '执行健康检查失败'
          }
        }
      }
    },

    /**
     * 获取密钥趋势数据
     */
    async getTrends(id: string, params?: {
      days?: number
      start_date?: string
      end_date?: string
    }): Promise<ApiResponse<TrendDataResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.days) queryParams.days = params.days.toString()
        if (params?.start_date) queryParams.start_date = params.start_date
        if (params?.end_date) queryParams.end_date = params.end_date

        return await apiClient.get<TrendDataResponse>(`/provider-keys/keys/${id}/trends`, queryParams)
      } catch (error) {
        console.error('[ProviderKeys] Failed to fetch key trends:', error)
        return {
          success: false,
          error: {
            code: 'PROVIDER_KEYS_TRENDS_ERROR',
            message: '获取密钥趋势数据失败'
          }
        }
      }
    }
  },

  // Logs 相关接口
  logs: {
    /**
     * 获取日志仪表板统计数据
     */
    async getDashboardStats(): Promise<ApiResponse<LogsDashboardStatsResponse>> {
      try {
        return await apiClient.get<LogsDashboardStatsResponse>('/logs/dashboard-stats')
      } catch (error) {
        console.error('[Logs] Failed to fetch dashboard stats:', error)
        return {
          success: false,
          error: {
            code: 'LOGS_DASHBOARD_STATS_ERROR',
            message: '获取日志仪表板统计失败'
          }
        }
      }
    },

    /**
     * 获取日志列表
     */
    async getList(params?: {
      page?: number
      limit?: number
      search?: string
      method?: string
      status_code?: number
      is_success?: boolean
      model_used?: string
      provider_type_id?: number
      user_service_api_id?: number
      user_service_api_name?: string
      user_provider_key_name?: string
      start_time?: string
      end_time?: string
    }): Promise<ApiResponse<ProxyTraceListEntry[]>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.page !== undefined) queryParams.page = params.page.toString()
        if (params?.limit !== undefined) queryParams.limit = params.limit.toString()
        if (params?.search) queryParams.search = params.search
        if (params?.method) queryParams.method = params.method
        if (params?.status_code !== undefined) queryParams.status_code = params.status_code.toString()
        if (params?.is_success !== undefined) queryParams.is_success = params.is_success.toString()
        if (params?.model_used) queryParams.model_used = params.model_used
        if (params?.provider_type_id !== undefined) queryParams.provider_type_id = params.provider_type_id.toString()
        if (params?.user_service_api_id !== undefined) queryParams.user_service_api_id = params.user_service_api_id.toString()
        if (params?.user_service_api_name) queryParams.user_service_api_name = params.user_service_api_name
        if (params?.user_provider_key_name) queryParams.user_provider_key_name = params.user_provider_key_name
        if (params?.start_time) queryParams.start_time = params.start_time
        if (params?.end_time) queryParams.end_time = params.end_time

        return await apiClient.get<ProxyTraceListEntry[]>('/logs/traces', queryParams)
      } catch (error) {
        console.error('[Logs] Failed to fetch logs list:', error)
        return {
          success: false,
          error: {
            code: 'LOGS_LIST_ERROR',
            message: '获取日志列表失败'
          }
        }
      }
    },

    /**
     * 获取日志详情
     */
    async getDetail(id: number): Promise<ApiResponse<ProxyTraceEntry>> {
      try {
        return await apiClient.get<ProxyTraceEntry>(`/logs/traces/${id}`)
      } catch (error) {
        console.error('[Logs] Failed to fetch log detail:', error)
        return {
          success: false,
          error: {
            code: 'LOGS_DETAIL_ERROR',
            message: '获取日志详情失败'
          }
        }
      }
    },

    /**
     * 获取日志统计分析
     */
    async getAnalytics(params?: {
      time_range?: '1h' | '6h' | '24h' | '7d' | '30d'
      group_by?: 'hour' | 'day' | 'model' | 'provider' | 'status'
    }): Promise<ApiResponse<LogsAnalyticsResponse>> {
      try {
        const queryParams: Record<string, string> = {}
        if (params?.time_range) queryParams.time_range = params.time_range
        if (params?.group_by) queryParams.group_by = params.group_by

        return await apiClient.get<LogsAnalyticsResponse>('/logs/analytics', queryParams)
      } catch (error) {
        console.error('[Logs] Failed to fetch logs analytics:', error)
        return {
          success: false,
          error: {
            code: 'LOGS_ANALYTICS_ERROR',
            message: '获取日志统计分析失败'
          }
        }
      }
    }
  },

  system: {
    async getMetrics(): Promise<ApiResponse<SystemMetrics>> {
      try {
        return await apiClient.get<SystemMetrics>('/system/metrics')
      } catch (error) {
        console.error('[System] Failed to fetch metrics:', error)
        return {
          success: false,
          error: {
            code: 'SYSTEM_METRICS_ERROR',
            message: '获取系统指标失败'
          }
        }
      }
    }
  },
  
  // 通用请求方法
  get: <T = any>(endpoint: string, params?: Record<string, string>) => 
    apiClient.get<T>(endpoint, params),
  post: <T = any>(endpoint: string, body?: any) => 
    apiClient.post<T>(endpoint, body),
  put: <T = any>(endpoint: string, body?: any) => 
    apiClient.put<T>(endpoint, body),
  delete: <T = any>(endpoint: string) => 
    apiClient.delete<T>(endpoint),
}

export default api
