// 认证相关API

import { HttpClient } from '@/utils/http'
import { MockDataService, useMockData } from '@/utils/mockData'
import type { LoginRequest, LoginResponse, RegisterRequest, User } from '@/types'

export class AuthAPI {
  // 用户登录
  static async login(data: LoginRequest): Promise<LoginResponse> {
    if (useMockData) {
      return MockDataService.login(data.username, data.password)
    }
    return HttpClient.post<LoginResponse>('/auth/login', data, {
      loadingText: '登录中...'
    })
  }

  // 用户注册
  static async register(data: RegisterRequest): Promise<{ success: boolean; user: User; message: string }> {
    return HttpClient.post('/auth/register', data, {
      loadingText: '注册中...'
    })
  }

  // 获取当前用户信息
  static async getCurrentUser(): Promise<User> {
    if (useMockData) {
      return MockDataService.getUser()
    }
    return HttpClient.get<User>('/auth/profile', undefined, {
      showLoading: false // 用户信息获取不显示loading
    })
  }

  // 刷新token (静默请求)
  static async refreshToken(): Promise<{ token: string }> {
    return HttpClient.post<{ token: string }>('/auth/refresh', undefined, {
      showLoading: false,
      showError: false // 静默刷新token
    })
  }

  // 用户登出
  static async logout(): Promise<{ success: boolean; message: string }> {
    return HttpClient.post('/auth/logout', undefined, {
      loadingText: '退出登录中...'
    })
  }

  // 修改密码
  static async changePassword(data: { old_password: string; new_password: string }): Promise<{ success: boolean; message: string }> {
    return HttpClient.put('/auth/password', data, {
      loadingText: '修改密码中...'
    })
  }

  // 验证token有效性 (静默请求)
  static async validateToken(): Promise<{ valid: boolean; user?: User }> {
    const result = await HttpClient.silentRequest('get', '/auth/validate')
    return result || { valid: false }
  }
}