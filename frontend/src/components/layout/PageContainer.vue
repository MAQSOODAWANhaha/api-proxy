<template>
  <div :class="containerClasses">
    <!-- 页面头部 -->
    <div v-if="$slots.header || title || breadcrumb" class="page-header">
      <slot name="header">
        <!-- 面包屑导航 -->
        <nav v-if="breadcrumb && breadcrumb.length > 0" class="page-breadcrumb">
          <ol class="breadcrumb-list">
            <li 
              v-for="(item, index) in breadcrumb" 
              :key="index" 
              class="breadcrumb-item"
              :class="{ 'breadcrumb-item--active': index === breadcrumb.length - 1 }"
            >
              <router-link 
                v-if="item.path && index < breadcrumb.length - 1" 
                :to="item.path"
                class="breadcrumb-link"
              >
                {{ item.title }}
              </router-link>
              <span v-else class="breadcrumb-text">{{ item.title }}</span>
              <el-icon v-if="index < breadcrumb.length - 1" class="breadcrumb-separator">
                <ArrowRight />
              </el-icon>
            </li>
          </ol>
        </nav>
        
        <!-- 页面标题区域 -->
        <div class="page-title-section">
          <div class="page-title-content">
            <h1 v-if="title" class="page-title">{{ title }}</h1>
            <p v-if="description" class="page-description">{{ description }}</p>
          </div>
          <div v-if="$slots.extra" class="page-extra">
            <slot name="extra" />
          </div>
        </div>
      </slot>
    </div>
    
    <!-- 页面内容 -->
    <div class="page-content">
      <slot />
    </div>
    
    <!-- 页面底部 */
    <div v-if="$slots.footer" class="page-footer">
      <slot name="footer" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ElIcon } from 'element-plus'
import { ArrowRight } from '@element-plus/icons-vue'

// 面包屑项类型
interface BreadcrumbItem {
  title: string
  path?: string
}

// 组件属性
interface Props {
  /** 页面标题 */
  title?: string
  /** 页面描述 */
  description?: string
  /** 面包屑导航 */
  breadcrumb?: BreadcrumbItem[]
  /** 容器大小 */
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full'
  /** 是否流式布局 */
  fluid?: boolean
  /** 是否有内边距 */
  padded?: boolean
  /** 是否居中对齐 */
  centered?: boolean
  /** 背景变种 */
  background?: 'default' | 'secondary' | 'transparent'
  /** 是否有边框 */
  bordered?: boolean
  /** 最小高度 */
  minHeight?: string
}

const props = withDefaults(defineProps<Props>(), {
  size: 'lg',
  fluid: false,
  padded: true,
  centered: false,
  background: 'default',
  bordered: false
})

// 计算容器样式类
const containerClasses = computed(() => [
  'page-container',
  `page-container--${props.size}`,
  `page-container--bg-${props.background}`,
  {
    'page-container--fluid': props.fluid,
    'page-container--padded': props.padded,
    'page-container--centered': props.centered,
    'page-container--bordered': props.bordered,
  }
])
</script>

<style scoped>
.page-container {
  width: 100%;
  min-height: 100%;
  background-color: var(--color-bg-primary);
  transition: all var(--transition-normal);
}

/* 容器大小 */
.page-container--sm {
  max-width: 640px;
}

.page-container--md {
  max-width: 768px;
}

.page-container--lg {
  max-width: 1024px;
}

.page-container--xl {
  max-width: 1280px;
}

.page-container--full {
  max-width: none;
}

/* 布局样式 */
.page-container--fluid {
  max-width: none;
  width: 100%;
}

.page-container--padded {
  padding: var(--spacing-6);
}

.page-container--centered {
  margin: 0 auto;
}

.page-container--bordered {
  border: var(--border-width-1) solid var(--color-border-primary);
  border-radius: var(--border-radius-lg);
}

/* 背景变种 */
.page-container--bg-default {
  background-color: var(--color-bg-primary);
}

