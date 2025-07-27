export { default as BaseChart } from './BaseChart.vue'
export { default as LineChart } from './LineChart.vue'
export { default as BarChart } from './BarChart.vue'
export { default as PieChart } from './PieChart.vue'
export { default as GaugeChart } from './GaugeChart.vue'
export { default as MixedChart } from './MixedChart.vue'

// 导出图表相关的类型定义
export interface ChartData {
  name: string
  value: number | number[]
  color?: string
}

export interface LineChartData {
  name: string
  data: (number | null)[]
  color?: string
  smooth?: boolean
  areaStyle?: any
  lineStyle?: any
}

export interface BarChartData {
  name: string
  data: (number | null)[]
  color?: string
  stack?: string
  barWidth?: string | number
  itemStyle?: any
}

export interface PieChartData {
  name: string
  value: number
  color?: string
}

export interface GaugeData {
  name: string
  value: number
}

export interface MixedChartSeries {
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

export interface YAxisConfig {
  name?: string
  type?: 'value' | 'category'
  position?: 'left' | 'right'
  min?: number | 'dataMin'
  max?: number | 'dataMax'
  splitLine?: boolean
}

// 图表主题配置
export const chartThemes = {
  light: '',
  dark: 'dark'
}

// 默认颜色配置
export const defaultColors = {
  primary: [
    '#409eff', '#67c23a', '#e6a23c', '#f56c6c', '#9B59B6',
    '#1abc9c', '#34495e', '#f39c12', '#e74c3c', '#3498db'
  ],
  success: ['#67c23a', '#85ce61', '#95d475', '#a4da89', '#b3e19d'],
  warning: ['#e6a23c', '#ebb563', '#f0c78a', '#f5d9b1', '#faebd7'],
  danger: ['#f56c6c', '#f78989', '#f9a6a6', '#fbc4c4', '#fde1e1'],
  info: ['#909399', '#a6a9ad', '#bcbec2', '#d1d3d6', '#e6e8eb']
}

// 图表工具函数
export const chartUtils = {
  // 格式化数字
  formatNumber: (num: number): string => {
    if (num >= 1000000) {
      return (num / 1000000).toFixed(1) + 'M'
    } else if (num >= 1000) {
      return (num / 1000).toFixed(1) + 'K'
    }
    return num.toString()
  },

  // 格式化百分比
  formatPercentage: (value: number, decimals: number = 1): string => {
    return value.toFixed(decimals) + '%'
  },

  // 生成随机颜色
  generateColor: (): string => {
    return '#' + Math.floor(Math.random() * 16777215).toString(16)
  },

  // 获取渐变色
  getGradientColor: (startColor: string, endColor: string): any => {
    return {
      type: 'linear',
      x: 0,
      y: 0,
      x2: 0,
      y2: 1,
      colorStops: [{
        offset: 0,
        color: startColor
      }, {
        offset: 1,
        color: endColor
      }]
    }
  }
}