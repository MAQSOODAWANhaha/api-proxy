<template>
  <div class="overview-view">
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
                <div class="stats-value">{{ overviewData?.total_requests || 0 }}</div>
                <div class="stats-label">总请求数</div>
                <div class="stats-change" :class="getChangeClass(overviewData?.requests_change)">
                  <el-icon v-if="overviewData?.requests_change > 0"><ArrowUp /></el-icon>
                  <el-icon v-else-if="overviewData?.requests_change < 0"><ArrowDown /></el-icon>
                  {{ Math.abs(overviewData?.requests_change || 0).toFixed(1) }}%
                </div>
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
                <div class="stats-value">{{ (overviewData?.success_rate || 0).toFixed(1) }}%</div>
                <div class="stats-label">成功率</div>
                <div class="stats-change" :class="getChangeClass(overviewData?.success_rate_change)">
                  <el-icon v-if="overviewData?.success_rate_change > 0"><ArrowUp /></el-icon>
                  <el-icon v-else-if="overviewData?.success_rate_change < 0"><ArrowDown /></el-icon>
                  {{ Math.abs(overviewData?.success_rate_change || 0).toFixed(1) }}%
                </div>
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
                <div class="stats-value">{{ formatNumber(overviewData?.total_tokens || 0) }}</div>
                <div class="stats-label">Token消耗</div>
                <div class="stats-change" :class="getChangeClass(overviewData?.tokens_change)">
                  <el-icon v-if="overviewData?.tokens_change > 0"><ArrowUp /></el-icon>
                  <el-icon v-else-if="overviewData?.tokens_change < 0"><ArrowDown /></el-icon>
                  {{ Math.abs(overviewData?.tokens_change || 0).toFixed(1) }}%
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
        
        <el-col :xs="12" :sm="12" :md="6" :lg="6">
          <el-card class="stats-card">
            <div class="stats-content">
              <div class="stats-icon response-time">
                <el-icon><Timer /></el-icon>
              </div>
              <div class="stats-info">
                <div class="stats-value">{{ (overviewData?.avg_response_time || 0).toFixed(0) }}ms</div>
                <div class="stats-label">平均响应时间</div>
                <div class="stats-change" :class="getChangeClass(-overviewData?.response_time_change)">
                  <el-icon v-if="overviewData?.response_time_change < 0"><ArrowUp /></el-icon>
                  <el-icon v-else-if="overviewData?.response_time_change > 0"><ArrowDown /></el-icon>
                  {{ Math.abs(overviewData?.response_time_change || 0).toFixed(1) }}%
                </div>
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
                <div class="chart-controls">
                  <el-select v-model="trendPeriod" @change="refreshTrendChart" size="small">
                    <el-option label="最近7天" value="7d" />
                    <el-option label="最近30天" value="30d" />
                    <el-option label="最近90天" value="90d" />
                  </el-select>
                  <el-button type="text" @click="refreshTrendChart">
                    <el-icon><Refresh /></el-icon>
                  </el-button>
                </div>
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
                <div class="chart-controls">
                  <el-select v-model="distributionMetric" @change="refreshDistributionChart" size="small">
                    <el-option label="按请求数" value="requests" />
                    <el-option label="按Token数" value="tokens" />
                  </el-select>
                  <el-button type="text" @click="refreshDistributionChart">
                    <el-icon><Refresh /></el-icon>
                  </el-button>
                </div>
              </div>
            </template>
            <div class="chart-container" ref="distributionChartRef"></div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 响应时间分析 -->
    <div class="response-time-section">
      <el-row :gutter="24">
        <el-col :span="24">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>响应时间分析</h3>
                <div class="chart-controls">
                  <el-select v-model="responseTimePeriod" @change="refreshResponseTimeChart" size="small">
                    <el-option label="最近24小时" value="24h" />
                    <el-option label="最近7天" value="7d" />
                    <el-option label="最近30天" value="30d" />
                  </el-select>
                  <el-button type="text" @click="refreshResponseTimeChart">
                    <el-icon><Refresh /></el-icon>
                  </el-button>
                </div>
              </div>
            </template>
            <div class="chart-container large" ref="responseTimeChartRef"></div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 错误分析 -->
    <div class="error-analysis-section">
      <el-row :gutter="24">
        <!-- 错误趋势 -->
        <el-col :xs="24" :sm="24" :md="16" :lg="16">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>错误趋势</h3>
                <el-button type="text" @click="refreshErrorChart">
                  <el-icon><Refresh /></el-icon>
                </el-button>
              </div>
            </template>
            <div class="chart-container" ref="errorChartRef"></div>
          </el-card>
        </el-col>

        <!-- 热门错误 -->
        <el-col :xs="24" :sm="24" :md="8" :lg="8">
          <el-card class="error-list-card">
            <template #header>
              <div class="card-header">
                <h3>热门错误</h3>
                <el-button type="text" @click="refreshErrorList">
                  <el-icon><Refresh /></el-icon>
                </el-button>
              </div>
            </template>
            
            <div class="error-list" v-loading="errorListLoading">
              <div 
                v-for="error in topErrors" 
                :key="error.error_type"
                class="error-item"
              >
                <div class="error-info">
                  <div class="error-type">{{ error.error_type }}</div>
                  <div class="error-meta">
                    <span class="error-count">{{ error.count }}次</span>
                    <span class="error-rate">{{ error.percentage.toFixed(1) }}%</span>
                  </div>
                </div>
                <div class="error-progress">
                  <el-progress 
                    :percentage="error.percentage" 
                    :stroke-width="6"
                    :show-text="false"
                    color="#f56c6c"
                  />
                </div>
              </div>
              
              <div v-if="topErrors.length === 0" class="empty-errors">
                <el-empty description="暂无错误记录" :image-size="60" />
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import {
  DataAnalysis, CircleCheck, Coin, Timer, ArrowUp, ArrowDown, Refresh
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import { StatisticsAPI } from '@/api'
import { useAppStore } from '@/stores'

const appStore = useAppStore()

// 数据
const overviewData = ref<any>(null)
const trendData = ref<any[]>([])
const distributionData = ref<any[]>([])
const responseTimeData = ref<any>(null)
const errorData = ref<any>(null)
const topErrors = ref<any[]>([])

// 状态
const errorListLoading = ref(false)

// 控制参数
const trendPeriod = ref('7d')
const distributionMetric = ref('requests')
const responseTimePeriod = ref('24h')

// 图表引用
const trendChartRef = ref<HTMLElement>()
const distributionChartRef = ref<HTMLElement>()
const responseTimeChartRef = ref<HTMLElement>()
const errorChartRef = ref<HTMLElement>()

const trendChart = ref<echarts.ECharts>()
const distributionChart = ref<echarts.ECharts>()
const responseTimeChart = ref<echarts.ECharts>()
const errorChart = ref<echarts.ECharts>()

// 获取概览数据
const fetchOverviewData = async () => {
  try {
    const [overview, trend, distribution, responseTime, errorStats] = await Promise.all([
      StatisticsAPI.getOverview(),
      StatisticsAPI.getRequestStatistics({ 
        time_range: trendPeriod.value 
      }),
      StatisticsAPI.getProviderDistribution(),
      StatisticsAPI.getResponseTimeAnalysis({ 
        hours: responseTimePeriod.value === '24h' ? 24 : responseTimePeriod.value === '7d' ? 168 : 720 
      }),
      StatisticsAPI.getErrorStatistics()
    ])
    
    overviewData.value = overview
    trendData.value = trend.data || []
    distributionData.value = distribution
    responseTimeData.value = responseTime
    errorData.value = errorStats
    topErrors.value = errorStats.top_errors || []
    
    // 更新图表
    updateAllCharts()
  } catch (error: any) {
    ElMessage.error('获取统计数据失败')
    console.error('Failed to fetch overview data:', error)
  }
}

// 更新请求趋势图表
const updateTrendChart = () => {
  if (!trendChart.value || !trendData.value?.length) return

  const option = {
    title: {
      text: '请求趋势分析',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' }
    },
    legend: {
      data: ['总请求', '成功请求', '失败请求']
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
        name: '总请求',
        type: 'line',
        data: trendData.value.map(item => item.total_requests),
        smooth: true,
        lineStyle: { color: '#409eff' },
        areaStyle: { color: 'rgba(64, 158, 255, 0.1)' }
      },
      {
        name: '成功请求',
        type: 'line',
        data: trendData.value.map(item => item.successful_requests),
        smooth: true,
        lineStyle: { color: '#67c23a' }
      },
      {
        name: '失败请求',
        type: 'line',
        data: trendData.value.map(item => item.failed_requests),
        smooth: true,
        lineStyle: { color: '#f56c6c' }
      }
    ]
  }

  trendChart.value.setOption(option)
}

