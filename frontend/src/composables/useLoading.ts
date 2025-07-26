/**
 * 加载状态管理组合式函数
 */

import { ref, computed, onUnmounted } from 'vue'
import { notify } from '@/utils/notification'

// 加载状态类型
export type LoadingType = 'global' | 'page' | 'component' | 'button' | 'inline'

// 加载配置
export interface LoadingConfig {
  type?: LoadingType
  text?: string
  delay?: number
  timeout?: number
  showProgress?: boolean
  cancelable?: boolean
  onCancel?: () => void
  onTimeout?: () => void
}

// 全局加载状态管理
const globalLoadingStates = ref<Map<string, boolean>>(new Map())
const loadingTexts = ref<Map<string, string>>(new Map())
const loadingTimeouts = ref<Map<string, NodeJS.Timeout>>(new Map())

/**
 * 使用加载状态
 */
export function useLoading(key = 'default') {
  const isLoading = ref(false)
  const loadingText = ref('加载中...')
  const progress = ref(0)
  const startTime = ref(0)
  const timeoutId = ref<NodeJS.Timeout | null>(null)
  const delayId = ref<NodeJS.Timeout | null>(null)

  // 计算属性
  const loadingDuration = computed(() => {
    return startTime.value ? Date.now() - startTime.value : 0
  })

  /**
   * 开始加载
   */
  const startLoading = (config: LoadingConfig = {}) => {
    const {
      type = 'component',
      text = '加载中...',
      delay = 0,
      timeout = 0,
      showProgress = false,
      cancelable = false,
      onCancel,
      onTimeout
    } = config

    // 清除之前的定时器
    if (delayId.value) {
      clearTimeout(delayId.value)
      delayId.value = null
    }
    if (timeoutId.value) {
      clearTimeout(timeoutId.value)
      timeoutId.value = null
    }

    const doStartLoading = () => {
      isLoading.value = true
      loadingText.value = text
      startTime.value = Date.now()
      progress.value = 0

      // 全局状态管理
      if (type === 'global') {
        globalLoadingStates.value.set(key, true)
        loadingTexts.value.set(key, text)
        
        notify.showLoading({
          text,
          lock: true,
          body: true
        })
      }

      // 进度条动画
      if (showProgress) {
        const progressInterval = setInterval(() => {
          if (!isLoading.value) {
            clearInterval(progressInterval)
            return
          }
          
          const elapsed = Date.now() - startTime.value
          const estimatedDuration = Math.max(3000, elapsed * 2)
          progress.value = Math.min(90, (elapsed / estimatedDuration) * 100)
        }, 100)
      }

      // 超时处理
      if (timeout > 0) {
        timeoutId.value = setTimeout(() => {
          console.warn(`加载超时: ${key} (${timeout}ms)`)
          
          if (onTimeout) {
            onTimeout()
          } else {
            stopLoading()
            notify.warningMessage('加载超时，请重试')
          }
        }, timeout)
      }
    }

    // 延迟加载
    if (delay > 0) {
      delayId.value = setTimeout(doStartLoading, delay)
    } else {
      doStartLoading()
    }
  }

  /**
   * 停止加载
   */
  const stopLoading = () => {
    isLoading.value = false
    progress.value = 100
    
    // 清除定时器
    if (timeoutId.value) {
      clearTimeout(timeoutId.value)
      timeoutId.value = null
    }
    if (delayId.value) {
      clearTimeout(delayId.value)
      delayId.value = null
    }

    // 全局状态清理
    globalLoadingStates.value.delete(key)
    loadingTexts.value.delete(key)
    
    // 如果没有其他全局加载状态，隐藏全局加载
    if (globalLoadingStates.value.size === 0) {
      notify.hideLoading()
    }

    // 重置状态
    setTimeout(() => {
      if (!isLoading.value) {
        progress.value = 0
        startTime.value = 0
      }
    }, 300)
  }

  /**
   * 切换加载状态
   */
  const toggleLoading = (config?: LoadingConfig) => {
    if (isLoading.value) {
      stopLoading()
    } else {
      startLoading(config)
    }
  }

  /**
   * 设置进度
   */
  const setProgress = (value: number) => {
    progress.value = Math.max(0, Math.min(100, value))
  }

  /**
   * 增加进度
   */
  const incrementProgress = (increment = 10) => {
    setProgress(progress.value + increment)
  }

  // 组件卸载时清理
  onUnmounted(() => {
    stopLoading()
  })

  return {
    // 状态
    isLoading: computed(() => isLoading.value),
    loadingText: computed(() => loadingText.value),
    progress: computed(() => progress.value),
    loadingDuration,
    
    // 方法
    startLoading,
    stopLoading,
    toggleLoading,
    setProgress,
    incrementProgress
  }
}

/**
 * 全局加载状态
 */
export function useGlobalLoading() {
  const hasGlobalLoading = computed(() => globalLoadingStates.value.size > 0)
  const globalLoadingText = computed(() => {
    const texts = Array.from(loadingTexts.value.values())
    return texts[texts.length - 1] || '加载中...'
  })

  return {
    hasGlobalLoading,
    globalLoadingText
  }
}

/**
 * 异步操作加载装饰器
 */
export function withLoading<T extends (...args: any[]) => Promise<any>>(
  fn: T,
  config: LoadingConfig = {}
): T {
  return (async (...args: any[]) => {
    const { startLoading, stopLoading } = useLoading(config.type || 'default')
    
    try {
      startLoading(config)
      const result = await fn(...args)
      return result
    } finally {
      stopLoading()
    }
  }) as T
}

/**
 * 页面加载管理
 */
export function usePageLoading() {
  const { startLoading, stopLoading, isLoading } = useLoading('page')
  
  const startPageLoading = (text = '页面加载中...') => {
    startLoading({
      type: 'page',
      text,
      delay: 200,
      timeout: 30000
    })
  }

  const stopPageLoading = () => {
    stopLoading()
  }

  return {
    isPageLoading: isLoading,
    startPageLoading,
    stopPageLoading
  }
}

/**
 * 按钮加载管理
 */
export function useButtonLoading() {
  const loadingButtons = ref<Set<string>>(new Set())

  const setButtonLoading = (buttonId: string, loading: boolean) => {
    if (loading) {
      loadingButtons.value.add(buttonId)
    } else {
      loadingButtons.value.delete(buttonId)
    }
  }

  const isButtonLoading = (buttonId: string) => {
    return loadingButtons.value.has(buttonId)
  }

  const withButtonLoading = async <T>(
    buttonId: string,
    fn: () => Promise<T>
  ): Promise<T> => {
    try {
      setButtonLoading(buttonId, true)
      return await fn()
    } finally {
      setButtonLoading(buttonId, false)
    }
  }

  return {
    setButtonLoading,
    isButtonLoading,
    withButtonLoading
  }
}