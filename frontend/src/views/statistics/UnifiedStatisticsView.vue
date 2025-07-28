<template>
  <div class="unified-statistics-view">
    <!-- 总览面板 -->
    <div class="overview-section">
      <el-row :gutter="24">
        <el-col :xs="12" :sm="6" :md="6" :lg="6">
          <div class="metric-card requests">
            <div class="metric-icon">
              <el-icon><DataAnalysis /></el-icon>
            </div>
            <div class="metric-content">
              <div class="metric-value">{{ formatNumber(overviewData?.total_requests_today || 0) }}</div>
              <div class="metric-label">今日请求数</div>
            </div>
          </div>
        </el-col>
        
        <el-col :xs="12" :sm="6" :md="6" :lg="6">
          <div class="metric-card success">
            <div class="metric-icon">
              <el-icon><CircleCheck /></el-icon>
            </div>
            <div class="metric-content">
              <div class="metric-value">{{ (overviewData?.success_rate_today || 0).toFixed(1) }}%</div>
              <div class="metric-label">成功率</div>
            </div>
          </div>
        </el-col>
        
        <el-col :xs="12" :sm="6" :md="6" :lg="6">
          <div class="metric-card tokens">
            <div class="metric-icon">
              <el-icon><Coin /></el-icon>
            </div>
            <div class="metric-content">
              <div class="metric-value">{{ formatNumber(overviewData?.total_tokens_today || 0) }}</div>
              <div class="metric-label">Token消耗</div>
            </div>
          </div>
        </el-col>
        
        <el-col :xs="12" :sm="6" :md="6" :lg="6">
          <div class="metric-card response-time">
            <div class="metric-icon">
              <el-icon><Timer /></el-icon>
            </div>
            <div class="metric-content">
              <div class="metric-value">{{ (overviewData?.avg_response_time || 0) }}ms</div>
              <div class="metric-label">平均响应时间</div>
            </div>
          </div>
        </el-col>
      </el-row>
    </div>

    <!-- 趋势分析 -->
    <div class="trends-section">
      <el-row :gutter="24">
        <!-- 请求趋势 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>请求趋势</h3>
                <div class="chart-controls">
                  <el-select v-model="trendPeriod" @change="refreshTrendData" size="small">
                    <el-option label="最近7天" value="7" />
                    <el-option label="最近15天" value="15" />
                    <el-option label="最近30天" value="30" />
                  </el-select>
                </div>
              </div>
            </template>
            <div class="chart-container" ref="trendChartRef"></div>
          </el-card>
        </el-col>

        <!-- 响应时间趋势 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>响应时间分析</h3>
                <div class="chart-controls">
                  <el-select v-model="responseTimePeriod" @change="refreshResponseTimeData" size="small">
                    <el-option label="最近24小时" value="24" />
                    <el-option label="最近7天" value="168" />
                  </el-select>
                </div>
              </div>
            </template>
            <div class="chart-container" ref="responseTimeChartRef"></div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 服务商分析 -->
    <div class="providers-section">
      <el-row :gutter="24">
        <!-- 服务商分布 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>服务商分布</h3>
                <div class="chart-controls">
                  <el-select v-model="distributionMetric" @change="refreshDistributionData" size="small">
                    <el-option label="按请求数" value="requests" />
                    <el-option label="按Token数" value="tokens" />
                  </el-select>
                </div>
              </div>
            </template>
            <div class="chart-container" ref="distributionChartRef"></div>
          </el-card>
        </el-col>

        <!-- 服务商健康状态 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="status-card">
            <template #header>
              <div class="card-header">
                <h3>服务商状态</h3>
                <el-button type="text" @click="refreshProviderStatus">
                  <el-icon><Refresh /></el-icon>
                </el-button>
              </div>
            </template>
            <div class="provider-status-list" v-loading="providerStatusLoading">
              <div 
                v-for="provider in providerStatusList" 
                :key="provider.provider"
                class="provider-status-item"
              >
                <div class="provider-info">
                  <div class="provider-name">{{ provider.provider }}</div>
                  <div class="provider-metrics">
                    <span class="requests-count">{{ formatNumber(provider.requests) }} 请求</span>
                    <span class="success-rate">{{ provider.percentage.toFixed(1) }}%</span>
                  </div>
                </div>
                <div class="provider-health">
                  <el-progress 
                    :percentage="provider.percentage" 
                    :stroke-width="6"
                    :show-text="false"
                    :color="getHealthColor(provider.percentage)"
                  />
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- Token分析和错误分析 -->
    <div class="analysis-section">
      <el-row :gutter="24">
        <!-- Token使用分析 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>Token使用趋势</h3>
                <div class="chart-controls">
                  <el-select v-model="tokenAnalysisPeriod" @change="refreshTokenData" size="small">
                    <el-option label="最近24小时" value="24" />
                    <el-option label="最近7天" value="168" />
                  </el-select>
                </div>
              </div>
            </template>
            <div class="chart-container" ref="tokenChartRef"></div>
          </el-card>
        </el-col>

        <!-- 错误分析 -->
        <el-col :xs="24" :sm="24" :md="12" :lg="12">
          <el-card class="chart-card">
            <template #header>
              <div class="card-header">
                <h3>错误分析</h3>
                <div class="chart-controls">
                  <el-select v-model="errorAnalysisPeriod" @change="refreshErrorData" size="small">
                    <el-option label="最近24小时" value="24" />
                    <el-option label="最近7天" value="168" />
                  </el-select>
                </div>
              </div>
            </template>
            <div class="chart-container" ref="errorChartRef"></div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 错误详情列表 -->
    <div class="error-details-section">
      <el-card class="error-details-card">
        <template #header>
          <div class="card-header">
            <h3>热门错误类型</h3>
            <el-button type="text" @click="refreshErrorData">
              <el-icon><Refresh /></el-icon>
            </el-button>
          </div>
        </template>
        <div class="error-list" v-loading="errorDataLoading">
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
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import {
  DataAnalysis, CircleCheck, Coin, Timer, Refresh
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import { StatisticsAPI } from '@/api'
import { useAppStore } from '@/stores'

const appStore = useAppStore()

// 数据状态
const overviewData = ref<any>(null)
const trendData = ref<any[]>([])
const responseTimeData = ref<any>(null)
const distributionData = ref<any[]>([])
const providerStatusList = ref<any[]>([])
const tokenData = ref<any>(null)
const errorData = ref<any>(null)
const topErrors = ref<any[]>([])

// 加载状态
const providerStatusLoading = ref(false)
const errorDataLoading = ref(false)

// 控制参数
const trendPeriod = ref('7')
const responseTimePeriod = ref('24')
const distributionMetric = ref('requests')
const tokenAnalysisPeriod = ref('24')
const errorAnalysisPeriod = ref('24')

// 图表引用
const trendChartRef = ref<HTMLElement>()
const responseTimeChartRef = ref<HTMLElement>()
const distributionChartRef = ref<HTMLElement>()
const tokenChartRef = ref<HTMLElement>()
const errorChartRef = ref<HTMLElement>()

const trendChart = ref<echarts.ECharts>()
const responseTimeChart = ref<echarts.ECharts>()
const distributionChart = ref<echarts.ECharts>()
const tokenChart = ref<echarts.ECharts>()
const errorChart = ref<echarts.ECharts>()

// 获取所有统计数据
const fetchAllData = async () => {
  try {
    await Promise.all([
      fetchOverviewData(),
      fetchTrendData(),
      fetchResponseTimeData(),
      fetchDistributionData(),
      fetchTokenData(),
      fetchErrorData()
    ])
    
    await updateAllCharts()
  } catch (error: any) {
    ElMessage.error('获取统计数据失败')
    console.error('Failed to fetch statistics data:', error)
  }
}

// 获取概览数据
const fetchOverviewData = async () => {
  try {
    overviewData.value = await StatisticsAPI.getDashboardCards()
  } catch (error: any) {
    console.error('Failed to fetch overview data:', error)
  }
}

// 获取趋势数据
const fetchTrendData = async () => {
  try {
    const response = await StatisticsAPI.getDashboardTrend({ days: trendPeriod.value })
    trendData.value = response || []
  } catch (error: any) {
    console.error('Failed to fetch trend data:', error)
  }
}

// 获取响应时间数据
const fetchResponseTimeData = async () => {
  try {
    responseTimeData.value = await StatisticsAPI.getResponseTimeAnalysis({ 
      hours: responseTimePeriod.value 
    })
  } catch (error: any) {
    console.error('Failed to fetch response time data:', error)
  }
}

// 获取分布数据
const fetchDistributionData = async () => {
  try {
    distributionData.value = await StatisticsAPI.getProviderDistribution()
    providerStatusList.value = distributionData.value || []
  } catch (error: any) {
    console.error('Failed to fetch distribution data:', error)
  }
}

// 获取Token数据
const fetchTokenData = async () => {
  try {
    tokenData.value = await StatisticsAPI.getTokenUsage({ 
      hours: parseInt(tokenAnalysisPeriod.value) 
    })
  } catch (error: any) {
    console.error('Failed to fetch token data:', error)
  }
}

// 获取错误数据
const fetchErrorData = async () => {
  try {
    errorDataLoading.value = true
    errorData.value = await StatisticsAPI.getErrorStatistics({ 
      hours: errorAnalysisPeriod.value 
    })
    topErrors.value = errorData.value?.top_errors || []
  } catch (error: any) {
    console.error('Failed to fetch error data:', error)
  } finally {
    errorDataLoading.value = false
  }
}

// 刷新数据方法
const refreshTrendData = async () => {
  await fetchTrendData()
  updateTrendChart()
}

const refreshResponseTimeData = async () => {
  await fetchResponseTimeData()
  updateResponseTimeChart()
}

const refreshDistributionData = async () => {
  await fetchDistributionData()
  updateDistributionChart()
}

const refreshProviderStatus = async () => {
  providerStatusLoading.value = true
  try {
    await fetchDistributionData()
  } finally {
    providerStatusLoading.value = false
  }
}

const refreshTokenData = async () => {
  await fetchTokenData()
  updateTokenChart()
}

const refreshErrorData = async () => {
  await fetchErrorData()
  updateErrorChart()
}

// 图表更新方法
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
        data: trendData.value.map(item => item.requests),
        smooth: true,
        lineStyle: { color: '#409eff' },
        areaStyle: { color: 'rgba(64, 158, 255, 0.1)' }
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
      data: ['平均响应时间', 'P95', 'P99']
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

const updateDistributionChart = () => {
  if (!distributionChart.value || !distributionData.value?.length) return

  const dataKey = distributionMetric.value
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

const updateTokenChart = () => {
  if (!tokenChart.value || !tokenData.value?.data?.length) return

  const option = {
    title: {
      text: 'Token使用趋势',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis'
    },
    xAxis: {
      type: 'category',
      data: tokenData.value.data.map((item: any) => item.timestamp)
    },
    yAxis: {
      type: 'value',
      name: 'Token数'
    },
    series: [
      {
        name: 'Token使用量',
        type: 'bar',
        data: tokenData.value.data.map((item: any) => item.total_tokens),
        itemStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
            { offset: 0, color: '#e6a23c' },
            { offset: 1, color: '#f7ba2a' }
          ])
        }
      }
    ]
  }

  tokenChart.value.setOption(option)
}

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
  updateResponseTimeChart()
  updateDistributionChart()
  updateTokenChart()
  updateErrorChart()
}

