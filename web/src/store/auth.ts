/**
 * auth.ts
 * 全局认证状态管理：支持真实的API认证和token管理
 */

import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { api, type LoginRequest } from '../lib/api'

/** 用户信息接口 */
export interface User {
  id: number
  username: string
  email: string
  is_admin: boolean
}

/** 认证状态接口 */
export interface AuthState {
  /** 是否通过认证 */
  isAuthenticated: boolean
  /** 当前用户信息 */
  user: User | null
  /** 认证token */
  token: string | null
  /** 刷新token */
  refreshToken: string | null
  /** 登录加载状态 */
  isLoading: boolean
  /** 错误信息 */
  error: string | null
  /** token刷新中状态 */
  isRefreshing: boolean

  /** 登录方法 */
  login: (credentials: LoginRequest) => Promise<boolean>
  /** 登出方法 */
  logout: (callApi?: boolean) => Promise<void>
  /** 验证token */
  validateToken: () => Promise<boolean>
  /** 刷新token */
  refreshAccessToken: () => Promise<boolean>
  /** 清除错误 */
  clearError: () => void
  /** 设置加载状态 */
  setLoading: (loading: boolean) => void
}

/**
 * 全局认证Store
 * 使用Zustand持久化中间件保存认证状态
 */
export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      isAuthenticated: false,
      user: null,
      token: null,
      refreshToken: null,
      isLoading: false,
      error: null,
      isRefreshing: false,

      /**
       * 用户登录
       */
      login: async (credentials: LoginRequest): Promise<boolean> => {
        set({ isLoading: true, error: null })

        try {
          const response = await api.login(credentials)

          if (response.success && response.data) {
            const { token, refresh_token, user } = response.data

            set({
              isAuthenticated: true,
              user,
              token,
              refreshToken: refresh_token,
              isLoading: false,
              error: null,
            })

            console.log('Login successful:', user)
            return true
          } else {
            const errorMessage = response.error?.message || '登录失败'
            set({
              isAuthenticated: false,
              user: null,
              token: null,
              refreshToken: null,
              isLoading: false,
              error: errorMessage,
            })

            console.error('Login failed:', errorMessage)
            return false
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : '网络错误'
          set({
            isAuthenticated: false,
            user: null,
            token: null,
            refreshToken: null,
            isLoading: false,
            error: errorMessage,
          })

          console.error('Login exception:', error)
          return false
        }
      },

      /**
       * 用户登出
       */
      logout: async (callApi = true): Promise<void> => {
        set({ isLoading: true })

        if (callApi) {
          try {
            // 调用后端登出API
            await api.logout()
          } catch (error) {
            console.error('Logout API error:', error)
            // 即使API调用失败，也要清除本地状态
          }
        }

        // 清除所有认证状态
        set({
          isAuthenticated: false,
          user: null,
          token: null,
          refreshToken: null,
          isLoading: false,
          error: null,
          isRefreshing: false,
        })

        console.log('Logout completed')
      },

      /**
       * 验证token有效性
       */
      validateToken: async (): Promise<boolean> => {
        const { token } = get()

        if (!token) {
          set({
            isAuthenticated: false,
            user: null,
            token: null,
            refreshToken: null,
          })
          return false
        }

        try {
          const response = await api.validateToken()

          if (response.success && response.data?.valid && response.data.user) {
            set({
              isAuthenticated: true,
              user: response.data.user,
              error: null,
            })

            console.log('Token validation successful:', response.data.user)
            return true
          } else {
            set({
              isAuthenticated: false,
              user: null,
              token: null,
              refreshToken: null,
              error: null,
            })

            console.log('Token validation failed')
            return false
          }
        } catch (error) {
          console.error('Token validation error:', error)
          set({
            isAuthenticated: false,
            user: null,
            token: null,
            refreshToken: null,
            error: null,
          })
          return false
        }
      },

      /**
       * 刷新access token
       */
      refreshAccessToken: async (): Promise<boolean> => {
        const { refreshToken: currentRefreshToken } = get()

        if (!currentRefreshToken) {
          console.log('No refresh token available')
          await get().logout(false)
          return false
        }

        set({ isRefreshing: true, error: null })

        try {
          const response = await api.refreshToken(currentRefreshToken)

          if (response.success && response.data) {
            const { access_token } = response.data

            set({
              token: access_token,
              isRefreshing: false,
              error: null,
            })

            console.log('Token refresh successful')
            return true
          } else {
            const errorMessage = response.error?.message || '刷新令牌失败'
            set({
              isAuthenticated: false,
              user: null,
              token: null,
              refreshToken: null,
              isRefreshing: false,
              error: errorMessage,
            })

            console.error('Token refresh failed:', errorMessage)
            return false
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : '网络错误'
          set({
            isAuthenticated: false,
            user: null,
            token: null,
            refreshToken: null,
            isRefreshing: false,
            error: errorMessage,
          })

          console.error('Token refresh exception:', error)
          return false
        }
      },

      /**
       * 清除错误信息
       */
      clearError: () => set({ error: null }),

      /**
       * 设置加载状态
       */
      setLoading: (loading: boolean) => set({ isLoading: loading }),
    }),
    {
      name: 'auth-storage', // localStorage key
      partialize: (state) => ({
        isAuthenticated: state.isAuthenticated,
        user: state.user,
        token: state.token,
        refreshToken: state.refreshToken,
      }),
    }
  )
)
