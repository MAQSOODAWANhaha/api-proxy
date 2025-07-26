/**
 * 环境配置管理
 */

export interface AppConfig {
  /** 应用标题 */
  title: string
  /** API基础URL */
  apiBaseUrl: string
  /** WebSocket基础URL */
  wsBaseUrl: string
  /** 环境标识 */
  env: 'development' | 'production' | 'local'
  /** 是否启用Mock数据 */
  enableMock: boolean
}

/**
 * 获取环境变量配置
 */
function getEnvConfig(): AppConfig {
  return {
    title: import.meta.env.VITE_APP_TITLE || 'AI Proxy Admin',
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL || 'http://127.0.0.1:9090/api',
    wsBaseUrl: import.meta.env.VITE_WS_BASE_URL || 'ws://127.0.0.1:9090',
    env: import.meta.env.VITE_APP_ENV || 'development',
    enableMock: import.meta.env.VITE_ENABLE_MOCK === 'true',
  }
}

/**
 * 应用配置
 */
export const appConfig = getEnvConfig()

/**
 * 是否为开发环境
 */
export const isDev = appConfig.env === 'development'

/**
 * 是否为生产环境
 */
export const isProd = appConfig.env === 'production'

/**
 * 获取完整的API URL
 * @param path API路径
 * @returns 完整的API URL
 */
export function getApiUrl(path: string): string {
  // 如果path已经是完整URL，直接返回
  if (path.startsWith('http://') || path.startsWith('https://')) {
    return path
  }
  
  // 移除开头的斜杠
  const cleanPath = path.startsWith('/') ? path.slice(1) : path
  
  // 确保baseUrl以斜杠结尾
  const baseUrl = appConfig.apiBaseUrl.endsWith('/') 
    ? appConfig.apiBaseUrl 
    : `${appConfig.apiBaseUrl}/`
  
  return `${baseUrl}${cleanPath}`
}

/**
 * 获取完整的WebSocket URL
 * @param path WebSocket路径
 * @returns 完整的WebSocket URL
 */
export function getWsUrl(path: string): string {
  const cleanPath = path.startsWith('/') ? path.slice(1) : path
  const baseUrl = appConfig.wsBaseUrl.endsWith('/') 
    ? appConfig.wsBaseUrl 
    : `${appConfig.wsBaseUrl}/`
  
  return `${baseUrl}${cleanPath}`
}

/**
 * 运行时配置检查和警告
 */
export function validateConfig(): void {
  if (isDev) {
    console.group('🔧 应用配置信息')
    console.log('环境:', appConfig.env)
    console.log('API基础URL:', appConfig.apiBaseUrl)
    console.log('WebSocket基础URL:', appConfig.wsBaseUrl)
    console.log('启用Mock:', appConfig.enableMock)
    console.groupEnd()
  }

  // 检查关键配置
  if (!appConfig.apiBaseUrl) {
    console.warn('⚠️ API基础URL未配置，可能影响API调用')
  }
}