// 初始化图表
const initCharts = async () => {
  await nextTick()
  
  if (trendChartRef.value) {
    trendChart.value = echarts.init(trendChartRef.value)
  }
  
  if (responseTimeChartRef.value) {
    responseTimeChart.value = echarts.init(responseTimeChartRef.value)
  }
  
  if (distributionChartRef.value) {
    distributionChart.value = echarts.init(distributionChartRef.value)
  }
  
  if (tokenChartRef.value) {
    tokenChart.value = echarts.init(tokenChartRef.value)
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
  responseTimeChart.value?.resize()
  distributionChart.value?.resize()
  tokenChart.value?.resize()
  errorChart.value?.resize()
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

const getHealthColor = (percentage: number) => {
  if (percentage >= 95) return '#67c23a'
  if (percentage >= 80) return '#e6a23c'
  return '#f56c6c'
}

// 生命周期
onMounted(async () => {
  appStore.setPageTitle('统计分析')
  
  // 初始化图表
  await initCharts()
  
  // 获取数据
  await fetchAllData()
})

onUnmounted(() => {
  // 清理事件监听
  window.removeEventListener('resize', handleResize)
  
  // 销毁图表
  trendChart.value?.dispose()
  responseTimeChart.value?.dispose()
  distributionChart.value?.dispose()
  tokenChart.value?.dispose()
  errorChart.value?.dispose()
})
</script>

<style scoped>
.unified-statistics-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 24px;
  overflow-y: auto;
  padding-bottom: 24px;
}

/* 总览面板 */
.overview-section {
  flex-shrink: 0;
}

.metric-card {
  background: white;
  border-radius: 12px;
  padding: 24px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  transition: all 0.3s;
  cursor: pointer;
  display: flex;
  align-items: center;
  height: 120px;
}

.metric-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}

