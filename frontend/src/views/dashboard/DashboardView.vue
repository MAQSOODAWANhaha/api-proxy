<template>
  <div class="dashboard-view">
    <!-- 统计卡片 -->
    <div class="stats-cards">
      <el-row :gutter="24">
        <el-col :xs="12" :sm="12" :md="6" :lg="6">
          <el-card class="stats-card">
            <div class="stats-content">
              <div class="stats-icon requests">
                <el-icon><DataAnalysis /></el-icon>
              </div>
              <div class="stats-info">
                <div class="stats-value">{{ dashboardData?.total_requests_today || 0 }}</div>
                <div class="stats-label">今日请求</div>
              </div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :xs="12" :sm="12" :md="6" :lg="6">
          <el-card class="stats-card">
            <div class="stats-content">
              <div class="stats-icon success">
                <el-icon><CircleCheck /></el-icon>
              </div>
              <div class="stats-info">
                <div class="stats-value">{{ (dashboardData?.success_rate_today || 0).toFixed(1) }}%</div>
                <div class="stats-label">成功率</div>
              </div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :xs="12" :sm="12" :md="6" :lg="6">
          <el-card class="stats-card">
            <div class="stats-content">
              <div class="stats-icon tokens">
                <el-icon><Coin /></el-icon>
              </div>
              <div class="stats-info">
                <div class="stats-value">{{ formatNumber(dashboardData?.total_tokens_today || 0) }}</div>
                <div class="stats-label">Token消耗</div>
              </div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :xs="12" :sm="12" :md="6" :lg="6">
          <el-card class="stats-card">
            <div class="stats-content">
              <div class="stats-icon services">
                <el-icon><Connection /></el-icon>
              </div>
              <div class="stats-info">
                <div class="stats-value">{{ dashboardData?.active_api_services || 0 }}</div>
                <div class="stats-label">活跃服务</div>
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 图表区域 -->
    <div class="charts-section">
      <el-row :gutter="24">
        <!-- 请求趋势图 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>请求趋势</h3>
                <el-button type="text" @click="refreshTrendData">
                  <el-icon><Refresh /></el-icon>
                </el-button>
              </div>
            </template>
            <div class="chart-container" ref="trendChartRef"></div>
          </el-card>
        </el-col>

        <!-- 服务商分布图 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>服务商分布</h3>
                <el-button type="text" @click="refreshProviderData">
                  <el-icon><Refresh /></el-icon>
                </el-button>
              </div>
            </template>
            <div class="chart-container" ref="providerChartRef"></div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 实时状态 -->
    <div class="realtime-section">
      <el-row :gutter="24">
        <el-col :xs="24" :sm="24" :md="16" :lg="16">
          <el-card class="realtime-card">
            <template #header>
              <div class="card-header">
                <h3>实时监控</h3>
                <el-switch
                  v-model="realtimeEnabled"
                  active-text="自动刷新"
                  @change="toggleRealtime"
                />
              </div>
            </template>
            
            <div class="realtime-stats">
              <div class="realtime-item">
                <div class="realtime-label">当前请求数</div>
                <div class="realtime-value">{{ realtimeData?.current_requests || 0 }}</div>
              </div>
              <div class="realtime-item">
                <div class="realtime-label">每秒请求数</div>
                <div class="realtime-value">{{ (realtimeData?.requests_per_second || 0).toFixed(2) }}</div>
              </div>
              <div class="realtime-item">
                <div class="realtime-label">活跃连接</div>
                <div class="realtime-value">{{ realtimeData?.active_connections || 0 }}</div>
              </div>
              <div class="realtime-item">
                <div class="realtime-label">平均响应时间</div>
                <div class="realtime-value">{{ (realtimeData?.avg_response_time || 0).toFixed(0) }}ms</div>
              </div>
              <div class="realtime-item">
                <div class="realtime-label">错误率</div>
                <div class="realtime-value" :class="{ 'error-rate': (realtimeData?.error_rate || 0) > 5 }">
                  {{ (realtimeData?.error_rate || 0).toFixed(2) }}%
                </div>
              </div>
            </div>
          </el-card>
        </el-col>

        <el-col :xs="24" :sm="24" :md="8" :lg="8">
          <el-card class="health-card">
            <template #header>
              <h3>密钥健康状态</h3>
            </template>
            
            <div class="health-overview">
              <div class="health-summary">
                <div class="health-total">
                  <span class="health-number">{{ dashboardData?.total_keys || 0 }}</span>
                  <span class="health-label">总密钥数</span>
                </div>
                <div class="health-healthy">
                  <span class="health-number healthy">{{ dashboardData?.healthy_keys || 0 }}</span>
                  <span class="health-label">健康密钥</span>
                </div>
              </div>
              
              <div class="health-progress">
                <el-progress
                  :percentage="healthPercentage"
                  :color="getHealthColor(healthPercentage)"
                  :stroke-width="12"
                />
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import {
  DataAnalysis, CircleCheck, Coin, Connection, Refresh
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import { StatisticsAPI } from '@/api'
import { useAppStore } from '@/stores'

const appStore = useAppStore()

// 数据
const dashboardData = ref<any>(null)
const realtimeData = ref<any>(null)
const trendData = ref<any[]>([])
const providerData = ref<any[]>([])

// 状态
const realtimeEnabled = ref(false)
const realtimeTimer = ref<number | null>(null)

// 图表引用
const trendChartRef = ref<HTMLElement>()
const providerChartRef = ref<HTMLElement>()
const trendChart = ref<echarts.ECharts>()
const providerChart = ref<echarts.ECharts>()

// 计算属性
const healthPercentage = computed(() => {
  if (!dashboardData.value?.total_keys) return 0
  return Math.round((dashboardData.value.healthy_keys / dashboardData.value.total_keys) * 100)
})

// 方法
const formatNumber = (num: number) => {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toString()
}

const getHealthColor = (percentage: number) => {
  if (percentage >= 90) return '#67c23a'
  if (percentage >= 70) return '#e6a23c'
  return '#f56c6c'
}

// 获取仪表盘数据
const fetchDashboardData = async () => {
  try {
    dashboardData.value = await StatisticsAPI.getDashboardCards()
  } catch (error: any) {
    ElMessage.error('获取仪表盘数据失败')
    console.error('Failed to fetch dashboard data:', error)
  }
}

// 获取实时数据
const fetchRealtimeData = async () => {
  try {
    realtimeData.value = await StatisticsAPI.getRealTimeStats()
  } catch (error: any) {
    console.error('Failed to fetch realtime data:', error)
  }
}

// 获取趋势数据
const fetchTrendData = async () => {
  try {
    trendData.value = await StatisticsAPI.getRequestTrend(7)
    updateTrendChart()
  } catch (error: any) {
    ElMessage.error('获取趋势数据失败')
    console.error('Failed to fetch trend data:', error)
  }
}

// 获取服务商分布数据
const fetchProviderData = async () => {
  try {
    providerData.value = await StatisticsAPI.getProviderDistribution()
    updateProviderChart()
  } catch (error: any) {
    ElMessage.error('获取服务商数据失败')
    console.error('Failed to fetch provider data:', error)
  }
}

// 更新趋势图表
const updateTrendChart = () => {
  if (!trendChart.value || !trendData.value?.length) return

  const option = {
    title: {
      text: '过去7天请求趋势',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' }
    },
    legend: {
      data: ['请求总数', '成功请求', '失败请求']
    },
    xAxis: {
      type: 'category',
      data: trendData.value.map(item => item.date)
    },
    yAxis: {
      type: 'value'
    },
    series: [
      {
        name: '请求总数',
        type: 'line',
        data: trendData.value.map(item => item.requests),
        smooth: true,
        lineStyle: { color: '#409eff' }
      },
      {
        name: '成功请求',
        type: 'line',
        data: trendData.value.map(item => item.successful),
        smooth: true,
        lineStyle: { color: '#67c23a' }
      },
      {
        name: '失败请求',
        type: 'line',
        data: trendData.value.map(item => item.failed),
        smooth: true,
        lineStyle: { color: '#f56c6c' }
      }
    ]
  }

  trendChart.value.setOption(option)
}

// 更新服务商分布图表
const updateProviderChart = () => {
  if (!providerChart.value || !providerData.value?.length) return

  const option = {
    title: {
      text: '服务商请求分布',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'item',
      formatter: '{a} <br/>{b}: {c} ({d}%)'
    },
    legend: {
      orient: 'horizontal',
      bottom: '0%'
    },
    series: [
      {
        name: '请求数',
        type: 'pie',
        radius: ['40%', '70%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 10,
          borderColor: '#fff',
          borderWidth: 2
        },
        label: {
          show: false,
          position: 'center'
        },
        emphasis: {
          label: {
            show: true,
            fontSize: 20,
            fontWeight: 'bold'
          }
        },
        labelLine: {
          show: false
        },
        data: providerData.value.map(item => ({
          value: item.requests,
          name: item.provider
        }))
      }
    ]
  }

  providerChart.value.setOption(option)
}

// 初始化图表
const initCharts = async () => {
  await nextTick()
  
  if (trendChartRef.value) {
    trendChart.value = echarts.init(trendChartRef.value)
  }
  
  if (providerChartRef.value) {
    providerChart.value = echarts.init(providerChartRef.value)
  }

  // 监听窗口大小变化
  window.addEventListener('resize', handleResize)
}

// 处理窗口大小变化
const handleResize = () => {
  trendChart.value?.resize()
  providerChart.value?.resize()
}

// 刷新数据
const refreshTrendData = () => {
  fetchTrendData()
}

const refreshProviderData = () => {
  fetchProviderData()
}

// 切换实时监控
const toggleRealtime = (enabled: boolean) => {
  if (enabled) {
    fetchRealtimeData()
    realtimeTimer.value = window.setInterval(fetchRealtimeData, 5000)
  } else {
    if (realtimeTimer.value) {
      clearInterval(realtimeTimer.value)
      realtimeTimer.value = null
    }
  }
}

// 生命周期
onMounted(async () => {
  appStore.setPageTitle('仪表盘')
  
  // 初始化图表
  await initCharts()
  
  // 获取数据
  await Promise.all([
    fetchDashboardData(),
    fetchTrendData(),
    fetchProviderData()
  ])
})

onUnmounted(() => {
  // 清理定时器
  if (realtimeTimer.value) {
    clearInterval(realtimeTimer.value)
  }
  
  // 清理事件监听
  window.removeEventListener('resize', handleResize)
  
  // 销毁图表
  trendChart.value?.dispose()
  providerChart.value?.dispose()
})
</script>

<style scoped>
.dashboard-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

/* 统计卡片 */
.stats-cards {
  flex-shrink: 0;
}

.stats-card {
  height: 100px;
  cursor: pointer;
  transition: all 0.3s;
}

.stats-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
}

