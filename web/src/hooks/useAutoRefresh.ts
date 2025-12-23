/**
 * useAutoRefresh.ts
 * 自动刷新令牌的React Hook
 */

import { useEffect } from 'react'
import { useAuthStore } from '../store/auth'
import { logger } from '../lib/logger'
import { parseJwt } from '../lib/jwt'

/**
 * 自动刷新令牌的Hook
 * @param refreshThreshold 刷新阈值（毫秒），默认提前5分钟刷新
 */
export const useAutoRefresh = (refreshThreshold: number = 5 * 60 * 1000) => {
  const { isAuthenticated, token, refreshToken: refreshTokenValue, isRefreshing, refreshAccessToken } = useAuthStore()

  useEffect(() => {
    if (!isAuthenticated || !token || !refreshTokenValue || isRefreshing) {
      return
    }

    const jwtPayload = parseJwt(token)
    if (!jwtPayload || !jwtPayload.exp) {
      logger.warn('Invalid JWT token format')
      return
    }

    const expirationTime = jwtPayload.exp * 1000 // 转换为毫秒
    const now = Date.now()
    const timeUntilExpiration = expirationTime - now

    // 如果token即将过期，触发刷新
    if (timeUntilExpiration > 0 && timeUntilExpiration <= refreshThreshold) {
      logger.debug('Token即将过期，自动刷新...')
      refreshAccessToken()
    }

    // 设置定时器，在刷新阈值时触发刷新
    if (timeUntilExpiration > refreshThreshold) {
      const timeoutId = setTimeout(() => {
        logger.debug('定时触发token刷新...')
        refreshAccessToken()
      }, timeUntilExpiration - refreshThreshold)

      // 清理定时器
      return () => clearTimeout(timeoutId)
    }
  }, [isAuthenticated, token, refreshTokenValue, isRefreshing, refreshAccessToken, refreshThreshold])

  // 设置每分钟检查一次token状态的定时器
  useEffect(() => {
    if (!isAuthenticated) {
      return
    }

    const intervalId = setInterval(() => {
      const { token, refreshToken: currentRefreshToken, isRefreshing, refreshAccessToken } = useAuthStore.getState()

      if (!token || !currentRefreshToken || isRefreshing) {
        return
      }

      const jwtPayload = parseJwt(token)
      if (!jwtPayload || !jwtPayload.exp) {
        return
      }

      const expirationTime = jwtPayload.exp * 1000
      const now = Date.now()
      const timeUntilExpiration = expirationTime - now

      // 如果在刷新阈值内，触发刷新
      if (timeUntilExpiration > 0 && timeUntilExpiration <= refreshThreshold) {
        logger.debug('定时检查：Token即将过期，触发刷新...')
        refreshAccessToken()
      }
    }, 60000) // 每分钟检查一次

    return () => clearInterval(intervalId)
  }, [isAuthenticated, refreshThreshold])
}
