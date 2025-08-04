// 用户管理相关API

import { HttpClient } from '@/utils/http'
import type { User, UserListParams, UserListResponse, PaginationParams } from '@/types'

export class UserAPI {
  // 获取用户列表
  static async getUsers(params: UserListParams = {}): Promise<UserListResponse> {
    return HttpClient.get<UserListResponse>('/users', params)
  }

  // 获取单个用户
  static async getUser(id: number): Promise<User> {
    return HttpClient.get<User>(`/users/${id}`)
  }

  // 创建用户
  static async createUser(data: {
    username: string
    email: string
    password: string
    role?: 'user' | 'admin'
  }): Promise<{
    success: boolean
    user: User
    message: string
  }> {
    return HttpClient.post('/users', data)
  }

  // 更新用户
  static async updateUser(id: number, data: {
    username?: string
    email?: string
    role?: 'user' | 'admin'
    status?: 'active' | 'inactive'
  }): Promise<{
    success: boolean
    user: User
    message: string
  }> {
    return HttpClient.put(`/users/${id}`, data)
  }

  // 删除用户
  static async deleteUser(id: number): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.delete(`/users/${id}`)
  }

  // 重置用户密码
  static async resetPassword(id: number, newPassword: string): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.put(`/users/${id}/password`, { password: newPassword })
  }

  // 启用/禁用用户
  static async toggleUserStatus(id: number, status: 'active' | 'inactive'): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.patch(`/users/${id}/status`, { status })
  }

  // 获取用户统计信息
  static async getUserStats(): Promise<{
    total_users: number
    active_users: number
    admin_users: number
    recent_logins: number
  }> {
    return HttpClient.get('/users/stats')
  }

  // ========== 用户中心相关API ==========

  // 获取个人资料
  static async getProfile(): Promise<User> {
    return HttpClient.get<User>('/user/profile')
  }

  // 更新个人资料
  static async updateProfile(data: {
    username?: string
    email?: string
  }): Promise<{
    success: boolean
    user: User
    message: string
  }> {
    return HttpClient.put('/user/profile', data)
  }

  // 上传头像
  static async uploadAvatar(file: File): Promise<{
    success: boolean
    avatar_url: string
    message: string
  }> {
    const formData = new FormData()
    formData.append('avatar', file)
    return HttpClient.upload('/user/avatar', formData)
  }

  // 获取安全设置
  static async getSecuritySettings(): Promise<{
    two_factor_enabled: boolean
    login_notifications: boolean
    security_alerts: boolean
  }> {
    return HttpClient.get('/user/security')
  }

  // 更新安全设置
  static async updateSecuritySettings(settings: {
    two_factor_enabled?: boolean
    login_notifications?: boolean
    security_alerts?: boolean
  }): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.put('/user/security', settings)
  }

  // 启用双因子认证
  static async enableTwoFactor(): Promise<{
    success: boolean
    qr_code: string
    secret: string
    message: string
  }> {
    return HttpClient.post('/user/2fa/enable')
  }

  // 禁用双因子认证
  static async disableTwoFactor(code: string): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.post('/user/2fa/disable', { code })
  }

  // 获取登录历史
  static async getLoginHistory(params: {
    page?: number
    limit?: number
  } = {}): Promise<{
    items: Array<{
      id: number
      ip_address: string
      user_agent: string
      location: string
      login_time: string
      is_current: boolean
    }>
    total: number
    page: number
    limit: number
  }> {
    return HttpClient.get('/user/login-history', params)
  }

  // 修改密码
  static async changePassword(data: {
    current_password: string
    new_password: string
  }): Promise<{
    success: boolean
    message: string
  }> {
    return HttpClient.post('/user/password', data)
  }
}