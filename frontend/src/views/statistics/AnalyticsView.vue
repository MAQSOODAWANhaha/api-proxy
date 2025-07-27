<template>
  <div class="analytics-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>数据分析</h2>
          <div class="header-actions">
            <el-button @click="refreshAnalytics" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button @click="exportAnalytics" :loading="exportLoading">
              <el-icon><Download /></el-icon>
              导出报告
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="analytics-content">
        <!-- 时间范围选择器 -->
        <div class="analytics-filters">
          <el-form :model="filters" inline>
            <el-form-item label="时间范围">
              <el-date-picker
                v-model="dateRange"
                type="datetimerange"
                format="YYYY-MM-DD HH:mm:ss"
                value-format="YYYY-MM-DD HH:mm:ss"
                range-separator="至"
                start-placeholder="开始时间"
                end-placeholder="结束时间"
                @change="handleDateRangeChange"
                style="width: 350px;"
              />
            </el-form-item>
            
            <el-form-item label="服务商">
              <el-select v-model="filters.provider_type" clearable placeholder="全部">
                <el-option label="OpenAI" value="openai" />
                <el-option label="Google" value="google" />
                <el-option label="Anthropic" value="anthropic" />
              </el-select>
            </el-form-item>
            
            <el-form-item label="时间粒度">
              <el-select v-model="filters.granularity" @change="refreshAnalytics">
                <el-option label="小时" value="hour" />
                <el-option label="天" value="day" />
                <el-option label="周" value="week" />
                <el-option label="月" value="month" />
              </el-select>
            </el-form-item>
            
            <el-form-item>
              <el-button type="primary" @click="searchAnalytics">
                <el-icon><Search /></el-icon>
                查询
              </el-button>
            </el-form-item>
          </el-form>
        </div>

        <!-- 核心指标卡片 -->
        <div class="metrics-cards">
          <el-row :gutter="24">
            <el-col :xs="12" :sm="6" :md="6" :lg="6">
              <div class="metric-card requests">
                <div class="metric-header">
                  <el-icon><DataAnalysis /></el-icon>
                  <span>总请求数</span>
                </div>
                <div class="metric-value">{{ formatNumber(analyticsData?.metrics?.total_requests || 0) }}</div>
                <div class="metric-trend">
                  <span :class="getTrendClass(analyticsData?.metrics?.requests_growth)">
                    {{ formatPercentage(analyticsData?.metrics?.requests_growth) }}
                  </span>
                  <span class="trend-label">相比上期</span>
                </div>
              </div>
            </el-col>
            
            <el-col :xs="12" :sm="6" :md="6" :lg="6">
              <div class="metric-card tokens">
                <div class="metric-header">
                  <el-icon><Coin /></el-icon>
                  <span>Token消耗</span>
                </div>
                <div class="metric-value">{{ formatNumber(analyticsData?.metrics?.total_tokens || 0) }}</div>
                <div class="metric-trend">
                  <span :class="getTrendClass(analyticsData?.metrics?.tokens_growth)">
                    {{ formatPercentage(analyticsData?.metrics?.tokens_growth) }}
                  </span>
                  <span class="trend-label">相比上期</span>
                </div>
              </div>
            </el-col>
            
            <el-col :xs="12" :sm="6" :md="6" :lg="6">
              <div class="metric-card costs">
                <div class="metric-header">
                  <el-icon><Money /></el-icon>
                  <span>预估成本</span>
                </div>
                <div class="metric-value">${{ formatCurrency(analyticsData?.metrics?.estimated_cost || 0) }}</div>
                <div class="metric-trend">
                  <span :class="getTrendClass(analyticsData?.metrics?.cost_growth)">
                    {{ formatPercentage(analyticsData?.metrics?.cost_growth) }}
                  </span>
                  <span class="trend-label">相比上期</span>
                </div>
              </div>
            </el-col>
            
            <el-col :xs="12" :sm="6" :md="6" :lg="6">
              <div class="metric-card users">
                <div class="metric-header">
                  <el-icon><User /></el-icon>
                  <span>活跃用户</span>
                </div>
                <div class="metric-value">{{ analyticsData?.metrics?.active_users || 0 }}</div>
                <div class="metric-trend">
                  <span :class="getTrendClass(analyticsData?.metrics?.users_growth)">
                    {{ formatPercentage(analyticsData?.metrics?.users_growth) }}
                  </span>
                  <span class="trend-label">相比上期</span>
                </div>
              </div>
            </el-col>
          </el-row>
        </div>

        <!-- 图表区域 -->
        <div class="charts-section">
          <el-row :gutter="24">
            <!-- 请求量趋势分析 -->
            <el-col :xs="24" :sm="24" :md="12" :lg="12">
              <el-card class="chart-card">
                <template #header>
                  <div class="card-header">
                    <h3>请求量趋势分析</h3>
                    <div class="chart-controls">
                      <el-select v-model="requestTrendType" @change="updateRequestTrendChart" size="small">
                        <el-option label="总量" value="total" />
                        <el-option label="成功/失败" value="status" />
                        <el-option label="服务商对比" value="provider" />
                      </el-select>
                    </div>
                  </div>
                </template>
                <div class="chart-container" ref="requestTrendChartRef"></div>
              </el-card>
            </el-col>

            <!-- Token消耗分析 -->
            <el-col :xs="24" :sm="24" :md="12" :lg="12">
              <el-card class="chart-card">
                <template #header>
                  <div class="card-header">
                    <h3>Token消耗分析</h3>
                    <div class="chart-controls">
                      <el-select v-model="tokenAnalysisType" @change="updateTokenAnalysisChart" size="small">
                        <el-option label="输入/输出" value="io" />
                        <el-option label="服务商分布" value="provider" />
                        <el-option label="用户排行" value="user" />
                      </el-select>
                    </div>
                  </div>
                </template>
                <div class="chart-container" ref="tokenAnalysisChartRef"></div>
              </el-card>
            </el-col>
          </el-row>
        </div>

        <!-- 成本分析和用户活跃度 -->
        <div class="cost-user-section">
          <el-row :gutter="24">
            <!-- 成本分析 -->
            <el-col :xs="24" :sm="24" :md="12" :lg="12">
              <el-card class="chart-card">
                <template #header>
                  <div class="card-header">
                    <h3>成本分析</h3>
                    <div class="chart-controls">
                      <el-select v-model="costPeriod" @change="updateCostAnalysisChart" size="small">
                        <el-option label="每日成本" value="daily" />
                        <el-option label="每周成本" value="weekly" />
                        <el-option label="每月成本" value="monthly" />
                      </el-select>
                    </div>
                  </div>
                </template>
                <div class="chart-container" ref="costAnalysisChartRef"></div>
              </el-card>
            </el-col>

            <!-- 用户活跃度分析 -->
            <el-col :xs="24" :sm="24" :md="12" :lg="12">
              <el-card class="chart-card">
                <template #header>
                  <div class="card-header">
                    <h3>用户活跃度分析</h3>
                  </div>
                </template>
                <div class="chart-container" ref="userActivityChartRef"></div>
              </el-card>
            </el-col>
          </el-row>
        </div>

        <!-- 性能分析 -->
        <div class="performance-section">
          <el-row :gutter="24">
            <!-- 响应时间分布 -->
            <el-col :xs="24" :sm="24" :md="16" :lg="16">
              <el-card class="chart-card">
                <template #header>
                  <div class="card-header">
                    <h3>响应时间分布</h3>
                    <div class="chart-controls">
                      <el-select v-model="performanceMetric" @change="updatePerformanceChart" size="small">
                        <el-option label="响应时间" value="response_time" />
                        <el-option label="吞吐量" value="throughput" />
                        <el-option label="并发数" value="concurrency" />
                      </el-select>
                    </div>
                  </div>
                </template>
                <div class="chart-container large" ref="performanceChartRef"></div>
              </el-card>
            </el-col>

            <!-- 热门API排行 -->
            <el-col :xs="24" :sm="24" :md="8" :lg="8">
              <el-card class="ranking-card">
                <template #header>
                  <div class="card-header">
                    <h3>热门API排行</h3>
                  </div>
                </template>
                <div class="ranking-list" v-loading="loading">
                  <div 
                    v-for="(api, index) in topApis" 
                    :key="api.endpoint"
                    class="ranking-item"
                  >
                    <div class="ranking-number" :class="`rank-${index + 1}`">
                      {{ index + 1 }}
                    </div>
                    <div class="ranking-info">
                      <div class="api-endpoint">{{ api.endpoint }}</div>
                      <div class="api-stats">
                        <span class="requests-count">{{ formatNumber(api.requests) }}次</span>
                        <span class="success-rate">{{ api.success_rate.toFixed(1) }}%</span>
                      </div>
                    </div>
                    <div class="ranking-progress">
                      <el-progress 
                        :percentage="(api.requests / topApis[0]?.requests * 100) || 0" 
                        :stroke-width="4"
                        :show-text="false"
                        color="#409eff"
                      />
                    </div>
                  </div>
                  
                  <div v-if="topApis.length === 0" class="empty-ranking">
                    <el-empty description="暂无数据" :image-size="80" />
                  </div>
                </div>
              </el-card>
            </el-col>
          </el-row>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Refresh, Download, Search, DataAnalysis, Coin, Money, User
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import { StatisticsAPI } from '@/api'
import { useAppStore } from '@/stores'

