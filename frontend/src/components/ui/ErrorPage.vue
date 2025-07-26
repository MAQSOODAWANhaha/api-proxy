<template>
  <div class="error-page">
    <div class="error-container">
      <!-- 错误插图 -->
      <div class="error-illustration">
        <div :class="illustrationClasses">
          <component :is="errorIcon" />
        </div>
      </div>
      
      <!-- 错误内容 -->
      <div class="error-content">
        <h1 class="error-code">{{ errorCode }}</h1>
        <h2 class="error-title">{{ errorTitle }}</h2>
        <p class="error-description">{{ errorDescription }}</p>
        
        <!-- 错误建议 -->
        <div v-if="suggestions.length > 0" class="error-suggestions">
          <h3>您可以尝试：</h3>
          <ul>
            <li v-for="(suggestion, index) in suggestions" :key="index">
              {{ suggestion }}
            </li>
          </ul>
        </div>
        
        <!-- 搜索框（仅404页面） -->
        <div v-if="errorCode === '404'" class="error-search">
          <el-input
            v-model="searchQuery"
            placeholder="搜索您需要的内容..."
            size="lg"
            clearable
            @keyup.enter="handleSearch"
          >
            <template #append>
              <el-button @click="handleSearch">
                <el-icon><Search /></el-icon>
              </el-button>
            </template>
          </el-input>
        </div>
        
        <!-- 操作按钮 -->
        <div class="error-actions">
          <Button 
            type="primary" 
            size="lg"
            @click="handlePrimaryAction"
          >
            {{ primaryActionText }}
          </Button>
          
          <Button 
            v-if="showSecondaryAction"
            size="lg"
            @click="handleSecondaryAction"
          >
            {{ secondaryActionText }}
          </Button>
          
          <Button 
            v-if="showContactSupport"
            type="default"
            size="lg"
            @click="handleContactSupport"
          >
            联系客服
          </Button>
        </div>
        
        <!-- 额外信息 -->
        <div v-if="showExtraInfo" class="error-extra">
          <details>
            <summary>技术详细信息</summary>
            <div class="error-details">
              <p><strong>时间:</strong> {{ formatTime(timestamp) }}</p>
              <p><strong>URL:</strong> {{ currentUrl }}</p>
              <p><strong>用户代理:</strong> {{ userAgent }}</p>
              <p v-if="requestId"><strong>请求ID:</strong> {{ requestId }}</p>
              <p v-if="errorMessage"><strong>错误信息:</strong> {{ errorMessage }}</p>
            </div>
          </details>
        </div>
      </div>
    </div>
    
    <!-- 相关链接（仅404页面） -->
    <div v-if="errorCode === '404' && relatedLinks.length > 0" class="related-links">
      <h3>您可能在寻找：</h3>
      <div class="links-grid">
        <Card 
          v-for="link in relatedLinks" 
          :key="link.path"
          hoverable
          clickable
          @click="$router.push(link.path)"
        >
          <div class="link-item">
            <el-icon class="link-icon">
              <component :is="link.icon" />
            </el-icon>
            <div class="link-content">
              <h4>{{ link.title }}</h4>
              <p>{{ link.description }}</p>
            </div>
          </div>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { 
  Warning, 
  QuestionFilled, 
  Close,
  Lock, 
  Tools, 
  Search,
  House,
  DataLine,
  Setting,
  User,
  Key
} from '@element-plus/icons-vue'
import { Button, Card } from '@/components/ui'
import { notify } from '@/utils/notification'

// 错误类型
type ErrorPageType = '404' | '403' | '500' | '503' | 'network' | 'maintenance'

// 相关链接
interface RelatedLink {
  title: string
  description: string
  path: string
  icon: any
}

// 组件属性
interface Props {
  /** 错误代码 */
  errorCode?: string
  /** 错误类型 */
  type?: ErrorPageType
  /** 自定义标题 */
  title?: string
  /** 自定义描述 */
  description?: string
  /** 错误消息 */
  errorMessage?: string
  /** 请求ID */
  requestId?: string
  /** 是否显示技术详细信息 */
  showExtraInfo?: boolean
  /** 是否显示联系客服按钮 */
  showContactSupport?: boolean
  /** 自定义主要操作 */
  primaryAction?: () => void
  /** 自定义次要操作 */
  secondaryAction?: () => void
}

const props = withDefaults(defineProps<Props>(), {
  errorCode: '404',
  type: '404',
  showExtraInfo: false,
  showContactSupport: true
})

// 路由
const router = useRouter()
const route = useRoute()

// 响应式数据
const searchQuery = ref('')
const timestamp = ref(Date.now())
const currentUrl = ref(window.location.href)
const userAgent = ref(navigator.userAgent)

