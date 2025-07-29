import { ElMessage } from 'element-plus'

/**
 * 复制文本到剪贴板的通用工具函数
 * 支持现代 Clipboard API 和备用方案
 */
export async function copyToClipboard(text: string, successMessage = '已复制到剪贴板'): Promise<boolean> {
  // 检查是否为空文本
  if (!text || text.trim() === '') {
    ElMessage.warning('复制内容不能为空')
    return false
  }

  // 方案1: 使用现代 Clipboard API
  if (navigator.clipboard && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(text)
      ElMessage.success(successMessage)
      return true
    } catch (error) {
      console.warn('Clipboard API 失败，尝试备用方案:', error)
      // 继续尝试备用方案
    }
  }

  // 方案2: 使用 document.execCommand (备用方案)
  try {
    return await fallbackCopyToClipboard(text, successMessage)
  } catch (error) {
    console.error('所有复制方案都失败了:', error)
    ElMessage.error('复制失败，请手动复制')
    return false
  }
}

/**
 * 备用复制方案 - 使用临时输入框和 document.execCommand
 */
async function fallbackCopyToClipboard(text: string, successMessage: string): Promise<boolean> {
  return new Promise((resolve) => {
    // 创建临时输入框
    const textArea = document.createElement('textarea')
    textArea.value = text
    
    // 设置样式，使其不可见但不影响布局
    textArea.style.position = 'fixed'
    textArea.style.top = '0'
    textArea.style.left = '0'
    textArea.style.width = '2em'
    textArea.style.height = '2em'
    textArea.style.padding = '0'
    textArea.style.border = 'none'
    textArea.style.outline = 'none'
    textArea.style.boxShadow = 'none'
    textArea.style.background = 'transparent'
    textArea.style.opacity = '0'
    textArea.style.zIndex = '-1'
    
    document.body.appendChild(textArea)
    
    try {
      // 选择文本
      textArea.focus()
      textArea.select()
      textArea.setSelectionRange(0, text.length)
      
      // 尝试复制 - 使用已弃用的 execCommand 作为备用方案
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      const successful = document.execCommand('copy')
      
      if (successful) {
        ElMessage.success(successMessage)
        resolve(true)
      } else {
        throw new Error('document.execCommand 返回 false')
      }
    } catch (error) {
      console.error('备用复制方案失败:', error)
      ElMessage.error('复制失败，请手动选择和复制文本')
      resolve(false)
    } finally {
      // 清理临时元素
      document.body.removeChild(textArea)
    }
  })
}

/**
 * 检查剪贴板功能是否可用
 */
export function isClipboardSupported(): {
  modern: boolean
  fallback: boolean
  reason?: string
} {
  const modern = !!(navigator.clipboard && window.isSecureContext)
  // eslint-disable-next-line @typescript-eslint/no-deprecated
  const fallback = !!document.execCommand
  
  let reason = ''
  if (!modern && !fallback) {
    reason = '浏览器不支持剪贴板功能'
  } else if (!modern) {
    if (!navigator.clipboard) {
      reason = '浏览器不支持现代剪贴板 API'
    } else if (!window.isSecureContext) {
      reason = '需要 HTTPS 协议或 localhost 环境'
    }
  }
  
  return {
    modern,
    fallback,
    reason
  }
}

/**
 * API 密钥专用复制函数
 */
export async function copyApiKey(apiKey: string): Promise<boolean> {
  if (!apiKey || typeof apiKey !== 'string') {
    ElMessage.warning('API密钥不能为空')
    return false
  }
  
  // 去除首尾空格
  const trimmedKey = apiKey.trim()
  if (!trimmedKey) {
    ElMessage.warning('API密钥不能为空')
    return false
  }
  
  // 检查密钥格式（可选的验证）
  if (trimmedKey.length < 10) {
    ElMessage.warning('API密钥格式可能不正确')
  }
  
  try {
    const result = await copyToClipboard(trimmedKey, 'API密钥已复制到剪贴板')
    if (result) {
      console.log('API密钥复制成功')
    }
    return result
  } catch (error) {
    console.error('API密钥复制失败:', error)
    ElMessage.error('复制失败，请手动选择和复制')
    return false
  }
}

/**
 * 带确认的复制函数（用于敏感信息）
 */
export async function copyWithConfirmation(
  text: string, 
  _confirmMessage = '确定要复制这个敏感信息吗？',
  successMessage = '已复制到剪贴板'
): Promise<boolean> {
  return new Promise((resolve) => {
    // 这里可以集成 Element Plus 的确认对话框
    // 为简化演示，直接返回复制结果
    // TODO: 实现确认对话框逻辑
    copyToClipboard(text, successMessage).then(resolve)
  })
}

/**
 * 获取剪贴板内容（需要用户权限）
 */
export async function getClipboardText(): Promise<string | null> {
  if (!navigator.clipboard || !window.isSecureContext) {
    console.warn('无法读取剪贴板：不支持或非安全上下文')
    return null
  }
  
  try {
    return await navigator.clipboard.readText()
  } catch (error) {
    console.error('读取剪贴板失败:', error)
    return null
  }
}

/**
 * 检查剪贴板权限状态
 */
export async function checkClipboardPermissions(): Promise<{
  write: PermissionState | 'unsupported'
  read: PermissionState | 'unsupported'
}> {
  const result = {
    write: 'unsupported' as PermissionState | 'unsupported',
    read: 'unsupported' as PermissionState | 'unsupported'
  }
  
  if (!navigator.permissions) {
    return result
  }
  
  try {
    const writePermission = await navigator.permissions.query({ name: 'clipboard-write' as PermissionName })
    result.write = writePermission.state
  } catch (error) {
    console.warn('检查写入权限失败:', error)
  }
  
  try {
    const readPermission = await navigator.permissions.query({ name: 'clipboard-read' as PermissionName })
    result.read = readPermission.state
  } catch (error) {
    console.warn('检查读取权限失败:', error)
  }
  
  return result
}