const appStore = useAppStore()

// 状态
const loading = ref(false)
const exportLoading = ref(false)

// 数据
const analyticsData = ref<any>(null)
const topApis = ref<any[]>([])
const dateRange = ref<[string, string] | null>(null)

// 筛选器
const filters = reactive({
  start_time: '',
  end_time: '',
  provider_type: '',
  granularity: 'day'
})

// 图表控制
const requestTrendType = ref('total')
const tokenAnalysisType = ref('io')
const costPeriod = ref('daily')
const performanceMetric = ref('response_time')

// 图表引用
const requestTrendChartRef = ref<HTMLElement>()
const tokenAnalysisChartRef = ref<HTMLElement>()
const costAnalysisChartRef = ref<HTMLElement>()
const userActivityChartRef = ref<HTMLElement>()
const performanceChartRef = ref<HTMLElement>()

const requestTrendChart = ref<echarts.ECharts>()
const tokenAnalysisChart = ref<echarts.ECharts>()
const costAnalysisChart = ref<echarts.ECharts>()
const userActivityChart = ref<echarts.ECharts>()
const performanceChart = ref<echarts.ECharts>()

// 获取分析数据
const fetchAnalyticsData = async () => {
  try {
    loading.value = true
    
    const params = {
      start_time: filters.start_time || undefined,
      end_time: filters.end_time || undefined,
      provider_type: filters.provider_type || undefined,
      granularity: filters.granularity
    }
    
    const [analytics, apiRanking] = await Promise.all([
      StatisticsAPI.getAdvancedAnalytics(params),
      StatisticsAPI.getApiRanking(params)
    ])
    
    analyticsData.value = analytics
    topApis.value = apiRanking.apis || []
    
    // 更新所有图表
    await updateAllCharts()
  } catch (error: any) {
    ElMessage.error('获取分析数据失败')
    console.error('Failed to fetch analytics data:', error)
  } finally {
    loading.value = false
  }
}

