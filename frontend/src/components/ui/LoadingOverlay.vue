<template>
  <Transition name="fade">
    <div v-if="visible" class="loading-overlay" :class="{ 'loading-overlay--absolute': absolute }">
      <div class="loading-content">
        <el-icon class="loading-spinner" :size="iconSize">
          <Loading />
        </el-icon>
        <div class="loading-text" v-if="text">{{ text }}</div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { Loading } from '@element-plus/icons-vue'

interface Props {
  visible: boolean
  text?: string
  absolute?: boolean
  iconSize?: number
}

withDefaults(defineProps<Props>(), {
  absolute: true,
  iconSize: 24
})
</script>

<style scoped>
.loading-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.loading-overlay--absolute {
  position: absolute;
  z-index: 1000;
  border-radius: 8px;
}

.loading-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px;
  background: var(--el-bg-color);
  border-radius: 8px;
  box-shadow: var(--el-box-shadow);
  border: 1px solid var(--el-border-color-lighter);
}

.loading-spinner {
  color: var(--el-color-primary);
  animation: rotate 1s linear infinite;
}

.loading-text {
  font-size: 14px;
  color: var(--el-text-color-secondary);
  text-align: center;
  white-space: nowrap;
}

@keyframes rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* 深色模式适配 */
@media (prefers-color-scheme: dark) {
  .loading-overlay {
    background: rgba(0, 0, 0, 0.6);
  }
}
</style>