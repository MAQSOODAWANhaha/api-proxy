/**
 * 统一错误处理工具
 */

import { ElMessage, ElNotification, ElMessageBox } from 'element-plus'
import { h } from 'vue'

// 错误类型定义
export interface AppError {
  code: string
  message: string
  details?: any
  timestamp: number
  stack?: string
}

export interface ApiError {
  status: number
  statusText: string
  data?: any
  url?: string
  method?: string
}

export interface NetworkError {
  code: string
  message: string
  timeout?: boolean
  offline?: boolean
}

// 错误级别
export const ErrorLevel = {
  INFO: 'info',
  WARNING: 'warning',
  ERROR: 'error',
  CRITICAL: 'critical'
} as const

export type ErrorLevel = typeof ErrorLevel[keyof typeof ErrorLevel]

// 错误类型
export const ErrorType = {
  NETWORK: 'network',
  API: 'api',
  VALIDATION: 'validation',
  AUTHENTICATION: 'authentication',
  AUTHORIZATION: 'authorization',
  BUSINESS: 'business',
  SYSTEM: 'system',
  UNKNOWN: 'unknown'
} as const

export type ErrorType = typeof ErrorType[keyof typeof ErrorType]

// 错误处理配置
export interface ErrorHandlerConfig {
  showNotification?: boolean
  showMessage?: boolean
  logError?: boolean
  reportError?: boolean
  retryable?: boolean
  level?: ErrorLevel
}

// 默认错误消息映射
const ERROR_MESSAGES: Record<string, string> = {
  // 网络错误
  'NETWORK_ERROR': '网络连接失败，请检查网络设置',
  'TIMEOUT_ERROR': '请求超时，请稍后重试',
  'OFFLINE_ERROR': '网络连接已断开，请检查网络连接',
  
  // API错误
  'API_ERROR': '服务器响应错误',
  'INVALID_RESPONSE': '服务器返回数据格式错误',
  
  // 认证错误
  'AUTH_REQUIRED': '请先登录',
  'AUTH_EXPIRED': '登录已过期，请重新登录',
  'AUTH_INVALID': '登录信息无效',
  
  // 权限错误
  'PERMISSION_DENIED': '权限不足，无法执行此操作',
  'RESOURCE_FORBIDDEN': '访问被拒绝',
  
  // 业务错误
  'VALIDATION_ERROR': '输入数据验证失败',
  'BUSINESS_ERROR': '业务处理失败',
  'RESOURCE_NOT_FOUND': '请求的资源不存在',
  'RESOURCE_CONFLICT': '资源冲突，请刷新后重试',
  
  // 系统错误
  'SYSTEM_ERROR': '系统内部错误',
  'SERVICE_UNAVAILABLE': '服务暂时不可用，请稍后重试',
  'MAINTENANCE': '系统维护中，请稍后访问',
  
  // 默认错误
  'UNKNOWN_ERROR': '未知错误，请联系管理员'
}

// HTTP状态码错误消息映射
const HTTP_ERROR_MESSAGES: Record<number, string> = {
  400: '请求参数错误',
  401: '未授权，请重新登录',
  403: '权限不足',
  404: '请求的资源不存在',
  405: '请求方法不被允许',
  408: '请求超时',
  409: '资源冲突',
  422: '请求数据验证失败',
  429: '请求过于频繁，请稍后重试',
  500: '服务器内部错误',
  502: '网关错误',
  503: '服务不可用',
  504: '网关超时',
  511: '网络认证失败'
}

/**
 * 创建标准化错误对象
 */
export function createError(
  code: string,
  message?: string,
  details?: any,
  stack?: string
): AppError {
  return {
    code,
    message: message || ERROR_MESSAGES[code] || ERROR_MESSAGES.UNKNOWN_ERROR,
    details,
    timestamp: Date.now(),
    stack
  }
}

/**
 * 从HTTP响应创建API错误
 */
export function createApiError(response: {
  status: number
  statusText: string
  data?: any
  config?: any
}): ApiError {
  return {
    status: response.status,
    statusText: response.statusText,
    data: response.data,
    url: response.config?.url,
    method: response.config?.method?.toUpperCase()
  }
}