// 刷新分析数据
const refreshAnalytics = () => {
  fetchAnalyticsData()
}

// 搜索分析数据
const searchAnalytics = () => {
  fetchAnalyticsData()
}

// 导出分析报告
const exportAnalytics = async () => {
  try {
    exportLoading.value = true
    const params = {
      start_time: filters.start_time,
      end_time: filters.end_time,
      provider_type: filters.provider_type || undefined,
      format: 'pdf'
    }
    
    await StatisticsAPI.exportAnalyticsReport(params)
    ElMessage.success('分析报告导出请求已提交')
  } catch (error: any) {
    ElMessage.error(error.message || '导出失败')
  } finally {
    exportLoading.value = false
  }
}

// 处理时间范围变化
const handleDateRangeChange = (value: [string, string] | null) => {
  if (value) {
    filters.start_time = value[0]
    filters.end_time = value[1]
  } else {
    filters.start_time = ''
    filters.end_time = ''
  }
}

// 更新请求趋势图表
const updateRequestTrendChart = () => {
  if (!requestTrendChart.value || !analyticsData.value?.request_trend) return

  const data = analyticsData.value.request_trend
  let option: any = {
    title: {
      text: '请求量趋势',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' }
    },
    xAxis: {
      type: 'category',
      data: data.map((item: any) => item.timestamp)
    },
    yAxis: {
      type: 'value'
    }
  }

  if (requestTrendType.value === 'total') {
    option.series = [{
      name: '总请求数',
      type: 'line',
      data: data.map((item: any) => item.total_requests),
      smooth: true,
      areaStyle: { color: 'rgba(64, 158, 255, 0.1)' },
      lineStyle: { color: '#409eff' }
    }]
  } else if (requestTrendType.value === 'status') {
    option.legend = { data: ['成功请求', '失败请求'] }
    option.series = [
      {
        name: '成功请求',
        type: 'line',
        data: data.map((item: any) => item.successful_requests),
        smooth: true,
        lineStyle: { color: '#67c23a' }
      },
      {
        name: '失败请求',
        type: 'line',
        data: data.map((item: any) => item.failed_requests),
        smooth: true,
        lineStyle: { color: '#f56c6c' }
      }
    ]
  } else if (requestTrendType.value === 'provider') {
    const providers = ['openai', 'google', 'anthropic']
    option.legend = { data: providers }
    option.series = providers.map((provider, index) => ({
      name: provider,
      type: 'line',
      data: data.map((item: any) => item[`${provider}_requests`] || 0),
      smooth: true,
      lineStyle: { color: ['#409eff', '#67c23a', '#e6a23c'][index] }
    }))
  }

  requestTrendChart.value.setOption(option, true)
}

