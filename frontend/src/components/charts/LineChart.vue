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
import type { LineSeriesOption } from 'echarts/charts'
import type { 
  TitleComponentOption,
  TooltipComponentOption,
  GridComponentOption,
  LegendComponentOption
} from 'echarts/components'

type EChartsOption = echarts.ComposeOption<
  | LineSeriesOption
  | TitleComponentOption
  | TooltipComponentOption
  | GridComponentOption
  | LegendComponentOption
>

interface LineChartData {
  name: string
  data: (number | null)[]
  color?: string
  smooth?: boolean
  areaStyle?: any
  lineStyle?: any
}

interface Props {
  data: LineChartData[]
  xAxis: string[]
  title?: string
  height?: string
  width?: string
  theme?: string
  autoResize?: boolean
  smooth?: boolean
  showArea?: boolean
  showGrid?: boolean
  showLegend?: boolean
  yAxisName?: string
  xAxisName?: string
  colors?: string[]
}

const props = withDefaults(defineProps<Props>(), {
  height: '300px',
  width: '100%',
  theme: '',
  autoResize: true,
  smooth: false,
  showArea: false,
  showGrid: true,
  showLegend: true
})

const emit = defineEmits<{
  chartClick: [params: any]
  chartReady: [chart: echarts.ECharts]
}>()

const baseChartRef = ref<InstanceType<typeof BaseChart>>()

// 默认颜色配置
const defaultColors = [
  '#409eff', '#67c23a', '#e6a23c', '#f56c6c', '#9B59B6',
  '#1abc9c', '#34495e', '#f39c12', '#e74c3c', '#3498db'
]

// 图表配置
const chartOption = computed<EChartsOption>(() => {
  const option: EChartsOption = {
    color: props.colors || defaultColors,
    title: props.title ? {
      text: props.title,
      textStyle: {
        fontSize: 16,
        fontWeight: 'normal'
      }
    } : undefined,
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross',
        label: {
          backgroundColor: '#6a7985'
        }
      }
    },
    legend: props.showLegend ? {
      data: props.data.map(item => item.name),
      bottom: 0
    } : undefined,
    grid: props.showGrid ? {
      left: '3%',
      right: '4%',
      bottom: props.showLegend ? '10%' : '3%',
      containLabel: true
    } : undefined,
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: props.xAxis,
      name: props.xAxisName,
      nameLocation: 'middle',
      nameGap: 25
    },
    yAxis: {
      type: 'value',
      name: props.yAxisName,
      nameLocation: 'middle',
      nameGap: 40,
      nameRotate: 90
    },
    series: props.data.map((item, index) => ({
      name: item.name,
      type: 'line',
      data: item.data,
      smooth: item.smooth !== undefined ? item.smooth : props.smooth,
      lineStyle: item.lineStyle || {
        color: item.color || (props.colors || defaultColors)[index % (props.colors || defaultColors).length]
      },
      areaStyle: (item.areaStyle || props.showArea) ? {
        color: {
          type: 'linear',
          x: 0,
          y: 0,
          x2: 0,
          y2: 1,
          colorStops: [{
            offset: 0,
            color: item.color || (props.colors || defaultColors)[index % (props.colors || defaultColors).length]
          }, {
            offset: 1,
            color: 'rgba(255, 255, 255, 0.1)'
          }]
        },
        ...item.areaStyle
      } : undefined
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
watch(() => [props.data, props.xAxis], () => {
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