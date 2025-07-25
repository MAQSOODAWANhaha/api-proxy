<template>
  <div class="page-container">
    <!-- Stat Cards -->
    <el-row :gutter="20">
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-icon" style="background-color: #e6f7ff;">
              <el-icon color="#1890ff"><DataLine /></el-icon>
            </div>
            <div class="stat-text">
              <div class="stat-title">今日请求数</div>
              <div class="stat-value">{{ formatNumber(todayRequests) }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
           <div class="stat-card">
            <div class="stat-icon" style="background-color: #f6ffed;">
              <el-icon color="#52c41a"><Select /></el-icon>
            </div>
            <div class="stat-text">
              <div class="stat-title">成功率</div>
              <div class="stat-value">{{ successRate }}%</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
           <div class="stat-card">
            <div class="stat-icon" style="background-color: #fffbe6;">
              <el-icon color="#faad14"><Coin /></el-icon>
            </div>
            <div class="stat-text">
              <div class="stat-title">Token总使用量</div>
              <div class="stat-value">{{ formatTokens(totalTokens) }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
           <div class="stat-card">
            <div class="stat-icon" style="background-color: #fff1f0;">
              <el-icon color="#f5222d"><Warning /></el-icon>
            </div>
            <div class="stat-text">
              <div class="stat-title">不健康密钥</div>
              <div class="stat-value">{{ unhealthyKeysCount }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Charts -->
    <el-row :gutter="20" style="margin-top: 20px;">
      <el-col :span="16">
        <el-card>
          <template #header>请求数趋势 (近7日)</template>
          <div ref="trendChart" style="height: 400px;"></div>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card>
          <template #header>服务商分布</template>
          <div ref="distChart" style="height: 400px;"></div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Unhealthy Keys Table -->
    <el-row style="margin-top: 20px;">
      <el-col :span="24">
        <el-card>
          <template #header>近期不健康密钥</template>
          <el-table :data="unhealthyKeys" stripe>
            <el-table-column prop="name" label="密钥名称" />
            <el-table-column prop="provider" label="服务商" />
            <el-table-column prop="lastFailure" label="上次失败时间" />
            <el-table-column prop="errorMessage" label="错误信息" />
          </el-table>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, nextTick, computed } from 'vue'
import * as echarts from 'echarts'
import { getDailyStats, type DailyStat, type ProviderDistribution } from '@/api/statistics'
import { getHealthStatuses, type HealthStatus } from '@/api/health'
import { ElMessage } from 'element-plus'
import { DataLine, Select, Coin, Warning } from '@element-plus/icons-vue'

// Refs for charts
const trendChart = ref<HTMLElement | null>(null)
const distChart = ref<HTMLElement | null>(null)

// Data state
const unhealthyKeys = ref<HealthStatus[]>([])
const unhealthyKeysCount = computed(() => unhealthyKeys.value.length)

// Statistics data
const statsData = ref<{ stats: DailyStat[], distribution: ProviderDistribution[] }>({
  stats: [],
  distribution: []
})

// Computed statistics
const todayRequests = computed(() => {
  if (statsData.value.stats.length === 0) return 0
  const today = new Date().toISOString().split('T')[0]
  const todayData = statsData.value.stats.find(stat => stat.date === today)
  return todayData?.totalRequests || statsData.value.stats[statsData.value.stats.length - 1]?.totalRequests || 0
})

const successRate = computed(() => {
  if (statsData.value.stats.length === 0) return '0.0'
  const totalReqs = statsData.value.stats.reduce((sum, stat) => sum + stat.totalRequests, 0)
  const successReqs = statsData.value.stats.reduce((sum, stat) => sum + stat.successfulRequests, 0)
  const rate = totalReqs > 0 ? (successReqs / totalReqs) * 100 : 0
  return rate.toFixed(1)
})

const totalTokens = computed(() => {
  return statsData.value.stats.reduce((sum, stat) => sum + stat.totalTokens, 0)
})

// Helper functions
const formatNumber = (num: number): string => {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toLocaleString()
}

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

// ECharts instances
let trendChartInstance: echarts.ECharts | null = null
let distChartInstance: echarts.ECharts | null = null

// Methods to initialize charts
const initTrendChart = (data: DailyStat[]) => {
  if (!trendChart.value) return
  trendChartInstance = echarts.init(trendChart.value)
  const option = {
    tooltip: { trigger: 'axis' },
    xAxis: { type: 'category', data: data.map(item => item.date) },
    yAxis: { type: 'value' },
    series: [
      { name: '总请求数', type: 'line', data: data.map(item => item.totalRequests), smooth: true },
      { name: '成功请求数', type: 'line', data: data.map(item => item.successfulRequests), smooth: true },
    ],
    legend: { data: ['总请求数', '成功请求数'] },
    grid: { left: '3%', right: '4%', bottom: '3%', containLabel: true }
  }
  trendChartInstance.setOption(option)
}

const initDistChart = (data: ProviderDistribution[]) => {
  if (!distChart.value) return
  distChartInstance = echarts.init(distChart.value)
  const option = {
    tooltip: { trigger: 'item' },
    legend: { top: '5%', left: 'center' },
    series: [{
      name: '服务商分布',
      type: 'pie',
      radius: ['40%', '70%'],
      avoidLabelOverlap: false,
      itemStyle: {
        borderRadius: 10,
        borderColor: '#fff',
        borderWidth: 2
      },
      label: { show: false, position: 'center' },
      emphasis: {
        label: { show: true, fontSize: '20', fontWeight: 'bold' }
      },
      labelLine: { show: false },
      data: data.map(item => ({ value: item.count, name: item.provider })),
    }],
  }
  distChartInstance.setOption(option)
}

// Fetching data
const fetchData = async () => {
  try {
    const [statsRes, healthRes] = await Promise.all([getDailyStats(), getHealthStatuses()])
    
    // Store statistics data
    statsData.value = statsRes.data
    
    await nextTick() // Ensure DOM is ready
    
    initTrendChart(statsRes.data.stats)
    initDistChart(statsRes.data.distribution)
    unhealthyKeys.value = healthRes.data.filter(key => !key.isHealthy)

  } catch (error) {
    console.error('Failed to fetch dashboard data:', error)
    ElMessage.error('获取仪表盘数据失败')
    // Set fallback data to prevent UI errors
    statsData.value = { stats: [], distribution: [] }
  }
}

// Resize handler
const handleResize = () => {
  trendChartInstance?.resize()
  distChartInstance?.resize()
}

// Lifecycle hooks
onMounted(() => {
  fetchData()
  window.addEventListener('resize', handleResize)
})
</script>

<style scoped>
.stat-card {
  display: flex;
  align-items: center;
}
.stat-icon {
  height: 54px;
  width: 54px;
  border-radius: 50%;
  display: flex;
  justify-content: center;
  align-items: center;
  font-size: 28px;
  margin-right: 16px;
}
.stat-text {
  display: flex;
  flex-direction: column;
}
.stat-title {
  font-size: 14px;
  color: var(--text-color-secondary);
  margin-bottom: 4px;
}
.stat-value {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-color-primary);
}
</style>