// 更新Token分析图表
const updateTokenAnalysisChart = () => {
  if (!tokenAnalysisChart.value || !analyticsData.value?.token_analysis) return

  const data = analyticsData.value.token_analysis
  let option: any = {
    title: {
      text: 'Token消耗分析',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis'
    }
  }

  if (tokenAnalysisType.value === 'io') {
    option.legend = { data: ['输入Token', '输出Token'] }
    option.xAxis = {
      type: 'category',
      data: data.map((item: any) => item.timestamp)
    }
    option.yAxis = { type: 'value' }
    option.series = [
      {
        name: '输入Token',
        type: 'bar',
        data: data.map((item: any) => item.input_tokens),
        itemStyle: { color: '#409eff' }
      },
      {
        name: '输出Token',
        type: 'bar',
        data: data.map((item: any) => item.output_tokens),
        itemStyle: { color: '#67c23a' }
      }
    ]
  } else if (tokenAnalysisType.value === 'provider') {
    option.tooltip = { trigger: 'item' }
    option.series = [{
      type: 'pie',
      radius: ['40%', '70%'],
      data: analyticsData.value.token_by_provider?.map((item: any) => ({
        value: item.tokens,
        name: item.provider
      })) || [],
      emphasis: {
        itemStyle: {
          shadowBlur: 10,
          shadowOffsetX: 0,
          shadowColor: 'rgba(0, 0, 0, 0.5)'
        }
      }
    }]
  }

  tokenAnalysisChart.value.setOption(option, true)
}

// 更新成本分析图表
const updateCostAnalysisChart = () => {
  if (!costAnalysisChart.value || !analyticsData.value?.cost_analysis) return

  const data = analyticsData.value.cost_analysis
  const option = {
    title: {
      text: '成本分析',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      formatter: '{b}: ${c}'
    },
    xAxis: {
      type: 'category',
      data: data.map((item: any) => item.period)
    },
    yAxis: {
      type: 'value',
      name: '成本 ($)'
    },
    series: [{
      name: '成本',
      type: 'bar',
      data: data.map((item: any) => item.cost),
      itemStyle: {
        color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
          { offset: 0, color: '#e6a23c' },
          { offset: 1, color: '#f7ba2a' }
        ])
      }
    }]
  }

  costAnalysisChart.value.setOption(option, true)
}