/**
 * 从网络错误创建网络错误对象
 */
export function createNetworkError(error: any): NetworkError {
  const isTimeout = error.code === 'ECONNABORTED' || error.message?.includes('timeout')
  const isOffline = !navigator.onLine
  
  return {
    code: isTimeout ? 'TIMEOUT_ERROR' : isOffline ? 'OFFLINE_ERROR' : 'NETWORK_ERROR',
    message: isTimeout 
      ? ERROR_MESSAGES.TIMEOUT_ERROR 
      : isOffline 
        ? ERROR_MESSAGES.OFFLINE_ERROR 
        : ERROR_MESSAGES.NETWORK_ERROR,
    timeout: isTimeout,
    offline: isOffline
  }
}

/**
 * 获取友好的错误消息
 */
export function getErrorMessage(error: any): string {
  if (typeof error === 'string') {
    return error
  }
  
  if (error?.message) {
    return error.message
  }
  
  if (error?.response?.status) {
    const status = error.response.status
    return HTTP_ERROR_MESSAGES[status] || `HTTP ${status} 错误`
  }
  
  if (error?.code && ERROR_MESSAGES[error.code]) {
    return ERROR_MESSAGES[error.code]
  }
  
  return ERROR_MESSAGES.UNKNOWN_ERROR
}

/**
 * 获取错误类型
 */
export function getErrorType(error: any): ErrorType {
  if (!error) return ErrorType.UNKNOWN
  
  // 网络错误
  if (error.code === 'NETWORK_ERROR' || error.message?.includes('Network Error')) {
    return ErrorType.NETWORK
  }
  
  // API错误
  if (error.response || error.status) {
    const status = error.response?.status || error.status
    
    if (status === 401) return ErrorType.AUTHENTICATION
    if (status === 403) return ErrorType.AUTHORIZATION
    if (status >= 400 && status < 500) return ErrorType.VALIDATION
    if (status >= 500) return ErrorType.SYSTEM
    
    return ErrorType.API
  }
  
  // 业务错误
  if (error.code?.startsWith('BUSINESS_') || error.code?.startsWith('VALIDATION_')) {
    return ErrorType.BUSINESS
  }
  
  // 认证错误
  if (error.code?.startsWith('AUTH_')) {
    return ErrorType.AUTHENTICATION
  }
  
  return ErrorType.UNKNOWN
}

/**
 * 错误处理器类
 */
export class ErrorHandler {
  private static instance: ErrorHandler
  private errorQueue: AppError[] = []
  private config: ErrorHandlerConfig = {
    showNotification: true,
    showMessage: true,
    logError: true,
    reportError: false,
    level: ErrorLevel.ERROR
  }
  
  static getInstance(): ErrorHandler {
    if (!ErrorHandler.instance) {
      ErrorHandler.instance = new ErrorHandler()
    }
    return ErrorHandler.instance
  }
  
  /**
   * 配置错误处理器
   */
  configure(config: Partial<ErrorHandlerConfig>): void {
    this.config = { ...this.config, ...config }
  }
  
  /**
   * 处理错误
   */
  async handle(
    error: any, 
    context?: string, 
    config?: Partial<ErrorHandlerConfig>
  ): Promise<void> {
    const finalConfig = { ...this.config, ...config }
    const appError = this.normalizeError(error, context)
    
    // 记录错误
    if (finalConfig.logError) {
      this.logError(appError, context)
    }
    
    // 添加到错误队列
    this.errorQueue.push(appError)
    
    // 限制错误队列大小
    if (this.errorQueue.length > 100) {
      this.errorQueue.shift()
    }
    
    // 显示用户反馈
    await this.showUserFeedback(appError, finalConfig)
    
    // 上报错误
    if (finalConfig.reportError) {
      await this.reportError(appError, context)
    }
  }
  