// 计算属性
const errorIcon = computed(() => {
  switch (props.type) {
    case '404':
      return QuestionFilled
    case '403':
      return Lock
    case '500':
      return Warning
    case '503':
    case 'maintenance':
      return Tools
    case 'network':
      return Close
    default:
      return Warning
  }
})

const illustrationClasses = computed(() => [
  'error-icon',
  `error-icon--${props.type}`
])

const errorTitle = computed(() => {
  if (props.title) return props.title
  
  switch (props.type) {
    case '404':
      return '页面未找到'
    case '403':
      return '访问被拒绝'
    case '500':
      return '服务器内部错误'
    case '503':
      return '服务不可用'
    case 'network':
      return '网络连接错误'
    case 'maintenance':
      return '系统维护中'
    default:
      return '发生了错误'
  }
})

const errorDescription = computed(() => {
  if (props.description) return props.description
  
  switch (props.type) {
    case '404':
      return '抱歉，您访问的页面不存在或已被删除。'
    case '403':
      return '您没有权限访问此页面，请联系管理员获取权限。'
    case '500':
      return '服务器遇到了一个错误，无法完成您的请求。'
    case '503':
      return '服务暂时不可用，请稍后再试。'
    case 'network':
      return '网络连接失败，请检查您的网络设置。'
    case 'maintenance':
      return '系统正在维护中，预计很快恢复正常。'
    default:
      return '系统遇到了一个未知错误。'
  }
})

const suggestions = computed(() => {
  switch (props.type) {
    case '404':
      return [
        '检查URL是否输入正确',
        '使用上方搜索框查找内容',
        '浏览下方的相关链接',
        '返回首页重新导航'
      ]
    case '403':
      return [
        '确认您已登录系统',
        '联系管理员申请权限',
        '尝试使用其他账号登录'
      ]
    case '500':
      return [
        '刷新页面重试',
        '稍后再试',
        '如果问题持续存在，请联系技术支持'
      ]
    case '503':
      return [
        '等待几分钟后重试',
        '检查系统公告了解维护信息',
        '联系技术支持了解详情'
      ]
    case 'network':
      return [
        '检查网络连接',
        '尝试刷新页面',
        '联系网络管理员'
      ]
    case 'maintenance':
      return [
        '关注官方公告了解恢复时间',
        '稍后重新访问',
        '如有紧急需求请联系客服'
      ]
    default:
      return [
        '刷新页面重试',
        '清除浏览器缓存',
        '联系技术支持'
      ]
  }
})

const primaryActionText = computed(() => {
  switch (props.type) {
    case '404':
      return '返回首页'
    case '403':
      return '重新登录'
    case 'network':
      return '重试连接'
    default:
      return '刷新页面'
  }
})

const secondaryActionText = computed(() => {
  switch (props.type) {
    case '404':
      return '返回上页'
    case '403':
      return '联系管理员'
    default:
      return '返回上页'
  }
})

const showSecondaryAction = computed(() => {
  return window.history.length > 1 || props.type === '403'
})

const relatedLinks = computed((): RelatedLink[] => {
  if (props.type !== '404') return []
  
  return [
    {
      title: '仪表板',
      description: '查看系统概览和关键指标',
      path: '/dashboard',
      icon: DataLine
    },
    {
      title: '用户中心',
      description: '管理您的个人信息',
      path: '/user-center',
      icon: User
    },
    {
      title: 'API密钥',
      description: '管理您的API访问密钥',
      path: '/api-keys',
      icon: Key
    },
    {
      title: '系统设置',
      description: '配置系统参数',
      path: '/settings',
      icon: Setting
    }
  ]
})

// 方法
const handlePrimaryAction = () => {
  if (props.primaryAction) {
    props.primaryAction()
    return
  }
  
  switch (props.type) {
    case '404':
      router.push('/')
      break
    case '403':
      router.push('/login')
      break
    case 'network':
      window.location.reload()
      break
    default:
      window.location.reload()
  }
}

const handleSecondaryAction = () => {
  if (props.secondaryAction) {
    props.secondaryAction()
    return
  }
  
  if (props.type === '403') {
    // 联系管理员的逻辑
    notify.info('请联系系统管理员获取访问权限')
    return
  }
  
  if (window.history.length > 1) {
    router.back()
  } else {
    router.push('/')
  }
}

const handleContactSupport = () => {
  // 这里可以集成客服系统
  notify.info('客服功能正在开发中，请通过邮件联系技术支持')
}

const handleSearch = () => {
  if (!searchQuery.value.trim()) {
    notify.warning('请输入搜索关键词')
    return
  }
  
  // 这里可以集成搜索功能
  router.push(`/search?q=${encodeURIComponent(searchQuery.value)}`)
}