// 更新用户活跃度图表
const updateUserActivityChart = () => {
  if (!userActivityChart.value || !analyticsData.value?.user_activity) return

  const data = analyticsData.value.user_activity
  const option = {
    title: {
      text: '用户活跃度',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis'
    },
    legend: {
      data: ['新用户', '活跃用户', '留存用户']
    },
    xAxis: {
      type: 'category',
      data: data.map((item: any) => item.date)
    },
    yAxis: {
      type: 'value'
    },
    series: [
      {
        name: '新用户',
        type: 'bar',
        stack: 'users',
        data: data.map((item: any) => item.new_users),
        itemStyle: { color: '#409eff' }
      },
      {
        name: '活跃用户',
        type: 'bar',
        stack: 'users',
        data: data.map((item: any) => item.active_users),
        itemStyle: { color: '#67c23a' }
      },
      {
        name: '留存用户',
        type: 'bar',
        stack: 'users',
        data: data.map((item: any) => item.retained_users),
        itemStyle: { color: '#e6a23c' }
      }
    ]
  }

  userActivityChart.value.setOption(option, true)
}

// 更新性能分析图表
const updatePerformanceChart = () => {
  if (!performanceChart.value || !analyticsData.value?.performance_data) return

  const data = analyticsData.value.performance_data
  let option: any = {
    title: {
      text: '性能分析',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis'
    },
    xAxis: {
      type: 'category',
      data: data.map((item: any) => item.timestamp)
    },
    yAxis: {
      type: 'value'
    }
  }

  if (performanceMetric.value === 'response_time') {
    option.legend = { data: ['平均响应时间', 'P95响应时间', 'P99响应时间'] }
    option.yAxis.name = '响应时间 (ms)'
    option.series = [
      {
        name: '平均响应时间',
        type: 'line',
        data: data.map((item: any) => item.avg_response_time),
        smooth: true,
        lineStyle: { color: '#409eff' }
      },
      {
        name: 'P95响应时间',
        type: 'line',
        data: data.map((item: any) => item.p95_response_time),
        smooth: true,
        lineStyle: { color: '#e6a23c' }
      },
      {
        name: 'P99响应时间',
        type: 'line',
        data: data.map((item: any) => item.p99_response_time),
        smooth: true,
        lineStyle: { color: '#f56c6c' }
      }
    ]
  } else if (performanceMetric.value === 'throughput') {
    option.yAxis.name = '吞吐量 (req/s)'
    option.series = [{
      name: '吞吐量',
      type: 'line',
      data: data.map((item: any) => item.throughput),
      smooth: true,
      areaStyle: { color: 'rgba(103, 194, 58, 0.1)' },
      lineStyle: { color: '#67c23a' }
    }]
  }

  performanceChart.value.setOption(option, true)
}

// 更新所有图表
const updateAllCharts = async () => {
  await nextTick()
  updateRequestTrendChart()
  updateTokenAnalysisChart()
  updateCostAnalysisChart()
  updateUserActivityChart()
  updatePerformanceChart()
}

// 初始化图表
const initCharts = async () => {
  await nextTick()
  
  if (requestTrendChartRef.value) {
    requestTrendChart.value = echarts.init(requestTrendChartRef.value)
  }
  
  if (tokenAnalysisChartRef.value) {
    tokenAnalysisChart.value = echarts.init(tokenAnalysisChartRef.value)
  }
  
  if (costAnalysisChartRef.value) {
    costAnalysisChart.value = echarts.init(costAnalysisChartRef.value)
  }
  
  if (userActivityChartRef.value) {
    userActivityChart.value = echarts.init(userActivityChartRef.value)
  }
  
  if (performanceChartRef.value) {
    performanceChart.value = echarts.init(performanceChartRef.value)
  }

  // 监听窗口大小变化
  window.addEventListener('resize', handleResize)
}

