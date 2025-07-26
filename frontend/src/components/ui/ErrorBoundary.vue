<template>
  <div v-if="!hasError">
    <slot />
  </div>
  
  <!-- 错误状态 -->
  <div v-else class="error-boundary">
    <Card :class="errorCardClasses" variant="outlined">
      <!-- 错误图标 -->
      <div class="error-icon">
        <component :is="errorIcon" />
      </div>
      
      <!-- 错误信息 -->
      <div class="error-content">
        <h3 class="error-title">{{ errorTitle }}</h3>
        <p class="error-message">{{ errorMessage }}</p>
        
        <!-- 错误详情（可展开） -->
        <details v-if="showDetails && errorDetails" class="error-details">
          <summary>错误详细信息</summary>
          <pre class="error-stack">{{ errorDetails }}</pre>
        </details>
        
        <!-- 建议操作 -->
        <div v-if="suggestions.length > 0" class="error-suggestions">
          <h4>建议解决方案：</h4>
          <ul>
            <li v-for="(suggestion, index) in suggestions" :key="index">
              {{ suggestion }}
            </li>
          </ul>
        </div>
      </div>
      
      <!-- 操作按钮 -->
      <div class="error-actions">
        <Button 
          v-if="showRetry"
          type="primary"
          :loading="retrying"
          @click="handleRetry"
        >
          重试
        </Button>
        
        <Button 
          v-if="showReload"
          type="default"
          @click="handleReload"
        >
          刷新页面
        </Button>
        
        <Button 
          v-if="showReport"
          type="default"
          @click="handleReport"
        >
          报告问题
        </Button>
        
        <Button 
          v-if="showGoBack"
          type="default"
          @click="handleGoBack"
        >
          返回上页
        </Button>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onErrorCaptured, watch } from 'vue'
import { useRouter } from 'vue-router'
import { 
  Warning, 
  RefreshLeft, 
  Close, 
  QuestionFilled 
} from '@element-plus/icons-vue'
import { Card, Button } from '@/components/ui'
import { handleError, createError, ErrorType } from '@/utils/error'
import { notify } from '@/utils/notification'

// 错误类型枚举
enum BoundaryErrorType {
  RENDER = 'render',
  ASYNC = 'async',
  NETWORK = 'network',
  COMPONENT = 'component',
  UNKNOWN = 'unknown'
}

// 组件属性
interface Props {
  /** 错误边界名称，用于错误上报 */
  name?: string
  /** 是否显示重试按钮 */
  showRetry?: boolean
  /** 是否显示刷新页面按钮 */
  showReload?: boolean
  /** 是否显示报告问题按钮 */
  showReport?: boolean
  /** 是否显示返回按钮 */
  showGoBack?: boolean
  /** 是否显示错误详情 */
  showDetails?: boolean
  /** 自定义错误处理函数 */
  onError?: (error: any, instance: any, info: string) => void
  /** 自定义重试函数 */
  onRetry?: () => void | Promise<void>
  /** 错误恢复策略 */
  fallbackComponent?: any
}

const props = withDefaults(defineProps<Props>(), {
  name: 'ErrorBoundary',
  showRetry: true,
  showReload: true,
  showReport: false,
  showGoBack: true,
  showDetails: process.env.NODE_ENV === 'development'
})

// 组件事件
const emit = defineEmits<{
  error: [error: any, info?: string]
  retry: []
  recover: []
}>()

// 响应式数据
const hasError = ref(false)
const errorInfo = ref<any>(null)
const errorDetails = ref<string>('')
const retrying = ref(false)
const errorType = ref<BoundaryErrorType>(BoundaryErrorType.UNKNOWN)

// 路由器
const router = useRouter()

// 计算属性
const errorIcon = computed(() => {
  switch (errorType.value) {
    case BoundaryErrorType.NETWORK:
      return RefreshLeft
    case BoundaryErrorType.RENDER:
    case BoundaryErrorType.COMPONENT:
      return Warning
    default:
      return QuestionFilled
  }
})

const errorTitle = computed(() => {
  switch (errorType.value) {
    case BoundaryErrorType.RENDER:
      return '页面渲染错误'
    case BoundaryErrorType.ASYNC:
      return '异步操作错误'
    case BoundaryErrorType.NETWORK:
      return '网络连接错误'
    case BoundaryErrorType.COMPONENT:
      return '组件错误'
    default:
      return '未知错误'
  }
})

