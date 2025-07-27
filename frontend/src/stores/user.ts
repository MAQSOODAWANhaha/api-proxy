// 用户状态管理

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { AuthAPI } from '@/api'
import type { User, LoginRequest, RegisterRequest } from '@/types'
import { config } from '@/config'

export const useUserStore = defineStore('user', () => {
  // 状态
  const user = ref<User | null>(null)
  const token = ref<string>('')
  const isLoggedIn = ref<boolean>(false)
  const loginLoading = ref<boolean>(false)
  const permissions = ref<string[]>([])

  // 计算属性
  const isAdmin = computed(() => user.value?.role === 'admin')
  const userName = computed(() => user.value?.username || '')
  const userEmail = computed(() => user.value?.email || '')
  const userAvatar = computed(() => {
    // 生成用户头像（基于用户名首字母）
    if (user.value?.username) {
      return user.value.username.charAt(0).toUpperCase()
    }
    return 'U'
  })

  // 从localStorage初始化状态
  const initializeStore = () => {
    const savedToken = localStorage.getItem(config.auth.tokenKey)
    const savedUser = localStorage.getItem(config.auth.userInfoKey)
    
    if (savedToken && savedUser) {
      try {
        token.value = savedToken
        user.value = JSON.parse(savedUser)
        isLoggedIn.value = true
        // 验证token有效性
        validateToken()
      } catch (error) {
        console.error('Failed to parse saved user info:', error)
        clearUserData()
      }
    }
  }

  // 验证token有效性
  const validateToken = async () => {
    try {
      const result = await AuthAPI.validateToken()
      if (!result.valid) {
        clearUserData()
        return false
      }
      if (result.user) {
        user.value = result.user
      }
      return true
    } catch (error) {
      console.error('Token validation failed:', error)
      clearUserData()
      return false
    }
  }

  // 用户登录
  const login = async (loginData: LoginRequest): Promise<boolean> => {
    try {
      loginLoading.value = true
      
      const response = await AuthAPI.login(loginData)
      
      // 保存用户信息和token
      token.value = response.token
      user.value = response.user
      isLoggedIn.value = true
      
      // 持久化到localStorage
      localStorage.setItem(config.auth.tokenKey, response.token)
      localStorage.setItem(config.auth.userInfoKey, JSON.stringify(response.user))
      
      ElMessage.success('登录成功')
      return true
    } catch (error: any) {
      ElMessage.error(error.message || '登录失败')
      return false
    } finally {
      loginLoading.value = false
    }
  }

  // 用户注册
  const register = async (registerData: RegisterRequest): Promise<boolean> => {
    try {
      const response = await AuthAPI.register(registerData)
      ElMessage.success(response.message || '注册成功')
      return true
    } catch (error: any) {
      ElMessage.error(error.message || '注册失败')
      return false
    }
  }

  // 用户登出
  const logout = async (showMessage: boolean = true) => {
    try {
      await AuthAPI.logout()
    } catch (error) {
      console.error('Logout API failed:', error)
    } finally {
      clearUserData()
      if (showMessage) {
        ElMessage.success('已安全退出')
      }
    }
  }

  // 清除用户数据
  const clearUserData = () => {
    user.value = null
    token.value = ''
    isLoggedIn.value = false
    permissions.value = []
    
    // 清除localStorage数据
    localStorage.removeItem(config.auth.tokenKey)
    localStorage.removeItem(config.auth.userInfoKey)
    localStorage.removeItem(config.auth.refreshTokenKey)
  }

  // 刷新用户信息
  const refreshUserInfo = async (): Promise<boolean> => {
    try {
      const userInfo = await AuthAPI.getCurrentUser()
      user.value = userInfo
      
      // 更新localStorage
      localStorage.setItem(config.auth.userInfoKey, JSON.stringify(userInfo))
      
      return true
    } catch (error) {
      console.error('Failed to refresh user info:', error)
      return false
    }
  }

  // 修改密码
  const changePassword = async (oldPassword: string, newPassword: string): Promise<boolean> => {
    try {
      const response = await AuthAPI.changePassword({
        old_password: oldPassword,
        new_password: newPassword
      })
      ElMessage.success(response.message || '密码修改成功')
      return true
    } catch (error: any) {
      ElMessage.error(error.message || '密码修改失败')
      return false
    }
  }

  // 刷新token
  const refreshToken = async (): Promise<boolean> => {
    try {
      const response = await AuthAPI.refreshToken()
      token.value = response.token
      localStorage.setItem(config.auth.tokenKey, response.token)
      return true
    } catch (error) {
      console.error('Token refresh failed:', error)
      clearUserData()
      return false
    }
  }

  // 检查权限
  const hasPermission = (permission: string): boolean => {
    if (isAdmin.value) return true
    return permissions.value.includes(permission)
  }

  // 检查角色
  const hasRole = (role: string): boolean => {
    return user.value?.role === role
  }

  // 更新用户信息
  const updateUserInfo = (updates: Partial<User>) => {
    if (user.value) {
      user.value = { ...user.value, ...updates }
      localStorage.setItem(config.auth.userInfoKey, JSON.stringify(user.value))
    }
  }

  // 初始化store
  initializeStore()

  return {
    // 状态
    user: readonly(user),
    token: readonly(token),
    isLoggedIn: readonly(isLoggedIn),
    loginLoading: readonly(loginLoading),
    permissions: readonly(permissions),
    
    // 计算属性
    isAdmin,
    userName,
    userEmail,
    userAvatar,
    
    // 方法
    login,
    register,
    logout,
    clearUserData,
    refreshUserInfo,
    changePassword,
    refreshToken,
    validateToken,
    hasPermission,
    hasRole,
    updateUserInfo,
    initializeStore,
  }
})