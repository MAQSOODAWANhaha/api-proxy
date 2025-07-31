/**
 * 运行时配置注入脚本
 * 
 * 此脚本在应用启动前加载，提供运行时配置注入功能
 * 配置通过容器环境变量在部署时动态生成
 */

// 默认配置（后备配置，防止配置文件缺失）
const DEFAULT_CONFIG = {
  api: {
    baseURL: 'http://localhost:9090/api',
    timeout: 30000,
    retryCount: 3,
    retryDelay: 1000
  },
  websocket: {
    url: 'ws://localhost:9090/ws',
    reconnectInterval: 5000,
    maxReconnectAttempts: 5,
    pingInterval: 30000
  },
  app: {
    name: 'AI代理平台',
    version: '1.0.0',
    author: 'AI Proxy Team',
    description: '企业级AI服务代理平台',
    debug: false
  },
  development: {
    mockData: false,
    showDevtools: false,
    logLevel: 'info'
  }
}

// 从环境变量或页面元数据中读取配置
function loadRuntimeConfig() {
  // 尝试从页面 meta 标签读取配置
  const apiBaseURL = document.querySelector('meta[name="api-base-url"]')?.getAttribute('content')
  const wsURL = document.querySelector('meta[name="ws-url"]')?.getAttribute('content')
  const appVersion = document.querySelector('meta[name="app-version"]')?.getAttribute('content')
  const logLevel = document.querySelector('meta[name="log-level"]')?.getAttribute('content')
  const mockData = document.querySelector('meta[name="mock-data"]')?.getAttribute('content')
  
  // 构建运行时配置
  const runtimeConfig = JSON.parse(JSON.stringify(DEFAULT_CONFIG))
  
  // API 配置
  if (apiBaseURL && apiBaseURL !== '{{VITE_API_BASE_URL}}') {
    runtimeConfig.api.baseURL = apiBaseURL
  }
  
  // WebSocket 配置
  if (wsURL && wsURL !== '{{VITE_WS_URL}}') {
    runtimeConfig.websocket.url = wsURL
  }
  
  // 应用配置
  if (appVersion && appVersion !== '{{VITE_APP_VERSION}}') {
    runtimeConfig.app.version = appVersion
  }
  
  // 开发配置
  if (logLevel && logLevel !== '{{VITE_LOG_LEVEL}}') {
    runtimeConfig.development.logLevel = logLevel
  }
  
  if (mockData && mockData !== '{{VITE_USE_MOCK}}') {
    runtimeConfig.development.mockData = mockData === 'true'
  }
  
  return runtimeConfig
}

// 配置验证和错误处理
function validateConfig(config) {
  const errors = []
  
  // 验证必需的配置项
  if (!config.api.baseURL) {
    errors.push('API baseURL is required')
  }
  
  if (!config.websocket.url) {
    errors.push('WebSocket URL is required')
  }
  
  // 验证 URL 格式
  try {
    new URL(config.api.baseURL)
  } catch (e) {
    errors.push(`Invalid API baseURL: ${config.api.baseURL}`)
  }
  
  if (errors.length > 0) {
    console.warn('[Config] Configuration validation warnings:', errors)
    console.warn('[Config] Using default configuration as fallback')
    return false
  }
  
  return true
}

// 初始化配置
function initializeConfig() {
  try {
    const config = loadRuntimeConfig()
    
    // 验证配置
    if (!validateConfig(config)) {
      console.warn('[Config] Using default configuration due to validation errors')
    }
    
    // 将配置挂载到全局对象
    window.__APP_CONFIG__ = config
    
    // 开发环境下输出配置信息
    if (config.development.showDevtools || window.location.hostname === 'localhost') {
      console.log('[Config] Runtime configuration loaded:', config)
    }
    
    // 触发自定义事件，通知应用配置已加载
    window.dispatchEvent(new CustomEvent('app-config-loaded', { detail: config }))
    
  } catch (error) {
    console.error('[Config] Failed to initialize runtime configuration:', error)
    console.warn('[Config] Using default configuration')
    window.__APP_CONFIG__ = DEFAULT_CONFIG
  }
}

// 页面加载完成后初始化配置
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initializeConfig)
} else {
  initializeConfig()
}

// 导出配置获取函数（供其他脚本使用）
window.getAppConfig = function(path, defaultValue) {
  const config = window.__APP_CONFIG__ || DEFAULT_CONFIG
  
  if (!path) return config
  
  const keys = path.split('.')
  let result = config
  
  for (const key of keys) {
    if (result && typeof result === 'object' && key in result) {
      result = result[key]
    } else {
      return defaultValue
    }
  }
  
  return result
}

// 配置更新函数（供热更新使用）
window.updateAppConfig = function(newConfig) {
  if (typeof newConfig === 'object' && newConfig !== null) {
    window.__APP_CONFIG__ = { ...window.__APP_CONFIG__, ...newConfig }
    window.dispatchEvent(new CustomEvent('app-config-updated', { detail: window.__APP_CONFIG__ }))
    console.log('[Config] Configuration updated:', window.__APP_CONFIG__)
  }
}