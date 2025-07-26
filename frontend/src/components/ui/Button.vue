<template>
  <button
    :class="buttonClasses"
    :disabled="disabled || loading"
    :type="htmlType"
    v-bind="$attrs"
    @click="handleClick"
  >
    <!-- 加载图标 -->
    <el-icon v-if="loading" class="button-loading-icon">
      <Loading />
    </el-icon>
    
    <!-- 前置图标 -->
    <el-icon v-else-if="slots.icon || icon" class="button-icon button-icon--prefix">
      <slot name="icon">
        <component :is="icon" />
      </slot>
    </el-icon>
    
    <!-- 按钮文本 -->
    <span v-if="slots.default" class="button-content">
      <slot />
    </span>
    
    <!-- 后置图标 -->
    <el-icon v-if="slots.suffix && !loading" class="button-icon button-icon--suffix">
      <slot name="suffix" />
    </el-icon>
  </button>
</template>

<script setup lang="ts">
import { computed, useSlots } from 'vue'
import { ElIcon } from 'element-plus'
import { Loading } from '@element-plus/icons-vue'
import type { Component } from 'vue'

// 获取插槽
const slots = useSlots()

// 组件属性
interface Props {
  /** 按钮类型 */
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
  /** 按钮变种 */
  variant?: 'filled' | 'outlined' | 'text' | 'ghost'
  /** 按钮大小 */
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  /** 是否禁用 */
  disabled?: boolean
  /** 是否加载中 */
  loading?: boolean
  /** 是否块级按钮 */
  block?: boolean
  /** 是否圆形按钮 */
  circle?: boolean
  /** 是否圆角按钮 */
  round?: boolean
  /** HTML按钮类型 */
  htmlType?: 'button' | 'submit' | 'reset'
  /** 图标 */
  icon?: Component
  /** 是否自动获取焦点 */
  autoFocus?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  type: 'default',
  variant: 'filled',
  size: 'md',
  disabled: false,
  loading: false,
  block: false,
  circle: false,
  round: false,
  htmlType: 'button',
  autoFocus: false
})

// 发射事件
const emit = defineEmits<{
  click: [event: MouseEvent]
}>()

// 计算样式类
const buttonClasses = computed(() => [
  'ui-button',
  `ui-button--${props.type}`,
  `ui-button--${props.variant}`,
  `ui-button--${props.size}`,
  {
    'ui-button--disabled': props.disabled,
    'ui-button--loading': props.loading,
    'ui-button--block': props.block,
    'ui-button--circle': props.circle,
    'ui-button--round': props.round,
    'ui-button--icon-only': !props.loading && !slots.default && (props.icon || slots.icon),
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
.ui-button {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-2);
  font-family: var(--font-family-sans);
  font-weight: var(--font-weight-medium);
  text-align: center;
  text-decoration: none;
  white-space: nowrap;
  border: var(--border-width-1) solid transparent;
  border-radius: var(--border-radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
  user-select: none;
  outline: none;
  vertical-align: middle;
}

.ui-button:focus {
  outline: 2px solid var(--color-interactive-focus);
  outline-offset: 2px;
}

/* 大小样式 */
.ui-button--xs {
  padding: var(--spacing-1) var(--spacing-2);
  font-size: var(--font-size-xs);
  min-height: 24px;
}

.ui-button--sm {
  padding: var(--spacing-2) var(--spacing-3);
  font-size: var(--font-size-sm);
  min-height: 32px;
}

.ui-button--md {
  padding: var(--spacing-3) var(--spacing-4);
  font-size: var(--font-size-base);
  min-height: 40px;
}

.ui-button--lg {
  padding: var(--spacing-4) var(--spacing-6);
  font-size: var(--font-size-lg);
  min-height: 48px;
}

.ui-button--xl {
  padding: var(--spacing-5) var(--spacing-8);
  font-size: var(--font-size-xl);
  min-height: 56px;
}

/* 类型样式 - Filled 变种 */
.ui-button--primary.ui-button--filled {
  background-color: var(--color-brand-primary);
  border-color: var(--color-brand-primary);
  color: var(--color-text-inverse);
}

.ui-button--primary.ui-button--filled:hover:not(:disabled) {
  background-color: var(--color-brand-secondary);
  border-color: var(--color-brand-secondary);
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-sm);
}

.ui-button--success.ui-button--filled {
  background-color: var(--color-success);
  border-color: var(--color-success);
  color: var(--color-text-inverse);
}

.ui-button--success.ui-button--filled:hover:not(:disabled) {
  background-color: #16a34a;
  border-color: #16a34a;
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-sm);
}

.ui-button--warning.ui-button--filled {
  background-color: var(--color-warning);
  border-color: var(--color-warning);
  color: var(--color-text-inverse);
}

.ui-button--warning.ui-button--filled:hover:not(:disabled) {
  background-color: #d97706;
  border-color: #d97706;
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-sm);
}