.stats-content {
  display: flex;
  align-items: center;
  height: 100%;
}

.stats-icon {
  width: 60px;
  height: 60px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 16px;
  color: white;
  font-size: 24px;
}

.stats-icon.requests {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.stats-icon.success {
  background: linear-gradient(135deg, #67c23a 0%, #85ce61 100%);
}

.stats-icon.tokens {
  background: linear-gradient(135deg, #e6a23c 0%, #f7ba2a 100%);
}

.stats-icon.services {
  background: linear-gradient(135deg, #409eff 0%, #36a3f7 100%);
}

.stats-info {
  flex: 1;
}

.stats-value {
  font-size: 28px;
  font-weight: bold;
  color: #333;
  line-height: 1;
  margin-bottom: 4px;
}

.stats-label {
  font-size: 14px;
  color: #666;
}

/* 图表区域 */
.charts-section {
  flex: 1;
  min-height: 400px;
}

.chart-card {
  height: 400px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-header h3 {
  margin: 0;
  color: #333;
  font-size: 16px;
  font-weight: 600;
}

.chart-container {
  height: 320px;
  width: 100%;
}

/* 实时状态 */
.realtime-section {
  flex-shrink: 0;
}

.realtime-card {
  height: 200px;
}

.realtime-stats {
  display: flex;
  justify-content: space-between;
  height: 140px;
  align-items: center;
}

.realtime-item {
  text-align: center;
  flex: 1;
}

.realtime-label {
  font-size: 12px;
  color: #666;
  margin-bottom: 8px;
}

.realtime-value {
  font-size: 20px;
  font-weight: bold;
  color: #333;
}

.realtime-value.error-rate {
  color: #f56c6c;
}

/* 健康状态 */
.health-card {
  height: 200px;
}

.health-overview {
  height: 140px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.health-summary {
  display: flex;
  justify-content: space-between;
  margin-bottom: 20px;
}

.health-total,
.health-healthy {
  text-align: center;
}

.health-number {
  display: block;
  font-size: 24px;
  font-weight: bold;
  color: #333;
  margin-bottom: 4px;
}

.health-number.healthy {
  color: #67c23a;
}

.health-label {
  font-size: 12px;
  color: #666;
}

.health-progress {
  margin-top: auto;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .stats-value {
    font-size: 24px;
  }
  
  .realtime-stats {
    flex-wrap: wrap;
    height: auto;
  }
  
  .realtime-item {
    width: 50%;
    margin-bottom: 16px;
  }
  
  .chart-container {
    height: 280px;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-card__body) {
  padding: 20px;
  height: calc(100% - 60px);
}

:deep(.el-card__header) {
  padding: 16px 20px;
  border-bottom: 1px solid #f0f0f0;
}

:deep(.el-progress-bar__outer) {
  border-radius: 6px;
}

:deep(.el-progress-bar__inner) {
  border-radius: 6px;
}
</style>