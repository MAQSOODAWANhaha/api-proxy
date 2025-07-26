/**
 * HTTP 请求工具 - 集成错误处理系统
 */

import axios, { 
  AxiosInstance, 
  AxiosRequestConfig, 
  AxiosResponse, 
  AxiosError,
  InternalAxiosRequestConfig
} from 'axios'

// 扩展 InternalAxiosRequestConfig 类型
declare module 'axios' {
  interface InternalAxiosRequestConfig {
    metadata?: {
      startTime: number
    }
  }
}
import { useUserStore } from '@/stores/user'
import { useConfigStore } from '@/stores/config'
import { handleError, createApiError, createNetworkError } from '@/utils/error'
import { notify } from '@/utils/notification'
import router from '@/router'

// 请求配置接口
export interface RequestConfig extends AxiosRequestConfig {
  skipErrorHandler?: boolean
  showLoading?: boolean
  showSuccessMessage?: boolean
  successMessage?: string
  retryable?: boolean
  maxRetries?: number
}

// 响应数据接口
export interface ApiResponse<T = any> {
  code: number
  message: string
  data: T
  success: boolean
  timestamp?: number
  requestId?: string
}

/**
 * 创建 axios 实例
 */
function createAxiosInstance(): AxiosInstance {
  const configStore = useConfigStore()
  
  const instance = axios.create({
    baseURL: configStore.getApiBaseUrl(),
    timeout: 30000,
    headers: {
      'Content-Type': 'application/json',
    }
  })

  // 请求拦截器
  instance.interceptors.request.use(
    (config: InternalAxiosRequestConfig) => {
      const userStore = useUserStore()
      const requestConfig = config as InternalAxiosRequestConfig & RequestConfig
      
      // 添加认证令牌
      if (userStore.token) {
        config.headers.Authorization = `Bearer ${userStore.token}`
      }
      
      // 显示加载状态
      if (requestConfig.showLoading) {
        notify.showLoading({
          text: '请求中...',
          target: requestConfig.showLoading === true ? undefined : requestConfig.showLoading as string
        })
      }
      
      // 添加请求时间戳
      config.metadata = {
        startTime: Date.now()
      }
      
      console.log(`🚀 API请求: ${config.method?.toUpperCase()} ${config.url}`, {
        params: config.params,
        data: config.data
      })
      
      return config
    },
    (error: AxiosError) => {
      console.error('请求拦截器错误:', error)
      handleError(error, 'request-interceptor')
      return Promise.reject(error)
    }
  )

  // 响应拦截器
  instance.interceptors.response.use(
    (response: AxiosResponse) => {
      const requestConfig = response.config as InternalAxiosRequestConfig & RequestConfig
      const duration = Date.now() - (response.config.metadata?.startTime || 0)
      
      // 隐藏加载状态
      if (requestConfig.showLoading) {
        notify.hideLoading()
      }
      
      console.log(`✅ API响应: ${response.config.method?.toUpperCase()} ${response.config.url} (${duration}ms)`, {
        status: response.status,
        data: response.data
      })
      
      // 显示成功消息
      if (requestConfig.showSuccessMessage) {
        notify.successMessage(
          requestConfig.successMessage || '操作成功'
        )
      }
      
      // 检查业务状态码
      const apiResponse = response.data as ApiResponse
      if (apiResponse && typeof apiResponse.code === 'number') {
        if (apiResponse.code !== 200 && apiResponse.code !== 0) {
          const error = createApiError({
            status: apiResponse.code,
            statusText: apiResponse.message || 'API业务错误',
            data: apiResponse,
            config: response.config
          })
          
          if (!requestConfig.skipErrorHandler) {
            handleError(error, 'api-business-error', {
              showNotification: true,
              showMessage: false
            })
          }
          
          return Promise.reject(error)
        }
      }
      
      return response
    },
    async (error: AxiosError) => {
      const requestConfig = error.config as InternalAxiosRequestConfig & RequestConfig
      const duration = Date.now() - (error.config?.metadata?.startTime || 0)
      
      // 隐藏加载状态
      if (requestConfig?.showLoading) {
        notify.hideLoading()
      }
      
      console.error(`❌ API错误: ${error.config?.method?.toUpperCase()} ${error.config?.url} (${duration}ms)`, {
        status: error.response?.status,
        message: error.message,
        response: error.response?.data
      })
      
      // 跳过错误处理的请求
      if (requestConfig?.skipErrorHandler) {
        return Promise.reject(error)
      }
      
      // 处理不同类型的错误
      if (error.response) {
        // 服务器响应错误
        const apiError = createApiError(error.response)
        
        // 特殊状态码处理
        switch (error.response.status) {
          case 401:
            // 未授权，清除用户信息并重定向到登录页
            const userStore = useUserStore()
            userStore.logout()
            
            notify.warningMessage('登录已过期，请重新登录')
            
            // 避免在登录页面重复重定向
            if (router.currentRoute.value.path !== '/login') {
              router.push(`/login?redirect=${encodeURIComponent(router.currentRoute.value.fullPath)}`)
            }
            break
            
          case 403:
            // 权限不足
            router.push('/error/403')
            break
            
          case 404:
            // 资源不存在
            if (error.config?.url?.includes('/api/')) {
              notify.warningMessage('请求的资源不存在')
            }
            break
            
          case 429:
            // 请求过于频繁
            notify.warningMessage('请求过于频繁，请稍后重试')
            break
            
          case 500:
          case 502:
          case 503:
          case 504:
            // 服务器错误
            notify.error('服务器错误，请稍后重试', {
              duration: 6000
            })
            break
        }
        
        // 处理错误
        await handleError(apiError, 'api-response-error', {
          showNotification: error.response.status >= 500,
          showMessage: error.response.status < 500 && error.response.status !== 401,
          logError: true,
          reportError: error.response.status >= 500
        })
        
        return Promise.reject(apiError)
      } else if (error.request) {
        // 网络错误
        const networkError = createNetworkError(error)
        
        await handleError(networkError, 'network-error', {
          showNotification: true,
          showMessage: false,
          logError: true,
          reportError: false
        })
        
        return Promise.reject(networkError)
      } else {
        // 其他错误
        await handleError(error, 'request-setup-error')
        return Promise.reject(error)
      }
    }
  )

  return instance
}