.ui-button--danger.ui-button--filled {
  background-color: var(--color-error);
  border-color: var(--color-error);
  color: var(--color-text-inverse);
}

.ui-button--danger.ui-button--filled:hover:not(:disabled) {
  background-color: #dc2626;
  border-color: #dc2626;
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-sm);
}

.ui-button--info.ui-button--filled {
  background-color: var(--color-info);
  border-color: var(--color-info);
  color: var(--color-text-inverse);
}

.ui-button--info.ui-button--filled:hover:not(:disabled) {
  background-color: #2563eb;
  border-color: #2563eb;
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-sm);
}

.ui-button--default.ui-button--filled {
  background-color: var(--color-bg-elevated);
  border-color: var(--color-border-primary);
  color: var(--color-text-primary);
}

.ui-button--default.ui-button--filled:hover:not(:disabled) {
  background-color: var(--color-interactive-hover);
  border-color: var(--color-border-secondary);
  transform: translateY(-1px);
  box-shadow: var(--box-shadow-sm);
}

/* 类型样式 - Outlined 变种 */
.ui-button--outlined {
  background-color: transparent;
}

.ui-button--primary.ui-button--outlined {
  border-color: var(--color-brand-primary);
  color: var(--color-brand-primary);
}

.ui-button--primary.ui-button--outlined:hover:not(:disabled) {
  background-color: var(--color-brand-primary);
  color: var(--color-text-inverse);
}

.ui-button--success.ui-button--outlined {
  border-color: var(--color-success);
  color: var(--color-success);
}

.ui-button--success.ui-button--outlined:hover:not(:disabled) {
  background-color: var(--color-success);
  color: var(--color-text-inverse);
}

.ui-button--warning.ui-button--outlined {
  border-color: var(--color-warning);
  color: var(--color-warning);
}

.ui-button--warning.ui-button--outlined:hover:not(:disabled) {
  background-color: var(--color-warning);
  color: var(--color-text-inverse);
}

.ui-button--danger.ui-button--outlined {
  border-color: var(--color-error);
  color: var(--color-error);
}

.ui-button--danger.ui-button--outlined:hover:not(:disabled) {
  background-color: var(--color-error);
  color: var(--color-text-inverse);
}

.ui-button--info.ui-button--outlined {
  border-color: var(--color-info);
  color: var(--color-info);
}

.ui-button--info.ui-button--outlined:hover:not(:disabled) {
  background-color: var(--color-info);
  color: var(--color-text-inverse);
}

.ui-button--default.ui-button--outlined {
  border-color: var(--color-border-primary);
  color: var(--color-text-primary);
}

.ui-button--default.ui-button--outlined:hover:not(:disabled) {
  background-color: var(--color-interactive-hover);
  border-color: var(--color-border-secondary);
}

/* 类型样式 - Text 变种 */
.ui-button--text {
  background-color: transparent;
  border-color: transparent;
}