const errorMessage = computed(() => {
  if (!errorInfo.value) return '发生了未知错误'
  
  if (typeof errorInfo.value === 'string') {
    return errorInfo.value
  }
  
  if (errorInfo.value.message) {
    return errorInfo.value.message
  }
  
  return '应用程序遇到了一个错误，无法继续运行'
})

const errorCardClasses = computed(() => [
  'error-card',
  `error-card--${errorType.value}`
])

const suggestions = computed(() => {
  const baseSuggestions = []
  
  switch (errorType.value) {
    case BoundaryErrorType.NETWORK:
      baseSuggestions.push(
        '检查网络连接是否正常',
        '尝试刷新页面',
        '如果问题持续存在，请联系管理员'
      )
      break
    case BoundaryErrorType.RENDER:
      baseSuggestions.push(
        '尝试刷新页面',
        '清除浏览器缓存',
        '使用其他浏览器访问'
      )
      break
    case BoundaryErrorType.COMPONENT:
      baseSuggestions.push(
        '尝试重新执行操作',
        '刷新页面重试',
        '检查输入数据是否正确'
      )
      break
    default:
      baseSuggestions.push(
        '尝试刷新页面',
        '如果问题持续存在，请联系技术支持',
        '您可以尝试返回上一页继续使用'
      )
  }
  
  return baseSuggestions
})

// 错误捕获
onErrorCaptured((error, instance, info) => {
  console.error('ErrorBoundary 捕获到错误:', error, info)
  
  hasError.value = true
  errorInfo.value = error
  errorDetails.value = error.stack || error.toString()
  errorType.value = determineErrorType(error, info)
  
  // 创建标准化错误对象
  const appError = createError(
    `BOUNDARY_ERROR_${errorType.value.toUpperCase()}`,
    error.message || error.toString(),
    {
      boundaryName: props.name,
      componentInfo: info,
      errorType: errorType.value,
      instance: instance?.$?.type?.name || 'Unknown'
    },
    error.stack
  )
  
  // 处理错误
  handleError(appError, `ErrorBoundary:${props.name}`, {
    showNotification: false, // 错误边界自己处理UI显示
    showMessage: false,
    logError: true,
    reportError: true
  })
  
  // 触发自定义错误处理
  if (props.onError) {
    props.onError(error, instance, info)
  }
  
  // 发出错误事件
  emit('error', error, info)
  
  // 阻止错误向上传播
  return false
})

// 确定错误类型
function determineErrorType(error: any, info: string): BoundaryErrorType {
  if (info.includes('render') || info.includes('template')) {
    return BoundaryErrorType.RENDER
  }
  
  if (error.name === 'ChunkLoadError' || error.message?.includes('Loading chunk')) {
    return BoundaryErrorType.NETWORK
  }
  
  if (error.name === 'TypeError' && error.message?.includes('Cannot read')) {
    return BoundaryErrorType.COMPONENT
  }
  
  if (error.name === 'NetworkError' || error.message?.includes('fetch')) {
    return BoundaryErrorType.NETWORK
  }
  
  if (info.includes('async')) {
    return BoundaryErrorType.ASYNC
  }
  
  return BoundaryErrorType.UNKNOWN
}

// 处理重试
async function handleRetry(): Promise<void> {
  try {
    retrying.value = true
    
    // 执行自定义重试逻辑
    if (props.onRetry) {
      await props.onRetry()
    }
    
    // 重置错误状态
    hasError.value = false
    errorInfo.value = null
    errorDetails.value = ''
    
    emit('retry')
    emit('recover')
    
    notify.successMessage('重试成功')
  } catch (retryError) {
    console.error('重试失败:', retryError)
    notify.errorMessage('重试失败，请尝试刷新页面')
    
    // 处理重试错误
    handleError(retryError, `ErrorBoundary:${props.name}:retry`)
  } finally {
    retrying.value = false
  }
}

// 处理页面刷新
function handleReload(): void {
  window.location.reload()
}

