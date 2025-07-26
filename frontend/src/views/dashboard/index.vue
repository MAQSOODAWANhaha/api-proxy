<template>
  <PageContainer 
    :title="$t('dashboard.title')" 
    :description="$t('dashboard.overview')"
    :breadcrumb="breadcrumb"
  >
    <template #extra>
      <div class="dashboard-controls">
        <LanguageSelector size="small" variant="minimal" />
        <ThemeToggle size="small" />
      </div>
    </template>
    <!-- 统计卡片 -->
    <Grid :cols="{ xs: 1, sm: 2, lg: 4 }" :gap="6" class="mb-8">
      <GridItem>
        <StatCard
          :title="$t('dashboard.todayRequests')"
          :value="todayRequests"
          :icon="DataLine"
          type="primary"
          :change="requestsChange.change"
          :change-type="requestsChange.changeType"
          hoverable
          :formatter="formatNumber"
          :description="$t('dashboard.last24Hours')"
        />
      </GridItem>
      
      <GridItem>
        <StatCard
          :title="$t('dashboard.successRate')"
          :value="successRate"
          :icon="Select"
          type="success"
          :change="successRateChange.change"
          :change-type="successRateChange.changeType"
          hoverable
          :formatter="(v) => v + '%'"
          :description="$t('dashboard.last24Hours')"
        />
      </GridItem>
      
      <GridItem>
        <StatCard
          title="Token总使用量"
          :value="totalTokens"
          :icon="Coin"
          type="warning"
          :change="tokensChange.change"
          :change-type="tokensChange.changeType"
          hoverable
          :formatter="formatTokens"
          :description="$t('dashboard.last24Hours')"
        />
      </GridItem>
      
      <GridItem>
        <StatCard
          title="不健康密钥"
          :value="unhealthyKeysCount"
          :icon="Warning"
          type="danger"
          hoverable
          clickable
          @click="scrollToUnhealthyKeys"
          description="需要关注"
        />
      </GridItem>
    </Grid>

    <!-- 图表区域 -->
    <Grid :cols="{ xs: 1, lg: 3 }" :gap="6" class="mb-8">
      <GridItem :span="{ xs: 1, lg: 2 }">
        <Card :title="$t('dashboard.requestTrend')" :subtitle="$t('dashboard.last7Days')">
          <template #extra>
            <div class="flex gap-2">
              <Tag 
                v-for="period in timePeriods" 
                :key="period.value"
                :type="selectedPeriod === period.value ? 'primary' : 'default'"
                :variant="selectedPeriod === period.value ? 'filled' : 'outlined'"
                clickable
                size="sm"
                @click="selectedPeriod = period.value"
              >
                {{ period.label }}
              </Tag>
            </div>
          </template>
          
          <LineChart
            :series="trendChartSeries"
            :x-axis-data="trendChartDates"
            :loading="chartsLoading"
            :height="400"
            smooth
            area
          />
        </Card>
      </GridItem>
      
      <GridItem>
        <Card :title="$t('dashboard.providerDistribution')" subtitle="请求量占比">
          <PieChart
            :data="providerDistributionData"
            :loading="chartsLoading"
            :height="400"
            inner-radius="30%"
            outer-radius="70%"
          />
        </Card>
      </GridItem>
    </Grid>

    <!-- 实时监控区域 -->
    <Grid :cols="{ xs: 1, md: 2 }" :gap="6" class="mb-8">
      <GridItem>
        <Card title="实时请求监控" variant="elevated">
          <template #extra>
            <Badge :count="realTimeRequests.length" type="primary" />
          </template>
          
          <div class="realtime-monitor">
            <div class="realtime-header">
              <div class="realtime-status">
                <div class="status-dot" :class="{ 'status-dot--active': isRealTimeActive }" />
                <span class="status-text">
                  {{ isRealTimeActive ? '实时监控中' : '监控已暂停' }}
                </span>
              </div>
              <Button 
                :type="isRealTimeActive ? 'default' : 'primary'" 
                size="sm"
                @click="toggleRealTimeMonitoring"
              >
                {{ isRealTimeActive ? '暂停' : '开始' }}
              </Button>
            </div>
            
            <div class="realtime-requests">
              <TransitionGroup name="request-item" tag="div">
                <div 
                  v-for="request in recentRequests" 
                  :key="request.id"
                  class="request-item"
                >
                  <div class="request-time">{{ formatTime(request.timestamp) }}</div>
                  <div class="request-info">
                    <Tag :type="request.status === 'success' ? 'success' : 'danger'" size="xs">
                      {{ request.provider }}
                    </Tag>
                    <span class="request-duration">{{ request.duration }}ms</span>
                  </div>
                </div>
              </TransitionGroup>
            </div>
          </div>
        </Card>
      </GridItem>
      
      <GridItem>
        <Card title="系统健康状态" variant="elevated">
          <div class="health-metrics">
            <div class="health-item">
              <div class="health-label">服务状态</div>
              <div class="health-value">
                <Tag type="success" size="sm">正常运行</Tag>
              </div>
            </div>
            
            <div class="health-item">
              <div class="health-label">平均响应时间</div>
              <div class="health-value">{{ averageResponseTime }}ms</div>
            </div>
            
            <div class="health-item">
              <div class="health-label">可用服务商</div>
              <div class="health-value">{{ healthyProvidersCount }}/{{ totalProvidersCount }}</div>
            </div>
            
            <div class="health-item">
              <div class="health-label">内存使用率</div>
              <div class="health-value">
                <div class="progress-bar">
                  <div 
                    class="progress-fill" 
                    :style="{ width: memoryUsage + '%' }"
                    :class="{
                      'progress-fill--warning': memoryUsage > 70,
                      'progress-fill--danger': memoryUsage > 85
                    }"
                  />
                </div>
                <span class="progress-text">{{ memoryUsage }}%</span>
              </div>
            </div>
          </div>
        </Card>
      </GridItem>
    </Grid>

    <!-- 不健康密钥表格 -->
    <Card 
      ref="unhealthyKeysCard"
      title="不健康密钥详情" 
      :subtitle="`共 ${unhealthyKeysCount} 个密钥需要关注`"
      class="mb-8"
    >
      <template #extra>
        <div class="flex gap-2">
          <Button size="sm" @click="refreshUnhealthyKeys">
            刷新
          </Button>
          <Button type="primary" size="sm" @click="batchCheckHealth">
            批量检查
          </Button>
        </div>
      </template>
      
      <el-table 
        :data="unhealthyKeys" 
        stripe
        empty-text="暂无不健康密钥"
        class="unhealthy-keys-table"
      >
        <el-table-column prop="name" label="密钥名称" min-width="150">
          <template #default="{ row }">
            <div class="key-name">
              <Tag size="xs" :type="getProviderType(row.provider)">
                {{ row.provider }}
              </Tag>
              <span>{{ row.name }}</span>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="lastFailure" label="上次失败时间" min-width="180">
          <template #default="{ row }">
            <div class="failure-time">
              <span>{{ formatDateTime(row.lastFailure) }}</span>
              <Tag size="xs" type="info">{{ getTimeAgo(row.lastFailure) }}</Tag>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="errorMessage" label="错误信息" min-width="200">
          <template #default="{ row }">
            <div class="error-message" :title="row.errorMessage">
              {{ truncateText(row.errorMessage, 50) }}
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="failureCount" label="连续失败次数" width="120" align="center">
          <template #default="{ row }">
            <Badge :count="row.failureCount" :type="row.failureCount > 5 ? 'danger' : 'warning'" />
          </template>
        </el-table-column>
        
        <el-table-column label="操作" width="150" align="center">
          <template #default="{ row }">
            <div class="table-actions">
              <Button size="xs" @click="checkSingleHealth(row)">
                检查
              </Button>
              <Button size="xs" type="danger" @click="disableKey(row)">
                禁用
              </Button>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </Card>
  </PageContainer>
