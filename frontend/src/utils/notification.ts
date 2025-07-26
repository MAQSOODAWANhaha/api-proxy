/**
 * 统一通知管理系统
 */

import { ElMessage, ElNotification, ElLoading } from 'element-plus'
import { h } from 'vue'
import type { VNode } from 'vue'

// 通知类型
export type NotificationType = 'success' | 'warning' | 'info' | 'error'

// 通知位置
export type NotificationPosition = 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left'

// 通知配置
export interface NotificationConfig {
  title?: string
  message: string | VNode
  type?: NotificationType
  duration?: number
  showClose?: boolean
  position?: NotificationPosition
  onClick?: () => void
  onClose?: () => void
}

// 消息配置
export interface MessageConfig {
  message: string | VNode
  type?: NotificationType
  duration?: number
  showClose?: boolean
  center?: boolean
  grouping?: boolean
  onClose?: () => void
}

// 加载配置
export interface LoadingConfig {
  text?: string
  spinner?: string
  background?: string
  target?: string | HTMLElement
  body?: boolean
  lock?: boolean
  customClass?: string
}

// 确认对话框配置
export interface ConfirmConfig {
  title?: string
  message: string | VNode
  type?: NotificationType
  confirmButtonText?: string
  cancelButtonText?: string
  showInput?: boolean
  inputPattern?: RegExp
  inputValidator?: (value?: string) => boolean | string
  inputErrorMessage?: string
  inputPlaceholder?: string
  inputType?: string
  inputValue?: string
}

/**
 * 通知管理器类
 */
export class NotificationManager {
  private static instance: NotificationManager
  private loadingInstance: any = null
  private messageQueue: Set<string> = new Set()
  
  static getInstance(): NotificationManager {
    if (!NotificationManager.instance) {
      NotificationManager.instance = new NotificationManager()
    }
    return NotificationManager.instance
  }
  
  /**
   * 显示成功通知
   */
  success(message: string, config?: Partial<NotificationConfig>): void {
    this.showNotification({
      message,
      type: 'success',
      duration: 4000,
      ...config
    })
  }
  
  /**
   * 显示警告通知
   */
  warning(message: string, config?: Partial<NotificationConfig>): void {
    this.showNotification({
      message,
      type: 'warning',
      duration: 5000,
      ...config
    })
  }
  
  /**
   * 显示信息通知
   */
  info(message: string, config?: Partial<NotificationConfig>): void {
    this.showNotification({
      message,
      type: 'info',
      duration: 4000,
      ...config
    })
  }
  
  /**
   * 显示错误通知
   */
  error(message: string, config?: Partial<NotificationConfig>): void {
    this.showNotification({
      message,
      type: 'error',
      duration: 6000,
      ...config
    })
  }
  
  /**
   * 显示持久通知（不自动关闭）
   */
  persistent(
    message: string, 
    type: NotificationType = 'info',
    config?: Partial<NotificationConfig>
  ): void {
    this.showNotification({
      message,
      type,
      duration: 0,
      showClose: true,
      ...config
    })
  }
  
  /**
   * 显示带操作的通知
   */
  withAction(
    message: string,
    actionText: string,
    onAction: () => void,
    config?: Partial<NotificationConfig>
  ): void {
    this.showNotification({
      message: h('div', [
        h('p', message),
        h('div', { style: 'margin-top: 8px;' }, [
          h('button', {
            style: 'background: #409eff; color: white; border: none; padding: 4px 8px; border-radius: 4px; cursor: pointer; font-size: 12px;',
            onClick: onAction
          }, actionText)
        ])
      ]),
      duration: 8000,
      showClose: true,
      ...config
    })
  }
  
  /**
   * 显示进度通知
   */
  progress(
    message: string,
    progress: number,
    config?: Partial<NotificationConfig>
  ): void {
    const progressBar = h('div', { 
      style: 'margin-top: 8px; background: #f0f0f0; border-radius: 4px; overflow: hidden;' 
    }, [
      h('div', {
        style: `background: #409eff; height: 4px; width: ${Math.min(100, Math.max(0, progress))}%; transition: width 0.3s ease;`
      })
    ])
    
    this.showNotification({
      message: h('div', [
        h('p', message),
        progressBar,
        h('p', { 
          style: 'font-size: 12px; color: #909399; margin-top: 4px; text-align: right;' 
        }, `${Math.round(progress)}%`)
      ]),
      duration: 0,
      showClose: false,
      ...config
    })
  }
  
  /**
   * 显示通知
   */
  private showNotification(config: NotificationConfig): void {
    ElNotification({
      title: config.title,
      message: config.message,
      type: config.type || 'info',
      duration: config.duration ?? 4000,
      showClose: config.showClose ?? true,
      position: config.position || 'top-right',
      onClick: config.onClick,
      onClose: config.onClose
    })
  }
  
  /**
   * 显示成功消息
   */
  successMessage(message: string, config?: Partial<MessageConfig>): void {
    this.showMessage({
      message,
      type: 'success',
      duration: 3000,
      ...config
    })
  }
  
