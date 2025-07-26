<template>
  <div 
    :class="skeletonClasses"
    :style="skeletonStyles"
  >
    <!-- 基础骨架屏 -->
    <div v-if="!complex" class="skeleton-item" />
    
    <!-- 复杂骨架屏布局 -->
    <div v-else class="skeleton-complex">
      <!-- 头像 -->
      <div v-if="avatar" :class="avatarClasses" />
      
      <!-- 内容区域 -->
      <div class="skeleton-content">
        <!-- 标题行 -->
        <div v-if="title" class="skeleton-title" />
        
        <!-- 段落行 -->
        <div 
          v-for="(line, index) in paragraphLines" 
          :key="index"
          :class="getLineClasses(index, paragraphLines.length)"
          :style="getLineStyle(index, paragraphLines.length)"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

// 骨架屏形状
type SkeletonShape = 'default' | 'circle' | 'square'

// 骨架屏大小
type SkeletonSize = 'small' | 'default' | 'large'

// 骨架屏动画
type SkeletonAnimation = 'pulse' | 'wave' | 'none'

// 组件属性
interface Props {
  /** 是否显示骨架屏 */
  loading?: boolean
  /** 是否使用复杂布局 */
  complex?: boolean
  /** 显示头像 */
  avatar?: boolean
  /** 头像形状 */
  avatarShape?: SkeletonShape
  /** 头像大小 */
  avatarSize?: SkeletonSize
  /** 显示标题 */
  title?: boolean
  /** 段落行数 */
  paragraph?: number | boolean
  /** 形状 */
  shape?: SkeletonShape
  /** 大小 */
  size?: SkeletonSize
  /** 动画类型 */
  animation?: SkeletonAnimation
  /** 宽度 */
  width?: string | number
  /** 高度 */
  height?: string | number
  /** 是否激活状态 */
  active?: boolean
  /** 行间距 */
  rowGap?: string | number
}

const props = withDefaults(defineProps<Props>(), {
  loading: true,
  complex: false,
  avatar: false,
  avatarShape: 'circle',
  avatarSize: 'default',
  title: true,
  paragraph: 3,
  shape: 'default',
  size: 'default',
  animation: 'pulse',
  active: true,
  rowGap: '12px'
})

// 计算属性
const skeletonClasses = computed(() => [
  'skeleton',
  `skeleton--${props.size}`,
  `skeleton--${props.shape}`,
  {
    'skeleton--active': props.active,
    'skeleton--pulse': props.animation === 'pulse',
    'skeleton--wave': props.animation === 'wave',
    'skeleton--complex': props.complex
  }
])

const skeletonStyles = computed(() => {
  const styles: Record<string, any> = {}
  
  if (props.width) {
    styles.width = typeof props.width === 'number' ? `${props.width}px` : props.width
  }
  
  if (props.height && !props.complex) {
    styles.height = typeof props.height === 'number' ? `${props.height}px` : props.height
  }
  
  if (props.complex && props.rowGap) {
    styles.gap = typeof props.rowGap === 'number' ? `${props.rowGap}px` : props.rowGap
  }
  
  return styles
})

const avatarClasses = computed(() => [
  'skeleton-avatar',
  `skeleton-avatar--${props.avatarShape}`,
  `skeleton-avatar--${props.avatarSize}`
])

const paragraphLines = computed(() => {
  if (typeof props.paragraph === 'boolean') {
    return props.paragraph ? 3 : 0
  }
  return Math.max(0, props.paragraph)
})

// 方法
const getLineClasses = (index: number, total: number) => [
  'skeleton-line',
  {
    'skeleton-line--last': index === total - 1
  }
]

const getLineStyle = (index: number, total: number) => {
  // 最后一行通常比较短
  if (index === total - 1) {
    return { width: '60%' }
  }
  
  // 随机化行宽度以模拟真实内容
  const widths = ['100%', '95%', '85%', '90%', '100%']
  return { width: widths[index % widths.length] }
}
</script>

<style scoped>
.skeleton {
  position: relative;
  overflow: hidden;
  background-color: var(--color-bg-secondary);
  border-radius: var(--border-radius-md);
}

.skeleton--default {
  border-radius: var(--border-radius-md);
}

