// 用户相关类型定义

export interface User {
  id: number
  username: string
  email: string
  role: 'admin' | 'user'
  status: 'active' | 'inactive'
  created_at: string
  last_login?: string
  avatar?: string
  last_login_at?: string
  is_active?: boolean
}

export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  token: string
  user: User
}

export interface RegisterRequest {
  username: string
  email: string
  password: string
  role?: 'user' | 'admin'
}

export interface ChangePasswordRequest {
  old_password: string
  new_password: string
}

export interface UserListParams {
  page?: number
  limit?: number
  status?: 'active' | 'inactive'
}

export interface UserListResponse {
  users: User[]
  pagination: {
    page: number
    limit: number
    total: number
    pages: number
  }
}