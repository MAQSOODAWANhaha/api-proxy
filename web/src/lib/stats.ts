import { publicApiClient, type ApiResponse } from './api'
import type {
  StatsLogsQuery,
  StatsLogsResponse,
  StatsModelShareQuery,
  StatsModelShareResponse,
  StatsOverviewQuery,
  StatsOverviewResponse,
  StatsTrendQuery,
  StatsTrendResponse,
} from '@/types/stats'

/**
 * 统计API的基础端点
 * 所有这些端点都是公开的，不需要JWT认证，但需要user_service_key参数
 */
const BASE_ENDPOINT = '/stats'

/**
 * Stats查询参数的联合类型
 */
type StatsQueryParams =
  | StatsOverviewQuery
  | StatsTrendQuery
  | StatsModelShareQuery
  | StatsLogsQuery

/**
 * 清理查询参数，移除undefined和null值并转换为字符串
 *
 * @param params 查询参数对象
 * @returns 清理后的参数对象
 */
const sanitizeParams = (params: StatsQueryParams): Record<string, string> => {
  const entries = Object.entries(params)
    .filter(([, value]) => value !== undefined && value !== null)
    .map(([key, value]) => [key, String(value)])

  return Object.fromEntries(entries)
}

/**
 * 统计API客户端
 *
 * 提供对公开统计数据的访问接口，用于外部用户查询API使用情况
 * 所有接口都通过user_service_key进行身份验证，不需要JWT token
 */
export const statsApi = {
  /**
   * 获取统计数据概览
   *
   * @param params 查询参数，必须包含user_service_key
   * @returns 概览统计数据的Promise
   */
  async fetchOverview(params: StatsOverviewQuery): Promise<ApiResponse<StatsOverviewResponse>> {
    return publicApiClient.get<StatsOverviewResponse>(
      `${BASE_ENDPOINT}/overview`,
      sanitizeParams(params)
    )
  },

  /**
   * 获取趋势数据
   *
   * @param params 查询参数，必须包含user_service_key
   * @returns 趋势数据的Promise
   */
  async fetchTrend(params: StatsTrendQuery): Promise<ApiResponse<StatsTrendResponse>> {
    return publicApiClient.get<StatsTrendResponse>(
      `${BASE_ENDPOINT}/trend`,
      sanitizeParams(params)
    )
  },

  /**
   * 获取模型使用占比数据
   *
   * @param params 查询参数，必须包含user_service_key
   * @returns 模型占比数据的Promise
   */
  async fetchModelShare(
    params: StatsModelShareQuery
  ): Promise<ApiResponse<StatsModelShareResponse>> {
    return publicApiClient.get<StatsModelShareResponse>(
      `${BASE_ENDPOINT}/model-share`,
      sanitizeParams(params)
    )
  },

  /**
   * 获取详细日志数据
   *
   * @param params 查询参数，必须包含user_service_key
   * @returns 日志数据的Promise
   */
  async fetchLogs(params: StatsLogsQuery): Promise<ApiResponse<StatsLogsResponse>> {
    return publicApiClient.get<StatsLogsResponse>(
      `${BASE_ENDPOINT}/logs`,
      sanitizeParams(params)
    )
  },
}
