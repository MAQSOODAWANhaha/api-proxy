// 应用状态管理

import { defineStore } from 'pinia'
import { ref, computed, readonly } from 'vue'

export const useAppStore = defineStore('app', () => {
  // 状态
  const sidebarCollapsed = ref<boolean>(false)
  const loading = ref<boolean>(false)
  const breadcrumbs = ref<Array<{ name: string; path?: string }>>([])
  const theme = ref<'light' | 'dark'>('light')
  const deviceType = ref<'desktop' | 'tablet' | 'mobile'>('desktop')
  const globalLoadingCount = ref<number>(0)
  
  // 计算属性
  const isLoading = computed(() => globalLoadingCount.value > 0)
  const isMobile = computed(() => deviceType.value === 'mobile')
  const isTablet = computed(() => deviceType.value === 'tablet')
  const isDesktop = computed(() => deviceType.value === 'desktop')
  
  // 侧边栏控制
  const toggleSidebar = () => {
    sidebarCollapsed.value = !sidebarCollapsed.value
    // 保存到localStorage
    localStorage.setItem('sidebarCollapsed', String(sidebarCollapsed.value))
  }
  
  const setSidebarCollapsed = (collapsed: boolean) => {
    sidebarCollapsed.value = collapsed
    localStorage.setItem('sidebarCollapsed', String(collapsed))
  }
  
  // 全局loading控制
  const showLoading = () => {
    globalLoadingCount.value++
    loading.value = true
  }
  
  const hideLoading = () => {
    if (globalLoadingCount.value > 0) {
      globalLoadingCount.value--
    }
    loading.value = globalLoadingCount.value > 0
  }
  
  const resetLoading = () => {
    globalLoadingCount.value = 0
    loading.value = false
  }
  
  // 面包屑导航
  const setBreadcrumbs = (crumbs: Array<{ name: string; path?: string }>) => {
    breadcrumbs.value = crumbs
  }
  
  const addBreadcrumb = (crumb: { name: string; path?: string }) => {
    breadcrumbs.value.push(crumb)
  }
  
  const clearBreadcrumbs = () => {
    breadcrumbs.value = []
  }
  
  // 设备类型检测
  const updateDeviceType = () => {
    const width = window.innerWidth
    if (width < 768) {
      deviceType.value = 'mobile'
    } else if (width < 1024) {
      deviceType.value = 'tablet'
    } else {
      deviceType.value = 'desktop'
    }
    
    // 移动端自动折叠侧边栏
    if (deviceType.value === 'mobile') {
      sidebarCollapsed.value = true
    }
  }
  
  // 主题设置
  const setTheme = (newTheme: 'light' | 'dark') => {
    theme.value = newTheme
    localStorage.setItem('theme', newTheme)
    
    // 更新document类名
    if (newTheme === 'dark') {
      document.documentElement.classList.add('dark')
    } else {
      document.documentElement.classList.remove('dark')
    }
  }
  
  const toggleTheme = () => {
    setTheme(theme.value === 'light' ? 'dark' : 'light')
  }
  
  // 页面标题设置
  const setPageTitle = (title: string) => {
    document.title = title ? `${title} - AI代理平台` : 'AI代理平台'
  }
  
  // 初始化应用状态
  const initializeApp = () => {
    // 恢复侧边栏状态
    const savedSidebarState = localStorage.getItem('sidebarCollapsed')
    if (savedSidebarState !== null) {
      sidebarCollapsed.value = savedSidebarState === 'true'
    }
    
    // 恢复主题设置
    const savedTheme = localStorage.getItem('theme') as 'light' | 'dark' | null
    if (savedTheme) {
      setTheme(savedTheme)
    }
    
    // 初始化设备类型
    updateDeviceType()
    
    // 监听窗口大小变化
    window.addEventListener('resize', updateDeviceType)
  }
  
  // 网络状态
  const networkStatus = ref<'online' | 'offline'>('online')
  
  const updateNetworkStatus = () => {
    networkStatus.value = navigator.onLine ? 'online' : 'offline'
  }
  
  // 错误状态管理
  const errors = ref<Array<{ id: string; message: string; type: 'error' | 'warning'; timestamp: number }>>([])
  
  const addError = (message: string, type: 'error' | 'warning' = 'error') => {
    const error = {
      id: Date.now().toString(),
      message,
      type,
      timestamp: Date.now()
    }
    errors.value.push(error)
    
    // 自动清除错误（5秒后）
    setTimeout(() => {
      removeError(error.id)
    }, 5000)
  }
  
  const removeError = (id: string) => {
    const index = errors.value.findIndex(error => error.id === id)
    if (index > -1) {
      errors.value.splice(index, 1)
    }
  }
  
  const clearErrors = () => {
    errors.value = []
  }
  
  // 应用配置
  const config = ref({
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL || 'http://localhost:9090',
    enableDebug: import.meta.env.DEV,
    version: import.meta.env.VITE_APP_VERSION || '1.0.0',
    maxRetryAttempts: 3,
    requestTimeout: 30000,
    autoRefreshInterval: 30000 // 自动刷新间隔(毫秒)
  })
  
  // 性能监控
  const performanceMetrics = ref({
    loadTime: 0,
    firstContentfulPaint: 0,
    largestContentfulPaint: 0
  })
  
  const updatePerformanceMetrics = () => {
    // 页面加载时间
    const perfData = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming
    if (perfData) {
      performanceMetrics.value.loadTime = perfData.loadEventEnd - perfData.loadEventStart
    }
    
    // 其他性能指标可以通过Performance Observer API获取
  }
  
  // 清理函数
  const cleanup = () => {
    window.removeEventListener('resize', updateDeviceType)
  }
  
  // 监听网络状态变化
  window.addEventListener('online', updateNetworkStatus)
  window.addEventListener('offline', updateNetworkStatus)
  
  // 初始化
  initializeApp()
  updateNetworkStatus()
  
  return {
    // 状态
    sidebarCollapsed: readonly(sidebarCollapsed),
    loading: readonly(loading),
    breadcrumbs: readonly(breadcrumbs),
    theme: readonly(theme),
    deviceType: readonly(deviceType),
    networkStatus: readonly(networkStatus),
    errors: readonly(errors),
    config: readonly(config),
    performanceMetrics: readonly(performanceMetrics),
    
    // 计算属性
    isLoading,
    isMobile,
    isTablet,
    isDesktop,
    
    // 方法
    toggleSidebar,
    setSidebarCollapsed,
    showLoading,
    hideLoading,
    resetLoading,
    setBreadcrumbs,
    addBreadcrumb,
    clearBreadcrumbs,
    updateDeviceType,
    setTheme,
    toggleTheme,
    setPageTitle,
    initializeApp,
    updateNetworkStatus,
    addError,
    removeError,
    clearErrors,
    updatePerformanceMetrics,
    cleanup
  }
})