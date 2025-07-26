<template>
  <span :class="tagClasses" :style="tagStyle">
    <!-- 前置图标 -->
    <el-icon v-if="$slots.icon || icon" class="tag-icon tag-icon--prefix">
      <slot name="icon">
        <component :is="icon" />
      </slot>
    </el-icon>
    
    <!-- 标签内容 -->
    <span class="tag-content">
      <slot />
    </span>
    
    <!-- 关闭按钮 -->
    <el-icon 
      v-if="closable" 
      class="tag-close" 
      @click.stop="handleClose"
    >
      <Close />
    </el-icon>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ElIcon } from 'element-plus'
import { Close } from '@element-plus/icons-vue'
import type { Component } from 'vue'

// 组件属性
interface Props {
  /** 标签类型 */
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
  /** 标签变种 */
  variant?: 'filled' | 'outlined' | 'light' | 'ghost'
  /** 标签大小 */
  size?: 'xs' | 'sm' | 'md' | 'lg'
  /** 是否可关闭 */
  closable?: boolean
  /** 是否禁用 */
  disabled?: boolean
  /** 是否圆角 */
  round?: boolean
  /** 前置图标 */
  icon?: Component
  /** 自定义颜色 */
  color?: string
  /** 自定义文本颜色 */
  textColor?: string
  /** 是否可点击 */
  clickable?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  type: 'default',
  variant: 'filled',
  size: 'md',
  closable: false,
  disabled: false,
  round: false,
  clickable: false
})

// 发射事件
const emit = defineEmits<{
  close: []
  click: [event: MouseEvent]
}>()

// 计算样式类
const tagClasses = computed(() => [
  'ui-tag',
  `ui-tag--${props.type}`,
  `ui-tag--${props.variant}`,
  `ui-tag--${props.size}`,
  {
    'ui-tag--closable': props.closable,
    'ui-tag--disabled': props.disabled,
    'ui-tag--round': props.round,
    'ui-tag--clickable': props.clickable,
    'ui-tag--custom-color': props.color,
  }
])

// 计算自定义样式
const tagStyle = computed(() => {
  const style: Record<string, string> = {}
  if (props.color) {
    if (props.variant === 'filled') {
      style.backgroundColor = props.color
      style.borderColor = props.color
      style.color = props.textColor || '#ffffff'
    } else if (props.variant === 'outlined') {
      style.borderColor = props.color
      style.color = props.textColor || props.color
    } else if (props.variant === 'light') {
      style.backgroundColor = `${props.color}20`
      style.borderColor = `${props.color}40`
      style.color = props.textColor || props.color
    }
  } else if (props.textColor) {
    style.color = props.textColor
  }
  return style
})

// 处理关闭事件
const handleClose = () => {
  if (!props.disabled) {
    emit('close')
  }
}

// 处理点击事件
const handleClick = (event: MouseEvent) => {
  if (!props.disabled && props.clickable) {
    emit('click', event)
  }
}
</script>

<style scoped>
.ui-tag {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-1);
  font-family: var(--font-family-sans);
  font-weight: var(--font-weight-normal);
  text-align: center;
  white-space: nowrap;
  border: var(--border-width-1) solid transparent;
  border-radius: var(--border-radius-base);
  transition: all var(--transition-fast);
  user-select: none;
  vertical-align: middle;
}

/* 大小样式 */
.ui-tag--xs {
  padding: 2px var(--spacing-1);
  font-size: 10px;
  line-height: 1.2;
  min-height: 16px;
}

.ui-tag--sm {
  padding: var(--spacing-1) var(--spacing-2);
  font-size: var(--font-size-xs);
  line-height: 1.2;
  min-height: 20px;
}

.ui-tag--md {
  padding: var(--spacing-1) var(--spacing-2);
  font-size: var(--font-size-sm);
  line-height: 1.3;
  min-height: 24px;
}

.ui-tag--lg {
  padding: var(--spacing-2) var(--spacing-3);
  font-size: var(--font-size-base);
  line-height: 1.4;
  min-height: 32px;
}

/* 类型样式 - Filled 变种 */
.ui-tag--primary.ui-tag--filled {
  background-color: var(--color-brand-primary);
  border-color: var(--color-brand-primary);
  color: var(--color-text-inverse);
}

.ui-tag--success.ui-tag--filled {
  background-color: var(--color-success);
  border-color: var(--color-success);
  color: var(--color-text-inverse);
}

.ui-tag--warning.ui-tag--filled {
  background-color: var(--color-warning);
  border-color: var(--color-warning);
  color: var(--color-text-inverse);
}

.ui-tag--danger.ui-tag--filled {
  background-color: var(--color-error);
  border-color: var(--color-error);
  color: var(--color-text-inverse);
}

.ui-tag--info.ui-tag--filled {
  background-color: var(--color-info);
  border-color: var(--color-info);
  color: var(--color-text-inverse);
}