// 更新服务商分布图表
const updateDistributionChart = () => {
  if (!distributionChart.value || !distributionData.value?.length) return

  const dataKey = distributionMetric.value === 'requests' ? 'requests' : 'tokens'
  const unit = distributionMetric.value === 'requests' ? '请求' : 'Token'

  const option = {
    title: {
      text: `服务商${unit}分布`,
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'item',
      formatter: `{a} <br/>{b}: {c} (${unit}) ({d}%)`
    },
    legend: {
      orient: 'horizontal',
      bottom: '5%'
    },
    series: [
      {
        name: unit,
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
        data: distributionData.value.map(item => ({
          value: item[dataKey],
          name: item.provider
        }))
      }
    ]
  }

  distributionChart.value.setOption(option)
}

// 更新响应时间图表
const updateResponseTimeChart = () => {
  if (!responseTimeChart.value || !responseTimeData.value?.data?.length) return

  const option = {
    title: {
      text: '响应时间分析',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' }
    },
    legend: {
      data: ['平均响应时间', 'P50', 'P95', 'P99']
    },
    xAxis: {
      type: 'category',
      data: responseTimeData.value.data.map((item: any) => item.timestamp)
    },
    yAxis: {
      type: 'value',
      name: '响应时间 (ms)'
    },
    series: [
      {
        name: '平均响应时间',
        type: 'line',
        data: responseTimeData.value.data.map((item: any) => item.avg_response_time),
        smooth: true,
        lineStyle: { color: '#409eff' }
      },
      {
        name: 'P50',
        type: 'line',
        data: responseTimeData.value.data.map((item: any) => item.p50_response_time),
        smooth: true,
        lineStyle: { color: '#67c23a' }
      },
      {
        name: 'P95',
        type: 'line',
        data: responseTimeData.value.data.map((item: any) => item.p95_response_time),
        smooth: true,
        lineStyle: { color: '#e6a23c' }
      },
      {
        name: 'P99',
        type: 'line',
        data: responseTimeData.value.data.map((item: any) => item.p99_response_time),
        smooth: true,
        lineStyle: { color: '#f56c6c' }
      }
    ]
  }

  responseTimeChart.value.setOption(option)
}

