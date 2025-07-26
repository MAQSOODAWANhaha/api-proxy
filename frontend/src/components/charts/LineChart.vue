<template>
  <div :class="chartClasses" :style="chartStyle">
    <Loading v-if="loading" :visible="loading" text="图表加载中..." />
    <div ref="chartContainer" class="chart-container" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, computed, nextTick } from 'vue'
import * as echarts from 'echarts'
import { Loading } from '@/components/ui'
import { useDesignSystem } from '@/composables/useDesignSystem'

// 数据类型定义
interface ChartDataItem {
  name: string
  value: number
  date?: string
}

interface ChartSeries {
  name: string
  data: number[]
  type?: 'line' | 'bar'
  smooth?: boolean
  areaStyle?: any
}

// 组件属性
interface Props {
  /** 图表数据 */
  data?: ChartDataItem[]
  /** 图表系列 */
  series?: ChartSeries[]
  /** X轴数据 */
  xAxisData?: string[]
  /** 图表标题 */
  title?: string
  /** 图表高度 */
  height?: string | number
  /** 是否加载中 */
  loading?: boolean
  /** 是否平滑曲线 */
  smooth?: boolean
  /** 是否显示面积 */
  area?: boolean
  /** 颜色配置 */
  colors?: string[]
  /** 图表配置 */
  options?: any
}

const props = withDefaults(defineProps<Props>(), {
  height: '400px',
  loading: false,
  smooth: true,
  area: false
})

// 组件事件
const emit = defineEmits<{
  click: [data: any]
  legendselectchanged: [data: any]
}>()

// 响应式数据
const chartContainer = ref<HTMLElement>()
let chartInstance: echarts.ECharts | null = null

// 使用设计系统
const { theme, colors } = useDesignSystem()

// 计算样式类
const chartClasses = computed(() => [
  'ui-line-chart',
  {
    'ui-line-chart--loading': props.loading
  }
])

// 计算图表样式
const chartStyle = computed(() => ({
  height: typeof props.height === 'number' ? `${props.height}px` : props.height
}))

// 获取主题色彩
const getThemeColors = () => {
  if (props.colors) return props.colors
  
  return [
    colors.primary[500],
    colors.success[500],
    colors.warning[500],
    colors.info[500],
    colors.error[500],
    colors.secondary[500]
  ]
}

// 构建图表配置
const buildChartOption = () => {
  const themeColors = getThemeColors()
  const isDark = theme.value.mode === 'dark'
  
  const baseOption = {
    title: props.title ? {
      text: props.title,
      textStyle: {
        color: isDark ? '#ffffff' : '#1f2937',
        fontSize: 16,
        fontWeight: 'normal'
      },
      left: 'center',
      top: 10
    } : undefined,
    
    tooltip: {
      trigger: 'axis',
      backgroundColor: isDark ? '#374151' : '#ffffff',
      borderColor: isDark ? '#4b5563' : '#e5e7eb',
      borderWidth: 1,
      textStyle: {
        color: isDark ? '#f9fafb' : '#1f2937'
      },
      axisPointer: {
        type: 'cross',
        crossStyle: {
          color: isDark ? '#6b7280' : '#9ca3af'
        }
      }
    },
    
    legend: {
      data: props.series?.map(s => s.name) || [],
      textStyle: {
        color: isDark ? '#d1d5db' : '#4b5563'
      },
      top: props.title ? 45 : 15
    },
    
    grid: {
      left: 50,
      right: 30,
      bottom: 50,
      top: props.title ? 80 : 50,
      containLabel: true
    },
    
    xAxis: {
      type: 'category',
      data: props.xAxisData || props.data?.map(item => item.name) || [],
      axisLine: {
        lineStyle: {
          color: isDark ? '#4b5563' : '#d1d5db'
        }
      },
      axisLabel: {
        color: isDark ? '#9ca3af' : '#6b7280'
      },
      splitLine: {
        show: false
      }
    },
    
    yAxis: {
      type: 'value',
      axisLine: {
        lineStyle: {
          color: isDark ? '#4b5563' : '#d1d5db'
        }
      },
      axisLabel: {
        color: isDark ? '#9ca3af' : '#6b7280'
      },
      splitLine: {
        lineStyle: {
          color: isDark ? '#374151' : '#f3f4f6',
          type: 'dashed'
        }
      }
    },
    
    color: themeColors,
    
    series: props.series?.map((s, index) => ({
      name: s.name,
      type: s.type || 'line',
      data: s.data,
      smooth: s.smooth !== undefined ? s.smooth : props.smooth,
      lineStyle: {
        width: 3
      },
      symbol: 'circle',
      symbolSize: 6,
      areaStyle: props.area || s.areaStyle ? {
        opacity: 0.3
      } : undefined,
      emphasis: {
        focus: 'series'
      }
    })) || (props.data ? [{
      name: '数据',
      type: 'line',
      data: props.data.map(item => item.value),
      smooth: props.smooth,
      lineStyle: {
        width: 3,
        color: themeColors[0]
      },
      symbol: 'circle',
      symbolSize: 6,
      areaStyle: props.area ? {
        opacity: 0.3,
        color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
          { offset: 0, color: themeColors[0] + '40' },
          { offset: 1, color: themeColors[0] + '10' }
        ])
      } : undefined
    }] : [])
  }
  
  // 合并自定义配置
  return props.options ? echarts.util.merge(baseOption, props.options) : baseOption
}

// 初始化图表
const initChart = async () => {
  if (!chartContainer.value) return
  
  await nextTick()
  
  if (chartInstance) {
    chartInstance.dispose()
  }
  
  chartInstance = echarts.init(chartContainer.value, theme.value.mode === 'dark' ? 'dark' : undefined)
  
  const option = buildChartOption()
  chartInstance.setOption(option, true)
  
  // 绑定事件
  chartInstance.on('click', (params) => {
    emit('click', params)
  })
  
  chartInstance.on('legendselectchanged', (params) => {
    emit('legendselectchanged', params)
  })
}

// 更新图表
const updateChart = () => {
  if (!chartInstance) return
  
  const option = buildChartOption()
  chartInstance.setOption(option, true)
}

// 调整图表大小
const resizeChart = () => {
  chartInstance?.resize()
}

// 监听数据变化
watch([() => props.data, () => props.series, () => props.xAxisData], () => {
  updateChart()
}, { deep: true })

// 监听主题变化
watch(theme, () => {
  initChart()
})

// 监听窗口大小变化
const handleResize = () => {
  resizeChart()
}

// 生命周期
onMounted(() => {
  initChart()
  window.addEventListener('resize', handleResize)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize)
  if (chartInstance) {
    chartInstance.dispose()
    chartInstance = null
  }
})

// 暴露方法
defineExpose({
  resize: resizeChart,
  getInstance: () => chartInstance
})
</script>

<style scoped>
.ui-line-chart {
  position: relative;
  width: 100%;
  min-height: 200px;
}

.ui-line-chart--loading {
  pointer-events: none;
}

.chart-container {
  width: 100%;
  height: 100%;
  min-height: inherit;
}

/* 深色主题适配 */
.theme-dark .ui-line-chart {
  background-color: transparent;
}

/* 响应式适配 */
@media (max-width: 768px) {
  .ui-line-chart {
    min-height: 250px;
  }
}

@media (max-width: 480px) {
  .ui-line-chart {
    min-height: 200px;
  }
}
</style>