.ui-button--primary.ui-button--text {
  color: var(--color-brand-primary);
}

.ui-button--primary.ui-button--text:hover:not(:disabled) {
  background-color: var(--color-interactive-focus);
}

.ui-button--success.ui-button--text {
  color: var(--color-success);
}

.ui-button--success.ui-button--text:hover:not(:disabled) {
  background-color: rgba(34, 197, 94, 0.1);
}

.ui-button--warning.ui-button--text {
  color: var(--color-warning);
}

.ui-button--warning.ui-button--text:hover:not(:disabled) {
  background-color: rgba(245, 158, 11, 0.1);
}

.ui-button--danger.ui-button--text {
  color: var(--color-error);
}

.ui-button--danger.ui-button--text:hover:not(:disabled) {
  background-color: rgba(239, 68, 68, 0.1);
}

.ui-button--info.ui-button--text {
  color: var(--color-info);
}

.ui-button--info.ui-button--text:hover:not(:disabled) {
  background-color: rgba(59, 130, 246, 0.1);
}

.ui-button--default.ui-button--text {
  color: var(--color-text-primary);
}

.ui-button--default.ui-button--text:hover:not(:disabled) {
  background-color: var(--color-interactive-hover);
}

/* 类型样式 - Ghost 变种 */
.ui-button--ghost {
  background-color: transparent;
  border-color: transparent;
  color: var(--color-text-secondary);
}

.ui-button--ghost:hover:not(:disabled) {
  background-color: var(--color-interactive-hover);
  color: var(--color-text-primary);
}

/* 形状样式 */
.ui-button--round {
  border-radius: var(--border-radius-full);
}

.ui-button--circle {
  border-radius: var(--border-radius-full);
  aspect-ratio: 1;
  padding: 0;
}

.ui-button--circle.ui-button--xs {
  width: 24px;
  height: 24px;
}

.ui-button--circle.ui-button--sm {
  width: 32px;
  height: 32px;
}

.ui-button--circle.ui-button--md {
  width: 40px;
  height: 40px;
}

.ui-button--circle.ui-button--lg {
  width: 48px;
  height: 48px;
}

.ui-button--circle.ui-button--xl {
  width: 56px;
  height: 56px;
}

/* 块级按钮 */
.ui-button--block {
  display: flex;
  width: 100%;
}

/* 图标按钮 */
.ui-button--icon-only {
  aspect-ratio: 1;
  padding: 0;
}

.ui-button--icon-only.ui-button--xs {
  width: 24px;
}

.ui-button--icon-only.ui-button--sm {
  width: 32px;
}

.ui-button--icon-only.ui-button--md {
  width: 40px;
}

.ui-button--icon-only.ui-button--lg {
  width: 48px;
}

.ui-button--icon-only.ui-button--xl {
  width: 56px;
}

/* 状态样式 */
.ui-button--disabled {
  opacity: 0.6;
  cursor: not-allowed;
  pointer-events: none;
}

.ui-button--loading {
  pointer-events: none;
}

.ui-button:active:not(:disabled) {
  transform: translateY(0);
  box-shadow: none;
}

/* 图标样式 */
.button-icon {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.button-loading-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.button-content {
  flex: 1;
  min-width: 0;
  line-height: 1;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .ui-button:hover:not(:disabled) {
    transform: none;
    box-shadow: none;
  }
  
  .ui-button--xl {
    padding: var(--spacing-4) var(--spacing-6);
    font-size: var(--font-size-lg);
    min-height: 48px;
  }
}

/* 深色主题适配 */
.theme-dark .ui-button--default.ui-button--filled {
  background-color: var(--color-bg-tertiary);
}

.theme-dark .ui-button--ghost {
  color: var(--color-text-tertiary);
}

.theme-dark .ui-button--ghost:hover:not(:disabled) {
  color: var(--color-text-secondary);
}
</style>