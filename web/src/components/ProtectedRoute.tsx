/**
 * ProtectedRoute.tsx
 * 受保护路由包装：根据认证状态决定渲染子路由或跳转登录页。
 * 支持自动token验证和加载状态
 */

import React, { useEffect, useState } from 'react'
import { Navigate, Outlet, useLocation } from 'react-router'
import { useAuthStore } from '../store/auth'
import { LoadingSpinner } from '@/components/ui/loading'

/**
 * ProtectedRoute
 * - 自动验证token有效性
 * - 已登录：渲染 <Outlet />
 * - 未登录：跳转 /login
 * - 验证中：显示加载状态
 */
const ProtectedRoute: React.FC = () => {
  const { isAuthenticated, token, validateToken } = useAuthStore()
  const location = useLocation()
  const [isValidating, setIsValidating] = useState(false)

  useEffect(() => {
    // 如果有token但未认证，尝试验证token
    if (token && !isAuthenticated) {
      setIsValidating(true)
      validateToken().finally(() => {
        setIsValidating(false)
      })
    }
  }, [token, isAuthenticated, validateToken])

  // 正在验证token，显示加载状态
  if (isValidating) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <LoadingSpinner size="lg" tone="primary" />
          <p className="text-sm text-neutral-600">验证登录状态...</p>
        </div>
      </div>
    )
  }

  // 未登录，跳转到登录页
  if (!isAuthenticated) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />
  }

  // 已登录，渲染子路由
  return <Outlet />
}

export default ProtectedRoute
