/**
 * useDashboardCards Hook
 * 管理仪表板卡片数据的获取、加载状态和错误处理
 * 更新：使用新的后端API接口，简化数据处理
 */

import { useState, useEffect, useCallback, useRef } from 'react'
import { api, type DashboardCardsResponse, type ApiResponse } from '../lib/api'

/**
 * 格式化数值显示
 */
const formatValue = (value: number, type: 'number' | 'percentage' | 'duration'): string => {
  if (!Number.isFinite(value)) return '--'
  
  switch (type) {
    case 'number':
      return value >= 1000 ? `${(value / 1000).toFixed(1)}k` : value.toLocaleString()
    case 'percentage':
      return `${value.toFixed(2)}%`
    case 'duration':
      return `${Math.round(value)} ms`
    default:
      return value.toString()
  }
}

// 处理后的指标卡片数据接口（保持UI组件兼容性）
export interface ProcessedDashboardCard {
  key: string
  label: string
  value: string
  delta: string
  icon: React.ReactNode
  color: string
  rawValue: number
  deltaValue: number
  isPositive: boolean
}

export interface UseDashboardCardsReturn {
  /** 处理后的卡片数据 */
  cards: ProcessedDashboardCard[]
  /** 是否正在加载 */
  isLoading: boolean
  /** 错误信息 */
  error: string | null
  /** 手动刷新数据 */
  refresh: () => Promise<void>
  /** 最后更新时间 */
  lastUpdated: Date | null
}

export function useDashboardCards(): UseDashboardCardsReturn {
  const [cards, setCards] = useState<ProcessedDashboardCard[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)
  
  // 使用ref防止重复请求
  const fetchingRef = useRef(false)
  const mountedRef = useRef(true)

  const fetchCards = useCallback(async () => {
    // 防止重复请求
    if (fetchingRef.current) return
    
    try {
      fetchingRef.current = true
      setIsLoading(true)
      setError(null)

      console.log('[useDashboardCards] Fetching dashboard cards...')
      
      const response: ApiResponse<DashboardCardsResponse> = await api.statistics.getDashboardCards()
      
      // 检查组件是否还mounted
      if (!mountedRef.current) return

      if (response.success && response.data) {
        console.log('[useDashboardCards] Cards fetched successfully:', response.data)
        
        // 转换后端数据为前端组件需要的格式
        const processedCards: ProcessedDashboardCard[] = [
          {
            key: 'requests',
            label: '今日请求数',
            value: formatValue(response.data.requests_today, 'number'),
            rawValue: response.data.requests_today,
            delta: response.data.rate_requests_today,
            deltaValue: parseFloat(response.data.rate_requests_today.replace(/[+%]/g, '')) || 0,
            isPositive: response.data.rate_requests_today.startsWith('+'),
            icon: null, // 将由组件设置
            color: '#7c3aed'
          },
          {
            key: 'tokens',
            label: '今日 Token 消耗',
            value: formatValue(response.data.tokens_today, 'number'),
            rawValue: response.data.tokens_today,
            delta: response.data.rate_tokens_today,
            deltaValue: parseFloat(response.data.rate_tokens_today.replace(/[+%]/g, '')) || 0,
            isPositive: response.data.rate_tokens_today.startsWith('+'),
            icon: null,
            color: '#0ea5e9'
          },
          {
            key: 'latency',
            label: '平均响应时间',
            value: formatValue(response.data.avg_response_time_today, 'duration'),
            rawValue: response.data.avg_response_time_today,
            delta: response.data.rate_avg_response_time_today,
            deltaValue: parseFloat(response.data.rate_avg_response_time_today.replace(/[+%]/g, '')) || 0,
            // 对于响应时间，降低是好事，所以反转正负判断
            isPositive: !response.data.rate_avg_response_time_today.startsWith('+'),
            icon: null,
            color: '#f59e0b'
          },
          {
            key: 'success',
            label: '成功率',
            value: formatValue(response.data.successes_today, 'percentage'),
            rawValue: response.data.successes_today,
            delta: response.data.rate_successes_today,
            deltaValue: parseFloat(response.data.rate_successes_today.replace(/[+%]/g, '')) || 0,
            isPositive: response.data.rate_successes_today.startsWith('+'),
            icon: null,
            color: '#10b981'
          }
        ]
        
        setCards(processedCards)
        setLastUpdated(new Date())
        setError(null)
      } else {
        const errorMessage = response.error?.message || '获取仪表板数据失败'
        console.error('[useDashboardCards] API error:', response.error)
        setError(errorMessage)
        
        // 在错误情况下提供默认数据，确保UI不会完全破坏
        if (cards.length === 0) {
          setCards(getDefaultCards())
        }
      }
    } catch (error) {
      console.error('[useDashboardCards] Fetch error:', error)
      
      if (!mountedRef.current) return
      
      const errorMessage = error instanceof Error ? error.message : '网络请求失败'
      setError(errorMessage)
      
      // 提供默认数据
      if (cards.length === 0) {
        setCards(getDefaultCards())
      }
    } finally {
      if (mountedRef.current) {
        setIsLoading(false)
      }
      fetchingRef.current = false
    }
  }, [])

  const refresh = useCallback(async () => {
    await fetchCards()
  }, [fetchCards])

  // 初始化数据获取
  useEffect(() => {
    fetchCards()
  }, [fetchCards])

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      mountedRef.current = false
    }
  }, [])

  return {
    cards,
    isLoading,
    error,
    refresh,
    lastUpdated
  }
}

/**
 * 提供默认的卡片数据，用于错误情况下的降级显示
 */
function getDefaultCards(): ProcessedDashboardCard[] {
  return [
    {
      key: 'requests',
      label: '今日请求数',
      value: '--',
      delta: '0%',
      icon: null,
      color: '#7c3aed',
      rawValue: 0,
      deltaValue: 0,
      isPositive: false
    },
    {
      key: 'tokens',
      label: '今日 Token 消耗',
      value: '--',
      delta: '0%',
      icon: null,
      color: '#0ea5e9',
      rawValue: 0,
      deltaValue: 0,
      isPositive: false
    },
    {
      key: 'latency',
      label: '平均响应时间',
      value: '-- ms',
      delta: '0%',
      icon: null,
      color: '#f59e0b',
      rawValue: 0,
      deltaValue: 0,
      isPositive: false
    },
    {
      key: 'success',
      label: '成功率',
      value: '--%',
      delta: '0%',
      icon: null,
      color: '#10b981',
      rawValue: 0,
      deltaValue: 0,
      isPositive: false
    }
  ]
}