const formatTime = (timestamp: number): string => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

// 生命周期
onMounted(() => {
  // 记录页面访问
  console.warn(`Error page visited: ${props.type} - ${props.errorCode}`)
})
</script>

<style scoped>
.error-page {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-8) var(--spacing-6);
  background: linear-gradient(135deg, var(--color-bg-primary) 0%, var(--color-bg-secondary) 100%);
}

.error-container {
  max-width: 600px;
  text-align: center;
  margin-bottom: var(--spacing-8);
}

.error-illustration {
  margin-bottom: var(--spacing-6);
}

.error-icon {
  font-size: 120px;
  color: var(--color-neutral-400);
  transition: all var(--transition-normal);
}

.error-icon--404 {
  color: var(--color-status-info);
}

.error-icon--403 {
  color: var(--color-status-warning);
}

.error-icon--500,
.error-icon--network {
  color: var(--color-status-danger);
}

.error-icon--503,
.error-icon--maintenance {
  color: var(--color-status-warning);
}

.error-code {
  font-size: var(--font-size-4xl);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-2);
  font-family: var(--font-family-mono);
}

.error-title {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-3);
}

.error-description {
  font-size: var(--font-size-lg);
  color: var(--color-text-secondary);
  line-height: var(--line-height-relaxed);
  margin-bottom: var(--spacing-6);
}

.error-suggestions {
  text-align: left;
  margin-bottom: var(--spacing-6);
  padding: var(--spacing-4);
  background-color: var(--color-bg-secondary);
  border-radius: var(--border-radius-lg);
  border-left: 4px solid var(--color-brand-primary);
}

.error-suggestions h3 {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-3);
}

.error-suggestions ul {
  margin: 0;
  padding-left: var(--spacing-6);
}

.error-suggestions li {
  color: var(--color-text-secondary);
  line-height: var(--line-height-relaxed);
  margin-bottom: var(--spacing-2);
}

.error-search {
  margin-bottom: var(--spacing-6);
}

.error-actions {
  display: flex;
  gap: var(--spacing-4);
  justify-content: center;
  flex-wrap: wrap;
  margin-bottom: var(--spacing-6);
}

.error-extra {
  margin-top: var(--spacing-6);
  text-align: left;
}

.error-extra details {
  border: 1px solid var(--color-border-secondary);
  border-radius: var(--border-radius-md);
  overflow: hidden;
}

.error-extra summary {
  padding: var(--spacing-3);
  background-color: var(--color-bg-tertiary);
  cursor: pointer;
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}

.error-extra summary:hover {
  background-color: var(--color-interactive-hover);
}

.error-details {
  padding: var(--spacing-4);
  background-color: var(--color-bg-primary);
}

.error-details p {
  margin-bottom: var(--spacing-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  word-break: break-all;
}

.error-details strong {
  color: var(--color-text-primary);
}

.related-links {
  max-width: 800px;
  margin-top: var(--spacing-8);
}

.related-links h3 {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-4);
  text-align: center;
}

.links-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: var(--spacing-4);
}

.link-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-4);
}

.link-icon {
  font-size: 24px;
  color: var(--color-brand-primary);
  flex-shrink: 0;
}

.link-content h4 {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-1);
}

.link-content p {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  line-height: var(--line-height-normal);
  margin: 0;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .error-page {
    padding: var(--spacing-6) var(--spacing-4);
  }
  
  .error-icon {
    font-size: 80px;
  }
  
  .error-code {
    font-size: var(--font-size-3xl);
  }
  
  .error-title {
    font-size: var(--font-size-xl);
  }
  
  .error-description {
    font-size: var(--font-size-base);
  }
  
  .error-actions {
    flex-direction: column;
    align-items: center;
  }
  
  .error-actions .ui-button {
    width: 100%;
    max-width: 240px;
  }
  
  .links-grid {
    grid-template-columns: 1fr;
  }
  
  .link-item {
    flex-direction: column;
    text-align: center;
  }
  
  .error-suggestions {
    text-align: center;
  }
  
  .error-suggestions ul {
    text-align: left;
  }
}

/* 深色主题适配 */
.theme-dark .error-page {
  background: linear-gradient(135deg, var(--color-bg-primary) 0%, var(--color-bg-secondary) 100%);
}

/* 动画效果 */
.error-icon {
  animation: float 3s ease-in-out infinite;
}

@keyframes float {
  0%, 100% {
    transform: translateY(0px);
  }
  50% {
    transform: translateY(-10px);
  }
}

/* 打印样式 */
@media print {
  .error-page {
    background: none;
    color: black;
  }
  
  .error-actions,
  .error-search {
    display: none;
  }
  
  .related-links {
    break-inside: avoid;
  }
}
</style>