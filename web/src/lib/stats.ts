import { apiClient, type ApiResponse } from './api'
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

const BASE_ENDPOINT = '/stats'

const sanitizeParams = (
  params: StatsOverviewQuery | StatsTrendQuery | StatsModelShareQuery | StatsLogsQuery
): Record<string, string> => {
  const entries = Object.entries(params).filter(([, value]) => value !== undefined && value !== null)
  return Object.fromEntries(entries.map(([key, value]) => [key, String(value)]))
}

export const statsApi = {
  async fetchOverview(params: StatsOverviewQuery): Promise<ApiResponse<StatsOverviewResponse>> {
    return apiClient.get<StatsOverviewResponse>(`${BASE_ENDPOINT}/overview`, sanitizeParams(params))
  },

  async fetchTrend(params: StatsTrendQuery): Promise<ApiResponse<StatsTrendResponse>> {
    return apiClient.get<StatsTrendResponse>(`${BASE_ENDPOINT}/trend`, sanitizeParams(params))
  },

  async fetchModelShare(
    params: StatsModelShareQuery
  ): Promise<ApiResponse<StatsModelShareResponse>> {
    return apiClient.get<StatsModelShareResponse>(
      `${BASE_ENDPOINT}/model-share`,
      sanitizeParams(params)
    )
  },

  async fetchLogs(params: StatsLogsQuery): Promise<ApiResponse<StatsLogsResponse>> {
    return apiClient.get<StatsLogsResponse>(`${BASE_ENDPOINT}/logs`, sanitizeParams(params))
  },
}
