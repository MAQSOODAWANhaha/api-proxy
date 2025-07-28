// API密钥管理相关API

import { HttpClient } from '@/utils/http'
import { MockDataService, useMockData } from '@/utils/mockData'
import type {
  UserProviderKey,
  UserServiceApi,
  CreateProviderKeyRequest,
  CreateServiceApiRequest,
  ApiKeyListParams,
  ApiKeyListResponse,
  ApiHealthStatus,
  ProviderType,
  SchedulingStrategy
} from '@/types'

export class ApiKeyAPI {
  // ===== 用户内部API密钥池管理 =====

  // 获取用户的内部API密钥列表
  static async getProviderKeys(params: ApiKeyListParams = {}): Promise<{
    keys: UserProviderKey[]
    pagination: any
  }> {
    if (useMockData) {
      const apiKeys = await MockDataService.getApiKeys()
      return {
        keys: apiKeys as any,
        pagination: {
          page: params.page || 1,
          limit: params.limit || 20,
          total: apiKeys.length,
          pages: Math.ceil(apiKeys.length / (params.limit || 20))
        }
      }
    }
    return HttpClient.get('/api-keys/provider', params)
  }

  // 获取单个内部API密钥
  static async getProviderKey(id: number): Promise<UserProviderKey> {
    return HttpClient.get(`/api-keys/provider/${id}`)
  }

  // 创建内部API密钥
  static async createProviderKey(data: CreateProviderKeyRequest): Promise<{
    success: boolean
    key: UserProviderKey
    message: string
  }> {
    return HttpClient.post('/api-keys/provider', data)
  }

  // 更新内部API密钥
  static async updateProviderKey(id: number, data: {
    name?: string
    api_key?: string
    weight?: number
    max_requests_per_minute?: number
    max_tokens_per_day?: number
    is_active?: boolean
  }): Promise<{
    success: boolean
    key?: UserProviderKey
    message: string
  }> {
    return HttpClient.put(`/api-keys/provider/${id}`, data)
  }

  // 删除内部API密钥
  static async deleteProviderKey(id: number): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.delete(`/api-keys/provider/${id}`)
  }

  // 切换内部API密钥状态
  static async toggleProviderKeyStatus(id: number, is_active: boolean): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.patch(`/api-keys/provider/${id}/status`, { is_active })
  }

  // ===== 用户对外API服务管理 =====

  // 获取用户的对外API服务列表  
  static async getServiceApis(params: ApiKeyListParams = {}): Promise<ApiKeyListResponse> {
    return HttpClient.get('/api-keys/service', params)
  }

  // 获取单个对外API服务
  static async getServiceApi(id: number): Promise<UserServiceApi> {
    return HttpClient.get(`/api-keys/service/${id}`)
  }

  // 创建对外API服务
  static async createServiceApi(data: CreateServiceApiRequest): Promise<{
    success: boolean
    api: UserServiceApi
    message: string
  }> {
    return HttpClient.post('/api-keys/service', data)
  }

  // 更新对外API服务
  static async updateServiceApi(id: number, data: {
    name?: string
    description?: string
    scheduling_strategy?: SchedulingStrategy
    retry_count?: number
    timeout_seconds?: number
    rate_limit?: number
    max_tokens_per_day?: number
    is_active?: boolean
  }): Promise<{
    success: boolean
    api: UserServiceApi
    message: string
  }> {
    return HttpClient.put(`/api-keys/service/${id}`, data)
  }

  // 删除对外API服务
  static async deleteServiceApi(id: number): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.delete(`/api-keys/service/${id}`)
  }

  // 重新生成对外API密钥
  static async regenerateServiceApiKey(id: number): Promise<{
    success: boolean
    api_key: string
    message: string
  }> {
    return HttpClient.post(`/api-keys/service/${id}/regenerate`)
  }

  // 撤销对外API密钥
  static async revokeServiceApi(id: number): Promise<{
    success: boolean
    message: string
    revoked_at: string
  }> {
    return HttpClient.post(`/api-keys/service/${id}/revoke`)
  }

  // ===== 健康状态和统计 =====

  // 获取API密钥健康状态
  static async getHealthStatus(params: {
    user_id?: number
    provider_type?: ProviderType
    healthy?: boolean
  } = {}): Promise<{
    statuses: (ApiHealthStatus & {
      key_name: string
      provider_name: string
    })[]
    summary: {
      total: number
      healthy: number
      unhealthy: number
    }
  }> {
    return HttpClient.get('/api-keys/health', params)
  }

  // 手动触发健康检查
  static async triggerHealthCheck(keyId: number): Promise<{
    success: boolean
    result: ApiHealthStatus
    message: string
  }> {
    return HttpClient.post(`/api-keys/provider/${keyId}/health-check`)
  }

  // 获取API密钥使用统计
  static async getKeyUsageStats(keyId: number, params: {
    start_date?: string
    end_date?: string
    group_by?: 'hour' | 'day'
  } = {}): Promise<{
    usage: Array<{
      timestamp: string
      requests: number
      tokens: number
      success_rate: number
    }>
    summary: {
      total_requests: number
      total_tokens: number
      avg_response_time: number
    }
  }> {
    return HttpClient.get(`/api-keys/provider/${keyId}/usage`, params)
  }

  // ===== 其他辅助功能 =====

  // 获取支持的服务商类型
  static async getProviderTypes(): Promise<ProviderType[]> {
    return HttpClient.get('/api-keys/provider-types')
  }

  // 获取调度策略列表
  static async getSchedulingStrategies(): Promise<Array<{
    key: SchedulingStrategy
    name: string
    description: string
  }>> {
    return HttpClient.get('/api-keys/scheduling-strategies')
  }

  // 测试API密钥连通性
  static async testProviderKey(keyId: number): Promise<{
    success: boolean
    response_time: number
    status: 'healthy' | 'unhealthy'
    message: string
    details?: any
  }> {
    return HttpClient.post(`/api-keys/provider/${keyId}/test`)
  }
}