// 处理窗口大小变化
const handleResize = () => {
  requestTrendChart.value?.resize()
  tokenAnalysisChart.value?.resize()
  costAnalysisChart.value?.resize()
  userActivityChart.value?.resize()
  performanceChart.value?.resize()
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

const formatCurrency = (amount: number) => {
  return amount.toFixed(2)
}

const formatPercentage = (value: number) => {
  if (!value) return '0%'
  const sign = value > 0 ? '+' : ''
  return `${sign}${value.toFixed(1)}%`
}

const getTrendClass = (value: number) => {
  if (!value) return ''
  return value > 0 ? 'trend-up' : 'trend-down'
}

// 生命周期
onMounted(async () => {
  appStore.setPageTitle('数据分析')
  
  // 设置默认时间范围（最近7天）
  const endTime = new Date()
  const startTime = new Date(endTime.getTime() - 7 * 24 * 60 * 60 * 1000)
  dateRange.value = [
    startTime.toISOString().slice(0, 19).replace('T', ' '),
    endTime.toISOString().slice(0, 19).replace('T', ' ')
  ]
  filters.start_time = dateRange.value[0]
  filters.end_time = dateRange.value[1]
  
  // 初始化图表
  await initCharts()
  
  // 获取数据
  await fetchAnalyticsData()
})

onUnmounted(() => {
  // 清理事件监听
  window.removeEventListener('resize', handleResize)
  
  // 销毁图表
  requestTrendChart.value?.dispose()
  tokenAnalysisChart.value?.dispose()
  costAnalysisChart.value?.dispose()
  userActivityChart.value?.dispose()
  performanceChart.value?.dispose()
})
</script>

<style scoped>
.analytics-view {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.page-card {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-header h2 {
  margin: 0;
  color: #333;
}

.header-actions {
  display: flex;
  gap: 12px;
}

.analytics-content {
  height: calc(100% - 60px);
  display: flex;
  flex-direction: column;
  gap: 24px;
  overflow-y: auto;
  padding-bottom: 24px;
}

/* 筛选器 */
.analytics-filters {
  flex-shrink: 0;
  padding: 16px;
  background: #f8f9fa;
  border-radius: 6px;
}

/* 指标卡片 */
.metrics-cards {
  flex-shrink: 0;
}

.metric-card {
  background: white;
  border-radius: 12px;
  padding: 20px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  transition: all 0.3s;
}

.metric-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}

.metric-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
  font-size: 14px;
  color: #666;
}

.metric-card.requests .metric-header .el-icon {
  color: #409eff;
}

.metric-card.tokens .metric-header .el-icon {
  color: #67c23a;
}

.metric-card.costs .metric-header .el-icon {
  color: #e6a23c;
}

.metric-card.users .metric-header .el-icon {
  color: #f56c6c;
}

.metric-value {
  font-size: 28px;
  font-weight: bold;
  color: #333;
  margin-bottom: 8px;
}

.metric-trend {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.trend-up {
  color: #67c23a;
}

.trend-down {
  color: #f56c6c;
}

.trend-label {
  color: #999;
}

/* 图表区域 */
.charts-section,
.cost-user-section,
.performance-section {
  flex-shrink: 0;
}

.chart-card {
  height: 400px;
}

.chart-card .card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.chart-card .card-header h3 {
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

/* 排行榜 */
.ranking-card {
  height: 400px;
}

.ranking-list {
  height: 320px;
  overflow-y: auto;
}

.ranking-item {
  display: flex;
  align-items: center;
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}

.ranking-item:last-child {
  border-bottom: none;
}

.ranking-number {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: bold;
  font-size: 14px;
  margin-right: 12px;
}

.ranking-number.rank-1 {
  background: #ffd700;
  color: white;
}

.ranking-number.rank-2 {
  background: #c0c0c0;
  color: white;
}

.ranking-number.rank-3 {
  background: #cd7f32;
  color: white;
}

.ranking-number:not(.rank-1):not(.rank-2):not(.rank-3) {
  background: #f0f0f0;
  color: #666;
}

.ranking-info {
  flex: 1;
}

.api-endpoint {
  font-size: 14px;
  color: #333;
  font-weight: 500;
  margin-bottom: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-stats {
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

.ranking-progress {
  width: 60px;
  margin-left: 12px;
}

.empty-ranking {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .analytics-filters .el-form {
    flex-direction: column;
  }
  
  .analytics-filters .el-form-item {
    margin-bottom: 16px;
    margin-right: 0;
  }
  
  .metric-value {
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
  border-radius: 2px;
}

:deep(.el-progress-bar__inner) {
  border-radius: 2px;
}

:deep(.el-select) {
  width: 120px;
}
</style>