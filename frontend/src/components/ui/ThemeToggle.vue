<template>
  <div class="theme-toggle">
    <!-- 简单切换按钮 -->
    <el-button
      v-if="mode === 'simple'"
      :icon="isDark ? Sunny : Moon"
      circle
      :title="isDark ? '切换到浅色主题' : '切换到深色主题'"
      @click="toggleTheme"
      class="theme-toggle-button"
    />
    
    <!-- 下拉选择器 -->
    <el-dropdown v-else @command="handleCommand" class="theme-dropdown">
      <el-button :icon="currentIcon" circle :title="'当前主题: ' + currentModeText">
        <el-icon class="ml-1">
          <ArrowDown />
        </el-icon>
      </el-button>
      
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item command="light" :class="{ active: currentMode === 'light' }">
            <el-icon><Sunny /></el-icon>
            <span class="ml-2">浅色主题</span>
            <el-icon v-if="currentMode === 'light'" class="ml-auto"><Check /></el-icon>
          </el-dropdown-item>
          
          <el-dropdown-item command="dark" :class="{ active: currentMode === 'dark' }">
            <el-icon><Moon /></el-icon>
            <span class="ml-2">深色主题</span>
            <el-icon v-if="currentMode === 'dark'" class="ml-auto"><Check /></el-icon>
          </el-dropdown-item>
          
          <el-dropdown-item command="auto" :class="{ active: currentMode === 'auto' }">
            <el-icon><Monitor /></el-icon>
            <span class="ml-2">跟随系统</span>
            <el-icon v-if="currentMode === 'auto'" class="ml-auto"><Check /></el-icon>
          </el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ElButton, ElDropdown, ElDropdownMenu, ElDropdownItem, ElIcon } from 'element-plus'
import { Sunny, Moon, Monitor, ArrowDown, Check } from '@element-plus/icons-vue'
import { useTheme, type ThemeMode } from '../../styles/theme'

// 组件属性
interface Props {
  /** 显示模式：simple - 简单切换按钮，dropdown - 下拉选择器 */
  mode?: 'simple' | 'dropdown'
  /** 是否显示文本 */
  showText?: boolean
  /** 按钮大小 */
  size?: 'small' | 'default' | 'large'
}

const props = withDefaults(defineProps<Props>(), {
  mode: 'simple',
  showText: false,
  size: 'default'
})

// 使用主题系统
const { theme, mode: currentMode, isDark, setTheme, toggleTheme } = useTheme()

// 当前图标
const currentIcon = computed(() => {
  if (currentMode.value === 'auto') return Monitor
  return isDark.value ? Sunny : Moon
})

// 当前模式文本
const currentModeText = computed(() => {
  switch (currentMode.value) {
    case 'light': return '浅色主题'
    case 'dark': return '深色主题'
    case 'auto': return '跟随系统'
    default: return '未知'
  }
})

// 处理下拉选择
const handleCommand = (command: ThemeMode) => {
  setTheme(command)
}
</script>

<style scoped>
.theme-toggle {
  display: inline-flex;
  align-items: center;
}

.theme-toggle-button {
  transition: all var(--transition-normal);
}

.theme-toggle-button:hover {
  transform: scale(1.05);
}

.theme-dropdown {
  .el-button {
    transition: all var(--transition-normal);
  }
  
  .el-button:hover {
    transform: scale(1.05);
  }
}

:deep(.el-dropdown-menu__item) {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  
  &.active {
    color: var(--color-brand-primary);
    background-color: var(--color-interactive-focus);
  }
  
  .ml-2 {
    margin-left: var(--spacing-2);
  }
  
  .ml-auto {
    margin-left: auto;
  }
}

/* 主题切换动画 */
.theme-toggle-enter-active,
.theme-toggle-leave-active {
  transition: all var(--transition-normal);
}

.theme-toggle-enter-from,
.theme-toggle-leave-to {
  opacity: 0;
  transform: rotate(180deg) scale(0.8);
}

/* 响应式适配 */
@media (max-width: 768px) {
  .theme-toggle {
    .theme-dropdown .el-button {
      padding: var(--spacing-2);
    }
  }
}
</style>