// 创建默认实例
const request = createAxiosInstance()

// 重试机制
async function requestWithRetry<T = any>(
  config: RequestConfig,
  retryCount = 0
): Promise<AxiosResponse<T>> {
  const maxRetries = config.maxRetries || 3
  
  try {
    return await request(config)
  } catch (error) {
    const isRetryable = config.retryable !== false && 
      (error as any)?.code === 'NETWORK_ERROR' ||
      (error as any)?.response?.status >= 500
    
    if (isRetryable && retryCount < maxRetries) {
      console.warn(`请求重试 ${retryCount + 1}/${maxRetries}:`, config.url)
      
      // 指数退避重试
      const delay = Math.min(1000 * Math.pow(2, retryCount), 10000)
      await new Promise(resolve => setTimeout(resolve, delay))
      
      return requestWithRetry(config, retryCount + 1)
    }
    
    throw error
  }
}

// 请求方法封装
export const http = {
  get<T = any>(url: string, config?: RequestConfig): Promise<AxiosResponse<T>> {
    return requestWithRetry({ ...config, method: 'GET', url })
  },
  
  post<T = any>(url: string, data?: any, config?: RequestConfig): Promise<AxiosResponse<T>> {
    return requestWithRetry({ ...config, method: 'POST', url, data })
  },
  
  put<T = any>(url: string, data?: any, config?: RequestConfig): Promise<AxiosResponse<T>> {
    return requestWithRetry({ ...config, method: 'PUT', url, data })
  },
  
  delete<T = any>(url: string, config?: RequestConfig): Promise<AxiosResponse<T>> {
    return requestWithRetry({ ...config, method: 'DELETE', url })
  },
  
  patch<T = any>(url: string, data?: any, config?: RequestConfig): Promise<AxiosResponse<T>> {
    return requestWithRetry({ ...config, method: 'PATCH', url, data })
  },
  
  upload<T = any>(url: string, formData: FormData, config?: RequestConfig): Promise<AxiosResponse<T>> {
    return requestWithRetry({
      ...config,
      method: 'POST',
      url,
      data: formData,
      headers: {
        'Content-Type': 'multipart/form-data',
        ...config?.headers
      }
    })
  }
}

// 取消请求相关
export const CancelToken = axios.CancelToken
export const isCancel = axios.isCancel

// 导出实例和类型
export default request
export type { RequestConfig, ApiResponse, RequestConfig as HttpRequestConfig, ApiResponse as HttpApiResponse }