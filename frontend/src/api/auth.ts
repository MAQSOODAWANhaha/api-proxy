import request from './index'
import type { AxiosPromise } from 'axios'

// Define types for login
interface LoginData {
  username: string
  password: string
}

interface UserInfo {
  id: number
  username: string
  email: string
  is_admin: boolean
}

interface LoginResponse {
  token: string
  user: UserInfo
}

// Login API call
export function login(data: LoginData): AxiosPromise<LoginResponse> {
  return request({
    url: '/auth/login',
    method: 'post',
    data,
  })
}

// Logout API call
export function logout(): AxiosPromise<void> {
  // For JWT-based auth, logout is typically handled client-side
  // by removing the token from storage
  return new Promise((resolve) => {
    resolve({} as any)
  })
}
