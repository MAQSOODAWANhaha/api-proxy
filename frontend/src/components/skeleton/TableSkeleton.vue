<template>
  <div class="table-skeleton">
    <!-- 表格头部 -->
    <div class="table-skeleton-header">
      <div 
        v-for="column in columns" 
        :key="column"
        class="table-skeleton-header-cell"
        :style="{ width: getColumnWidth(column) }"
      >
        <Skeleton :width="getHeaderWidth(column)" height="20px" />
      </div>
    </div>
    
    <!-- 表格内容 -->
    <div class="table-skeleton-body">
      <div 
        v-for="row in rows" 
        :key="row"
        class="table-skeleton-row"
      >
        <div 
          v-for="column in columns" 
          :key="`${row}-${column}`"
          class="table-skeleton-cell"
          :style="{ width: getColumnWidth(column) }"
        >
          <Skeleton 
            :width="getCellWidth(column)" 
            height="16px"
            :animation="animation"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import Skeleton from '@/components/ui/Skeleton.vue'

// 组件属性
interface Props {
  /** 行数 */
  rows?: number
  /** 列数 */
  columns?: number
  /** 列宽配置 */
  columnWidths?: string[]
  /** 动画类型 */
  animation?: 'pulse' | 'wave' | 'none'
  /** 是否显示表头 */
  showHeader?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  rows: 5,
  columns: 4,
  columnWidths: [],
  animation: 'pulse',
  showHeader: true
})

// 计算属性
const columnsArray = computed(() => {
  return Array.from({ length: props.columns }, (_, i) => i)
})

const rowsArray = computed(() => {
  return Array.from({ length: props.rows }, (_, i) => i)
})

// 方法
const getColumnWidth = (columnIndex: number): string => {
  if (props.columnWidths[columnIndex]) {
    return props.columnWidths[columnIndex]
  }
  
  // 默认列宽分配
  const defaultWidths = ['25%', '20%', '30%', '25%']
  return defaultWidths[columnIndex % defaultWidths.length] || '25%'
}

const getHeaderWidth = (columnIndex: number): string => {
  // 表头宽度通常比较短
  const widths = ['60%', '50%', '70%', '55%', '65%']
  return widths[columnIndex % widths.length]
}

const getCellWidth = (columnIndex: number): string => {
  // 单元格宽度变化以模拟真实数据
  const widths = ['80%', '65%', '90%', '75%', '85%', '70%', '95%']
  return widths[columnIndex % widths.length]
}
</script>

<style scoped>
.table-skeleton {
  width: 100%;
  border: 1px solid var(--color-border-primary);
  border-radius: var(--border-radius-lg);
  overflow: hidden;
  background-color: var(--color-bg-primary);
}

.table-skeleton-header {
  display: flex;
  background-color: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-border-primary);
  padding: var(--spacing-4);
  gap: var(--spacing-4);
}

.table-skeleton-header-cell {
  display: flex;
  align-items: center;
}

.table-skeleton-body {
  display: flex;
  flex-direction: column;
}

.table-skeleton-row {
  display: flex;
  padding: var(--spacing-4);
  gap: var(--spacing-4);
  border-bottom: 1px solid var(--color-border-primary);
}

.table-skeleton-row:last-child {
  border-bottom: none;
}

.table-skeleton-cell {
  display: flex;
  align-items: center;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .table-skeleton-header,
  .table-skeleton-row {
    padding: var(--spacing-3);
    gap: var(--spacing-3);
  }
}
</style>