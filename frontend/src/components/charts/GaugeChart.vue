<template>
  <BaseChart
    ref="baseChartRef"
    :option="chartOption"
    :height="height"
    :width="width"
    :theme="theme"
    :auto-resize="autoResize"
    @chart-click="handleChartClick"
    @chart-ready="handleChartReady"
  />
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import BaseChart from './BaseChart.vue'
import type { GaugeSeriesOption } from 'echarts/charts'
import type { 
  TitleComponentOption,
  TooltipComponentOption
} from 'echarts/components'

type EChartsOption = echarts.ComposeOption<
  | GaugeSeriesOption
  | TitleComponentOption
  | TooltipComponentOption
>

interface GaugeData {
  name: string
  value: number
}

interface Props {
  data: GaugeData[]
  title?: string
  height?: string
  width?: string
  theme?: string
  autoResize?: boolean
  min?: number
  max?: number
  unit?: string
  colors?: string[]
  showDetail?: boolean
  radius?: string
  center?: [string, string]
}

const props = withDefaults(defineProps<Props>(), {
  height: '300px',
  width: '100%',
  theme: '',
  autoResize: true,
  min: 0,
  max: 100,
  unit: '%',
  showDetail: true,
  radius: '75%',
  center: () => ['50%', '55%'] as [string, string]
})

const emit = defineEmits<{
  chartClick: [params: any]
  chartReady: [chart: echarts.ECharts]
}>()

const baseChartRef = ref<InstanceType<typeof BaseChart>>()

// 默认颜色配置
const defaultColors = [
  ['#67c23a', '#e6a23c', '#f56c6c'],
  ['#409eff', '#67c23a', '#e6a23c'],
  ['#1abc9c', '#3498db', '#9b59b6']
]

// 图表配置
const chartOption = computed<EChartsOption>(() => {
  const colors = props.colors || defaultColors[0]
  
  const option: EChartsOption = {
    title: props.title ? {
      text: props.title,
      textStyle: {
        fontSize: 16,
        fontWeight: 'normal'
      },
      left: 'center',
      top: 20
    } : undefined,
    tooltip: {
      formatter: '{a} <br/>{b}: {c}' + props.unit
    },
    series: props.data.map((item) => ({
      name: item.name,
      type: 'gauge',
      radius: props.radius,
      center: props.center,
      min: props.min,
      max: props.max,
      splitNumber: 10,
      axisLine: {
        lineStyle: {
          color: [
            [0.3, colors[0]],
            [0.7, colors[1]],
            [1, colors[2]]
          ],
          width: 8
        }
      },
      pointer: {
        itemStyle: {
          color: 'auto'
        }
      },
      axisTick: {
        distance: -10,
        length: 8,
        lineStyle: {
          color: '#fff',
          width: 2
        }
      },
      splitLine: {
        distance: -10,
        length: 10,
        lineStyle: {
          color: '#fff',
          width: 3
        }
      },
      axisLabel: {
        color: 'auto',
        distance: 20,
        fontSize: 12
      },
      detail: props.showDetail ? {
        valueAnimation: true,
        formatter: '{value}' + props.unit,
        color: 'auto',
        fontSize: 18,
        fontWeight: 'bold',
        offsetCenter: [0, '70%']
      } : undefined,
      data: [item]
    }))
  }

  return option
})

// 处理图表点击事件
const handleChartClick = (params: any) => {
  emit('chartClick', params)
}

// 处理图表就绪事件
const handleChartReady = (chart: echarts.ECharts) => {
  emit('chartReady', chart)
}

// 更新图表
const updateChart = (newOption?: any) => {
  if (newOption) {
    baseChartRef.value?.updateChart(newOption)
  } else {
    baseChartRef.value?.updateChart(chartOption.value)
  }
}

// 清空图表
const clearChart = () => {
  baseChartRef.value?.clearChart()
}

// 获取图表实例
const getChart = () => {
  return baseChartRef.value?.getChart()
}

// 调整大小
const resize = () => {
  baseChartRef.value?.resize()
}

// 监听数据变化
watch(() => props.data, () => {
  updateChart()
}, { deep: true })

// 暴露方法
defineExpose({
  updateChart,
  clearChart,
  getChart,
  resize
})
</script>