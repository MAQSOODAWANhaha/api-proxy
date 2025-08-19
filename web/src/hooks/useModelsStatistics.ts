/**
 * useModelsStatistics Hook
 * 管理模型详细统计数据的获取、加载状态和错误处理
 */

import { useState, useEffect, useCallback, useRef } from 'react'
import { api, type ModelsStatisticsResponse, type ApiResponse } from '../lib/api'

export interface UseModelsStatisticsReturn {
  /** 模型详细统计数据 */
  modelsStatistics: ModelsStatisticsResponse | null
  /** 是否正在加载 */
  isLoading: boolean
  /** 错误信息 */
  error: string | null
  /** 手动刷新数据 */
  refresh: () => Promise<void>
  /** 最后更新时间 */
  lastUpdated: Date | null
}

export function useModelsStatistics(range: string = '7days', start?: string, end?: string): UseModelsStatisticsReturn {
  const [modelsStatistics, setModelsStatistics] = useState<ModelsStatisticsResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)
  
  // 使用ref防止重复请求
  const fetchingRef = useRef(false)
  const mountedRef = useRef(true)

  const fetchModelsStatistics = useCallback(async () => {
    // 防止重复请求
    if (fetchingRef.current) return
    
    try {
      fetchingRef.current = true
      setIsLoading(true)
      setError(null)

      console.log('[useModelsStatistics] Fetching models statistics data...')
      
      const response: ApiResponse<ModelsStatisticsResponse> = await api.statistics.getModelsStatistics(range, start, end)
      
      // 检查组件是否还mounted
      if (!mountedRef.current) return

      if (response.success && response.data) {
        console.log('[useModelsStatistics] Models statistics data fetched successfully:', response.data)
        setModelsStatistics(response.data)
        setLastUpdated(new Date())
        setError(null)
      } else {
        const errorMessage = response.error?.message || '获取模型详细统计失败'
        console.error('[useModelsStatistics] API error:', response.error)
        setError(errorMessage)
        setModelsStatistics(null)
      }
    } catch (error) {
      console.error('[useModelsStatistics] Fetch error:', error)
      
      if (!mountedRef.current) return
      
      const errorMessage = error instanceof Error ? error.message : '网络请求失败'
      setError(errorMessage)
      setModelsStatistics(null)
    } finally {
      if (mountedRef.current) {
        setIsLoading(false)
      }
      fetchingRef.current = false
    }
  }, [range, start, end])

  const refresh = useCallback(async () => {
    await fetchModelsStatistics()
  }, [fetchModelsStatistics])

  // 初始化数据获取和参数变化时重新获取
  useEffect(() => {
    fetchModelsStatistics()
  }, [fetchModelsStatistics])

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      mountedRef.current = false
    }
  }, [])

  return {
    modelsStatistics,
    isLoading,
    error,
    refresh,
    lastUpdated
  }
}