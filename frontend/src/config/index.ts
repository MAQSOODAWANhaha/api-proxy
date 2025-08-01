// 应用配置文件 - 运行时配置版本

// 声明全局配置接口
declare global {
  interface Window {
    __APP_CONFIG__?: any
    getAppConfig?: (path: string, defaultValue?: any) => any
    updateAppConfig?: (newConfig: any) => void
  }
}

// 获取运行时配置的辅助函数
const getRuntimeConfig = (path: string, fallback?: any) => {
  // 优先从全局配置对象读取
  if (window.__APP_CONFIG__) {
    return getNestedValue(window.__APP_CONFIG__, path, fallback)
  }
  
  // 如果全局配置不存在，使用 getAppConfig 函数
  if (window.getAppConfig) {
    return window.getAppConfig(path, fallback)
  }
  
  // 最后的后备方案：从构建时环境变量读取（向后兼容）
  return getFromBuildTimeEnv(path, fallback)
}

// 从构建时环境变量获取配置（后备方案）
const getFromBuildTimeEnv = (path: string, fallback?: any) => {
  switch (path) {
    case 'api.baseURL':
      return import.meta.env.VITE_API_BASE_URL || fallback
    case 'websocket.url':
      return import.meta.env.VITE_WS_URL || fallback
    case 'app.version':
      return import.meta.env.VITE_APP_VERSION || fallback
    case 'development.logLevel':
      return import.meta.env.VITE_LOG_LEVEL || fallback
    default:
      return fallback
  }
}

// 获取嵌套对象值的辅助函数
const getNestedValue = (obj: any, path: string, defaultValue?: any) => {
  const keys = path.split('.')
  let result = obj
  
  for (const key of keys) {
    if (result && typeof result === 'object' && key in result) {
      result = result[key]
    } else {
      return defaultValue
    }
  }
  
  return result
}

// 响应式配置对象
export const config = {
  // API相关配置
  api: {
    get baseURL() {
      return getRuntimeConfig('api.baseURL', '/api')
    },
    timeout: 30000,
    retryCount: 3,
    retryDelay: 1000
  },

  // 认证相关配置
  auth: {
    tokenKey: 'api_proxy_token',
    userInfoKey: 'api_proxy_user_info', 
    refreshTokenKey: 'api_proxy_refresh_token',
    tokenExpireTime: 24 * 60 * 60 * 1000, // 24小时
    autoRefreshThreshold: 5 * 60 * 1000 // 5分钟
  },

  // 应用基础配置
  app: {
    name: 'AI代理平台',
    get version() {
      return getRuntimeConfig('app.version', '1.0.0')
    },
    author: 'AI Proxy Team',
    description: '企业级AI服务代理平台',
    get debug() {
      return getRuntimeConfig('development.showDevtools', import.meta.env.DEV || false)
    }
  },

  // 分页配置
  pagination: {
    defaultPageSize: 20,
    pageSizes: [10, 20, 50, 100],
    maxPageSize: 1000
  },

  // 上传配置
  upload: {
    maxFileSize: 10 * 1024 * 1024, // 10MB
    allowedTypes: ['image/jpeg', 'image/png', 'image/gif', 'text/plain', 'application/json'],
    chunkSize: 1024 * 1024 // 1MB
  },

  // 缓存配置
  cache: {
    defaultExpire: 5 * 60 * 1000, // 5分钟
    maxSize: 100 // 最大缓存项数
  },

  // WebSocket配置
  websocket: {
    get url() {
      return getRuntimeConfig('websocket.url', '/ws')
    },
    reconnectInterval: 5000,
    maxReconnectAttempts: 5,
    pingInterval: 30000
  },

  // 图表配置
  chart: {
    theme: 'light',
    animation: true,
    responsive: true,
    colors: [
      '#409EFF', '#67C23A', '#E6A23C', '#F56C6C', 
      '#909399', '#C6E2FF', '#D3F261', '#FDE68A',
      '#FCA5A5', '#E5E7EB'
    ]
  },

  // 表格配置
  table: {
    stripe: true,
    border: false,
    size: 'default' as 'large' | 'default' | 'small',
    showOverflowTooltip: true,
    highlightCurrentRow: true
  },

  // 表单配置
  form: {
    labelPosition: 'right' as 'left' | 'right' | 'top',
    labelWidth: '120px',
    size: 'default' as 'large' | 'default' | 'small',
    validateOnRuleChange: false,
    hideRequiredAsterisk: false
  },

  // 消息提示配置
  message: {
    duration: 3000,
    showClose: true,
    center: false
  },

  // 通知配置
  notification: {
    duration: 4500,
    position: 'top-right' as 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left'
  },

  // 路由配置
  router: {
    mode: 'history',
    base: '/',
    scrollBehavior: 'smooth'
  },

  // 主题配置
  theme: {
    primaryColor: '#409EFF',
    successColor: '#67C23A',
    warningColor: '#E6A23C',
    dangerColor: '#F56C6C',
    infoColor: '#909399'
  },

  // 布局配置
  layout: {
    sidebarWidth: 250,
    collapsedSidebarWidth: 64,
    headerHeight: 60,
    footerHeight: 50,
    tagsViewHeight: 34
  },

  // 性能配置
  performance: {
    lazyLoading: true,
    virtualScroll: true,
    debounceDelay: 300,
    throttleDelay: 100
  },

  // 开发配置
  development: {
    get showDevtools() {
      return getRuntimeConfig('development.showDevtools', import.meta.env.DEV)
    },
    get logLevel() {
      return getRuntimeConfig('development.logLevel', import.meta.env.VITE_LOG_LEVEL || 'info')
    }
  },

  // 生产配置
  production: {
    enableAnalytics: true,
    enableErrorReporting: true,
    enablePerformanceMonitoring: true
  }
}

