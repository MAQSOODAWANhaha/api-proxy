import { ElMessage, ElNotification, ElMessageBox } from 'element-plus'
import type { MessageOptions } from 'element-plus/es/components/message/src/message'
import type { NotificationOptions } from 'element-plus/es/components/notification/src/notification'
import type { ElMessageBoxOptions } from 'element-plus/es/components/message-box/src/message-box.type'

// 消息类型
type MessageType = 'success' | 'warning' | 'info' | 'error'

// 消息配置
interface MessageConfig extends Partial<MessageOptions> {
  duration?: number
  showClose?: boolean
  center?: boolean
  grouping?: boolean
}

// 通知配置
interface NotificationConfig extends Partial<NotificationOptions> {
  title?: string
  duration?: number
  position?: 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left'
  showClose?: boolean
}

// 确认框配置
interface ConfirmConfig extends Partial<ElMessageBoxOptions> {
  title?: string
  confirmButtonText?: string
  cancelButtonText?: string
  type?: MessageType
  dangerouslyUseHTMLString?: boolean
}

class MessageManager {
  private messageInstances: Set<any> = new Set()
  private notificationInstances: Set<any> = new Set()

  // 默认配置
  private defaultMessageConfig: MessageConfig = {
    duration: 3000,
    showClose: true,
    center: false,
    grouping: false
  }

  private defaultNotificationConfig: NotificationConfig = {
    duration: 4500,
    position: 'top-right',
    showClose: true
  }

  private defaultConfirmConfig: ConfirmConfig = {
    confirmButtonText: '确定',
    cancelButtonText: '取消',
    type: 'warning',
    dangerouslyUseHTMLString: false
  }

  // 显示成功消息
  success(message: string, config?: MessageConfig) {
    return this.showMessage(message, 'success', config)
  }

  // 显示警告消息
  warning(message: string, config?: MessageConfig) {
    return this.showMessage(message, 'warning', config)
  }

  // 显示信息消息
  info(message: string, config?: MessageConfig) {
    return this.showMessage(message, 'info', config)
  }

  // 显示错误消息
  error(message: string, config?: MessageConfig) {
    return this.showMessage(message, 'error', {
      duration: 5000,
      ...config
    })
  }

  // 显示消息的核心方法
  private showMessage(message: string, type: MessageType, config?: MessageConfig) {
    const finalConfig = {
      ...this.defaultMessageConfig,
      ...config,
      type,
      message
    }

    const instance = ElMessage(finalConfig)
    this.messageInstances.add(instance)

    // 清理实例引用
    if (instance && typeof instance.close === 'function') {
      const originalClose = instance.close
      instance.close = () => {
        this.messageInstances.delete(instance)
        originalClose.call(instance)
      }
    }

    return instance
  }

  // 显示成功通知
  notifySuccess(message: string, title = '成功', config?: NotificationConfig) {
    return this.showNotification(message, 'success', title, config)
  }

  // 显示警告通知
  notifyWarning(message: string, title = '警告', config?: NotificationConfig) {
    return this.showNotification(message, 'warning', title, config)
  }

  // 显示信息通知
  notifyInfo(message: string, title = '提示', config?: NotificationConfig) {
    return this.showNotification(message, 'info', title, config)
  }

  // 显示错误通知
  notifyError(message: string, title = '错误', config?: NotificationConfig) {
    return this.showNotification(message, 'error', title, {
      duration: 8000,
      ...config
    })
  }

  // 显示通知的核心方法
  private showNotification(message: string, type: MessageType, title: string, config?: NotificationConfig) {
    const finalConfig = {
      ...this.defaultNotificationConfig,
      ...config,
      type,
      title,
      message
    }

    const instance = ElNotification(finalConfig)
    this.notificationInstances.add(instance)

    // 清理实例引用
    if (instance && typeof instance.close === 'function') {
      const originalClose = instance.close
      instance.close = () => {
        this.notificationInstances.delete(instance)
        originalClose.call(instance)
      }
    }

    return instance
  }

