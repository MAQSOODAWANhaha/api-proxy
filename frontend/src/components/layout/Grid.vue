<template>
  <div :class="gridClasses" :style="gridStyle">
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
  /** 列数 */
  cols?: ResponsiveValue<number>
  /** 间距 */
  gap?: ResponsiveValue<number | string>
  /** 行间距 */
  rowGap?: ResponsiveValue<number | string>
  /** 列间距 */
  colGap?: ResponsiveValue<number | string>
  /** 自动填充模式 */
  autoFit?: boolean
  /** 自动填充最小宽度 */
  minItemWidth?: string
  /** 对齐方式 */
  justify?: 'start' | 'end' | 'center' | 'stretch' | 'space-around' | 'space-between' | 'space-evenly'
  /** 垂直对齐 */
  align?: 'start' | 'end' | 'center' | 'stretch'
  /** 是否密集排列 */
  dense?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  cols: () => 12,
  gap: () => 4,
  autoFit: false,
  minItemWidth: '250px',
  justify: 'start',
  align: 'stretch',
  dense: false
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
    
    // 如果没找到，使用默认值（通常是最小断点的值）
    return (value as any).xs || (Object.values(value)[0] as T)
  }
  return value as T
}

// 计算样式类
const gridClasses = computed(() => [
  'ui-grid',
  {
    'ui-grid--auto-fit': props.autoFit,
    'ui-grid--dense': props.dense,
    [`ui-grid--justify-${props.justify}`]: props.justify !== 'start',
    [`ui-grid--align-${props.align}`]: props.align !== 'stretch',
  }
])

// 计算网格样式
const gridStyle = computed(() => {
  const style: Record<string, string> = {}
  
  // 获取当前响应式值
  const cols = getResponsiveValue(props.cols)
  const gap = getResponsiveValue(props.gap)
  const rowGap = getResponsiveValue(props.rowGap)
  const colGap = getResponsiveValue(props.colGap)
  
  // 设置网格模板
  if (props.autoFit) {
    style.gridTemplateColumns = `repeat(auto-fit, minmax(${props.minItemWidth}, 1fr))`
  } else if (typeof cols === 'number') {
    style.gridTemplateColumns = `repeat(${cols}, 1fr)`
  }
  
  // 设置间距
  if (gap !== undefined) {
    const gapValue = typeof gap === 'number' ? `var(--spacing-${gap})` : gap
    style.gap = gapValue
  }
  
  if (rowGap !== undefined) {
    const rowGapValue = typeof rowGap === 'number' ? `var(--spacing-${rowGap})` : rowGap
    style.rowGap = rowGapValue
  }
  
  if (colGap !== undefined) {
    const colGapValue = typeof colGap === 'number' ? `var(--spacing-${colGap})` : colGap
    style.columnGap = colGapValue
  }
  
  // 设置对齐方式
  if (props.justify !== 'start') {
    style.justifyItems = props.justify
  }
  
  if (props.align !== 'stretch') {
    style.alignItems = props.align
  }
  
  // 密集排列
  if (props.dense) {
    style.gridAutoFlow = 'dense'
  }
  
  return style
})
</script>

<style scoped>
.ui-grid {
  display: grid;
  width: 100%;
}

/* 对齐样式 */
.ui-grid--justify-end {
  justify-items: end;
}

.ui-grid--justify-center {
  justify-items: center;
}

.ui-grid--justify-stretch {
  justify-items: stretch;
}

.ui-grid--justify-space-around {
  justify-content: space-around;
}

.ui-grid--justify-space-between {
  justify-content: space-between;
}

.ui-grid--justify-space-evenly {
  justify-content: space-evenly;
}

.ui-grid--align-start {
  align-items: start;
}

.ui-grid--align-end {
  align-items: end;
}

.ui-grid--align-center {
  align-items: center;
}

/* 自适应网格 */
.ui-grid--auto-fit {
  grid-template-columns: repeat(auto-fit, minmax(var(--min-item-width, 250px), 1fr));
}

/* 密集排列 */
.ui-grid--dense {
  grid-auto-flow: dense;
}

/* 响应式断点调整 */
@media (max-width: 768px) {
  .ui-grid--auto-fit {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 480px) {
  .ui-grid {
    gap: var(--spacing-3) !important;
  }
}
</style>