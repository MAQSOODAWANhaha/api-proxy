<template>
  <div v-if="visible" :class="loadingClasses">
    <!-- 背景遮罩 -->
    <div v-if="showOverlay" class="loading-overlay" />
    
    <!-- 加载内容 -->
    <div class="loading-content">
      <!-- 加载指示器 -->
      <div class="loading-indicator">
        <!-- 旋转器 -->
        <div v-if="spinner === 'spin'" class="loading-spinner" />
        
        <!-- 脉冲点 -->
        <div v-else-if="spinner === 'dots'" class="loading-dots">
          <div class="loading-dot" />
          <div class="loading-dot" />
          <div class="loading-dot" />
        </div>
        
        <!-- 进度条 -->
        <div v-else-if="spinner === 'progress'" class="loading-progress">
          <div class="loading-progress-bar" :style="progressStyle" />
        </div>
        
        <!-- 圆形进度 -->
        <div v-else-if="spinner === 'circle'" class="loading-circle">
          <svg class="loading-circle-svg" viewBox="0 0 50 50">
            <circle
              class="loading-circle-path"
              cx="25"
              cy="25"
              r="20"
              fill="none"
              stroke="currentColor"
              stroke-width="4"
              stroke-linecap="round"
              :stroke-dasharray="circumference"
              :stroke-dashoffset="dashOffset"
            />
          </svg>
        </div>
        
        <!-- 自定义图标 -->
        <component 
          v-else-if="spinner === 'custom' && customIcon" 
          :is="customIcon" 
          class="loading-custom-icon"
        />
        
        <!-- 默认旋转器 -->
        <div v-else class="loading-spinner" />
      </div>
      
      <!-- 加载文本 -->
      <div v-if="text" class="loading-text">
        {{ text }}
      </div>
      
      <!-- 加载提示 -->
      <div v-if="tip" class="loading-tip">
        {{ tip }}
      </div>
      
      <!-- 进度百分比 -->
      <div v-if="showProgress && progress > 0" class="loading-percentage">
        {{ Math.round(progress) }}%
      </div>
      
      <!-- 取消按钮 -->
      <Button 
        v-if="cancelable" 
        type="default" 
        size="small"
        class="loading-cancel"
        @click="handleCancel"
      >
        {{ cancelText }}
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import Button from './Button.vue'

// 加载器类型
type SpinnerType = 'spin' | 'dots' | 'progress' | 'circle' | 'custom'

// 加载器大小
type LoadingSize = 'small' | 'default' | 'large'

// 组件属性
interface Props {
  /** 是否显示 */
  visible?: boolean
  /** 加载器类型 */
  spinner?: SpinnerType
  /** 大小 */
  size?: LoadingSize
  /** 加载文本 */
  text?: string
  /** 提示文本 */
  tip?: string
  /** 进度值 (0-100) */
  progress?: number
  /** 显示进度百分比 */
  showProgress?: boolean
  /** 是否可取消 */
  cancelable?: boolean
  /** 取消按钮文本 */
  cancelText?: string
  /** 显示遮罩 */
  overlay?: boolean
  /** 全屏模式 */
  fullscreen?: boolean
  /** 自定义图标 */
  customIcon?: any
}

const props = withDefaults(defineProps<Props>(), {
  visible: true,
  spinner: 'spin',
  size: 'default',
  progress: 0,
  showProgress: false,
  cancelable: false,
  cancelText: '取消',
  overlay: false,
  fullscreen: false
})

// 事件
const emit = defineEmits<{
  cancel: []
}>()

// 计算属性
const loadingClasses = computed(() => [
  'loading',
  `loading--${props.size}`,
  {
    'loading--overlay': props.overlay,
    'loading--fullscreen': props.fullscreen
  }
])

const showOverlay = computed(() => {
  return props.overlay || props.fullscreen
})

const circumference = computed(() => {
  return 2 * Math.PI * 20 // r = 20
})

const dashOffset = computed(() => {
  const progress = Math.max(0, Math.min(100, props.progress))
  return circumference.value - (progress / 100) * circumference.value
})