// 环境特定配置
export const isDevelopment = import.meta.env.DEV
export const isProduction = import.meta.env.PROD

// 获取配置值的辅助函数
export const getConfig = (path: string, defaultValue?: any) => {
  // 首先尝试从运行时配置获取
  const runtimeValue = getRuntimeConfig(path, undefined)
  if (runtimeValue !== undefined) {
    return runtimeValue
  }
  
  // 然后从静态配置对象获取
  const keys = path.split('.')
  let result: any = config
  
  for (const key of keys) {
    if (result && typeof result === 'object' && key in result) {
      result = result[key]
    } else {
      return defaultValue
    }
  }
  
  return result
}

// 检查功能是否启用的辅助函数  
export const isFeatureEnabled = (feature: string): boolean => {
  return getConfig(`features.${feature}`, false)
}

// 获取API端点的辅助函数
export const getApiUrl = (endpoint: string): string => {
  const baseURL = config.api.baseURL.replace(/\/$/, '')
  const cleanEndpoint = endpoint.replace(/^\//, '')
  return `${baseURL}/${cleanEndpoint}`
}

// 配置更新监听器
const configUpdateListeners: Array<(config: any) => void> = []

// 添加配置更新监听器
export const onConfigUpdate = (listener: (config: any) => void) => {
  configUpdateListeners.push(listener)
  
  // 返回取消监听的函数
  return () => {
    const index = configUpdateListeners.indexOf(listener)
    if (index > -1) {
      configUpdateListeners.splice(index, 1)
    }
  }
}

// 监听配置更新事件
if (typeof window !== 'undefined') {
  window.addEventListener('app-config-updated', (event: any) => {
    const newConfig = event.detail
    configUpdateListeners.forEach(listener => {
      try {
        listener(newConfig)
      } catch (error) {
        console.error('[Config] Error in config update listener:', error)
      }
    })
  })
  
  // 配置加载完成事件
  window.addEventListener('app-config-loaded', (event: any) => {
    const loadedConfig = event.detail
    console.log('[Config] Runtime configuration loaded and ready:', loadedConfig)
  })
}

export default config