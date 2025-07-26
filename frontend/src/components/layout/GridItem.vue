<template>
  <div :class="itemClasses" :style="itemStyle">
    <slot />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useResponsive } from '@/composables/useDesignSystem'

// 响应式配置类型
type ResponsiveValue<T> = T | {
  xs?: T
  sm?: T
  md?: T
  lg?: T
  xl?: T
  '2xl'?: T
}

// 组件属性
interface Props {
  /** 占用列数 */
  span?: ResponsiveValue<number>
  /** 列偏移 */
  offset?: ResponsiveValue<number>
  /** 行开始位置 */
  rowStart?: ResponsiveValue<number>
  /** 行结束位置 */
  rowEnd?: ResponsiveValue<number>
  /** 列开始位置 */
  colStart?: ResponsiveValue<number>
  /** 列结束位置 */
  colEnd?: ResponsiveValue<number>
  /** 占用行数 */
  rowSpan?: ResponsiveValue<number>
  /** 自对齐方式 */
  justify?: 'start' | 'end' | 'center' | 'stretch'
  /** 自垂直对齐 */
  align?: 'start' | 'end' | 'center' | 'stretch'
  /** 网格区域名称 */
  area?: string
  /** 显示顺序 */
  order?: ResponsiveValue<number>
}

const props = withDefaults(defineProps<Props>(), {
  span: 1,
  offset: 0,
  justify: 'stretch',
  align: 'stretch'
})

// 使用响应式工具
const { current: currentBreakpoint } = useResponsive()

// 获取响应式值
const getResponsiveValue = <T>(value: ResponsiveValue<T>): T => {
  if (typeof value === 'object' && value !== null) {
    const breakpoints = ['2xl', 'xl', 'lg', 'md', 'sm', 'xs'] as const
    const currentIndex = breakpoints.indexOf(currentBreakpoint.value as any)
    
    // 从当前断点开始向下查找
    for (let i = currentIndex; i < breakpoints.length; i++) {
      const bp = breakpoints[i]
      if (value[bp] !== undefined) {
        return value[bp] as T
      }
    }
    
    // 如果没找到，使用默认值
    return (value as any).xs || (Object.values(value)[0] as T)
  }
  return value as T
}

// 计算样式类
const itemClasses = computed(() => [
  'ui-grid-item',
  {
    [`ui-grid-item--justify-${props.justify}`]: props.justify !== 'stretch',
    [`ui-grid-item--align-${props.align}`]: props.align !== 'stretch',
  }
])

// 计算网格项样式
const itemStyle = computed(() => {
  const style: Record<string, string> = {}
  
  // 获取当前响应式值
  const span = getResponsiveValue(props.span)
  const offset = getResponsiveValue(props.offset)
  const rowStart = getResponsiveValue(props.rowStart)
  const rowEnd = getResponsiveValue(props.rowEnd)
  const colStart = getResponsiveValue(props.colStart)
  const colEnd = getResponsiveValue(props.colEnd)
  const rowSpan = getResponsiveValue(props.rowSpan)
  const order = getResponsiveValue(props.order)
  
  // 设置列跨度
  if (span && span > 0) {
    if (offset && offset > 0) {
      style.gridColumn = `${offset + 1} / span ${span}`
    } else {
      style.gridColumnEnd = `span ${span}`
    }
  }
  
  // 设置精确的网格位置
  if (colStart !== undefined) {
    style.gridColumnStart = colStart.toString()
  }
  
  if (colEnd !== undefined) {
    style.gridColumnEnd = colEnd.toString()
  }
  
  if (rowStart !== undefined) {
    style.gridRowStart = rowStart.toString()
  }
  
  if (rowEnd !== undefined) {
    style.gridRowEnd = rowEnd.toString()
  }
  
  // 设置行跨度
  if (rowSpan && rowSpan > 0) {
    style.gridRowEnd = `span ${rowSpan}`
  }
  
  // 设置网格区域
  if (props.area) {
    style.gridArea = props.area
  }
  
  // 设置显示顺序
  if (order !== undefined) {
    style.order = order.toString()
  }
  
  // 设置对齐方式
  if (props.justify !== 'stretch') {
    style.justifySelf = props.justify
  }
  
  if (props.align !== 'stretch') {
    style.alignSelf = props.align
  }
  
  return style
})
</script>

<style scoped>
.ui-grid-item {
  min-width: 0;
  min-height: 0;
}

/* 自对齐样式 */
.ui-grid-item--justify-start {
  justify-self: start;
}

.ui-grid-item--justify-end {
  justify-self: end;
}

.ui-grid-item--justify-center {
  justify-self: center;
}

.ui-grid-item--align-start {
  align-self: start;
}

.ui-grid-item--align-end {
  align-self: end;
}

.ui-grid-item--align-center {
  align-self: center;
}

/* 响应式优化 */
@media (max-width: 768px) {
  .ui-grid-item {
    /* 在小屏幕上，网格项可能需要额外的最小宽度 */
    min-width: 0;
  }
}

/* 打印优化 */
@media print {
  .ui-grid-item {
    break-inside: avoid;
    page-break-inside: avoid;
  }
}
</style>