</template>

<script lang="ts" setup>
import { ref, onMounted, onBeforeUnmount, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { DataLine, Select, Coin, Warning } from '@element-plus/icons-vue'
import { useI18n } from '@/locales'

// 导入组件和工具
import { 
  PageContainer, 
  Grid, 
  GridItem, 
  Card, 
  Button, 
  Tag, 
  Badge,
  LanguageSelector,
  ThemeToggle
} from '@/components/ui'
import { 
  LineChart, 
  PieChart, 
  StatCard, 
  formatNumber as chartFormatNumber,
  calculateChange 
} from '@/components/charts'

// 导入API
import { getDailyStats, type DailyStat, type ProviderDistribution } from '@/api/statistics'
import { getHealthStatuses, type HealthStatus } from '@/api/health'

// 国际化
const { t } = useI18n()

// 面包屑导航
const breadcrumb = computed(() => [
  { title: t('nav.dashboard'), path: '/' },
  { title: t('nav.dashboard') }
])

// 数据状态
const statsData = ref<{ stats: DailyStat[], distribution: ProviderDistribution[] }>({
  stats: [],
  distribution: []
})
const unhealthyKeys = ref<HealthStatus[]>([])
const chartsLoading = ref(true)
const selectedPeriod = ref('7d')
const unhealthyKeysCard = ref()

// 实时监控状态
const isRealTimeActive = ref(false)
const realTimeRequests = ref<any[]>([])
const recentRequests = ref<any[]>([])
let realTimeInterval: number | null = null

// 时间周期配置
const timePeriods = computed(() => [
  { value: '24h', label: t('dashboard.last24Hours') },
  { value: '7d', label: t('dashboard.last7Days') },
  { value: '30d', label: t('dashboard.last30Days') }
])

// 计算统计数据
const todayRequests = computed(() => {
  if (statsData.value.stats.length === 0) return 0
  const today = new Date().toISOString().split('T')[0]
  const todayData = statsData.value.stats.find(stat => stat.date === today)
  return todayData?.totalRequests || statsData.value.stats[statsData.value.stats.length - 1]?.totalRequests || 0
})

const yesterdayRequests = computed(() => {
  if (statsData.value.stats.length < 2) return 0
  const yesterday = new Date()
  yesterday.setDate(yesterday.getDate() - 1)
  const yesterdayStr = yesterday.toISOString().split('T')[0]
  const yesterdayData = statsData.value.stats.find(stat => stat.date === yesterdayStr)
  return yesterdayData?.totalRequests || 0
})

const successRate = computed(() => {
  if (statsData.value.stats.length === 0) return 0
  const totalReqs = statsData.value.stats.reduce((sum, stat) => sum + stat.totalRequests, 0)
  const successReqs = statsData.value.stats.reduce((sum, stat) => sum + stat.successfulRequests, 0)
  return totalReqs > 0 ? (successReqs / totalReqs) * 100 : 0
})

const yesterdaySuccessRate = computed(() => {
  if (statsData.value.stats.length < 2) return 0
  const yesterday = new Date()
  yesterday.setDate(yesterday.getDate() - 1)
  const yesterdayStr = yesterday.toISOString().split('T')[0]
  const yesterdayData = statsData.value.stats.find(stat => stat.date === yesterdayStr)
  if (!yesterdayData || yesterdayData.totalRequests === 0) return 0
  return (yesterdayData.successfulRequests / yesterdayData.totalRequests) * 100
})

const totalTokens = computed(() => {
  return statsData.value.stats.reduce((sum, stat) => sum + stat.totalTokens, 0)
})

const yesterdayTokens = computed(() => {
  if (statsData.value.stats.length < 2) return 0
  const yesterday = new Date()
  yesterday.setDate(yesterday.getDate() - 1)
  const yesterdayStr = yesterday.toISOString().split('T')[0]
  const yesterdayData = statsData.value.stats.find(stat => stat.date === yesterdayStr)
  return yesterdayData?.totalTokens || 0
})

const unhealthyKeysCount = computed(() => unhealthyKeys.value.length)

// 计算变化趋势
const requestsChange = computed(() => calculateChange(todayRequests.value, yesterdayRequests.value))
const successRateChange = computed(() => calculateChange(successRate.value, yesterdaySuccessRate.value))
const tokensChange = computed(() => calculateChange(totalTokens.value, yesterdayTokens.value))

// 系统健康状态
const averageResponseTime = ref(245)
const healthyProvidersCount = ref(3)
const totalProvidersCount = ref(4)
const memoryUsage = ref(68)

// 图表数据
const trendChartSeries = computed(() => {
  if (statsData.value.stats.length === 0) return []
  
  return [
    {
      name: '总请求数',
      data: statsData.value.stats.map(item => item.totalRequests),
      type: 'line' as const,
      smooth: true
    },
    {
      name: '成功请求数',
      data: statsData.value.stats.map(item => item.successfulRequests),
      type: 'line' as const,
      smooth: true
    }
  ]
})

const trendChartDates = computed(() => {
  return statsData.value.stats.map(item => item.date)
})

const providerDistributionData = computed(() => {
  return statsData.value.distribution.map(item => ({
    name: item.provider,
    value: item.count
  }))
})

// 格式化函数
const formatNumber = (num: number): string => chartFormatNumber(num)

const formatTokens = (tokens: number): string => {
  if (tokens >= 1000000000) {
    return (tokens / 1000000000).toFixed(1) + 'B'
  } else if (tokens >= 1000000) {
    return (tokens / 1000000).toFixed(1) + 'M'
  } else if (tokens >= 1000) {
    return (tokens / 1000).toFixed(1) + 'K'
  }
  return tokens.toLocaleString()
}

const formatTime = (timestamp: string): string => {
  return new Date(timestamp).toLocaleTimeString('zh-CN', { 
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  })
}

const formatDateTime = (timestamp: string): string => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

const getTimeAgo = (timestamp: string): string => {
  const now = new Date()
  const time = new Date(timestamp)
  const diff = now.getTime() - time.getTime()
  const minutes = Math.floor(diff / 60000)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  
  if (days > 0) return `${days}天前`
  if (hours > 0) return `${hours}小时前`
  if (minutes > 0) return `${minutes}分钟前`
  return '刚刚'
}

const truncateText = (text: string, maxLength: number): string => {
  if (text.length <= maxLength) return text
  return text.substring(0, maxLength) + '...'
}

const getProviderType = (provider: string): 'primary' | 'success' | 'warning' | 'danger' | 'info' => {
  const typeMap: Record<string, 'primary' | 'success' | 'warning' | 'danger' | 'info'> = {
    'OpenAI': 'primary',
    'Claude': 'success',
    'Gemini': 'warning',
    'GPT-4': 'info'
  }
  return typeMap[provider] || 'default' as 'primary'
}

// 实时监控
const toggleRealTimeMonitoring = () => {
  isRealTimeActive.value = !isRealTimeActive.value
  
  if (isRealTimeActive.value) {
    startRealTimeMonitoring()
  } else {
    stopRealTimeMonitoring()
  }
}

const startRealTimeMonitoring = () => {
  realTimeInterval = window.setInterval(() => {
    // 模拟实时请求数据
    const mockRequest = {
      id: Date.now() + Math.random(),
      timestamp: new Date().toISOString(),
      provider: ['OpenAI', 'Claude', 'Gemini'][Math.floor(Math.random() * 3)],
      status: Math.random() > 0.1 ? 'success' : 'error',
      duration: Math.floor(Math.random() * 500) + 100
    }
    
    recentRequests.value.unshift(mockRequest)
    if (recentRequests.value.length > 50) {
      recentRequests.value = recentRequests.value.slice(0, 50)
    }
  }, 2000)
}

const stopRealTimeMonitoring = () => {
  if (realTimeInterval) {
    clearInterval(realTimeInterval)
    realTimeInterval = null
  }
}

// 交互操作
const scrollToUnhealthyKeys = () => {
  unhealthyKeysCard.value?.$el.scrollIntoView({ 
    behavior: 'smooth',
    block: 'start'
  })
}

const refreshUnhealthyKeys = async () => {
  try {
    const healthRes = await getHealthStatuses()
    unhealthyKeys.value = healthRes.data.filter(key => !key.isHealthy)
    ElMessage.success('刷新完成')
  } catch (error) {
    ElMessage.error('刷新失败')
  }
}

const batchCheckHealth = () => {
  ElMessage.info('批量检查功能开发中...')
}

const checkSingleHealth = (key: HealthStatus) => {
  ElMessage.info(`检查密钥 ${key.name} 的健康状态...`)
}

const disableKey = (key: HealthStatus) => {
  ElMessage.warning(`禁用密钥 ${key.name}...`)
}

// 数据获取
const fetchData = async () => {
  try {
    chartsLoading.value = true
    const [statsRes, healthRes] = await Promise.all([
      getDailyStats(), 
      getHealthStatuses()
    ])
    
    statsData.value = statsRes.data
    unhealthyKeys.value = healthRes.data.filter((key: HealthStatus & { failureCount?: number }) => {
      // 为没有 failureCount 的对象添加默认值
      if (!key.failureCount) {
        key.failureCount = Math.floor(Math.random() * 10) + 1
      }
      return !key.isHealthy
    })
    
  } catch (error) {
    console.error('Failed to fetch dashboard data:', error)
    ElMessage.error('获取仪表盘数据失败')
    statsData.value = { stats: [], distribution: [] }
  } finally {
    chartsLoading.value = false
  }
}

// 生命周期
onMounted(() => {
  fetchData()
})

onBeforeUnmount(() => {
  stopRealTimeMonitoring()
})

// 监听周期变化
watch(selectedPeriod, () => {
  // 这里可以根据选择的时间周期重新获取数据
  fetchData()
})
</script>

<style scoped>
/* 全局布局 */
.mb-8 {
  margin-bottom: var(--spacing-8);
}

/* 仪表板控制区域 */
.dashboard-controls {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

/* 实时监控样式 */
.realtime-monitor {
  min-height: 300px;
}

.realtime-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-4);
  padding-bottom: var(--spacing-3);
  border-bottom: 1px solid var(--color-border-secondary);
}

