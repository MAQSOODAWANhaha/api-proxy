<template>
  <span class="status-indicator" :class="statusClass">
    <span class="status-dot"></span>
    <span class="status-text">{{ text }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  status: 'active' | 'inactive' | 'warning' | 'error' | 'pending'
  text?: string
  size?: 'small' | 'medium' | 'large'
}

const props = withDefaults(defineProps<Props>(), {
  size: 'medium'
})

const statusClass = computed(() => {
  return [
    `status-${props.status}`,
    `status-${props.size}`
  ]
})

const text = computed(() => {
  if (props.text) return props.text
  
  const statusTexts = {
    active: '正常',
    inactive: '停用',
    warning: '警告',
    error: '错误',
    pending: '待处理'
  }
  
  return statusTexts[props.status] || '未知'
})
</script>

<style scoped>
.status-indicator {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-weight: 500;
}

.status-small {
  font-size: 12px;
}

.status-small .status-dot {
  width: 6px;
  height: 6px;
}

.status-medium {  
  font-size: 14px;
}

.status-medium .status-dot {
  width: 8px;
  height: 8px;
}

.status-large {
  font-size: 16px;
}

.status-large .status-dot {
  width: 10px;
  height: 10px;
}

.status-dot {
  display: inline-block;
  border-radius: 50%;
  transition: all 0.3s ease;
}

.status-active .status-dot {
  background-color: var(--el-color-success);
  box-shadow: 0 0 0 2px var(--el-color-success-light-8);
}

.status-active .status-text {
  color: var(--el-color-success);
}

.status-inactive .status-dot {
  background-color: var(--el-color-info);
  box-shadow: 0 0 0 2px var(--el-color-info-light-8);
}

.status-inactive .status-text {
  color: var(--el-color-info);
}

.status-warning .status-dot {
  background-color: var(--el-color-warning);
  box-shadow: 0 0 0 2px var(--el-color-warning-light-8);
}

.status-warning .status-text {
  color: var(--el-color-warning);
}

.status-error .status-dot {
  background-color: var(--el-color-danger);
  box-shadow: 0 0 0 2px var(--el-color-danger-light-8);
}

.status-error .status-text {
  color: var(--el-color-danger);
}

.status-pending .status-dot {
  background-color: var(--el-color-primary);
  box-shadow: 0 0 0 2px var(--el-color-primary-light-8);
  animation: pulse 2s infinite;
}

.status-pending .status-text {
  color: var(--el-color-primary);
}

@keyframes pulse {
  0% {
    box-shadow: 0 0 0 2px var(--el-color-primary-light-8);
  }
  50% {
    box-shadow: 0 0 0 6px var(--el-color-primary-light-9);
  }
  100% {
    box-shadow: 0 0 0 2px var(--el-color-primary-light-8);
  }
}
</style>