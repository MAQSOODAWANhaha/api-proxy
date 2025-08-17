/**
 * ProtectedRoute.tsx
 * 受保护路由包装：根据认证状态决定渲染子路由或跳转登录页。
 */

import React from 'react'
import { Navigate, Outlet, useLocation } from 'react-router'
import { useAuthStore } from '../store/auth'

/**
 * ProtectedRoute
 * - 已登录：渲染 <Outlet />
 * - 未登录：跳转 /login
 */
const ProtectedRoute: React.FC = () => {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const location = useLocation()

  if (!isAuthenticated) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />
  }
  return <Outlet />
}

export default ProtectedRoute
