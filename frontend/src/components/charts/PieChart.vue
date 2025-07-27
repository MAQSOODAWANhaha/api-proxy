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
import type { PieSeriesOption } from 'echarts/charts'
import type { 
  TitleComponentOption,
  TooltipComponentOption,
  LegendComponentOption
} from 'echarts/components'

type EChartsOption = echarts.ComposeOption<
  | PieSeriesOption
  | TitleComponentOption
  | TooltipComponentOption
  | LegendComponentOption
>

interface PieChartData {
  name: string
  value: number
  color?: string
}

interface Props {
  data: PieChartData[]
  title?: string
  height?: string
  width?: string
  theme?: string
  autoResize?: boolean
  donut?: boolean
  showLegend?: boolean
  legendPosition?: 'top' | 'bottom' | 'left' | 'right'
  radius?: [string, string] | string
  center?: [string, string]
  colors?: string[]
  roseType?: boolean | 'area' | 'radius'
  showLabel?: boolean
  labelPosition?: 'inner' | 'outside' | 'center'
}

const props = withDefaults(defineProps<Props>(), {
  height: '300px',
  width: '100%',
  theme: '',
  autoResize: true,
  donut: false,
  showLegend: true,
  legendPosition: 'bottom',
  radius: '70%',
  center: () => ['50%', '50%'] as [string, string],
  roseType: false,
  showLabel: true,
  labelPosition: 'outside'
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
  // 处理颜色映射
  const dataWithColors = props.data.map((item, index) => ({
    ...item,
    itemStyle: {
      color: item.color || (props.colors || defaultColors)[index % (props.colors || defaultColors).length]
    }
  }))

  const option: EChartsOption = {
    color: props.colors || defaultColors,
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
      trigger: 'item',
      formatter: '{a} <br/>{b}: {c} ({d}%)'
    },
    legend: props.showLegend ? {
      orient: ['left', 'right'].includes(props.legendPosition) ? 'vertical' : 'horizontal',
      [props.legendPosition]: props.legendPosition === 'bottom' ? 0 : 
                             props.legendPosition === 'top' ? 20 :
                             props.legendPosition === 'left' ? 0 : 0,
      data: props.data.map(item => item.name)
    } : undefined,
    series: [{
      name: props.title || '数据分布',
      type: 'pie',
      radius: props.donut ? 
        (Array.isArray(props.radius) ? props.radius : ['40%', props.radius]) : 
        props.radius,
      center: props.center,
      data: dataWithColors,
      roseType: props.roseType === true ? 'radius' : (props.roseType === 'area' ? 'area' : undefined),
      emphasis: {
        itemStyle: {
          shadowBlur: 10,
          shadowOffsetX: 0,
          shadowColor: 'rgba(0, 0, 0, 0.5)'
        }
      },
      label: props.showLabel ? {
        show: true,
        position: props.labelPosition,
        formatter: props.labelPosition === 'center' ? '{b}\n{d}%' : '{b}: {d}%'
      } : {
        show: false
      },
      labelLine: {
        show: props.showLabel && props.labelPosition === 'outside'
      },
      itemStyle: {
        borderRadius: 4,
        borderColor: '#fff',
        borderWidth: 2
      }
    }]
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