<template>
  <el-button
    :type="type"
    :size="size"
    :loading="copying"
    :disabled="disabled || !text"
    @click="handleCopy"
    v-bind="$attrs"
  >
    <el-icon v-if="!copying">
      <component :is="iconComponent" />
    </el-icon>
    <span v-if="showText">{{ buttonText }}</span>
  </el-button>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { CopyDocument, Check } from '@element-plus/icons-vue'
import { copyToClipboard, isClipboardSupported } from '@/utils/clipboard'

interface Props {
  /** 要复制的文本内容 */
  text: string
  /** 按钮类型 */
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'text' | ''
  /** 按钮尺寸 */
  size?: 'large' | 'default' | 'small'
  /** 是否显示按钮文字 */
  showText?: boolean
  /** 自定义按钮文字 */
  buttonText?: string
  /** 自定义成功消息 */
  successMessage?: string
  /** 是否禁用 */
  disabled?: boolean
  /** 复制成功后的图标显示时间（毫秒） */
  successDuration?: number
}

const props = withDefaults(defineProps<Props>(), {
  type: 'text',
  size: 'small',
  showText: false,
  buttonText: '复制',
  successMessage: '已复制到剪贴板',
  disabled: false,
  successDuration: 1500
})

const emit = defineEmits<{
  success: [text: string]
  error: [error: Error]
}>()

const copying = ref(false)
const justCopied = ref(false)

// 计算按钮图标
const iconComponent = computed(() => {
  return justCopied.value ? Check : CopyDocument
})

// 检查复制功能是否可用
const clipboardSupport = isClipboardSupported()

const handleCopy = async () => {
  if (!props.text || copying.value) return

  copying.value = true
  
  try {
    const success = await copyToClipboard(props.text, props.successMessage)
    
    if (success) {
      emit('success', props.text)
      
      // 显示成功图标
      justCopied.value = true
      setTimeout(() => {
        justCopied.value = false
      }, props.successDuration)
    } else {
      throw new Error('复制失败')
    }
  } catch (error) {
    const err = error instanceof Error ? error : new Error('未知错误')
    emit('error', err)
  } finally {
    copying.value = false
  }
}

// 暴露方法给父组件
defineExpose({
  copy: handleCopy,
  isSupported: clipboardSupport,
  text: props.text
})
</script>

<style scoped>
.el-button {
  transition: all 0.3s ease;
}

.el-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 成功状态的特殊样式 */
.el-button:has(.el-icon-check) {
  color: var(--el-color-success);
}
</style>