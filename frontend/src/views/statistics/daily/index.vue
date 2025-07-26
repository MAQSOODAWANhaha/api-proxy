<template>
  <div class="page-container">
    <el-row :gutter="20">
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
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, nextTick } from 'vue'
import * as echarts from 'echarts'
import { getDailyStats, type DailyStat, type ProviderDistribution } from '@/api/statistics'
import { ElMessage } from 'element-plus'

const trendChart = ref<HTMLElement | null>(null)
const distChart = ref<HTMLElement | null>(null)

const initTrendChart = (data: DailyStat[]) => {
  const chart = echarts.init(trendChart.value!)
  const option = {
    tooltip: { trigger: 'axis' },
    xAxis: {
      type: 'category',
      data: data.map(item => item.date),
    },
    yAxis: { type: 'value' },
    series: [
      {
        name: '总请求数',
        type: 'line',
        data: data.map(item => item.totalRequests),
        smooth: true,
      },
      {
        name: '成功请求数',
        type: 'line',
        data: data.map(item => item.successfulRequests),
        smooth: true,
      },
    ],
    legend: { data: ['总请求数', '成功请求数'] },
  }
  chart.setOption(option)
}

const initDistChart = (data: ProviderDistribution[]) => {
  const chart = echarts.init(distChart.value!)
  const option = {
    tooltip: { trigger: 'item' },
    legend: {
      orient: 'vertical',
      left: 'left',
    },
    series: [
      {
        name: '服务商分布',
        type: 'pie',
        radius: '50%',
        data: data.map(item => ({ value: item.count, name: item.provider })),
        emphasis: {
          itemStyle: {
            shadowBlur: 10,
            shadowOffsetX: 0,
            shadowColor: 'rgba(0, 0, 0, 0.5)',
          },
        },
      },
    ],
  }
  chart.setOption(option)
}

const fetchData = async () => {
  try {
    const response = await getDailyStats()
    await nextTick()
    initTrendChart(response.stats)
    initDistChart(response.distribution)
  } catch (error) {
    ElMessage.error('获取统计数据失败')
  }
}

onMounted(() => {
  fetchData()
})
</script>

<style scoped>
.page-container {
  padding: 10px;
}
</style>