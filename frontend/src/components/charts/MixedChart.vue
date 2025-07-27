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
import type { 
  LineSeriesOption,
  BarSeriesOption,
  ScatterSeriesOption
} from 'echarts/charts'
import type { 
  TitleComponentOption,
  TooltipComponentOption,
  GridComponentOption,
  LegendComponentOption
} from 'echarts/components'

type EChartsOption = echarts.ComposeOption<
  | LineSeriesOption
  | BarSeriesOption
  | ScatterSeriesOption
  | TitleComponentOption
  | TooltipComponentOption
  | GridComponentOption
  | LegendComponentOption
>

interface MixedChartSeries {
  name: string
  type: 'line' | 'bar' | 'scatter'
  data: (number | null)[]
  yAxisIndex?: number
  color?: string
  smooth?: boolean
  areaStyle?: any
  lineStyle?: any
  itemStyle?: any
  stack?: string
  barWidth?: string | number
}

interface YAxisConfig {
  name?: string
  type?: 'value' | 'category'
  position?: 'left' | 'right'
  min?: number | 'dataMin'
  max?: number | 'dataMax'
  splitLine?: boolean
}

interface Props {
  series: MixedChartSeries[]
  xAxis: string[]
  yAxis?: YAxisConfig[]
  title?: string
  height?: string
  width?: string
  theme?: string
  autoResize?: boolean
  showGrid?: boolean
  showLegend?: boolean
  xAxisName?: string
  colors?: string[]
  dataZoom?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  height: '400px',
  width: '100%',
  theme: '',
  autoResize: true,
  showGrid: true,
  showLegend: true,
  yAxis: () => [{ name: '', position: 'left' }],
  dataZoom: false
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
      },
      left: 'center'
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
      data: props.series.map(item => item.name),
      bottom: 0
    } : undefined,
    grid: props.showGrid ? {
      left: '3%',
      right: props.yAxis && props.yAxis.length > 1 ? '8%' : '4%',
      bottom: props.showLegend ? '15%' : '3%',
      top: props.title ? '15%' : '3%',
      containLabel: true
    } : undefined,
    xAxis: {
      type: 'category',
      data: props.xAxis,
      name: props.xAxisName,
      nameLocation: 'middle',
      nameGap: 25,
      axisLabel: {
        interval: 0,
        rotate: props.xAxis.some(item => item.length > 6) ? 45 : 0
      }
    },
    yAxis: props.yAxis?.map((axis, index) => ({
      type: axis.type || 'value',
      name: axis.name,
      nameLocation: 'middle',
      nameGap: axis.position === 'right' ? 50 : 40,
      nameRotate: 90,
      position: axis.position || (index === 0 ? 'left' : 'right'),
      min: axis.min,
      max: axis.max,
      splitLine: {
        show: axis.splitLine !== false && index === 0
      }
    })) || [{
      type: 'value',
      position: 'left'
    }],
    dataZoom: props.dataZoom ? [
      {
        type: 'inside',
        start: 0,
        end: 100
      },
      {
        type: 'slider',
        start: 0,
        end: 100,
        height: 20,
        bottom: props.showLegend ? '8%' : '2%'
      }
    ] : undefined,
    series: props.series.map((item, index) => {
      const baseConfig = {
        name: item.name,
        type: item.type,
        data: item.data,
        yAxisIndex: item.yAxisIndex || 0,
        color: item.color || (props.colors || defaultColors)[index % (props.colors || defaultColors).length]
      }

      switch (item.type) {
        case 'line':
          return {
            ...baseConfig,
            smooth: item.smooth || false,
            lineStyle: item.lineStyle,
            areaStyle: item.areaStyle,
            symbol: 'circle',
            symbolSize: 6
          }
        case 'bar':
          return {
            ...baseConfig,
            stack: item.stack,
            barWidth: item.barWidth,
            itemStyle: item.itemStyle || {
              borderRadius: [4, 4, 0, 0]
            }
          }
        case 'scatter':
          return {
            ...baseConfig,
            symbolSize: 8,
            itemStyle: item.itemStyle
          }
        default:
          return baseConfig
      }
    })
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
watch(() => [props.series, props.xAxis], () => {
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