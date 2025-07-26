<template>
  <Card class="card-skeleton">
    <!-- 卡片头部 -->
    <div v-if="showHeader" class="card-skeleton-header">
      <div class="card-skeleton-header-content">
        <Skeleton v-if="showAvatar" shape="circle" :size="avatarSize" />
        <div class="card-skeleton-header-text">
          <Skeleton width="40%" height="20px" />
          <Skeleton v-if="showSubtitle" width="60%" height="16px" />
        </div>
      </div>
      <Skeleton v-if="showAction" width="24px" height="24px" shape="circle" />
    </div>
    
    <!-- 卡片内容 -->
    <div class="card-skeleton-content">
      <!-- 图片区域 -->
      <Skeleton 
        v-if="showImage" 
        class="card-skeleton-image"
        :height="imageHeight"
        shape="square"
      />
      
      <!-- 文本内容 -->
      <div v-if="showText" class="card-skeleton-text">
        <Skeleton v-if="showTitle" width="70%" height="24px" />
        <div class="card-skeleton-paragraphs">
          <Skeleton 
            v-for="line in textLines" 
            :key="line"
            :width="getLineWidth(line)"
            height="16px"
            :animation="animation"
          />
        </div>
      </div>
      
      <!-- 标签区域 -->
      <div v-if="showTags" class="card-skeleton-tags">
        <Skeleton 
          v-for="tag in tagCount" 
          :key="tag"
          :width="getTagWidth(tag)"
          height="24px"
          shape="default"
        />
      </div>
    </div>
    
    <!-- 卡片底部 -->
    <div v-if="showFooter" class="card-skeleton-footer">
      <div class="card-skeleton-footer-left">
        <Skeleton 
          v-for="item in footerItems" 
          :key="item"
          :width="getFooterItemWidth(item)"
          height="16px"
        />
      </div>
      <Skeleton v-if="showFooterAction" width="60px" height="32px" />
    </div>
  </Card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import Card from '@/components/ui/Card.vue'
import Skeleton from '@/components/ui/Skeleton.vue'

// 组件属性
interface Props {
  /** 显示头部 */
  showHeader?: boolean
  /** 显示头像 */
  showAvatar?: boolean
  /** 头像大小 */
  avatarSize?: 'small' | 'default' | 'large'
  /** 显示副标题 */
  showSubtitle?: boolean
  /** 显示头部操作 */
  showAction?: boolean
  /** 显示图片 */
  showImage?: boolean
  /** 图片高度 */
  imageHeight?: string
  /** 显示文本 */
  showText?: boolean
  /** 显示标题 */
  showTitle?: boolean
  /** 文本行数 */
  textLines?: number
  /** 显示标签 */
  showTags?: boolean
  /** 标签数量 */
  tagCount?: number
  /** 显示底部 */
  showFooter?: boolean
  /** 底部项目数量 */
  footerItems?: number
  /** 显示底部操作 */
  showFooterAction?: boolean
  /** 动画类型 */
  animation?: 'pulse' | 'wave' | 'none'
}

const props = withDefaults(defineProps<Props>(), {
  showHeader: true,
  showAvatar: false,
  avatarSize: 'default',
  showSubtitle: false,
  showAction: false,
  showImage: false,
  imageHeight: '200px',
  showText: true,
  showTitle: true,
  textLines: 3,
  showTags: false,
  tagCount: 3,
  showFooter: false,
  footerItems: 2,
  showFooterAction: false,
  animation: 'pulse'
})

// 计算属性
const textLinesArray = computed(() => {
  return Array.from({ length: props.textLines }, (_, i) => i)
})

const tagCountArray = computed(() => {
  return Array.from({ length: props.tagCount }, (_, i) => i)
})

const footerItemsArray = computed(() => {
  return Array.from({ length: props.footerItems }, (_, i) => i)
})

// 方法
const getLineWidth = (lineIndex: number): string => {
  // 最后一行通常比较短
  if (lineIndex === props.textLines - 1) {
    return '65%'
  }
  
  const widths = ['100%', '90%', '85%', '95%']
  return widths[lineIndex % widths.length]
}

const getTagWidth = (tagIndex: number): string => {
  const widths = ['60px', '80px', '70px', '90px', '50px']
  return widths[tagIndex % widths.length]
}

const getFooterItemWidth = (itemIndex: number): string => {
  const widths = ['80px', '100px', '60px', '120px']
  return widths[itemIndex % widths.length]
}
</script>

<style scoped>
.card-skeleton {
  padding: 0;
  overflow: hidden;
}

.card-skeleton-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-6);
  border-bottom: 1px solid var(--color-border-primary);
}

.card-skeleton-header-content {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.card-skeleton-header-text {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.card-skeleton-content {
  padding: var(--spacing-6);
}

.card-skeleton-image {
  width: 100%;
  margin-bottom: var(--spacing-4);
}

.card-skeleton-text {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.card-skeleton-paragraphs {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.card-skeleton-tags {
  display: flex;
  gap: var(--spacing-2);
  flex-wrap: wrap;
  margin-top: var(--spacing-4);
}

.card-skeleton-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-4) var(--spacing-6);
  border-top: 1px solid var(--color-border-primary);
  background-color: var(--color-bg-secondary);
}

.card-skeleton-footer-left {
  display: flex;
  gap: var(--spacing-4);
  align-items: center;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .card-skeleton-header,
  .card-skeleton-content {
    padding: var(--spacing-4);
  }
  
  .card-skeleton-footer {
    padding: var(--spacing-3) var(--spacing-4);
  }
  
  .card-skeleton-tags {
    gap: var(--spacing-1);
  }
}
</style>