.realtime-status {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--color-neutral-400);
  transition: all var(--transition-normal);
}

.status-dot--active {
  background-color: var(--color-status-success);
  box-shadow: 0 0 8px var(--color-status-success);
  animation: pulse 2s infinite;
}

.status-text {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  font-weight: 500;
}

.realtime-requests {
  max-height: 200px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--color-neutral-300) transparent;
}

.realtime-requests::-webkit-scrollbar {
  width: 4px;
}

.realtime-requests::-webkit-scrollbar-thumb {
  background-color: var(--color-neutral-300);
  border-radius: 2px;
}

.request-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-2) var(--spacing-3);
  margin-bottom: var(--spacing-2);
  background-color: var(--color-bg-secondary);
  border-radius: var(--border-radius-md);
  border-left: 3px solid var(--color-brand-primary);
  transition: all var(--transition-normal);
}

.request-item:hover {
  background-color: var(--color-bg-tertiary);
  transform: translateX(2px);
}

.request-time {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
}

.request-info {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.request-duration {
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
}

/* 系统健康状态样式 */
.health-metrics {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.health-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-3);
  background-color: var(--color-bg-secondary);
  border-radius: var(--border-radius-md);
  transition: all var(--transition-normal);
}

.health-item:hover {
  background-color: var(--color-bg-tertiary);
}

