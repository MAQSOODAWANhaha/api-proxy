import { ElMessage, ElNotification } from 'element-plus'
import type { AxiosError } from 'axios'
import router from '@/router'

// 错误类型定义
export interface ApiError {
  code: string
  message: string
  details?: any
  timestamp?: string
}

export interface NetworkError {
  type: 'network' | 'timeout' | 'abort'
  message: string
  originalError?: Error
}

export interface ValidationError {
  field: string
  message: string
  value?: any
}

// 错误处理配置
export interface ErrorHandlerConfig {
  showMessage?: boolean
  showNotification?: boolean
  logToConsole?: boolean
  reportToServer?: boolean
  redirectOnAuth?: boolean
}

const defaultConfig: ErrorHandlerConfig = {
  showMessage: true,
  showNotification: false,
  logToConsole: true,
  reportToServer: false,
  redirectOnAuth: true
}

class ErrorHandler {
  private config: ErrorHandlerConfig

  constructor(config: Partial<ErrorHandlerConfig> = {}) {
    this.config = { ...defaultConfig, ...config }
  }

  // 处理 API 错误
  handleApiError(error: AxiosError, customConfig?: Partial<ErrorHandlerConfig>) {
    const config = { ...this.config, ...customConfig }
    const response = error.response

    if (config.logToConsole) {
      console.error('API Error:', error)
    }

    // 处理不同的 HTTP 状态码
    switch (response?.status) {
      case 400:
        this.handleBadRequest(error, config)
        break
      case 401:
        this.handleUnauthorized(error, config)
        break
      case 403:
        this.handleForbidden(error, config)
        break
      case 404:
        this.handleNotFound(error, config)
        break
      case 429:
        this.handleTooManyRequests(error, config)
        break
      case 500:
      case 502:
      case 503:
      case 504:
        this.handleServerError(error, config)
        break
      default:
        this.handleGenericError(error, config)
    }

    // 上报错误到服务器
    if (config.reportToServer) {
      this.reportErrorToServer(error)
    }
  }

  // 处理网络错误
  handleNetworkError(error: NetworkError, customConfig?: Partial<ErrorHandlerConfig>) {
    const config = { ...this.config, ...customConfig }

    if (config.logToConsole) {
      console.error('Network Error:', error)
    }

    let message = '网络连接失败'
    let type: 'error' | 'warning' = 'error'

    switch (error.type) {
      case 'timeout':
        message = '请求超时，请检查网络连接'
        type = 'warning'
        break
      case 'abort':
        message = '请求被取消'
        type = 'warning'
        break
      case 'network':
        message = '网络连接异常，请检查网络设置'
        break
    }

    if (config.showMessage) {
      ElMessage({
        type,
        message,
        duration: 5000
      })
    }

    if (config.showNotification) {
      ElNotification({
        type,
        title: '网络错误',
        message,
        duration: 8000
      })
    }
  }

  // 处理表单验证错误
  handleValidationErrors(errors: ValidationError[], customConfig?: Partial<ErrorHandlerConfig>) {
    const config = { ...this.config, ...customConfig }

    if (config.logToConsole) {
      console.error('Validation Errors:', errors)
    }

    const errorMessages = errors.map(error => `${error.field}: ${error.message}`).join('\n')

    if (config.showMessage) {
      ElMessage({
        type: 'warning',
        message: errorMessages,
        duration: 5000
      })
    }

    if (config.showNotification) {
      ElNotification({
        type: 'warning',
        title: '表单验证失败',
        message: errorMessages,
        duration: 8000
      })
    }

    return errors
  }

  // 处理 400 错误
  private handleBadRequest(error: AxiosError, config: ErrorHandlerConfig) {
    const data = error.response?.data as any
    let message = '请求参数错误'

    if (data?.message) {
      message = data.message
    } else if (data?.errors && Array.isArray(data.errors)) {
      // 处理验证错误
      this.handleValidationErrors(data.errors, config)
      return
    }

    this.showError(message, config)
  }