.ui-tag--default.ui-tag--filled {
  background-color: var(--color-neutral-100);
  border-color: var(--color-neutral-200);
  color: var(--color-neutral-700);
}

/* 类型样式 - Outlined 变种 */
.ui-tag--outlined {
  background-color: transparent;
}

.ui-tag--primary.ui-tag--outlined {
  border-color: var(--color-brand-primary);
  color: var(--color-brand-primary);
}

.ui-tag--success.ui-tag--outlined {
  border-color: var(--color-success);
  color: var(--color-success);
}

.ui-tag--warning.ui-tag--outlined {
  border-color: var(--color-warning);
  color: var(--color-warning);
}

.ui-tag--danger.ui-tag--outlined {
  border-color: var(--color-error);
  color: var(--color-error);
}

.ui-tag--info.ui-tag--outlined {
  border-color: var(--color-info);
  color: var(--color-info);
}

.ui-tag--default.ui-tag--outlined {
  border-color: var(--color-border-primary);
  color: var(--color-text-primary);
}

/* 类型样式 - Light 变种 */
.ui-tag--light {
  border-color: transparent;
}

.ui-tag--primary.ui-tag--light {
  background-color: rgba(14, 165, 233, 0.1);
  color: var(--color-brand-primary);
}

.ui-tag--success.ui-tag--light {
  background-color: rgba(34, 197, 94, 0.1);
  color: var(--color-success);
}

.ui-tag--warning.ui-tag--light {
  background-color: rgba(245, 158, 11, 0.1);
  color: var(--color-warning);
}

.ui-tag--danger.ui-tag--light {
  background-color: rgba(239, 68, 68, 0.1);
  color: var(--color-error);
}

.ui-tag--info.ui-tag--light {
  background-color: rgba(59, 130, 246, 0.1);
  color: var(--color-info);
}

.ui-tag--default.ui-tag--light {
  background-color: var(--color-neutral-50);
  color: var(--color-neutral-600);
}

/* 类型样式 - Ghost 变种 */
.ui-tag--ghost {
  background-color: transparent;
  border-color: transparent;
  color: var(--color-text-secondary);
}

/* 形状样式 */
.ui-tag--round {
  border-radius: var(--border-radius-full);
}

/* 交互样式 */
.ui-tag--clickable {
  cursor: pointer;
}

.ui-tag--clickable:hover:not(.ui-tag--disabled) {
  opacity: 0.8;
  transform: translateY(-1px);
}

.ui-tag--clickable:active:not(.ui-tag--disabled) {
  transform: translateY(0);
}

/* 状态样式 */
.ui-tag--disabled {
  opacity: 0.6;
  cursor: not-allowed;
  pointer-events: none;
}

/* 图标样式 */
.tag-icon {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.ui-tag--xs .tag-icon {
  font-size: 10px;
}

.ui-tag--sm .tag-icon {
  font-size: var(--font-size-xs);
}

.ui-tag--md .tag-icon {
  font-size: var(--font-size-sm);
}

.ui-tag--lg .tag-icon {
  font-size: var(--font-size-base);
}

/* 关闭按钮 */
.tag-close {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  margin-left: var(--spacing-1);
  padding: 1px;
  border-radius: var(--border-radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
  opacity: 0.6;
}

.tag-close:hover {
  opacity: 1;
  background-color: rgba(0, 0, 0, 0.1);
}

.ui-tag--xs .tag-close {
  font-size: 8px;
  margin-left: 2px;
}

.ui-tag--sm .tag-close {
  font-size: 10px;
}

.ui-tag--md .tag-close {
  font-size: var(--font-size-xs);
}

.ui-tag--lg .tag-close {
  font-size: var(--font-size-sm);
}

/* 标签内容 */
.tag-content {
  flex: 1;
  min-width: 0;
  line-height: inherit;
}

/* 深色主题适配 */
.theme-dark .ui-tag--default.ui-tag--filled {
  background-color: var(--color-neutral-700);
  border-color: var(--color-neutral-600);
  color: var(--color-neutral-200);
}

.theme-dark .ui-tag--default.ui-tag--outlined {
  border-color: var(--color-border-secondary);
  color: var(--color-text-secondary);
}

.theme-dark .ui-tag--default.ui-tag--light {
  background-color: var(--color-neutral-800);
  color: var(--color-neutral-300);
}

.theme-dark .tag-close:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

/* 动画效果 */
.ui-tag {
  animation: tag-appear 0.2s ease-out;
}

@keyframes tag-appear {
  from {
    opacity: 0;
    transform: scale(0.8);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* 响应式设计 */
@media (max-width: 768px) {
  .ui-tag--clickable:hover:not(.ui-tag--disabled) {
    transform: none;
  }
  
  .ui-tag--lg {
    padding: var(--spacing-2);
    font-size: var(--font-size-sm);
    min-height: 28px;
  }
}
</style>