.skeleton--circle {
  border-radius: 50%;
}

.skeleton--square {
  border-radius: var(--border-radius-sm);
}

.skeleton--small {
  font-size: var(--font-size-sm);
}

.skeleton--default {
  font-size: var(--font-size-base);
}

.skeleton--large {
  font-size: var(--font-size-lg);
}

/* 基础骨架屏 */
.skeleton-item {
  width: 100%;
  height: 20px;
  background-color: var(--color-bg-tertiary);
  border-radius: inherit;
}

/* 复杂骨架屏布局 */
.skeleton-complex {
  display: flex;
  gap: var(--spacing-3);
  align-items: flex-start;
}

.skeleton-avatar {
  flex-shrink: 0;
  background-color: var(--color-bg-tertiary);
}

.skeleton-avatar--small {
  width: 32px;
  height: 32px;
}

.skeleton-avatar--default {
  width: 40px;
  height: 40px;
}

.skeleton-avatar--large {
  width: 48px;
  height: 48px;
}

.skeleton-avatar--circle {
  border-radius: 50%;
}

.skeleton-avatar--square {
  border-radius: var(--border-radius-sm);
}

.skeleton-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.skeleton-title {
  height: 20px;
  width: 40%;
  background-color: var(--color-bg-tertiary);
  border-radius: var(--border-radius-sm);
}

.skeleton-line {
  height: 16px;
  background-color: var(--color-bg-tertiary);
  border-radius: var(--border-radius-sm);
}

.skeleton-line--last {
  width: 60% !important;
}

/* 动画效果 */
.skeleton--active .skeleton-item,
.skeleton--active .skeleton-avatar,
.skeleton--active .skeleton-title,
.skeleton--active .skeleton-line {
  position: relative;
  overflow: hidden;
}

/* 脉冲动画 */
.skeleton--pulse .skeleton-item,
.skeleton--pulse .skeleton-avatar,
.skeleton--pulse .skeleton-title,
.skeleton--pulse .skeleton-line {
  animation: skeleton-pulse 2s ease-in-out infinite;
}

@keyframes skeleton-pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

/* 波浪动画 */
.skeleton--wave .skeleton-item::after,
.skeleton--wave .skeleton-avatar::after,
.skeleton--wave .skeleton-title::after,
.skeleton--wave .skeleton-line::after {
  content: '';
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.4),
    transparent
  );
  animation: skeleton-wave 2s infinite;
}

@keyframes skeleton-wave {
  0% {
    left: -100%;
  }
  100% {
    left: 100%;
  }
}

/* 深色主题适配 */
.theme-dark .skeleton {
  background-color: var(--color-bg-tertiary);
}

.theme-dark .skeleton-item,
.theme-dark .skeleton-avatar,
.theme-dark .skeleton-title,
.theme-dark .skeleton-line {
  background-color: var(--color-bg-secondary);
}

.theme-dark .skeleton--wave .skeleton-item::after,
.theme-dark .skeleton--wave .skeleton-avatar::after,
.theme-dark .skeleton--wave .skeleton-title::after,
.theme-dark .skeleton--wave .skeleton-line::after {
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.1),
    transparent
  );
}

/* 响应式设计 */
@media (max-width: 768px) {
  .skeleton-complex {
    gap: var(--spacing-2);
  }
  
  .skeleton-avatar--small {
    width: 28px;
    height: 28px;
  }
  
  .skeleton-avatar--default {
    width: 36px;
    height: 36px;
  }
  
  .skeleton-avatar--large {
    width: 44px;
    height: 44px;
  }
  
  .skeleton-title {
    height: 18px;
  }
  
  .skeleton-line {
    height: 14px;
  }
}

/* 无障碍支持 */
@media (prefers-reduced-motion: reduce) {
  .skeleton--pulse .skeleton-item,
  .skeleton--pulse .skeleton-avatar,
  .skeleton--pulse .skeleton-title,
  .skeleton--pulse .skeleton-line {
    animation: none;
  }
  
  .skeleton--wave .skeleton-item::after,
  .skeleton--wave .skeleton-avatar::after,
  .skeleton--wave .skeleton-title::after,
  .skeleton--wave .skeleton-line::after {
    animation: none;
  }
}
</style>