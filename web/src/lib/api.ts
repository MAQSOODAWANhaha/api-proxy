/**
 * API客户端工具
 * 统一处理HTTP请求，包括认证、错误处理等
 */

// API基础配置
const API_BASE_URL = process.env.NODE_ENV === 'production' 
  ? '/api'  // 生产环境使用相对路径，由 Caddy 转发
  : 'http://192.168.153.140:9090/api'  // 开发环境直连后端，WSL环境指定IP地址

// 请求配置接口
export interface RequestConfig {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH'
  headers?: Record<string, string>
  body?: any
  params?: Record<string, string>
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

// 新的统计接口响应类型定义
export interface ModelUsage {
  model: string
  usage: number
}

export interface ModelsRateResponse {
  model_usage: ModelUsage[]
}

export interface ModelStatistics {
  model: string
  usage: number
  percentage: number
  cost: string
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
  cost: string
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
    success: number
    failure: number
  }
  is_active: boolean
  last_used_at: string
  created_at: string
  expires_at?: string
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
  private buildHeaders(customHeaders?: Record<string, string>): Record<string, string> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...customHeaders,
    }

    if (this.token) {
      headers.Authorization = `Bearer ${this.token}`
    }

    return headers
  }

  /**
   * 通用请求方法
   */
  async request<T = any>(endpoint: string, config: RequestConfig = {}): Promise<ApiResponse<T>> {
    const {
      method = 'GET',
      headers: customHeaders,
      body,
      params,
    } = config

    const url = this.getURL(endpoint, params)
    const headers = this.buildHeaders(customHeaders)

    const requestConfig: RequestInit = {
      method,
      headers,
    }

    if (body && method !== 'GET') {
      requestConfig.body = JSON.stringify(body)
    }

    try {
      console.log(`[API] ${method} ${url}`, body ? { body } : '')
      
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
        
        console.error(`[API Error] ${method} ${url}:`, errorData)
        
        return {
          success: false,
          error: {
            code: `HTTP_${response.status}`,
            message: errorData.message || `Request failed with status ${response.status}`,
          },
        }
      }

      const data: ApiResponse<T> = await response.json()
      console.log(`[API Success] ${method} ${url}:`, data)
      
      return data
    } catch (error) {
      console.error(`[API Exception] ${method} ${url}:`, error)
      
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
   * GET请求
   */
  async get<T = any>(endpoint: string, params?: Record<string, string>): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'GET', params })
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
}

// 创建API客户端实例
export const apiClient = new ApiClient(API_BASE_URL)

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
  
  // Dashboard相关API
  dashboard: {
    /**
     * 获取仪表板卡片数据
     */
    async getCards(): Promise<ApiResponse<DashboardCardsResponse>> {
      try {
        return await apiClient.get<DashboardCardsResponse>('/statistics/today/cards')
      } catch (error) {
        console.error('[Dashboard] Failed to fetch cards:', error)
        return {
          success: false,
          error: {
            code: 'DASHBOARD_CARDS_ERROR',
            message: '获取仪表板数据失败'
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