// 更新错误图表
const updateErrorChart = () => {
  if (!errorChart.value || !errorData.value?.data?.length) return

  const option = {
    title: {
      text: '错误趋势',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' }
    },
    legend: {
      data: ['错误数量', '错误率']
    },
    xAxis: {
      type: 'category',
      data: errorData.value.data.map((item: any) => item.timestamp)
    },
    yAxis: [
      {
        type: 'value',
        name: '错误数量',
        position: 'left'
      },
      {
        type: 'value',
        name: '错误率 (%)',
        position: 'right',
        min: 0,
        max: 100
      }
    ],
    series: [
      {
        name: '错误数量',
        type: 'bar',
        data: errorData.value.data.map((item: any) => item.error_count),
        itemStyle: { color: '#f56c6c' }
      },
      {
        name: '错误率',
        type: 'line',
        yAxisIndex: 1,
        data: errorData.value.data.map((item: any) => item.error_rate),
        smooth: true,
        lineStyle: { color: '#e6a23c' }
      }
    ]
  }

  errorChart.value.setOption(option)
}

// 更新所有图表
const updateAllCharts = async () => {
  await nextTick()
  updateTrendChart()
  updateDistributionChart()
  updateResponseTimeChart()
  updateErrorChart()
}

// 初始化图表
const initCharts = async () => {
  await nextTick()
  
  if (trendChartRef.value) {
    trendChart.value = echarts.init(trendChartRef.value)
  }
  
  if (distributionChartRef.value) {
    distributionChart.value = echarts.init(distributionChartRef.value)
  }
  
  if (responseTimeChartRef.value) {
    responseTimeChart.value = echarts.init(responseTimeChartRef.value)
  }
  
  if (errorChartRef.value) {
    errorChart.value = echarts.init(errorChartRef.value)
  }

  // 监听窗口大小变化
  window.addEventListener('resize', handleResize)
}

