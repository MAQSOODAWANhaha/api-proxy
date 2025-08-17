/**
 * auth.ts
 * 全局认证状态：是否已登录及登录/登出方法。
 */

import { create } from 'zustand'

/** 认证状态接口 */
export interface AuthState {
  /** 是否通过认证（默认 true，避免首屏白屏） */
  isAuthenticated: boolean
  /** 登录 */
  login: () => void
  /** 登出 */
  logout: () => void
}

/**
 * 全局 Auth Store
 * 默认 isAuthenticated = true，保证受保护路由可正常进入。
 */
export const useAuthStore = create<AuthState>((set) => ({
  isAuthenticated: true,
  login: () => set({ isAuthenticated: true }),
  logout: () => set({ isAuthenticated: false }),
}))
