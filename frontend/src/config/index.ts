// 应用配置文件

export const config = {
  // API相关配置
  api: {
    baseURL: import.meta.env.VITE_API_BASE_URL || 'http://localhost:9090/api',
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
    version: import.meta.env.VITE_APP_VERSION || '1.0.0',
    author: 'AI Proxy Team',
    description: '企业级AI服务代理平台',
    debug: import.meta.env.DEV || false
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
    url: import.meta.env.VITE_WS_URL || 'ws://localhost:9090/ws',
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
    mockData: import.meta.env.VITE_USE_MOCK === 'true',
    showDevtools: import.meta.env.DEV,
    logLevel: import.meta.env.VITE_LOG_LEVEL || 'info'
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

export default config