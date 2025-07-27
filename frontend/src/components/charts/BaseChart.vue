<template>
  <div 
    ref="chartRef" 
    :style="{ height: height, width: width }" 
    class="base-chart"
  ></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import * as echarts from 'echarts'

interface Props {
  option: any
  height?: string
  width?: string
  theme?: string
  autoResize?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  height: '300px',
  width: '100%',
  theme: '',
  autoResize: true
})

const emit = defineEmits<{
  chartClick: [params: any]
  chartReady: [chart: echarts.ECharts]
}>()

const chartRef = ref<HTMLElement>()
const chart = ref<echarts.ECharts>()

// 初始化图表
const initChart = async () => {
  if (!chartRef.value) return
  
  await nextTick()
  
  // 销毁现有图表
  if (chart.value) {
    chart.value.dispose()
  }
  
  // 创建新图表
  chart.value = echarts.init(chartRef.value, props.theme)
  
  // 设置配置
  if (props.option) {
    chart.value.setOption(props.option, true)
  }
  
  // 绑定点击事件
  chart.value.on('click', (params) => {
    emit('chartClick', params)
  })
  
  // 触发就绪事件
  emit('chartReady', chart.value)
  
  // 自动调整大小
  if (props.autoResize) {
    window.addEventListener('resize', handleResize)
  }
}

// 处理窗口大小变化
const handleResize = () => {
  chart.value?.resize()
}

// 更新图表配置
const updateChart = (newOption: any) => {
  if (chart.value && newOption) {
    chart.value.setOption(newOption, true)
  }
}

// 清空图表
const clearChart = () => {
  chart.value?.clear()
}

// 获取图表实例
const getChart = () => {
  return chart.value
}

// 监听配置变化
watch(() => props.option, (newOption) => {
  if (newOption) {
    updateChart(newOption)
  }
}, { deep: true })

// 暴露方法
defineExpose({
  updateChart,
  clearChart,
  getChart,
  resize: handleResize
})

onMounted(() => {
  initChart()
})

onUnmounted(() => {
  if (props.autoResize) {
    window.removeEventListener('resize', handleResize)
  }
  chart.value?.dispose()
})
</script>

<style scoped>
.base-chart {
  position: relative;
}
</style>