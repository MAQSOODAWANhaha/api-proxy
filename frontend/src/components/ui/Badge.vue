<template>
  <span :class="badgeClasses">
    <!-- 徽章点 -->
    <span v-if="dot && !$slots.default" class="badge-dot" />
    
    <!-- 徽章内容 -->
    <span v-else-if="$slots.default || count !== undefined" class="badge-content">
      <slot>
        {{ displayCount }}
      </slot>
    </span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

// 组件属性
interface Props {
  /** 徽章数量 */
  count?: number
  /** 最大显示数量 */
  max?: number
  /** 徽章类型 */
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
  /** 徽章大小 */
  size?: 'xs' | 'sm' | 'md' | 'lg'
  /** 是否显示为点 */
  dot?: boolean
  /** 是否显示边框 */
  bordered?: boolean
  /** 是否隐藏 */
  hidden?: boolean
  /** 自定义颜色 */
  color?: string
  /** 自定义文本颜色 */
  textColor?: string
}

const props = withDefaults(defineProps<Props>(), {
  max: 99,
  type: 'danger',
  size: 'md',
  dot: false,
  bordered: false,
  hidden: false
})

// 计算显示的数量
const displayCount = computed(() => {
  if (props.count === undefined) return ''
  if (props.count <= props.max) return props.count.toString()
  return `${props.max}+`
})

// 计算样式类
const badgeClasses = computed(() => [
  'ui-badge',
  `ui-badge--${props.type}`,
  `ui-badge--${props.size}`,
  {
    'ui-badge--dot': props.dot,
    'ui-badge--bordered': props.bordered,
    'ui-badge--hidden': props.hidden || (props.count !== undefined && props.count <= 0 && !props.dot),
    'ui-badge--custom-color': props.color,
  }
])

// 计算自定义样式
const badgeStyle = computed(() => {
  const style: Record<string, string> = {}
  if (props.color) {
    style.backgroundColor = props.color
  }
  if (props.textColor) {
    style.color = props.textColor
  }
  return style
})
</script>

<style scoped>
.ui-badge {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-family-sans);
  font-weight: var(--font-weight-medium);
  text-align: center;
  white-space: nowrap;
  border-radius: var(--border-radius-full);
  transition: all var(--transition-fast);
  user-select: none;
  vertical-align: middle;
}

/* 大小样式 */
.ui-badge--xs {
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  font-size: 10px;
  line-height: 1;
}

.ui-badge--sm {
  min-width: 18px;
  height: 18px;
  padding: 0 6px;
  font-size: var(--font-size-xs);
  line-height: 1;
}

.ui-badge--md {
  min-width: 20px;
  height: 20px;
  padding: 0 6px;
  font-size: var(--font-size-xs);
  line-height: 1;
}

.ui-badge--lg {
  min-width: 24px;
  height: 24px;
  padding: 0 8px;
  font-size: var(--font-size-sm);
  line-height: 1;
}

/* 类型样式 */
.ui-badge--primary {
  background-color: var(--color-brand-primary);
  color: var(--color-text-inverse);
}

.ui-badge--success {
  background-color: var(--color-success);
  color: var(--color-text-inverse);
}

.ui-badge--warning {
  background-color: var(--color-warning);
  color: var(--color-text-inverse);
}

.ui-badge--danger {
  background-color: var(--color-error);
  color: var(--color-text-inverse);
}

.ui-badge--info {
  background-color: var(--color-info);
  color: var(--color-text-inverse);
}

.ui-badge--default {
  background-color: var(--color-neutral-500);
  color: var(--color-text-inverse);
}

/* 边框样式 */
.ui-badge--bordered {
  border: 2px solid var(--color-bg-primary);
  box-shadow: 0 0 0 1px var(--color-border-primary);
}

/* 点样式 */
.ui-badge--dot {
  width: 8px;
  height: 8px;
  min-width: 8px;
  padding: 0;
  border-radius: var(--border-radius-full);
}

.ui-badge--dot.ui-badge--xs {
  width: 6px;
  height: 6px;
  min-width: 6px;
}

.ui-badge--dot.ui-badge--sm {
  width: 7px;
  height: 7px;
  min-width: 7px;
}

.ui-badge--dot.ui-badge--lg {
  width: 10px;
  height: 10px;
  min-width: 10px;
}

/* 隐藏状态 */
.ui-badge--hidden {
  opacity: 0;
  pointer-events: none;
  transform: scale(0);
}

/* 徽章内容 */
.badge-content {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}

.badge-dot {
  width: 100%;
  height: 100%;
  border-radius: var(--border-radius-full);
  background-color: currentColor;
}

/* 动画效果 */
.ui-badge:not(.ui-badge--hidden) {
  animation: badge-appear 0.2s ease-out;
}

@keyframes badge-appear {
  from {
    opacity: 0;
    transform: scale(0.5);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* 脉动效果 */
.ui-badge--dot::after {
  content: '';
  position: absolute;
  top: -2px;
  left: -2px;
  right: -2px;
  bottom: -2px;
  border: 1px solid currentColor;
  border-radius: var(--border-radius-full);
  opacity: 0;
  animation: badge-pulse 2s infinite;
}

@keyframes badge-pulse {
  0% {
    opacity: 0.8;
    transform: scale(0.8);
  }
  100% {
    opacity: 0;
    transform: scale(1.5);
  }
}

/* 深色主题适配 */
.theme-dark .ui-badge--default {
  background-color: var(--color-neutral-400);
}

.theme-dark .ui-badge--bordered {
  border-color: var(--color-bg-primary);
  box-shadow: 0 0 0 1px var(--color-border-primary);
}
</style>