  /**
   * 标准化错误对象
   */
  private normalizeError(error: any, context?: string): AppError {
    if (error instanceof Error) {
      return createError(
        error.name || 'UNKNOWN_ERROR',
        error.message,
        { context, originalError: error },
        error.stack
      )
    }
    
    if (typeof error === 'string') {
      return createError('STRING_ERROR', error, { context })
    }
    
    if (error?.code && error?.message) {
      return {
        ...error,
        timestamp: error.timestamp || Date.now(),
        details: { ...error.details, context }
      }
    }
    
    return createError(
      'UNKNOWN_ERROR',
      getErrorMessage(error),
      { context, originalError: error }
    )
  }
  
  /**
   * 记录错误
   */
  private logError(error: AppError, context?: string): void {
    const logLevel = this.getLogLevel(error)
    const logMessage = `[${error.code}] ${error.message}`
    const logDetails = {
      timestamp: new Date(error.timestamp).toISOString(),
      context,
      details: error.details,
      stack: error.stack
    }
    
    switch (logLevel) {
      case ErrorLevel.CRITICAL:
        console.error('🔥 CRITICAL:', logMessage, logDetails)
        break
      case ErrorLevel.ERROR:
        console.error('❌ ERROR:', logMessage, logDetails)
        break
      case ErrorLevel.WARNING:
        console.warn('⚠️ WARNING:', logMessage, logDetails)
        break
      case ErrorLevel.INFO:
        console.info('ℹ️ INFO:', logMessage, logDetails)
        break
    }
  }
  
  /**
   * 获取日志级别
   */
  private getLogLevel(error: AppError): ErrorLevel {
    if (error.code.includes('CRITICAL') || error.code.includes('SYSTEM')) {
      return ErrorLevel.CRITICAL
    }
    
    if (error.code.includes('AUTH') || error.code.includes('PERMISSION')) {
      return ErrorLevel.ERROR
    }
    
    if (error.code.includes('VALIDATION') || error.code.includes('BUSINESS')) {
      return ErrorLevel.WARNING
    }
    
    return ErrorLevel.ERROR
  }
  
  /**
   * 显示用户反馈
   */
  private async showUserFeedback(
    error: AppError, 
    config: ErrorHandlerConfig
  ): Promise<void> {
    const errorType = getErrorType(error)
    const isRetryable = this.isRetryable(error)
    
    // 显示消息提示
    if (config.showMessage) {
      this.showMessage(error, errorType)
    }
    
    // 显示通知
    if (config.showNotification) {
      this.showNotification(error, errorType, isRetryable)
    }
  }
  
  /**
   * 显示消息提示
   */
  private showMessage(error: AppError, errorType: ErrorType): void {
    const messageType = this.getMessageType(errorType)
    
    ElMessage({
      type: messageType,
      message: error.message,
      duration: this.getMessageDuration(errorType),
      showClose: true,
      grouping: true
    })
  }
  
  /**
   * 显示通知
   */
  private showNotification(
    error: AppError, 
    errorType: ErrorType, 
    isRetryable: boolean
  ): void {
    const notificationType = this.getNotificationType(errorType)
    
    ElNotification({
      type: notificationType,
      title: this.getNotificationTitle(errorType),
      message: h('div', [
        h('p', error.message),
        ...(error.details?.context ? [h('p', { style: 'color: #909399; font-size: 12px; margin-top: 8px;' }, `上下文: ${error.details.context}`)] : []),
        ...(isRetryable ? [h('p', { style: 'color: #409eff; font-size: 12px; margin-top: 8px;' }, '💡 您可以稍后重试此操作')] : [])
      ]),
      duration: this.getNotificationDuration(errorType),
      showClose: true
    })
  }
  
  /**
   * 获取消息类型
   */
  private getMessageType(errorType: ErrorType): 'success' | 'warning' | 'info' | 'error' {
    switch (errorType) {
      case ErrorType.VALIDATION:
      case ErrorType.BUSINESS:
        return 'warning'
      case ErrorType.NETWORK:
        return 'info'
      default:
        return 'error'
    }
  }
  
  /**
   * 获取通知类型
   */
  private getNotificationType(errorType: ErrorType): 'success' | 'warning' | 'info' | 'error' {
    return this.getMessageType(errorType)
  }
  
