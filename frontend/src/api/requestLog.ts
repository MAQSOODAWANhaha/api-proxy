import type { AxiosPromise } from 'axios'

export interface RequestLog {
  id: string
  path: string
  statusCode: number
  responseTime: number // in ms
  modelUsed: string
  totalTokens: number
  createdAt: string
}

// Helper to generate random logs
const generateMockLogs = (count: number): RequestLog[] => {
  const logs: RequestLog[] = []
  for (let i = 0; i < count; i++) {
    logs.push({
      id: `req-${Math.random().toString(36).substring(2, 15)}`,
      path: '/v1/chat/completions',
      statusCode: [200, 200, 200, 401, 500][Math.floor(Math.random() * 5)],
      responseTime: Math.floor(Math.random() * 1000) + 50,
      modelUsed: ['gpt-4', 'gemini-pro', 'claude-3-sonnet'][Math.floor(Math.random() * 3)],
      totalTokens: Math.floor(Math.random() * 2000) + 100,
      createdAt: new Date(Date.now() - Math.random() * 1000 * 3600 * 24).toISOString(),
    })
  }
  return logs
}

const allLogs = generateMockLogs(100)

export function getRequestLogs(params: { page: number, limit: number, statusCode?: number }): AxiosPromise<{ logs: RequestLog[], total: number }> {
  return new Promise((resolve) => {
    setTimeout(() => {
      let filteredLogs = allLogs
      if (params.statusCode) {
        filteredLogs = allLogs.filter(log => log.statusCode === params.statusCode)
      }
      const start = (params.page - 1) * params.limit
      const end = start + params.limit
      const paginatedLogs = filteredLogs.slice(start, end)
      resolve({ data: { logs: paginatedLogs, total: filteredLogs.length } } as any)
    }, 300)
  })
}
