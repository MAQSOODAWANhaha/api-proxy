import { apiClient, type ApiResponse } from './api'
import type { StatsQuery, StatsResponse } from '@/types/stats'

const ENDPOINT = '/stats'

const sanitizeParams = (params: StatsQuery): Record<string, string> => {
  const entries = Object.entries(params).filter(([, value]) => value !== undefined && value !== null)
  return Object.fromEntries(entries.map(([key, value]) => [key, String(value)]))
}

export const statsApi = {
  async fetchStats(params: StatsQuery): Promise<ApiResponse<StatsResponse>> {
    return apiClient.get<StatsResponse>(ENDPOINT, sanitizeParams(params))
  },
}
