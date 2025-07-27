import { ElLoading } from 'element-plus'
import type { LoadingInstance } from 'element-plus/es/components/loading/src/loading'

interface LoadingConfig {
  text?: string
  background?: string
  customClass?: string
  lock?: boolean
  target?: string | HTMLElement
}

class LoadingManager {
  private loadingInstances: Map<string, LoadingInstance> = new Map()
  private loadingCount = 0
  private globalLoading: LoadingInstance | null = null

  // 显示全局加载
  showGlobal(config?: LoadingConfig): string {
    const loadingId = 'global-loading'
    
    if (this.globalLoading) {
      return loadingId
    }

    this.globalLoading = ElLoading.service({
      lock: config?.lock ?? true,
      text: config?.text || '加载中...',
      background: config?.background || 'rgba(0, 0, 0, 0.7)',
      customClass: config?.customClass || ''
    })

    this.loadingCount++
    return loadingId
  }

  // 隐藏全局加载
  hideGlobal(): void {
    if (this.globalLoading) {
      this.globalLoading.close()
      this.globalLoading = null
      this.loadingCount = Math.max(0, this.loadingCount - 1)
    }
  }

  // 显示局部加载
  show(target?: string | HTMLElement, config?: LoadingConfig): string {
    const loadingId = `loading-${Date.now()}-${Math.random()}`
    
    const loadingInstance = ElLoading.service({
      target: target || document.body,
      lock: config?.lock ?? false,
      text: config?.text || '加载中...',
      background: config?.background || 'rgba(255, 255, 255, 0.8)',
      customClass: config?.customClass || ''
    })

    this.loadingInstances.set(loadingId, loadingInstance)
    this.loadingCount++
    
    return loadingId
  }

  // 隐藏指定加载
  hide(loadingId: string): void {
    if (loadingId === 'global-loading') {
      this.hideGlobal()
      return
    }

    const loadingInstance = this.loadingInstances.get(loadingId)
    if (loadingInstance) {
      loadingInstance.close()
      this.loadingInstances.delete(loadingId)
      this.loadingCount = Math.max(0, this.loadingCount - 1)
    }
  }

  // 隐藏所有加载
  hideAll(): void {
    // 隐藏全局加载
    this.hideGlobal()

    // 隐藏所有局部加载
    this.loadingInstances.forEach((instance) => {
      instance.close()
    })
    this.loadingInstances.clear()
    this.loadingCount = 0
  }

  // 获取当前加载数量
  getLoadingCount(): number {
    return this.loadingCount
  }

  // 检查是否有加载中
  isLoading(): boolean {
    return this.loadingCount > 0
  }

  // 显示页面级加载（用于路由切换）
  showPageLoading(): string {
    return this.showGlobal({
      text: '页面加载中...',
      background: 'rgba(255, 255, 255, 0.9)'
    })
  }

  // 显示请求加载（用于API请求）
  showRequestLoading(text = '请求处理中...'): string {
    return this.showGlobal({
      text,
      background: 'rgba(0, 0, 0, 0.3)'
    })
  }

  // 显示表格加载
  showTableLoading(target: string | HTMLElement): string {
    return this.show(target, {
      text: '数据加载中...',
      background: 'rgba(255, 255, 255, 0.8)',
      lock: false
    })
  }

  // 显示图表加载
  showChartLoading(target: string | HTMLElement): string {
    return this.show(target, {
      text: '图表渲染中...',
      background: 'rgba(255, 255, 255, 0.9)',
      lock: false
    })
  }
}

// 创建加载管理器实例
export const loadingManager = new LoadingManager()

// 导出类以便自定义配置
export { LoadingManager }

// 便捷方法
export const showLoading = (target?: string | HTMLElement, config?: LoadingConfig) => {
  return loadingManager.show(target, config)
}

export const hideLoading = (loadingId: string) => {
  loadingManager.hide(loadingId)
}

export const showGlobalLoading = (config?: LoadingConfig) => {
  return loadingManager.showGlobal(config)
}

export const hideGlobalLoading = () => {
  loadingManager.hideGlobal()
}