  // 处理 401 未授权错误
  private handleUnauthorized(error: AxiosError, config: ErrorHandlerConfig) {
    const message = '登录已过期，请重新登录'
    
    this.showError(message, config)

    if (config.redirectOnAuth) {
      // 清除用户信息并跳转到登录页
      localStorage.removeItem('token')
      localStorage.removeItem('user')
      
      setTimeout(() => {
        router.push('/login')
      }, 1500)
    }
  }

  // 处理 403 禁止访问错误
  private handleForbidden(error: AxiosError, config: ErrorHandlerConfig) {
    const message = '没有权限访问该资源'
    this.showError(message, config)
  }

  // 处理 404 未找到错误
  private handleNotFound(error: AxiosError, config: ErrorHandlerConfig) {
    const data = error.response?.data as any
    const message = data?.message || '请求的资源不存在'
    this.showError(message, config)
  }

  // 处理 429 请求过多错误
  private handleTooManyRequests(error: AxiosError, config: ErrorHandlerConfig) {
    const message = '请求过于频繁，请稍后再试'
    this.showError(message, { ...config, showNotification: true })
  }

  // 处理服务器错误
  private handleServerError(error: AxiosError, config: ErrorHandlerConfig) {
    const status = error.response?.status
    let message = '服务器内部错误'

    switch (status) {
      case 502:
        message = '网关错误，服务暂时不可用'
        break
      case 503:
        message = '服务不可用，请稍后再试'
        break
      case 504:
        message = '网关超时，请稍后再试'
        break
    }

    this.showError(message, { ...config, showNotification: true })
  }

  // 处理通用错误
  private handleGenericError(error: AxiosError, config: ErrorHandlerConfig) {
    const data = error.response?.data as any
    const message = data?.message || error.message || '发生未知错误'
    this.showError(message, config)
  }

  // 显示错误信息
  private showError(message: string, config: ErrorHandlerConfig) {
    if (config.showMessage) {
      ElMessage({
        type: 'error',
        message,
        duration: 5000
      })
    }

    if (config.showNotification) {
      ElNotification({
        type: 'error',
        title: '错误',
        message,
        duration: 8000
      })
    }
  }

  // 上报错误到服务器
  private async reportErrorToServer(error: AxiosError) {
    try {
      const errorReport = {
        message: error.message,
        status: error.response?.status,
        url: error.config?.url,
        method: error.config?.method,
        timestamp: new Date().toISOString(),
        userAgent: navigator.userAgent,
        stack: error.stack
      }

      // 发送错误报告到服务器（这里需要根据实际API调整）
      // await fetch('/api/error-reports', {
      //   method: 'POST',
      //   headers: { 'Content-Type': 'application/json' },
      //   body: JSON.stringify(errorReport)
      // })
    } catch (reportError) {
      console.error('Failed to report error:', reportError)
    }
  }

  // 捕获全局未处理的 Promise 错误
  static setupGlobalErrorHandling() {
    // 捕获未处理的 Promise rejection
    window.addEventListener('unhandledrejection', (event) => {
      console.error('Unhandled promise rejection:', event.reason)
      
      // 阻止浏览器默认的错误提示
      event.preventDefault()
      
      ElMessage({
        type: 'error',
        message: '发生未知错误，请稍后再试',
        duration: 5000
      })
    })

    // 捕获全局 JavaScript 错误
    window.addEventListener('error', (event) => {
      console.error('Global error:', event.error)
      
      ElMessage({
        type: 'error',
        message: '页面发生错误，请刷新页面重试',
        duration: 5000
      })
    })

    // 捕获资源加载错误
    window.addEventListener('error', (event) => {
      if (event.target && event.target !== window) {
        console.error('Resource loading error:', event.target)
        
        ElMessage({
          type: 'warning',
          message: '资源加载失败，部分功能可能不可用',
          duration: 3000
        })
      }
    }, true)
  }
}

// 创建默认的错误处理器实例
export const errorHandler = new ErrorHandler()

// 导出错误处理器类以便自定义配置
export { ErrorHandler }