  /**
   * 显示警告消息
   */
  warningMessage(message: string, config?: Partial<MessageConfig>): void {
    this.showMessage({
      message,
      type: 'warning',
      duration: 4000,
      ...config
    })
  }
  
  /**
   * 显示信息消息
   */
  infoMessage(message: string, config?: Partial<MessageConfig>): void {
    this.showMessage({
      message,
      type: 'info',
      duration: 3000,
      ...config
    })
  }
  
  /**
   * 显示错误消息
   */
  errorMessage(message: string, config?: Partial<MessageConfig>): void {
    this.showMessage({
      message,
      type: 'error',
      duration: 5000,
      ...config
    })
  }
  
  /**
   * 显示消息（防重复）
   */
  private showMessage(config: MessageConfig): void {
    const messageKey = typeof config.message === 'string' 
      ? config.message 
      : config.message.toString()
    
    // 防重复显示相同消息
    if (config.grouping !== false && this.messageQueue.has(messageKey)) {
      return
    }
    
    if (config.grouping !== false) {
      this.messageQueue.add(messageKey)
      
      // 清理消息队列
      setTimeout(() => {
        this.messageQueue.delete(messageKey)
      }, config.duration || 3000)
    }
    
    ElMessage({
      message: config.message,
      type: config.type || 'info',
      duration: config.duration ?? 3000,
      showClose: config.showClose ?? false,
      center: config.center ?? false,
      grouping: config.grouping ?? true,
      onClose: config.onClose
    })
  }
  
  /**
   * 显示加载状态
   */
  showLoading(config?: LoadingConfig): void {
    this.hideLoading() // 先关闭之前的加载
    
    this.loadingInstance = ElLoading.service({
      text: config?.text || '加载中...',
      spinner: config?.spinner,
      background: config?.background || 'rgba(0, 0, 0, 0.7)',
      target: config?.target,
      body: config?.body ?? true,
      lock: config?.lock ?? true,
      customClass: config?.customClass
    })
  }
  
  /**
   * 隐藏加载状态
   */
  hideLoading(): void {
    if (this.loadingInstance) {
      this.loadingInstance.close()
      this.loadingInstance = null
    }
  }
  
  /**
   * 显示确认对话框
   */
  async confirm(
    message: string,
    config?: Partial<ConfirmConfig>
  ): Promise<{ action: string; value?: string }> {
    const { ElMessageBox } = await import('element-plus')
    
    try {
      const result = await ElMessageBox.confirm(
        config?.message || message,
        config?.title || '确认',
        {
          confirmButtonText: config?.confirmButtonText || '确定',
          cancelButtonText: config?.cancelButtonText || '取消',
          type: config?.type || 'warning',
          showInput: config?.showInput,
          inputPattern: config?.inputPattern,
          inputValidator: config?.inputValidator,
          inputErrorMessage: config?.inputErrorMessage,
          inputPlaceholder: config?.inputPlaceholder,
          inputType: config?.inputType,
          inputValue: config?.inputValue
        }
      )
      
      return { action: 'confirm', value: result as string }
    } catch (error: any) {
      return { action: error === 'cancel' ? 'cancel' : 'close' }
    }
  }
  
  /**
   * 显示输入对话框
   */
  async prompt(
    message: string,
    config?: Partial<ConfirmConfig>
  ): Promise<{ action: string; value?: string }> {
    const { ElMessageBox } = await import('element-plus')
    
    try {
      const { value } = await ElMessageBox.prompt(
        config?.message || message,
        config?.title || '输入',
        {
          confirmButtonText: config?.confirmButtonText || '确定',
          cancelButtonText: config?.cancelButtonText || '取消',
          inputPattern: config?.inputPattern,
          inputValidator: config?.inputValidator,
          inputErrorMessage: config?.inputErrorMessage,
          inputPlaceholder: config?.inputPlaceholder,
          inputType: config?.inputType,
          inputValue: config?.inputValue
        }
      )
      
      return { action: 'confirm', value }
    } catch (error: any) {
      return { action: error === 'cancel' ? 'cancel' : 'close' }
    }
  }
  
  /**
   * 显示操作结果反馈
   */
  operationFeedback(
    operation: string,
    success: boolean,
    message?: string,
    details?: any
  ): void {
    if (success) {
      this.successMessage(
        message || `${operation}成功`,
        { duration: 3000 }
      )
    } else {
      this.errorMessage(
        message || `${operation}失败`,
        { duration: 5000 }
      )
      
      if (details) {
        console.error(`${operation}失败详情:`, details)
      }
    }
  }
  
  /**
   * 清空所有消息
   */
  closeAll(): void {
    ElMessage.closeAll()
    this.hideLoading()
    this.messageQueue.clear()
  }
}

// 导出单例实例
export const notify = NotificationManager.getInstance()

// 便捷函数导出
export const {
  success,
  warning,
  info,
  error,
  persistent,
  withAction,
  progress,
  successMessage,
  warningMessage,
  infoMessage,
  errorMessage,
  showLoading,
  hideLoading,
  confirm,
  prompt,
  operationFeedback,
  closeAll
} = notify

// 默认导出
export default notify