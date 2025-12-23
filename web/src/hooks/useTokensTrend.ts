/**
 * useTokensTrend Hook
 * 管理Token使用趋势数据的获取、加载状态和错误处理
 */

import { useState, useEffect, useCallback, useRef } from 'react'
import { api, type TokensTrendResponse, type ApiResponse } from '../lib/api'
import { logger } from '../lib/logger'

export interface UseTokensTrendReturn {
  /** Token使用趋势数据 */
  tokensTrend: TokensTrendResponse | null
  /** 是否正在加载 */
  isLoading: boolean
  /** 错误信息 */
  error: string | null
  /** 手动刷新数据 */
  refresh: () => Promise<void>
  /** 最后更新时间 */
  lastUpdated: Date | null
}

export function useTokensTrend(): UseTokensTrendReturn {
  const [tokensTrend, setTokensTrend] = useState<TokensTrendResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)
  
  // 使用ref防止重复请求
  const fetchingRef = useRef(false)
  const mountedRef = useRef(true)

  const fetchTokensTrend = useCallback(async () => {
    // 防止重复请求
    if (fetchingRef.current) return
    
    try {
      fetchingRef.current = true
      setIsLoading(true)
      setError(null)

      logger.debug('[useTokensTrend] Fetching tokens trend data...')
      
      const response: ApiResponse<TokensTrendResponse> = await api.statistics.getTokensTrend()
      
      // 检查组件是否还mounted
      if (!mountedRef.current) return

      if (response.success && response.data) {
        logger.debug('[useTokensTrend] Tokens trend data fetched successfully:', response.data)
        setTokensTrend(response.data)
        setLastUpdated(new Date())
        setError(null)
      } else {
        const errorMessage = response.error?.message || '获取Token使用趋势失败'
        console.error('[useTokensTrend] API error:', response.error)
        setError(errorMessage)
        setTokensTrend(null)
      }
    } catch (error) {
      console.error('[useTokensTrend] Fetch error:', error)
      
      if (!mountedRef.current) return
      
      const errorMessage = error instanceof Error ? error.message : '网络请求失败'
      setError(errorMessage)
      setTokensTrend(null)
    } finally {
      if (mountedRef.current) {
        setIsLoading(false)
      }
      fetchingRef.current = false
    }
  }, [])

  const refresh = useCallback(async () => {
    await fetchTokensTrend()
  }, [fetchTokensTrend])

  // 初始化数据获取
  useEffect(() => {
    fetchTokensTrend()
  }, [fetchTokensTrend])

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      mountedRef.current = false
    }
  }, [])

  return {
    tokensTrend,
    isLoading,
    error,
    refresh,
    lastUpdated
  }
}