.page-container--bg-secondary {
  background-color: var(--color-bg-secondary);
}

.page-container--bg-transparent {
  background-color: transparent;
}

/* 页面头部 */
.page-header {
  margin-bottom: var(--spacing-6);
}

.page-container--padded .page-header {
  margin-bottom: var(--spacing-8);
}

/* 面包屑导航 */
.page-breadcrumb {
  margin-bottom: var(--spacing-4);
}

.breadcrumb-list {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--spacing-2);
}

.breadcrumb-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.breadcrumb-link {
  color: var(--color-text-secondary);
  text-decoration: none;
  font-size: var(--font-size-sm);
  transition: color var(--transition-fast);
}

.breadcrumb-link:hover {
  color: var(--color-brand-primary);
}

.breadcrumb-text {
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
}

.breadcrumb-item--active .breadcrumb-text {
  color: var(--color-text-primary);
  font-weight: var(--font-weight-medium);
}

.breadcrumb-separator {
  color: var(--color-text-tertiary);
  font-size: var(--font-size-xs);
}

/* 页面标题区域 */
.page-title-section {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--spacing-6);
}

.page-title-content {
  flex: 1;
  min-width: 0;
}

.page-title {
  margin: 0 0 var(--spacing-2);
  font-size: var(--font-size-3xl);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  line-height: var(--line-height-tight);
}

.page-description {
  margin: 0;
  font-size: var(--font-size-base);
  color: var(--color-text-secondary);
  line-height: var(--line-height-normal);
}

.page-extra {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

/* 页面内容 */
.page-content {
  flex: 1;
  min-height: 0;
}

/* 页面底部 */
.page-footer {
  margin-top: var(--spacing-8);
  padding-top: var(--spacing-6);
  border-top: var(--border-width-1) solid var(--color-border-primary);
}

/* 响应式设计 */
@media (max-width: 768px) {
  .page-container--padded {
    padding: var(--spacing-4);
  }
  
  .page-header {
    margin-bottom: var(--spacing-4);
  }
  
  .page-container--padded .page-header {
    margin-bottom: var(--spacing-6);
  }
  
  .page-title-section {
    flex-direction: column;
    gap: var(--spacing-4);
  }
  
  .page-title {
    font-size: var(--font-size-2xl);
  }
  
  .page-extra {
    align-self: stretch;
  }
  
  .breadcrumb-list {
    gap: var(--spacing-1);
  }
  
  .breadcrumb-item {
    gap: var(--spacing-1);
  }
  
  .breadcrumb-separator {
    margin: 0 var(--spacing-1);
  }
  
  .page-footer {
    margin-top: var(--spacing-6);
  }
}

@media (max-width: 480px) {
  .page-container--padded {
    padding: var(--spacing-3);
  }
  
  .page-title {
    font-size: var(--font-size-xl);
  }
  
  .page-description {
    font-size: var(--font-size-sm);
  }
  
  .breadcrumb-link,
  .breadcrumb-text {
    font-size: var(--font-size-xs);
  }
}

/* 深色主题适配 */
.theme-dark .page-container--bg-secondary {
  background-color: var(--color-bg-tertiary);
}

/* 打印样式 */
@media print {
  .page-container {
    background-color: transparent !important;
    box-shadow: none !important;
    border: none !important;
  }
  
  .page-extra {
    display: none !important;
  }
  
  .breadcrumb-link {
    color: inherit !important;
    text-decoration: none !important;
  }
}

/* 自定义最小高度 */
.page-container[style*="min-height"] {
  display: flex;
  flex-direction: column;
}

.page-container[style*="min-height"] .page-content {
  flex: 1;
}

/* 滚动优化 */
.page-container {
  scroll-behavior: smooth;
}

/* 无障碍优化 */
.page-title {
  scroll-margin-top: var(--spacing-6);
}

@media (prefers-reduced-motion: reduce) {
  .page-container {
    scroll-behavior: auto;
  }
  
  .page-container,
  .breadcrumb-link {
    transition: none;
  }
}
</style>