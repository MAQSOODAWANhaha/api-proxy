<template>
  <div class="stats-card" :class="`stats-card--${variant}`">
    <div class="stats-card__icon" v-if="icon">
      <el-icon :size="iconSize">
        <component :is="icon" />
      </el-icon>
    </div>
    
    <div class="stats-card__content">
      <div class="stats-card__value">{{ formattedValue }}</div>
      <div class="stats-card__label">{{ label }}</div>
      
      <div class="stats-card__trend" v-if="trend !== undefined">
        <el-icon :size="14">
          <ArrowUp v-if="trend > 0" />
          <ArrowDown v-if="trend < 0" />
          <Minus v-if="trend === 0" />
        </el-icon>
        <span>{{ Math.abs(trend) }}%</span>
        <span class="trend-text">{{ trendText }}</span>
      </div>
    </div>
    
    <div class="stats-card__extra" v-if="$slots.extra">
      <slot name="extra" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ArrowUp, ArrowDown, Minus } from '@element-plus/icons-vue'

interface Props {
  value: number | string
  label: string
  icon?: any
  iconSize?: number
  trend?: number
  variant?: 'primary' | 'success' | 'warning' | 'danger' | 'info'
  format?: 'number' | 'percent' | 'currency' | 'duration'
  precision?: number
}

const props = withDefaults(defineProps<Props>(), {
  iconSize: 24,
  variant: 'primary',
  format: 'number',
  precision: 0
})

const formattedValue = computed(() => {
  if (typeof props.value === 'string') return props.value
  
  switch (props.format) {
    case 'percent':
      return `${props.value.toFixed(props.precision)}%`
    case 'currency':
      return `¥${props.value.toFixed(2)}`
    case 'duration':
      return formatDuration(props.value)
    default:
      return props.value.toLocaleString(undefined, {
        maximumFractionDigits: props.precision
      })
  }
})

const trendText = computed(() => {
  if (props.trend === undefined) return ''
  if (props.trend > 0) return '较上期'
  if (props.trend < 0) return '较上期'
  return '与上期持平'
})

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
  return `${(ms / 60000).toFixed(1)}min`
}
</script>

<style scoped>
.stats-card {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 12px;
  padding: 24px;
  transition: all 0.3s ease;
  position: relative;
  overflow: hidden;
  display: flex;
  align-items: flex-start;
  gap: 16px;
}

.stats-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 3px;
  transition: all 0.3s ease;
}

.stats-card--primary::before {
  background: linear-gradient(90deg, var(--el-color-primary), var(--el-color-primary-light-3));
}

.stats-card--success::before {
  background: linear-gradient(90deg, var(--el-color-success), var(--el-color-success-light-3));
}

.stats-card--warning::before {
  background: linear-gradient(90deg, var(--el-color-warning), var(--el-color-warning-light-3));
}

.stats-card--danger::before {
  background: linear-gradient(90deg, var(--el-color-danger), var(--el-color-danger-light-3));
}

.stats-card--info::before {
  background: linear-gradient(90deg, var(--el-color-info), var(--el-color-info-light-3));
}

.stats-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
  border-color: var(--el-color-primary-light-7);
}

.stats-card__icon {
  flex-shrink: 0;
  width: 48px;
  height: 48px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-size: 24px;
}

.stats-card--primary .stats-card__icon {
  background: linear-gradient(135deg, var(--el-color-primary), var(--el-color-primary-light-3));
}

.stats-card--success .stats-card__icon {
  background: linear-gradient(135deg, var(--el-color-success), var(--el-color-success-light-3));
}

.stats-card--warning .stats-card__icon {
  background: linear-gradient(135deg, var(--el-color-warning), var(--el-color-warning-light-3));
}

.stats-card--danger .stats-card__icon {
  background: linear-gradient(135deg, var(--el-color-danger), var(--el-color-danger-light-3));
}

.stats-card--info .stats-card__icon {
  background: linear-gradient(135deg, var(--el-color-info), var(--el-color-info-light-3));
}

.stats-card__content {
  flex: 1;
  min-width: 0;
}

.stats-card__value {
  font-size: 32px;
  font-weight: 700;
  line-height: 1;
  margin-bottom: 8px;
  color: var(--el-text-color-primary);
}

.stats-card__label {
  font-size: 14px;
  color: var(--el-text-color-secondary);
  margin-bottom: 12px;
  line-height: 1.4;
}

.stats-card__trend {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 500;
}

.stats-card__trend .el-icon {
  flex-shrink: 0;
}

.stats-card__trend .trend-text {
  color: var(--el-text-color-placeholder);
  margin-left: 4px;
}

.stats-card--primary .stats-card__trend {
  color: var(--el-color-primary);
}

.stats-card--success .stats-card__trend {
  color: var(--el-color-success);
}

.stats-card--warning .stats-card__trend {
  color: var(--el-color-warning);
}

.stats-card--danger .stats-card__trend {
  color: var(--el-color-danger);
}

.stats-card--info .stats-card__trend {
  color: var(--el-color-info);
}

.stats-card__extra {
  flex-shrink: 0;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .stats-card {
    padding: 16px;
    flex-direction: column;
    gap: 12px;
  }
  
  .stats-card__icon {
    width: 40px;
    height: 40px;
    align-self: flex-start;
  }
  
  .stats-card__value {
    font-size: 24px;
  }
}

@media (max-width: 480px) {
  .stats-card {
    padding: 12px;
  }
  
  .stats-card__value {
    font-size: 20px;
  }
}
</style>