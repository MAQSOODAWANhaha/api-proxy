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
interface PieDataItem {
  name: string
  value: number
  color?: string
}

// 组件属性
interface Props {
  /** 图表数据 */
  data?: PieDataItem[]
  /** 图表标题 */
  title?: string
  /** 图表高度 */
  height?: string | number
  /** 是否加载中 */
  loading?: boolean
  /** 内径比例 */
  innerRadius?: string
  /** 外径比例 */
  outerRadius?: string
  /** 是否显示标签 */
  showLabel?: boolean
  /** 是否显示图例 */
  showLegend?: boolean
  /** 颜色配置 */
  colors?: string[]
  /** 图表配置 */
  options?: any
}

const props = withDefaults(defineProps<Props>(), {
  height: '400px',
  loading: false,
  innerRadius: '0%',
  outerRadius: '70%',
  showLabel: true,
  showLegend: true
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
  'ui-pie-chart',
  {
    'ui-pie-chart--loading': props.loading
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
    colors.secondary[500],
    colors.neutral[400],
    colors.neutral[500]
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
      trigger: 'item',
      backgroundColor: isDark ? '#374151' : '#ffffff',
      borderColor: isDark ? '#4b5563' : '#e5e7eb',
      borderWidth: 1,
      textStyle: {
        color: isDark ? '#f9fafb' : '#1f2937'
      },
      formatter: '{b}: {c} ({d}%)'
    },
    
    legend: props.showLegend ? {
      orient: 'horizontal',
      bottom: 10,
      textStyle: {
        color: isDark ? '#d1d5db' : '#4b5563'
      },
      type: 'scroll',
      pageButtonPosition: 'end'
    } : undefined,
    
    color: themeColors,
    
    series: [{
      type: 'pie',
      radius: [props.innerRadius, props.outerRadius],
      center: ['50%', '50%'],
      data: props.data?.map((item, index) => ({
        name: item.name,
        value: item.value,
        itemStyle: item.color ? {
          color: item.color
        } : undefined
      })) || [],
      emphasis: {
        itemStyle: {
          shadowBlur: 10,
          shadowOffsetX: 0,
          shadowColor: 'rgba(0, 0, 0, 0.5)',
          borderColor: isDark ? '#ffffff' : '#1f2937',
          borderWidth: 2
        },
        label: {
          show: true,
          fontSize: 14,
          fontWeight: 'bold'
        }
      },
      label: props.showLabel ? {
        show: true,
        position: 'outside',
        formatter: '{b}: {d}%',
        fontSize: 12,
        color: isDark ? '#d1d5db' : '#4b5563'
      } : {
        show: false
      },
      labelLine: props.showLabel ? {
        show: true,
        lineStyle: {
          color: isDark ? '#6b7280' : '#9ca3af'
        }
      } : {
        show: false
      },
      itemStyle: {
        borderRadius: 8,
        borderColor: isDark ? '#1f2937' : '#ffffff',
        borderWidth: 2
      },
      animationType: 'scale',
      animationEasing: 'elasticOut',
      animationDelay: (idx: number) => Math.random() * 200
    }]
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
watch(() => props.data, () => {
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
.ui-pie-chart {
  position: relative;
  width: 100%;
  min-height: 200px;
}

.ui-pie-chart--loading {
  pointer-events: none;
}

.chart-container {
  width: 100%;
  height: 100%;
  min-height: inherit;
}

/* 深色主题适配 */
.theme-dark .ui-pie-chart {
  background-color: transparent;
}

/* 响应式适配 */
@media (max-width: 768px) {
  .ui-pie-chart {
    min-height: 300px;
  }
}

@media (max-width: 480px) {
  .ui-pie-chart {
    min-height: 250px;
  }
}
</style>