  /**
   * 获取通知标题
   */
  private getNotificationTitle(errorType: ErrorType): string {
    switch (errorType) {
      case ErrorType.NETWORK:
        return '网络错误'
      case ErrorType.API:
        return 'API错误'
      case ErrorType.AUTHENTICATION:
        return '认证错误'
      case ErrorType.AUTHORIZATION:
        return '权限错误'
      case ErrorType.VALIDATION:
        return '验证错误'
      case ErrorType.BUSINESS:
        return '业务错误'
      case ErrorType.SYSTEM:
        return '系统错误'
      default:
        return '错误'
    }
  }
  
  /**
   * 获取消息持续时间
   */
  private getMessageDuration(errorType: ErrorType): number {
    switch (errorType) {
      case ErrorType.VALIDATION:
      case ErrorType.BUSINESS:
        return 4000
      case ErrorType.NETWORK:
        return 6000
      default:
        return 5000
    }
  }
  
  /**
   * 获取通知持续时间
   */
  private getNotificationDuration(errorType: ErrorType): number {
    switch (errorType) {
      case ErrorType.SYSTEM:
        return 0 // 不自动关闭
      case ErrorType.AUTHENTICATION:
      case ErrorType.AUTHORIZATION:
        return 8000
      default:
        return 6000
    }
  }
  
  /**
   * 判断错误是否可重试
   */
  private isRetryable(error: AppError): boolean {
    const retryableCodes = [
      'NETWORK_ERROR',
      'TIMEOUT_ERROR',
      'SERVICE_UNAVAILABLE',
      'GATEWAY_ERROR'
    ]
    
    return retryableCodes.includes(error.code) || 
           (error.details?.status >= 500 && error.details?.status < 600)
  }
  
  /**
   * 上报错误
   */
  private async reportError(error: AppError, context?: string): Promise<void> {
    try {
      // 这里可以集成错误上报服务，如 Sentry、Bugsnag 等
      // await errorReportingService.report(error, context)
      console.info('📊 错误已上报:', error.code)
    } catch (reportError) {
      console.warn('错误上报失败:', reportError)
    }
  }
  
  /**
   * 获取错误历史
   */
  getErrorHistory(limit = 50): AppError[] {
    return [...this.errorQueue].reverse().slice(0, limit)
  }
  
  /**
   * 清空错误历史
   */
  clearErrorHistory(): void {
    this.errorQueue = []
  }
  
  /**
   * 显示错误确认对话框
   */
  async showErrorDialog(
    error: AppError, 
    options?: {
      title?: string
      showDetails?: boolean
      showRetry?: boolean
      onRetry?: () => void | Promise<void>
    }
  ): Promise<void> {
    const { title, showDetails, showRetry, onRetry } = options || {}
    
    try {
      const action = await ElMessageBox.alert(
        h('div', [
          h('p', error.message),
          ...(showDetails && error.details ? [
            h('details', { style: 'margin-top: 16px;' }, [
              h('summary', '错误详细信息'),
              h('pre', { 
                style: 'background: #f5f5f5; padding: 12px; border-radius: 4px; margin-top: 8px; font-size: 12px; overflow: auto;' 
              }, JSON.stringify(error.details, null, 2))
            ])
          ] : [])
        ]),
        title || '错误',
        {
          confirmButtonText: showRetry ? '重试' : '确定',
          cancelButtonText: showRetry ? '取消' : undefined,
          showCancelButton: showRetry,
          type: 'error'
        }
      )
      
      if (action === 'confirm' && showRetry && onRetry) {
        await onRetry()
      }
    } catch {
      // 用户取消了对话框
    }
  }
}

// 导出单例实例
export const errorHandler = ErrorHandler.getInstance()

// 便捷函数
export const handleError = errorHandler.handle.bind(errorHandler)
export const showErrorDialog = errorHandler.showErrorDialog.bind(errorHandler)
export const getErrorHistory = errorHandler.getErrorHistory.bind(errorHandler)
export const clearErrorHistory = errorHandler.clearErrorHistory.bind(errorHandler)