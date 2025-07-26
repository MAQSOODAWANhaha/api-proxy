import { http } from '@/utils/request'
import type { AxiosResponse } from 'axios'
import type { ApiResponse } from '@/utils/request'

// Define types for login
export interface LoginData {
  username: string
  password: string
}

export interface UserInfo {
  id: number
  username: string
  email: string
  is_admin: boolean
}

export interface LoginResponse {
  token: string
  user: UserInfo
}

/**
 * 用户登录
 */
export function login(data: LoginData): Promise<AxiosResponse<ApiResponse<LoginResponse>>> {
  return http.post('/auth/login', data, {
    showLoading: true,
    showSuccessMessage: true,
    successMessage: '登录成功',
    skipErrorHandler: false
  })
}

/**
 * 用户登出
 */
export function logout(): Promise<AxiosResponse<ApiResponse<void>>> {
  return http.post('/auth/logout', {}, {
    showSuccessMessage: true,
    successMessage: '退出成功',
    skipErrorHandler: false
  })
}

/**
 * 获取当前用户信息
 */
export function getCurrentUser(): Promise<AxiosResponse<ApiResponse<UserInfo>>> {
  return http.get('/auth/user', {
    skipErrorHandler: false
  })
}

/**
 * 刷新用户令牌
 */
export function refreshToken(): Promise<AxiosResponse<ApiResponse<{ token: string }>>> {
  return http.post('/auth/refresh', {}, {
    skipErrorHandler: false,
    retryable: false
  })
}
