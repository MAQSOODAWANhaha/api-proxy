<template>
  <div class="empty-state" :class="{ 'empty-state--small': size === 'small' }">
    <div class="empty-state__icon">
      <el-icon :size="iconSize">
        <component :is="icon" v-if="icon" />
        <Box v-else />
      </el-icon>
    </div>
    
    <div class="empty-state__content">
      <div class="empty-state__title">{{ title }}</div>
      <div class="empty-state__description" v-if="description">
        {{ description }}
      </div>
    </div>
    
    <div class="empty-state__actions" v-if="$slots.actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Box } from '@element-plus/icons-vue'

interface Props {
  icon?: any
  title?: string
  description?: string
  size?: 'small' | 'large'
}

const props = withDefaults(defineProps<Props>(), {
  title: '暂无数据',
  size: 'large'
})

const iconSize = computed(() => {
  return props.size === 'small' ? 48 : 64
})
</script>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  text-align: center;
}

.empty-state--small {
  padding: 40px 20px;
}

.empty-state__icon {
  margin-bottom: 16px;
  color: var(--el-color-info-light-3);
  opacity: 0.8;
}

.empty-state__content {
  margin-bottom: 24px;
}

.empty-state__title {
  font-size: 16px;
  font-weight: 500;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
  line-height: 1.4;
}

.empty-state__description {
  font-size: 14px;
  color: var(--el-text-color-placeholder);
  line-height: 1.6;
  max-width: 300px;
}

.empty-state__actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: center;
}

.empty-state--small .empty-state__icon {
  margin-bottom: 12px;
}

.empty-state--small .empty-state__title {
  font-size: 14px;
}

.empty-state--small .empty-state__description {
  font-size: 12px;
}

@media (max-width: 768px) {
  .empty-state {
    padding: 40px 16px;
  }
  
  .empty-state--small {
    padding: 24px 16px;
  }
  
  .empty-state__actions {
    flex-direction: column;
    align-items: center;
  }
  
  .empty-state__actions .el-button {
    width: 100%;
    max-width: 200px;
  }
}
</style>