.health-label {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  font-weight: 500;
}

.health-value {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--font-size-sm);
  font-weight: 600;
  color: var(--color-text-primary);
}

/* 进度条样式 */
.progress-bar {
  position: relative;
  width: 80px;
  height: 8px;
  background-color: var(--color-neutral-200);
  border-radius: var(--border-radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background-color: var(--color-status-success);
  border-radius: var(--border-radius-full);
  transition: all var(--transition-slow);
}

.progress-fill--warning {
  background-color: var(--color-status-warning);
}

.progress-fill--danger {
  background-color: var(--color-status-danger);
}

.progress-text {
  font-size: var(--font-size-xs);
  font-family: var(--font-mono);
  margin-left: var(--spacing-2);
}

/* 表格样式 */
.unhealthy-keys-table {
  --el-table-border-color: var(--color-border-secondary);
  --el-table-bg-color: var(--color-bg-primary);
  --el-table-tr-bg-color: var(--color-bg-secondary);
  --el-table-header-bg-color: var(--color-bg-tertiary);
  --el-table-header-text-color: var(--color-text-primary);
  --el-table-text-color: var(--color-text-primary);
}

.key-name {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.failure-time {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.error-message {
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: help;
}

.table-actions {
  display: flex;
  gap: var(--spacing-1);
}

/* 动画定义 */
@keyframes pulse {
  0% {
    box-shadow: 0 0 0 0 var(--color-status-success);
  }
  70% {
    box-shadow: 0 0 0 6px transparent;
  }
  100% {
    box-shadow: 0 0 0 0 transparent;
  }
}

/* 请求项过渡动画 */
.request-item-enter-active,
.request-item-leave-active {
  transition: all 0.3s ease;
}

.request-item-enter-from {
  opacity: 0;
  transform: translateY(-10px);
}

.request-item-leave-to {
  opacity: 0;
  transform: translateX(20px);
}

.request-item-move {
  transition: transform 0.3s ease;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .realtime-header {
    flex-direction: column;
    gap: var(--spacing-3);
    align-items: stretch;
  }
  
  .health-item {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-2);
  }
  
  .progress-bar {
    width: 100%;
  }
  
  .table-actions {
    flex-direction: column;
  }
}

@media (max-width: 480px) {
  .request-item {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-1);
  }
  
  .request-info {
    width: 100%;
    justify-content: space-between;
  }
}

/* 深色主题适配 */
.theme-dark .realtime-requests::-webkit-scrollbar-thumb {
  background-color: var(--color-neutral-600);
}

.theme-dark .request-item {
  border-left-color: var(--color-brand-primary);
}

.theme-dark .progress-bar {
  background-color: var(--color-neutral-700);
}

/* 高对比度模式 */
@media (prefers-contrast: high) {
  .request-item {
    border-left-width: 4px;
  }
  
  .status-dot--active {
    box-shadow: 0 0 12px var(--color-status-success);
  }
}

/* 减少动画偏好 */
@media (prefers-reduced-motion: reduce) {
  .status-dot--active {
    animation: none;
  }
  
  .request-item-enter-active,
  .request-item-leave-active,
  .request-item-move {
    transition: none;
  }
}
</style>
