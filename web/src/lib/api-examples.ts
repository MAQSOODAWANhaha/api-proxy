/**
 * API使用示例
 *
 * 展示如何正确使用不同类型的API客户端
 */

import { apiClient, publicApiClient } from './api'
import { statsApi } from './stats'

/**
 * 示例1: 使用需要认证的API客户端
 * 用于访问需要JWT token的内部接口
 */
export async function exampleProtectedApi() {
  try {
    // 获取用户档案（需要认证）
    const profileResponse = await apiClient.users.getProfile()
    if (profileResponse.success) {
      console.log('用户档案:', profileResponse.data)
    }

    // 获取API密钥列表（需要认证）
    const keysResponse = await apiClient.userService.getKeys()
    if (keysResponse.success) {
      console.log('API密钥列表:', keysResponse.data)
    }
  } catch (error) {
    console.error('认证API调用失败:', error)
  }
}

/**
 * 示例2: 使用公开API客户端
 * 用于访问不需要JWT认证的公开接口
 */
export async function examplePublicApi() {
  try {
    // 直接使用publicApiClient调用公开接口
    const response = await publicApiClient.get('/stats/overview', {
      user_service_key: 'your_user_service_key_here',
      from: '2024-01-01T00:00:00Z',
      to: '2024-01-31T23:59:59Z'
    })

    if (response.success) {
      console.log('统计数据:', response.data)
    }
  } catch (error) {
    console.error('公开API调用失败:', error)
  }
}

/**
 * 示例3: 使用专门的统计API
 * 这是最推荐的方式，有完整的类型安全和文档
 */
export async function exampleStatsApi() {
  try {
    // 获取概览数据
    const overview = await statsApi.fetchOverview({
      user_service_key: 'your_user_service_key_here',
      from: '2024-01-01T00:00:00Z',
      to: '2024-01-31T23:59:59Z'
    })

    // 获取趋势数据
    const trend = await statsApi.fetchTrend({
      user_service_key: 'your_user_service_key_here',
      timeframe: '7d'
    })

    // 获取模型占比
    const modelShare = await statsApi.fetchModelShare({
      user_service_key: 'your_user_service_key_here',
      include_today: true
    })

    // 获取日志数据
    const logs = await statsApi.fetchLogs({
      user_service_key: 'your_user_service_key_here',
      page: 1,
      page_size: 20,
      search: 'gpt-4'
    })

    console.log('统计数据汇总:', {
      overview: overview.success ? overview.data : overview.error,
      trend: trend.success ? trend.data : trend.error,
      modelShare: modelShare.success ? modelShare.data : modelShare.error,
      logs: logs.success ? logs.data : logs.error
    })
  } catch (error) {
    console.error('统计API调用失败:', error)
  }
}

/**
 * 示例4: 错误处理最佳实践
 */
export async function exampleWithErrorHandling() {
  try {
    const response = await statsApi.fetchOverview({
      user_service_key: 'your_user_service_key_here',
      from: '2024-01-01T00:00:00Z',
      to: '2024-01-31T23:59:59Z'
    })

    if (!response.success) {
      // 根据错误码进行不同处理
      switch (response.error?.code) {
        case 'AUTHENTICATION_FAILED':
          console.error('用户服务密钥无效，请检查配置')
          break
        case 'HTTP_400':
          console.error('请求参数错误:', response.error.message)
          break
        case 'NETWORK_ERROR':
          console.error('网络连接失败，请检查网络设置')
          break
        default:
          console.error('未知错误:', response.error?.message)
      }
      return
    }

    // 处理成功响应
    console.log('获取统计数据成功:', response.data)
  } catch (error) {
    console.error('异常错误:', error)
  }
}