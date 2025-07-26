<template>
  <div :class="cardClasses" v-bind="$attrs">
    <!-- 卡片头部 -->
    <div v-if="$slots.header || title" class="card-header">
      <slot name="header">
        <div class="card-title-section">
          <h3 v-if="title" class="card-title">{{ title }}</h3>
          <p v-if="subtitle" class="card-subtitle">{{ subtitle }}</p>
        </div>
      </slot>
      <div v-if="$slots.extra" class="card-extra">
        <slot name="extra" />
      </div>
    </div>
    
    <!-- 卡片内容 -->
    <div class="card-body">
      <slot />
    </div>
    
    <!-- 卡片底部 -->
    <div v-if="$slots.footer" class="card-footer">
      <slot name="footer" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDesignSystem } from '@/composables/useDesignSystem'

// 组件属性
interface Props {
  /** 卡片标题 */
  title?: string
  /** 卡片副标题 */
  subtitle?: string
  /** 卡片变种 */
  variant?: 'default' | 'outlined' | 'elevated' | 'filled'
  /** 卡片大小 */
  size?: 'sm' | 'md' | 'lg'
  /** 是否可悬停 */
  hoverable?: boolean
  /** 是否可点击 */
  clickable?: boolean
  /** 是否有边框 */
  bordered?: boolean
  /** 是否有阴影 */
  shadow?: boolean
  /** 自定义内边距 */
  padding?: 'none' | 'sm' | 'md' | 'lg'
  /** 是否加载中 */
  loading?: boolean
  /** 是否禁用 */
  disabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
  size: 'md',
  hoverable: false,
  clickable: false,
  bordered: true,
  shadow: true,
  padding: 'md',
  loading: false,
  disabled: false
})

// 发射事件
const emit = defineEmits<{
  click: [event: MouseEvent]
}>()

// 使用设计系统
const { utils } = useDesignSystem()

// 计算样式类
const cardClasses = computed(() => [
  'ui-card',
  `ui-card--${props.variant}`,
  `ui-card--${props.size}`,
  `ui-card--padding-${props.padding}`,
  {
    'ui-card--hoverable': props.hoverable,
    'ui-card--clickable': props.clickable,
    'ui-card--bordered': props.bordered,
    'ui-card--shadow': props.shadow,
    'ui-card--loading': props.loading,
    'ui-card--disabled': props.disabled,
  }
])

// 处理点击事件
const handleClick = (event: MouseEvent) => {
  if (!props.disabled && !props.loading) {
    emit('click', event)
  }
}
</script>

<style scoped>
.ui-card {
  position: relative;
  background-color: var(--color-bg-elevated);
  border-radius: var(--border-radius-lg);
  transition: all var(--transition-normal);
  overflow: hidden;
}

/* 变种样式 */
.ui-card--default {
  background-color: var(--color-bg-elevated);
}

.ui-card--outlined {
  background-color: transparent;
  border: var(--border-width-1) solid var(--color-border-primary);
}

.ui-card--elevated {
  background-color: var(--color-bg-elevated);
  box-shadow: var(--box-shadow-lg);
}

.ui-card--filled {
  background-color: var(--color-bg-secondary);
}

/* 大小样式 */
.ui-card--sm {
  border-radius: var(--border-radius-md);
}

.ui-card--md {
  border-radius: var(--border-radius-lg);
}

.ui-card--lg {
  border-radius: var(--border-radius-xl);
}

/* 内边距样式 */
.ui-card--padding-none .card-body {
  padding: 0;
}

.ui-card--padding-sm .card-body {
  padding: var(--spacing-3);
}

.ui-card--padding-md .card-body {
  padding: var(--spacing-4);
}

.ui-card--padding-lg .card-body {
  padding: var(--spacing-6);
}

/* 边框样式 */
.ui-card--bordered {
  border: var(--border-width-1) solid var(--color-border-primary);
}

/* 阴影样式 */
.ui-card--shadow {
  box-shadow: var(--box-shadow-sm);
}

/* 交互样式 */
.ui-card--hoverable:hover {
  transform: translateY(-2px);
  box-shadow: var(--box-shadow-md);
}

.ui-card--clickable {
  cursor: pointer;
}

.ui-card--clickable:hover {
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-md);
}

.ui-card--clickable:active {
  transform: translateY(0);
  box-shadow: var(--box-shadow-sm);
}

/* 状态样式 */
.ui-card--loading {
  pointer-events: none;
  opacity: 0.7;
}

.ui-card--disabled {
  pointer-events: none;
  opacity: 0.5;
  cursor: not-allowed;
}

/* 卡片头部 */
.card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: var(--spacing-4) var(--spacing-4) 0;
  border-bottom: var(--border-width-1) solid var(--color-border-primary);
  margin-bottom: var(--spacing-4);
}

.ui-card--padding-sm .card-header {
  padding: var(--spacing-3) var(--spacing-3) 0;
  margin-bottom: var(--spacing-3);
}

.ui-card--padding-lg .card-header {
  padding: var(--spacing-6) var(--spacing-6) 0;
  margin-bottom: var(--spacing-6);
}

.ui-card--padding-none .card-header {
  padding: var(--spacing-4);
  margin-bottom: 0;
}

.card-title-section {
  flex: 1;
  min-width: 0;
}

.card-title {
  margin: 0 0 var(--spacing-1);
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  line-height: var(--line-height-tight);
}

.card-subtitle {
  margin: 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  line-height: var(--line-height-normal);
}

.card-extra {
  flex-shrink: 0;
  margin-left: var(--spacing-4);
}

/* 卡片内容 */
.card-body {
  padding: var(--spacing-4);
}

/* 卡片底部 */
.card-footer {
  padding: 0 var(--spacing-4) var(--spacing-4);
  border-top: var(--border-width-1) solid var(--color-border-primary);
  margin-top: var(--spacing-4);
  padding-top: var(--spacing-4);
}

.ui-card--padding-sm .card-footer {
  padding: 0 var(--spacing-3) var(--spacing-3);
  margin-top: var(--spacing-3);
  padding-top: var(--spacing-3);
}

.ui-card--padding-lg .card-footer {
  padding: 0 var(--spacing-6) var(--spacing-6);
  margin-top: var(--spacing-6);
  padding-top: var(--spacing-6);
}

.ui-card--padding-none .card-footer {
  padding: var(--spacing-4);
  margin-top: 0;
}

/* 加载状态 */
.ui-card--loading::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.1),
    transparent
  );
  animation: loading 1.5s infinite;
  pointer-events: none;
}

@keyframes loading {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(100%);
  }
}

/* 响应式设计 */
@media (max-width: 768px) {
  .card-header {
    flex-direction: column;
    gap: var(--spacing-3);
  }
  
  .card-extra {
    margin-left: 0;
    align-self: stretch;
  }
  
  .ui-card--hoverable:hover,
  .ui-card--clickable:hover {
    transform: none;
  }
}

/* 深色主题适配 */
.theme-dark .ui-card--filled {
  background-color: var(--color-bg-tertiary);
}

.theme-dark .ui-card--loading::after {
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.05),
    transparent
  );
}
</style>