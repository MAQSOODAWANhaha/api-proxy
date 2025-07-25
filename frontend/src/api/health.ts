import type { AxiosPromise } from 'axios'
import request from './index'

export interface HealthStatus {
  id: number
  name: string
  provider: string
  isHealthy: boolean
  responseTime: number // in ms
  lastSuccess: string
  lastFailure: string | null
  errorMessage: string | null
}

// Mock data
const mockHealthStatus: HealthStatus[] = [
  { id: 1, name: 'My OpenAI Key 1', provider: 'OpenAI', isHealthy: true, responseTime: 120, lastSuccess: '2025-07-22 10:30:00', lastFailure: null, errorMessage: null },
  { id: 2, name: 'My Gemini Key', provider: 'Gemini', isHealthy: false, responseTime: 5000, lastSuccess: '2025-07-21 18:00:00', lastFailure: '2025-07-22 09:00:00', errorMessage: 'Request timed out' },
  { id: 3, name: 'Claude Sonnet Key', provider: 'Claude', isHealthy: true, responseTime: 250, lastSuccess: '2025-07-22 10:28:00', lastFailure: null, errorMessage: null },
  { id: 4, name: 'Backup OpenAI Key', provider: 'OpenAI', isHealthy: false, responseTime: 0, lastSuccess: '2025-07-20 12:00:00', lastFailure: '2025-07-22 08:30:00', errorMessage: 'Invalid API Key' },
]

export function getHealthStatuses(): AxiosPromise<HealthStatus[]> {
  return new Promise(async (resolve, reject) => {
    try {
      const response = await request({
        url: '/health/servers',
        method: 'get'
      })
      
      // Transform backend response to frontend format
      const healthData = response.data.servers || []
      const transformedData: HealthStatus[] = healthData.map((server: any, index: number) => ({
        id: index + 1,
        name: `${server.provider} Server`,
        provider: server.provider,
        isHealthy: server.is_healthy,
        responseTime: server.avg_response_time_ms || 0,
        lastSuccess: server.last_success_time || new Date().toISOString(),
        lastFailure: server.is_healthy ? null : server.last_failure_time,
        errorMessage: server.is_healthy ? null : server.error_message || 'Service unavailable'
      }))
      
      resolve({ data: transformedData } as any)
    } catch (error) {
      console.error('Failed to fetch health status:', error)
      // Return mock data as fallback
      resolve({ data: mockHealthStatus } as any)
    }
  })
}
