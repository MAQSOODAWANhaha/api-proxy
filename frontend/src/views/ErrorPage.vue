<template>
  <ErrorPageComponent 
    :type="errorType"
    :error-code="errorCode"
    :title="errorTitle"
    :description="errorDescription"
    :error-message="errorMessage"
    :request-id="requestId"
    :show-extra-info="showExtraInfo"
    :show-contact-support="true"
    :primary-action="handlePrimaryAction"
    :secondary-action="handleSecondaryAction"
  />
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ErrorPageComponent from '@/components/ui/ErrorPage.vue'

// 路由和路由器
const route = useRoute()
const router = useRouter()

// 计算属性
const errorType = computed(() => {
  return (route.params.type as string) || '404'
})

const errorCode = computed(() => {
  return route.query.code as string || errorType.value
})

const errorTitle = computed(() => {
  return route.query.title as string || undefined
})

const errorDescription = computed(() => {
  return route.query.description as string || undefined
})

const errorMessage = computed(() => {
  return route.query.message as string || undefined
})

const requestId = computed(() => {
  return route.query.requestId as string || undefined
})

const showExtraInfo = computed(() => {
  return route.query.showDetails === 'true' || process.env.NODE_ENV === 'development'
})

// 方法
function handlePrimaryAction(): void {
  const action = route.query.primaryAction as string
  
  switch (action) {
    case 'home':
      router.push('/')
      break
    case 'login':
      router.push('/login')
      break
    case 'reload':
      window.location.reload()
      break
    case 'back':
      if (window.history.length > 1) {
        router.back()
      } else {
        router.push('/')
      }
      break
    default:
      // 根据错误类型执行默认操作
      if (errorType.value === '404') {
        router.push('/')
      } else if (errorType.value === '403') {
        router.push('/login')
      } else {
        window.location.reload()
      }
  }
}

function handleSecondaryAction(): void {
  const action = route.query.secondaryAction as string
  
  switch (action) {
    case 'back':
      if (window.history.length > 1) {
        router.back()
      } else {
        router.push('/')
      }
      break
    case 'home':
      router.push('/')
      break
    case 'contact':
      // 联系客服逻辑
      console.log('联系客服')
      break
    default:
      if (window.history.length > 1) {
        router.back()
      } else {
        router.push('/')
      }
  }
}

// 生命周期
onMounted(() => {
  // 记录错误页面访问
  console.info(`访问错误页面: ${errorType.value}`, {
    code: errorCode.value,
    url: route.fullPath,
    timestamp: new Date().toISOString()
  })
})
</script>

<style scoped>
/* 错误页面视图不需要额外样式 */
</style>