const progressStyle = computed(() => ({
  width: `${Math.max(0, Math.min(100, props.progress))}%`
}))

// 方法
const handleCancel = () => {
  emit('cancel')
}
</script>

<style scoped>
.loading {
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.loading--overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: var(--z-index-modal);
}

.loading--fullscreen {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: var(--z-index-modal);
}

.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(2px);
}

.theme-dark .loading-overlay {
  background-color: rgba(0, 0, 0, 0.6);
}

.loading-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-3);
  position: relative;
  z-index: 1;
}

/* 加载指示器 */
.loading-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 旋转器 */
.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--color-border-secondary);
  border-top-color: var(--color-brand-primary);
  border-radius: 50%;
  animation: loading-spin 1s linear infinite;
}

.loading--small .loading-spinner {
  width: 24px;
  height: 24px;
  border-width: 2px;
}

.loading--large .loading-spinner {
  width: 48px;
  height: 48px;
  border-width: 4px;
}

@keyframes loading-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 脉冲点 */
.loading-dots {
  display: flex;
  gap: var(--spacing-1);
}

.loading-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--color-brand-primary);
  animation: loading-dots 1.4s ease-in-out infinite both;
}

.loading-dot:nth-child(1) { animation-delay: -0.32s; }
.loading-dot:nth-child(2) { animation-delay: -0.16s; }

.loading--small .loading-dot {
  width: 6px;
  height: 6px;
}

.loading--large .loading-dot {
  width: 12px;
  height: 12px;
}

@keyframes loading-dots {
  0%, 80%, 100% {
    transform: scale(0);
  }
  40% {
    transform: scale(1);
  }
}

/* 进度条 */
.loading-progress {
  width: 200px;
  height: 4px;
  background-color: var(--color-bg-tertiary);
  border-radius: var(--border-radius-full);
  overflow: hidden;
}

.loading-progress-bar {
  height: 100%;
  background-color: var(--color-brand-primary);
  border-radius: inherit;
  transition: width var(--transition-normal);
}

.loading--small .loading-progress {
  width: 150px;
  height: 3px;
}

.loading--large .loading-progress {
  width: 300px;
  height: 6px;
}

/* 圆形进度 */
.loading-circle {
  position: relative;
}

.loading-circle-svg {
  width: 50px;
  height: 50px;
  color: var(--color-brand-primary);
  transform: rotate(-90deg);
  animation: loading-circle-rotate 2s linear infinite;
}

.loading-circle-path {
  transition: stroke-dashoffset var(--transition-normal);
}

.loading--small .loading-circle-svg {
  width: 40px;
  height: 40px;
}

.loading--large .loading-circle-svg {
  width: 60px;
  height: 60px;
}

@keyframes loading-circle-rotate {
  to {
    transform: rotate(270deg);
  }
}

/* 自定义图标 */
.loading-custom-icon {
  font-size: 32px;
  color: var(--color-brand-primary);
  animation: loading-pulse 2s ease-in-out infinite;
}

.loading--small .loading-custom-icon {
  font-size: 24px;
}

.loading--large .loading-custom-icon {
  font-size: 48px;
}

@keyframes loading-pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

/* 文本样式 */
.loading-text {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  text-align: center;
}

.loading--small .loading-text {
  font-size: var(--font-size-sm);
}

.loading--large .loading-text {
  font-size: var(--font-size-lg);
}

.loading-tip {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  text-align: center;
  max-width: 300px;
}

.loading-percentage {
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-brand-primary);
}

.loading-cancel {
  margin-top: var(--spacing-2);
}

/* 响应式设计 */
@media (max-width: 768px) {
  .loading-progress {
    width: 150px;
  }
  
  .loading--large .loading-progress {
    width: 200px;
  }
  
  .loading-tip {
    max-width: 250px;
    font-size: var(--font-size-xs);
  }
}

/* 无障碍支持 */
@media (prefers-reduced-motion: reduce) {
  .loading-spinner,
  .loading-dot,
  .loading-circle-svg,
  .loading-custom-icon {
    animation: none;
  }
}
</style>