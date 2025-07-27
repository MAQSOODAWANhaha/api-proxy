// 模拟数据服务
// 当后端API不可用时，提供模拟数据进行前端测试

import type { 
  User, 
  LoginResponse, 
  ApiKey, 
  Statistics, 
  SystemInfo, 
  HealthCheckResponse,
  RequestLog 
} from '@/types'

// 模拟用户数据
export const mockUser: User = {
  id: 1,
  username: 'admin',
  email: 'admin@example.com',
  role: 'admin',
  status: 'active',
  created_at: '2025-01-01T00:00:00Z',
  last_login: '2025-07-27T01:00:00Z',
  avatar: 'https://via.placeholder.com/150',
  last_login_at: '2025-07-27T01:00:00Z',
  is_active: true
}

// 模拟登录响应
export const mockLoginResponse: LoginResponse = {
  token: 'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.test.token',
  user: mockUser
}

// 模拟API密钥数据
export const mockApiKeys: ApiKey[] = [
  {
    id: 1,
    name: 'OpenAI GPT-4',
    provider: 'openai',
    api_key: 'sk-****************************',
    status: 'active',
    usage_count: 15420,
    rate_limit: 10000,
    created_at: '2025-01-15T08:30:00Z',
    last_used: '2025-07-27T00:45:00Z'
  },
  {
    id: 2,
    name: 'Claude 3.5 Sonnet',
    provider: 'anthropic',
    api_key: 'sk-ant-**********************',
    status: 'active',
    usage_count: 8750,
    rate_limit: 5000,
    created_at: '2025-02-01T10:15:00Z',
    last_used: '2025-07-26T23:30:00Z'
  },
  {
    id: 3,
    name: 'Google Gemini Pro',
    provider: 'google',
    api_key: 'AIza**********************',
    status: 'inactive',
    usage_count: 3200,
    rate_limit: 8000,
    created_at: '2025-03-10T14:20:00Z',
    last_used: '2025-07-25T16:12:00Z'
  }
]

// 模拟统计数据
export const mockStatistics: Statistics = {
  requests: {
    total: 1250000,
    today: 8500,
    success_rate: 96.8,
    avg_response_time: 245
  },
  providers: {
    openai: {
      requests: 650000,
      success_rate: 97.2,
      avg_response_time: 220
    },
    anthropic: {
      requests: 450000,
      success_rate: 96.5,
      avg_response_time: 280
    },
    google: {
      requests: 150000,
      success_rate: 95.8,
      avg_response_time: 195
    }
  },
  trends: {
    labels: ['07-20', '07-21', '07-22', '07-23', '07-24', '07-25', '07-26', '07-27'],
    requests: [7200, 8100, 7800, 8500, 9200, 8800, 8600, 8500],
    success_rates: [96.5, 97.1, 96.8, 97.3, 96.2, 96.9, 97.0, 96.8]
  }
}

// 模拟系统信息
export const mockSystemInfo: SystemInfo = {
  service: {
    name: 'AI Proxy',
    version: '0.1.0',
    build_time: '2025-07-20T10:00:00Z',
    git_commit: 'abc123def456'
  },
  runtime: {
    uptime_seconds: 86400 * 7, // 7天
    rust_version: '1.70.0',
    target: 'x86_64-unknown-linux-gnu',
    os_info: 'Ubuntu 22.04 LTS',
    arch: 'x86_64'
  },
  configuration: {
    server_port: 8080,
    https_port: 8443,
    workers: 4,
    database_url: 'sqlite://proxy.db'
  }
}

// 模拟健康检查响应
export const mockHealthCheck: HealthCheckResponse = {
  status: 'healthy',
  timestamp: new Date().toISOString(),
  details: {
    healthy_servers: 8,
    total_servers: 10,
    avg_response_time_ms: 245
  }
}

// 模拟请求日志
export const mockRequestLogs: RequestLog[] = Array.from({ length: 50 }, (_, i) => ({
  id: i + 1,
  timestamp: new Date(Date.now() - i * 60000).toISOString(),
  method: ['POST', 'GET'][Math.floor(Math.random() * 2)] as 'GET' | 'POST',
  path: ['/v1/chat/completions', '/v1/completions', '/v1/embeddings'][Math.floor(Math.random() * 3)],
  status_code: [200, 200, 200, 400, 500][Math.floor(Math.random() * 5)],
  response_time_ms: Math.floor(Math.random() * 1000) + 100,
  provider: ['openai', 'anthropic', 'google'][Math.floor(Math.random() * 3)],
  api_key_id: Math.floor(Math.random() * 3) + 1,
  user_agent: 'Mozilla/5.0 (compatible; API Client)',
  ip_address: `192.168.1.${Math.floor(Math.random() * 254) + 1}`,
  request_size: Math.floor(Math.random() * 10000) + 500,
  response_size: Math.floor(Math.random() * 50000) + 1000,
  error_message: Math.random() > 0.8 ? 'Rate limit exceeded' : undefined
}))

// 模拟API服务器检查是否可用
export const checkApiAvailability = async (): Promise<boolean> => {
  try {
    const response = await fetch(`${import.meta.env.VITE_API_BASE_URL || 'http://localhost:9090/api'}/health`, {
      method: 'GET',
      timeout: 5000
    } as any)
    return response.ok
  } catch {
    return false
  }
}

// 模拟数据获取器
export class MockDataService {
  static async getUser(): Promise<User> {
    await this.delay(300)
    return mockUser
  }

  static async login(username: string, password: string): Promise<LoginResponse> {
    await this.delay(800)
    if (username === 'admin') {
      return mockLoginResponse
    }
    throw new Error('Invalid credentials')
  }

  static async getApiKeys(): Promise<ApiKey[]> {
    await this.delay(500)
    return mockApiKeys
  }

  static async getStatistics(): Promise<Statistics> {
    await this.delay(600)
    return mockStatistics
  }

  static async getSystemInfo(): Promise<SystemInfo> {
    await this.delay(400)
    return mockSystemInfo
  }

  static async getHealthCheck(): Promise<HealthCheckResponse> {
    await this.delay(200)
    return mockHealthCheck
  }

  static async getRequestLogs(params?: any): Promise<{
    items: RequestLog[]
    total: number
    page: number
    limit: number
  }> {
    await this.delay(700)
    const page = params?.page || 1
    const limit = params?.limit || 20
    const start = (page - 1) * limit
    const end = start + limit
    
    return {
      items: mockRequestLogs.slice(start, end),
      total: mockRequestLogs.length,
      page,
      limit
    }
  }

  private static delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms))
  }
}

// 自动检测API可用性并决定是否使用模拟数据
export let useMockData = false

export const initializeMockData = async (): Promise<void> => {
  const isApiAvailable = await checkApiAvailability()
  useMockData = !isApiAvailable
  
  if (useMockData) {
    console.warn('🔧 后端API不可用，使用模拟数据进行前端测试')
  } else {
    console.log('✅ 后端API可用，使用真实数据')
  }
}