  // 确认对话框
  async confirm(message: string, title = '确认', config?: ConfirmConfig): Promise<boolean> {
    try {
      const finalConfig = {
        ...this.defaultConfirmConfig,
        ...config,
        title,
        message
      }

      await ElMessageBox.confirm(message, title, finalConfig)
      return true
    } catch {
      return false
    }
  }

  // 提示输入对话框
  async prompt(message: string, title = '输入', config?: ConfirmConfig): Promise<string | null> {
    try {
      const finalConfig = {
        ...this.defaultConfirmConfig,
        ...config,
        title,
        message
      }

      const { value } = await ElMessageBox.prompt(message, title, finalConfig)
      return value
    } catch {
      return null
    }
  }

  // 警告对话框
  async alert(message: string, title = '提示', config?: ConfirmConfig): Promise<void> {
    const finalConfig = {
      ...this.defaultConfirmConfig,
      ...config,
      title,
      message
    }

    return ElMessageBox.alert(message, title, finalConfig)
  }

  // 关闭所有消息
  closeAllMessages() {
    this.messageInstances.forEach(instance => {
      if (instance && typeof instance.close === 'function') {
        instance.close()
      }
    })
    this.messageInstances.clear()
  }

  // 关闭所有通知
  closeAllNotifications() {
    this.notificationInstances.forEach(instance => {
      if (instance && typeof instance.close === 'function') {
        instance.close()
      }
    })
    this.notificationInstances.clear()
  }

  // 关闭所有消息和通知
  closeAll() {
    this.closeAllMessages()
    this.closeAllNotifications()
  }

  // 快捷方法：操作成功
  operationSuccess(action = '操作', config?: MessageConfig) {
    return this.success(`${action}成功`, config)
  }

  // 快捷方法：操作失败
  operationError(action = '操作', error?: string, config?: MessageConfig) {
    const message = error ? `${action}失败：${error}` : `${action}失败`
    return this.error(message, config)
  }

  // 快捷方法：网络错误
  networkError(config?: MessageConfig) {
    return this.error('网络连接失败，请检查网络设置', {
      duration: 5000,
      ...config
    })
  }

  // 快捷方法：权限错误
  permissionError(config?: MessageConfig) {
    return this.error('没有权限执行此操作', config)
  }

  // 快捷方法：登录过期
  loginExpired(config?: MessageConfig) {
    return this.warning('登录已过期，请重新登录', {
      duration: 5000,
      ...config
    })
  }

  // 快捷方法：删除确认
  async confirmDelete(itemName = '此项', config?: ConfirmConfig): Promise<boolean> {
    return this.confirm(
      `确定要删除${itemName}吗？删除后不可恢复。`,
      '确认删除',
      {
        type: 'warning',
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        ...config
      }
    )
  }

  // 快捷方法：保存确认
  async confirmSave(config?: ConfirmConfig): Promise<boolean> {
    return this.confirm(
      '确定要保存当前修改吗？',
      '确认保存',
      {
        type: 'info',
        confirmButtonText: '保存',
        cancelButtonText: '取消',
        ...config
      }
    )
  }

  // 快捷方法：离开确认
  async confirmLeave(config?: ConfirmConfig): Promise<boolean> {
    return this.confirm(
      '当前有未保存的修改，确定要离开吗？',
      '确认离开',
      {
        type: 'warning',
        confirmButtonText: '离开',
        cancelButtonText: '留下',
        ...config
      }
    )
  }
}

// 创建消息管理器实例
export const messageManager = new MessageManager()

// 导出类以便自定义配置
export { MessageManager }

// 便捷方法导出
export const {
  success,
  warning,
  info,
  error,
  notifySuccess,
  notifyWarning,
  notifyInfo,
  notifyError,
  confirm,
  prompt,
  alert,
  operationSuccess,
  operationError,
  networkError,
  permissionError,
  loginExpired,
  confirmDelete,
  confirmSave,
  confirmLeave
} = messageManager