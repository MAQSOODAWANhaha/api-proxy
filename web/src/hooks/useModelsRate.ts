/**
 * useModelsRate Hook
 * 管理模型使用占比数据的获取、加载状态和错误处理
 */

import { useState, useEffect, useCallback, useRef } from 'react'
import { api, type ModelsRateResponse, type ApiResponse } from '../lib/api'

export interface UseModelsRateReturn {
  /** 模型使用占比数据 */
  modelsRate: ModelsRateResponse | null
  /** 是否正在加载 */
  isLoading: boolean
  /** 错误信息 */
  error: string | null
  /** 手动刷新数据 */
  refresh: () => Promise<void>
  /** 最后更新时间 */
  lastUpdated: Date | null
}

export function useModelsRate(range: string = '7days', start?: string, end?: string): UseModelsRateReturn {
  const [modelsRate, setModelsRate] = useState<ModelsRateResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)
  
  // 使用ref防止重复请求
  const fetchingRef = useRef(false)
  const mountedRef = useRef(true)

  const fetchModelsRate = useCallback(async () => {
    // 防止重复请求
    if (fetchingRef.current) return
    
    try {
      fetchingRef.current = true
      setIsLoading(true)
      setError(null)

      console.log('[useModelsRate] Fetching models rate data...')
      
      const response: ApiResponse<ModelsRateResponse> = await api.statistics.getModelsRate(range, start, end)
      
      // 检查组件是否还mounted
      if (!mountedRef.current) return

      if (response.success && response.data) {
        console.log('[useModelsRate] Models rate data fetched successfully:', response.data)
        setModelsRate(response.data)
        setLastUpdated(new Date())
        setError(null)
      } else {
        const errorMessage = response.error?.message || '获取模型使用占比失败'
        console.error('[useModelsRate] API error:', response.error)
        setError(errorMessage)
        setModelsRate(null)
      }
    } catch (error) {
      console.error('[useModelsRate] Fetch error:', error)
      
      if (!mountedRef.current) return
      
      const errorMessage = error instanceof Error ? error.message : '网络请求失败'
      setError(errorMessage)
      setModelsRate(null)
    } finally {
      if (mountedRef.current) {
        setIsLoading(false)
      }
      fetchingRef.current = false
    }
  }, [range, start, end])

  const refresh = useCallback(async () => {
    await fetchModelsRate()
  }, [fetchModelsRate])

  // 初始化数据获取和参数变化时重新获取
  useEffect(() => {
    fetchModelsRate()
  }, [fetchModelsRate])

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      mountedRef.current = false
    }
  }, [])

  return {
    modelsRate,
    isLoading,
    error,
    refresh,
    lastUpdated
  }
}