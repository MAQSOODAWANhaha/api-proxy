// HTTP客户端封装

import axios, { type AxiosInstance, type AxiosRequestConfig, type AxiosResponse, type AxiosError } from 'axios'
import { ElMessage, ElMessageBox } from 'element-plus'
import { config } from '@/config'
import { errorHandler } from './errorHandler'
import { loadingManager } from './loading'
import type { ApiResponse, ErrorResponse } from '@/types'

// 扩展AxiosRequestConfig类型
declare module 'axios' {
  interface AxiosRequestConfig {
    metadata?: {
      requestKey?: string
      loadingId?: string
    }
  }
}

// 活跃的请求映射（用于取消重复请求）
const pendingRequests = new Map<string, AbortController>()

// 创建axios实例
const http: AxiosInstance = axios.create({
  baseURL: config.api.baseURL,
  timeout: config.api.timeout,
  headers: {
    'Content-Type': 'application/json',
  },
})

// 生成请求唯一标识
const generateRequestKey = (config: AxiosRequestConfig): string => {
  const { method, url, params, data } = config
  return `${method}:${url}:${JSON.stringify(params)}:${JSON.stringify(data)}`
}

// 取消重复请求
const cancelDuplicateRequest = (config: AxiosRequestConfig) => {
  const requestKey = generateRequestKey(config)
  
  if (pendingRequests.has(requestKey)) {
    const controller = pendingRequests.get(requestKey)
    controller?.abort('Duplicate request cancelled')
    pendingRequests.delete(requestKey)
  }
  
  const controller = new AbortController()
  config.signal = controller.signal
  pendingRequests.set(requestKey, controller)
  
  return requestKey
}

// 移除已完成的请求
const removePendingRequest = (requestKey: string) => {
  pendingRequests.delete(requestKey)
}

