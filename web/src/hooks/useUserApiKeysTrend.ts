/**
 * useUserApiKeysTrend Hook
 * 管理用户API Keys趋势数据的获取、加载状态和错误处理
 * 支持请求趋势和Token趋势两种数据类型
 */

import { useState, useEffect, useCallback, useRef } from 'react'
import { api, type UserApiKeysRequestTrendResponse, type UserApiKeysTokenTrendResponse, type ApiResponse } from '../lib/api'
import { logger } from '../lib/logger'

export type TrendType = 'request' | 'token'

export interface UseUserApiKeysTrendReturn {
  /** 用户API Keys趋势数据 */
  trendData: UserApiKeysRequestTrendResponse | UserApiKeysTokenTrendResponse | null
  /** 是否正在加载 */
  isLoading: boolean
  /** 错误信息 */
  error: string | null
  /** 手动刷新数据 */
  refresh: () => Promise<void>
  /** 最后更新时间 */
  lastUpdated: Date | null
  /** 切换趋势类型 */
  switchTrendType: (type: TrendType) => void
  /** 当前趋势类型 */
  currentType: TrendType
}

export function useUserApiKeysTrend(initialType: TrendType = 'request'): UseUserApiKeysTrendReturn {
  const [trendData, setTrendData] = useState<UserApiKeysRequestTrendResponse | UserApiKeysTokenTrendResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)
  const [currentType, setCurrentType] = useState<TrendType>(initialType)
  
  // 使用ref防止重复请求
  const fetchingRef = useRef(false)
  const mountedRef = useRef(true)

  const fetchTrendData = useCallback(async (type: TrendType) => {
    // 防止重复请求
    if (fetchingRef.current) return
    
    try {
      fetchingRef.current = true
      setIsLoading(true)
      setError(null)

      logger.debug(`[useUserApiKeysTrend] Fetching ${type} trend data...`)
      
      let response: ApiResponse<UserApiKeysRequestTrendResponse | UserApiKeysTokenTrendResponse>
      
      if (type === 'request') {
        response = await api.statistics.getUserApiKeysRequestTrend()
      } else {
        response = await api.statistics.getUserApiKeysTokenTrend()
      }
      
      // 检查组件是否还mounted
      if (!mountedRef.current) return

      if (response.success && response.data) {
        logger.debug(`[useUserApiKeysTrend] ${type} trend data fetched successfully:`, response.data)
        setTrendData(response.data)
        setLastUpdated(new Date())
        setError(null)
      } else {
        const errorMessage = response.error?.message || `获取${type === 'request' ? '请求' : 'Token'}趋势失败`
        console.error(`[useUserApiKeysTrend] API error:`, response.error)
        setError(errorMessage)
        setTrendData(null)
      }
    } catch (error) {
      console.error(`[useUserApiKeysTrend] Fetch error:`, error)
      
      if (!mountedRef.current) return
      
      const errorMessage = error instanceof Error ? error.message : '网络请求失败'
      setError(errorMessage)
      setTrendData(null)
    } finally {
      if (mountedRef.current) {
        setIsLoading(false)
      }
      fetchingRef.current = false
    }
  }, [])

  const refresh = useCallback(async () => {
    await fetchTrendData(currentType)
  }, [fetchTrendData, currentType])

  const switchTrendType = useCallback((type: TrendType) => {
    setCurrentType(type)
  }, [])

  // 初始化数据获取和类型变化时重新获取
  useEffect(() => {
    fetchTrendData(currentType)
  }, [fetchTrendData, currentType])

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      mountedRef.current = false
    }
  }, [])

  return {
    trendData,
    isLoading,
    error,
    refresh,
    lastUpdated,
    switchTrendType,
    currentType
  }
}