// 处理窗口大小变化
const handleResize = () => {
  trendChart.value?.resize()
  distributionChart.value?.resize()
  responseTimeChart.value?.resize()
  errorChart.value?.resize()
}

// 刷新图表
const refreshTrendChart = async () => {
  try {
    const data = await StatisticsAPI.getRequestStatistics({ 
      time_range: trendPeriod.value 
    })
    trendData.value = data.data || []
    updateTrendChart()
  } catch (error: any) {
    ElMessage.error('刷新趋势图表失败')
  }
}

const refreshDistributionChart = async () => {
  updateDistributionChart()
}

const refreshResponseTimeChart = async () => {
  try {
    const hours = responseTimePeriod.value === '24h' ? 24 : 
                  responseTimePeriod.value === '7d' ? 168 : 720
    responseTimeData.value = await StatisticsAPI.getResponseTimeAnalysis({ hours })
    updateResponseTimeChart()
  } catch (error: any) {
    ElMessage.error('刷新响应时间图表失败')
  }
}

const refreshErrorChart = async () => {
  try {
    errorData.value = await StatisticsAPI.getErrorStatistics()
    topErrors.value = errorData.value.top_errors || []
    updateErrorChart()
  } catch (error: any) {
    ElMessage.error('刷新错误图表失败')
  }
}

const refreshErrorList = () => {
  refreshErrorChart()
}

// 工具函数
const formatNumber = (num: number) => {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toString()
}

const getChangeClass = (change: number) => {
  if (!change) return ''
  return change > 0 ? 'positive' : 'negative'
}

// 生命周期
onMounted(async () => {
  appStore.setPageTitle('数据概览')
  
  // 初始化图表
  await initCharts()
  
  // 获取数据
  await fetchOverviewData()
})

onUnmounted(() => {
  // 清理事件监听
  window.removeEventListener('resize', handleResize)
  
  // 销毁图表
  trendChart.value?.dispose()
  distributionChart.value?.dispose()
  responseTimeChart.value?.dispose()
  errorChart.value?.dispose()
})
</script>

<style scoped>
.overview-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 24px;
  overflow-y: auto;
  padding-bottom: 24px;
}

/* 统计卡片 */
.stats-cards {
  flex-shrink: 0;
}

.stats-card {
  height: 120px;
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
  width: 70px;
  height: 70px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 16px;
  color: white;
  font-size: 28px;
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

.stats-icon.response-time {
  background: linear-gradient(135deg, #409eff 0%, #36a3f7 100%);
}

.stats-info {
  flex: 1;
}

.stats-value {
  font-size: 32px;
  font-weight: bold;
  color: #333;
  line-height: 1;
  margin-bottom: 8px;
}

.stats-label {
  font-size: 14px;
  color: #666;
  margin-bottom: 6px;
}

.stats-change {
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 4px;
}

.stats-change.positive {
  color: #67c23a;
}

.stats-change.negative {
  color: #f56c6c;
}

/* 图表区域 */
.charts-section,
.response-time-section,
.error-analysis-section {
  flex-shrink: 0;
}

.chart-card {
  height: 400px;
}

.chart-card .large {
  height: 320px;
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

.chart-controls {
  display: flex;
  align-items: center;
  gap: 8px;
}

.chart-container {
  height: 320px;
  width: 100%;
}

.chart-container.large {
  height: 320px;
}

/* 错误列表 */
.error-list-card {
  height: 400px;
}

.error-list {
  height: 320px;
  overflow-y: auto;
}

.error-item {
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}

.error-item:last-child {
  border-bottom: none;
}

.error-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.error-type {
  font-size: 14px;
  color: #333;
  font-weight: 500;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.error-meta {
  display: flex;
  gap: 8px;
  font-size: 12px;
}

.error-count {
  color: #f56c6c;
  font-weight: 500;
}

.error-rate {
  color: #666;
}

.error-progress {
  margin-top: 4px;
}

.empty-errors {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .stats-value {
    font-size: 24px;
  }
  
  .stats-icon {
    width: 60px;
    height: 60px;
    font-size: 24px;
  }
  
  .chart-container,
  .chart-container.large {
    height: 280px;
  }
  
  .chart-card {
    height: 360px;
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
  border-radius: 3px;
}

:deep(.el-progress-bar__inner) {
  border-radius: 3px;
}

:deep(.el-select) {
  width: 120px;
}
</style>