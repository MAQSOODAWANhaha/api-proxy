<template>
  <ErrorBoundary 
    name="GlobalErrorBoundary"
    :show-retry="true"
    :show-reload="true" 
    :show-report="true"
    :show-go-back="false"
    :show-details="isDevelopment"
    @error="handleGlobalError"
    @retry="handleRetry"
  >
    <slot />
  </ErrorBoundary>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import ErrorBoundary from '@/components/ui/ErrorBoundary.vue'
import { handleError } from '@/utils/error'
import { notify } from '@/utils/notification'

// 环境检测
const isDevelopment = computed(() => process.env.NODE_ENV === 'development')

// 路由
const router = useRouter()

/**
 * 处理全局错误
 */
function handleGlobalError(error: any, info?: string): void {
  console.error('全局错误边界捕获到错误:', error, info)
  
  // 使用错误处理器处理错误
  handleError(error, 'GlobalErrorBoundary', {
    showNotification: true,
    showMessage: false,
    logError: true,
    reportError: true
  })
  
  // 特殊错误处理
  if (error?.name === 'ChunkLoadError') {
    notify.error('资源加载失败，建议刷新页面', {
      duration: 8000,
      showClose: true,
      onClick: () => window.location.reload()
    })
  }
}

/**
 * 处理重试
 */
function handleRetry(): void {
  // 尝试重新导航到当前路由
  const currentRoute = router.currentRoute.value
  
  if (currentRoute.path !== '/') {
    router.replace({ path: currentRoute.path, query: currentRoute.query })
  } else {
    window.location.reload()
  }
}
</script>

<style scoped>
/* 全局错误边界不需要额外样式 */
</style>