// 请求拦截器
http.interceptors.request.use(
  (config: AxiosRequestConfig) => {
    // 添加认证token
    const token = localStorage.getItem('api_proxy_token')
    if (token && config.headers) {
      config.headers.Authorization = `Bearer ${token}`
    }

    // 处理重复请求（可选，通过配置控制）
    if (config.headers?.['X-Cancel-Duplicate'] !== 'false') {
      const requestKey = cancelDuplicateRequest(config)
      config.metadata = { requestKey }
    }
    
    // 根据配置显示全局 loading
    if (config.headers?.['X-Show-Loading'] === 'true') {
      // 从metadata中获取loadingText，避免在HTTP头中传递非ASCII字符
      const loadingText = (config as any).loadingText || 'Loading...'
      const loadingId = loadingManager.showRequestLoading(loadingText)
      config.metadata = { ...config.metadata, loadingId }
    }

    // 添加请求ID（用于调试）
    if (import.meta.env.DEV) {
      const requestId = `req_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
      if (config.headers) {
        config.headers['X-Request-ID'] = requestId
      }
      console.log(`[HTTP Request] ${config.method?.toUpperCase()} ${config.url}`, {
        requestId,
        config,
      })
    }

    return config
  },
  (error: AxiosError) => {
    console.error('[HTTP Request Error]', error)
    return Promise.reject(error)
  }
)

// 响应拦截器
http.interceptors.response.use(
  (response: AxiosResponse) => {
    // 移除已完成的请求
    const requestKey = (response.config as any).metadata?.requestKey
    if (requestKey) {
      removePendingRequest(requestKey)
    }
    
    // 隐藏 loading
    const loadingId = (response.config as any).metadata?.loadingId
    if (loadingId) {
      loadingManager.hide(loadingId)
    }

    if (import.meta.env.DEV) {
      console.log(`[HTTP Response] ${response.config.method?.toUpperCase()} ${response.config.url}`, {
        status: response.status,
        data: response.data,
      })
    }

    // 直接返回响应数据
    return response
  },
  (error: AxiosError<ErrorResponse>) => {
    // 移除已完成的请求
    const requestKey = (error.config as any)?.metadata?.requestKey
    if (requestKey) {
      removePendingRequest(requestKey)
    }
    
    // 隐藏 loading
    const loadingId = (error.config as any)?.metadata?.loadingId
    if (loadingId) {
      loadingManager.hide(loadingId)
    }
    
    // 忽略已取消的请求
    if (axios.isCancel(error)) {
      return Promise.reject(error)
    }

    // 网络错误处理
    if (!error.response) {
      if (error.code === 'ECONNABORTED' || error.message.includes('timeout')) {
        errorHandler.handleNetworkError({
          type: 'timeout',
          message: '请求超时',
          originalError: error
        })
      } else {
        errorHandler.handleNetworkError({
          type: 'network',
          message: '网络连接失败',
          originalError: error
        })
      }
    } else {
      // API 错误处理
      const showError = (error.config as any)?.headers?.['X-Show-Error'] !== 'false'
      errorHandler.handleApiError(error, { 
        showMessage: showError,
        showNotification: false
      })
    }

    // 开发环境下打印详细错误信息
    if (import.meta.env.DEV) {
      console.error('[HTTP Response Error]', {
        status: error.response?.status,
        url: error.config?.url,
        method: error.config?.method,
        data: error.response?.data,
        error,
      })
    }

    return Promise.reject(error)
  }
)

// 扩展请求配置类型
interface ExtendedRequestConfig extends AxiosRequestConfig {
  showLoading?: boolean
  loadingText?: string
  showError?: boolean
  cancelDuplicate?: boolean
}

// 通用请求方法
export class HttpClient {
  // GET请求
  static async get<T = any>(url: string, params?: any, config?: ExtendedRequestConfig): Promise<T> {
    const response = await http.get(url, { 
      params, 
      ...this.buildRequestConfig(config) 
    })
    return response.data
  }

  // POST请求
  static async post<T = any>(url: string, data?: any, config?: ExtendedRequestConfig): Promise<T> {
    const response = await http.post(url, data, this.buildRequestConfig(config))
    return response.data
  }

  // PUT请求
  static async put<T = any>(url: string, data?: any, config?: ExtendedRequestConfig): Promise<T> {
    const response = await http.put(url, data, this.buildRequestConfig(config))
    return response.data
  }

  // DELETE请求
  static async delete<T = any>(url: string, params?: any, config?: ExtendedRequestConfig): Promise<T> {
    const response = await http.delete(url, { 
      params, 
      ...this.buildRequestConfig(config) 
    })
    return response.data
  }

  // PATCH请求
  static async patch<T = any>(url: string, data?: any, config?: ExtendedRequestConfig): Promise<T> {
    const response = await http.patch(url, data, this.buildRequestConfig(config))
    return response.data
  }

  // 带加载状态的请求
  static async requestWithLoading<T = any>(
    method: 'get' | 'post' | 'put' | 'delete' | 'patch',
    url: string,
    data?: any,
    loadingText = 'Loading...'
  ): Promise<T> {
    return this[method]<T>(url, data, {
      showLoading: true,
      loadingText
    })
  }

  // 静默请求（不显示错误信息）
  static async silentRequest<T = any>(
    method: 'get' | 'post' | 'put' | 'delete' | 'patch',
    url: string,
    data?: any
  ): Promise<T | null> {
    try {
      return await this[method]<T>(url, data, {
        showError: false
      })
    } catch {
      return null
    }
  }

  // 构建请求配置
  private static buildRequestConfig(config?: ExtendedRequestConfig): AxiosRequestConfig {
    if (!config) return {}

    const headers: any = { ...config.headers }

    // 设置加载状态
    if (config.showLoading !== false) {
      headers['X-Show-Loading'] = 'true'
      // 不将loadingText放入headers，避免非ASCII字符导致的ISO-8859-1错误
      // loadingText将在拦截器中使用，而不是作为HTTP头传递
    }

    // 设置错误显示
    if (config.showError === false) {
      headers['X-Show-Error'] = 'false'
    }

    // 设置重复请求取消
    if (config.cancelDuplicate === false) {
      headers['X-Cancel-Duplicate'] = 'false'
    }

    return {
      ...config,
      headers,
      // 保存loadingText到配置对象中，而不是headers
      loadingText: config.loadingText
    }
  }

  // 上传文件
  static async upload<T = any>(
    url: string, 
    formData: FormData, 
    onUploadProgress?: (progressEvent: any) => void,
    config?: ExtendedRequestConfig
  ): Promise<T> {
    const response = await http.post(url, formData, {
      ...this.buildRequestConfig(config),
      headers: {
        'Content-Type': 'multipart/form-data',
        ...this.buildRequestConfig(config).headers
      },
      onUploadProgress,
    })
    return response.data
  }

  // 下载文件
  static async download(
    url: string, 
    params?: any, 
    filename?: string,
    config?: ExtendedRequestConfig
  ): Promise<void> {
    const response = await http.get(url, {
      params,
      responseType: 'blob',
      ...this.buildRequestConfig(config)
    })

    const blob = new Blob([response.data])
    const link = document.createElement('a')
    link.href = window.URL.createObjectURL(blob)
    link.download = filename || 'download'
    link.click()
    window.URL.revokeObjectURL(link.href)
  }
}

export default http