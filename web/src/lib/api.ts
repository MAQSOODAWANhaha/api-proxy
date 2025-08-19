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