.metric-icon {
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

.metric-card.requests .metric-icon {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.metric-card.success .metric-icon {
  background: linear-gradient(135deg, #67c23a 0%, #85ce61 100%);
}

.metric-card.tokens .metric-icon {
  background: linear-gradient(135deg, #e6a23c 0%, #f7ba2a 100%);
}

.metric-card.response-time .metric-icon {
  background: linear-gradient(135deg, #409eff 0%, #36a3f7 100%);
}

.metric-content {
  flex: 1;
}

.metric-value {
  font-size: 32px;
  font-weight: bold;
  color: #333;
  line-height: 1;
  margin-bottom: 8px;
}

.metric-label {
  font-size: 14px;
  color: #666;
}

/* 图表区域 */
.trends-section,
.providers-section,
.analysis-section {
  flex-shrink: 0;
}

.chart-card {
  height: 400px;
}

.status-card {
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

.chart-controls {
  display: flex;
  align-items: center;
  gap: 8px;
}

.chart-container {
  height: 320px;
  width: 100%;
}

/* 服务商状态列表 */
.provider-status-list {
  height: 320px;
  overflow-y: auto;
}

.provider-status-item {
  display: flex;
  align-items: center;
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}

.provider-status-item:last-child {
  border-bottom: none;
}

.provider-info {
  flex: 1;
}

.provider-name {
  font-size: 14px;
  color: #333;
  font-weight: 500;
  margin-bottom: 4px;
}

.provider-metrics {
  display: flex;
  gap: 12px;
  font-size: 12px;
}

.requests-count {
  color: #409eff;
  font-weight: 500;
}

.success-rate {
  color: #67c23a;
}

.provider-health {
  width: 100px;
  margin-left: 12px;
}

/* 错误详情 */
.error-details-section {
  flex-shrink: 0;
}

.error-details-card {
  min-height: 300px;
}

.error-list {
  max-height: 250px;
  overflow-y: auto;
}

.error-item {
  display: flex;
  align-items: center;
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}

.error-item:last-child {
  border-bottom: none;
}

.error-info {
  flex: 1;
}

.error-type {
  font-size: 14px;
  color: #333;
  font-weight: 500;
  margin-bottom: 4px;
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
  width: 80px;
  margin-left: 12px;
}

.empty-errors {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 120px;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .metric-value {
    font-size: 24px;
  }
  
  .metric-icon {
    width: 60px;
    height: 60px;
    font-size: 24px;
  }
  
  .chart-container {
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