// 处理问题报告
function handleReport(): void {
  const reportData = {
    error: errorInfo.value?.toString(),
    stack: errorDetails.value,
    url: window.location.href,
    userAgent: navigator.userAgent,
    timestamp: new Date().toISOString(),
    boundaryName: props.name
  }
  
  // 这里可以集成问题报告系统
  console.info('问题报告数据:', reportData)
  
  notify.info('问题报告已发送，感谢您的反馈！')
}

// 处理返回上页
function handleGoBack(): void {
  if (window.history.length > 1) {
    router.back()
  } else {
    router.push('/')
  }
}

// 监听错误状态变化
watch(hasError, (newValue) => {
  if (!newValue) {
    emit('recover')
  }
})

// 暴露方法给父组件
defineExpose({
  hasError: () => hasError.value,
  reset: () => {
    hasError.value = false
    errorInfo.value = null
    errorDetails.value = ''
  },
  getError: () => errorInfo.value
})
</script>

<style scoped>
.error-boundary {
  min-height: 300px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-6);
}

.error-card {
  max-width: 600px;
  text-align: center;
}

.error-card--render {
  border-color: var(--color-status-warning);
}

.error-card--network {
  border-color: var(--color-status-info);
}

.error-card--component,
.error-card--async,
.error-card--unknown {
  border-color: var(--color-status-danger);
}

.error-icon {
  font-size: 64px;
  margin-bottom: var(--spacing-4);
  color: var(--color-status-danger);
}

.error-card--render .error-icon {
  color: var(--color-status-warning);
}

.error-card--network .error-icon {
  color: var(--color-status-info);
}

.error-content {
  margin-bottom: var(--spacing-6);
}

.error-title {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-3);
}

.error-message {
  font-size: var(--font-size-base);
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-4);
  line-height: var(--line-height-relaxed);
}

.error-details {
  text-align: left;
  margin: var(--spacing-4) 0;
  border: 1px solid var(--color-border-secondary);
  border-radius: var(--border-radius-md);
  overflow: hidden;
}

.error-details summary {
  padding: var(--spacing-3);
  background-color: var(--color-bg-secondary);
  cursor: pointer;
  font-weight: var(--font-weight-medium);
  border-bottom: 1px solid var(--color-border-secondary);
}

.error-details summary:hover {
  background-color: var(--color-bg-tertiary);
}

.error-stack {
  padding: var(--spacing-4);
  background-color: var(--color-bg-primary);
  font-family: var(--font-family-mono);
  font-size: var(--font-size-sm);
  line-height: var(--line-height-relaxed);
  color: var(--color-text-primary);
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
}

.error-suggestions {
  text-align: left;
  margin-top: var(--spacing-4);
  padding: var(--spacing-4);
  background-color: var(--color-bg-secondary);
  border-radius: var(--border-radius-md);
  border-left: 4px solid var(--color-status-info);
}

.error-suggestions h4 {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-2);
}

.error-suggestions ul {
  margin: 0;
  padding-left: var(--spacing-5);
}

.error-suggestions li {
  color: var(--color-text-secondary);
  line-height: var(--line-height-relaxed);
  margin-bottom: var(--spacing-1);
}

.error-actions {
  display: flex;
  gap: var(--spacing-3);
  justify-content: center;
  flex-wrap: wrap;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .error-boundary {
    padding: var(--spacing-4);
    min-height: 250px;
  }
  
  .error-icon {
    font-size: 48px;
    margin-bottom: var(--spacing-3);
  }
  
  .error-title {
    font-size: var(--font-size-xl);
  }
  
  .error-actions {
    flex-direction: column;
    align-items: center;
  }
  
  .error-actions .ui-button {
    width: 100%;
    max-width: 200px;
  }
}

/* 深色主题适配 */
.theme-dark .error-stack {
  background-color: var(--color-bg-tertiary);
}

.theme-dark .error-details summary {
  background-color: var(--color-bg-tertiary);
}

.theme-dark .error-details summary:hover {
  background-color: var(--color-bg-secondary);
}

/* 打印样式 */
@media print {
  .error-boundary {
    break-inside: avoid;
  }
  
  .error-actions {
    display: none;
  }
  
  .error-details {
    border: 2px solid #000;
  }
}
</style>