<template>
  <div id="app">
    <!-- 全局错误边界 -->
    <GlobalErrorBoundary>
      <!-- 设置当前主题类 -->
      <div :class="themeClass">
        <router-view />
      </div>
    </GlobalErrorBoundary>
  </div>
</template>

<script setup lang="ts">
import { watch, computed, onMounted, onErrorCaptured } from 'vue'
import { useUserStore } from '@/stores/user'
import { useI18n } from '@/locales'
import GlobalErrorBoundary from '@/components/layouts/GlobalErrorBoundary.vue'
import { handleError } from '@/utils/error'

const userStore = useUserStore()
const { locale, t } = useI18n()

// 计算主题类名
const themeClass = computed(() => {
  return `theme-${userStore.theme || 'light'}`
})

// Watch for changes in the store's lang property and update i18n
watch(
  () => userStore.lang,
  (newLang) => {
    locale.value = newLang as 'en' | 'zh'
  },
  { immediate: true } // Run immediately on component mount
)

// 捕获未被子组件错误边界捕获的错误
onErrorCaptured((error, instance, info) => {
  console.error('App级别错误捕获:', error, info)
  
  handleError(error, 'app-error-captured', {
    showNotification: true,
    showMessage: false,
    logError: true,
    reportError: true
  })
  
  // 返回false阻止错误向上传播
  return false
})

// 全局未处理的Promise错误
const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
  console.error('未处理的Promise错误:', event.reason)
  
  handleError(event.reason, 'unhandled-promise-rejection', {
    showNotification: true,
    showMessage: false,
    logError: true,
    reportError: true
  })
  
  // 阻止默认行为（控制台警告）
  event.preventDefault()
}

// 全局JavaScript错误
const handleGlobalError = (event: ErrorEvent) => {
  console.error('全局JavaScript错误:', event.error)
  
  handleError(event.error || new Error(event.message), 'global-javascript-error', {
    showNotification: true,
    showMessage: false,
    logError: true,
    reportError: true
  })
}

// 组件挂载时添加全局错误监听器
onMounted(() => {
  window.addEventListener('unhandledrejection', handleUnhandledRejection)
  window.addEventListener('error', handleGlobalError)
  
  // 设置应用初始状态
  document.title = t('login.subtitle')
  
  // 添加全局CSS类用于主题切换
  document.documentElement.className = themeClass.value
})

// 监听主题变化，更新文档根元素类名
watch(themeClass, (newTheme) => {
  document.documentElement.className = newTheme
}, { immediate: true })

// 组件卸载时移除全局错误监听器
import { onBeforeUnmount } from 'vue'
onBeforeUnmount(() => {
  window.removeEventListener('unhandledrejection', handleUnhandledRejection)
  window.removeEventListener('error', handleGlobalError)
})
</script>

<style>
/* 确保应用占满全屏 */
#app {
  min-height: 100vh;
  width: 100%;
}

/* 主题过渡动画 */
#app > div {
  transition: background-color var(--transition-normal), color var(--transition-normal);
}
</style>
