<template>
  <div class="api-key-cell">
    <span class="masked-key" :title="showFullKey ? apiKey : maskedKey">
      {{ displayKey }}
    </span>
    <el-tooltip content="点击复制完整密钥" placement="top">
      <el-button
        type="text"
        size="small"
        :loading="copying"
        @click="handleCopy"
        class="copy-button"
      >
        <el-icon>
          <component :is="iconComponent" />
        </el-icon>
      </el-button>
    </el-tooltip>
    <el-tooltip content="切换显示/隐藏" placement="top">
      <el-button
        type="text"
        size="small"
        @click="toggleDisplay"
        class="toggle-button"
      >
        <el-icon>
          <component :is="showFullKey ? Hide : View" />
        </el-icon>
      </el-button>
    </el-tooltip>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { CopyDocument, Check, View, Hide } from '@element-plus/icons-vue'
import { copyApiKey } from '@/utils/clipboard'

interface Props {
  /** API密钥 */
  apiKey: string
  /** 是否默认显示完整密钥 */
  defaultShowFull?: boolean
  /** 遮罩字符 */
  maskChar?: string
  /** 显示前几位字符 */
  prefixLength?: number
  /** 显示后几位字符 */
  suffixLength?: number
}

const props = withDefaults(defineProps<Props>(), {
  defaultShowFull: false,
  maskChar: '*',
  prefixLength: 4,
  suffixLength: 4
})

const emit = defineEmits<{
  copySuccess: [key: string]
  copyError: [error: Error]
}>()

const copying = ref(false)
const justCopied = ref(false)
const showFullKey = ref(props.defaultShowFull)

// 计算遮罩后的密钥
const maskedKey = computed(() => {
  if (!props.apiKey || props.apiKey.length < 8) {
    return props.apiKey
  }
  
  const { prefixLength, suffixLength, maskChar } = props
  const prefix = props.apiKey.substring(0, prefixLength)
  const suffix = props.apiKey.substring(props.apiKey.length - suffixLength)
  const maskLength = Math.max(0, props.apiKey.length - prefixLength - suffixLength)
  
  return prefix + maskChar.repeat(maskLength) + suffix
})

// 计算显示的密钥
const displayKey = computed(() => {
  return showFullKey.value ? props.apiKey : maskedKey.value
})

// 计算图标组件
const iconComponent = computed(() => {
  return justCopied.value ? Check : CopyDocument
})

// 切换显示状态
const toggleDisplay = () => {
  showFullKey.value = !showFullKey.value
}

// 处理复制
const handleCopy = async () => {
  if (!props.apiKey || copying.value) return

  copying.value = true
  
  try {
    const success = await copyApiKey(props.apiKey)
    
    if (success) {
      emit('copySuccess', props.apiKey)
      
      // 显示成功图标
      justCopied.value = true
      setTimeout(() => {
        justCopied.value = false
      }, 1500)
    } else {
      throw new Error('复制失败')
    }
  } catch (error) {
    const err = error instanceof Error ? error : new Error('未知错误')
    emit('copyError', err)
  } finally {
    copying.value = false
  }
}

// 暴露方法给父组件
defineExpose({
  copy: handleCopy,
  toggleDisplay,
  showFullKey: showFullKey.value,
  maskedKey: maskedKey.value
})
</script>

<style scoped>
.api-key-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 250px;
}

.masked-key {
  flex: 1;
  font-family: 'Courier New', 'Consolas', 'Monaco', monospace;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: help;
  padding: 2px 4px;
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.02);
  transition: all 0.2s ease;
}

.masked-key:hover {
  background: rgba(0, 0, 0, 0.05);
}

.copy-button,
.toggle-button {
  flex-shrink: 0;
  transition: all 0.2s ease;
}

.copy-button:hover {
  color: var(--el-color-primary);
}

.toggle-button:hover {
  color: var(--el-color-info);
}

/* 成功状态的特殊样式 */
.copy-button .el-icon {
  transition: color 0.3s ease;
}

.copy-button:has(.el-icon-check) {
  color: var(--el-color-success) !important;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .api-key-cell {
    max-width: 200px;
    gap: 4px;
  }
  
  .masked-key {
    font-size: 11px;
  }
}
</style>