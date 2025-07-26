<template>
  <el-dropdown 
    trigger="click" 
    :class="dropdownClasses"
    @command="handleCommand"
  >
    <div class="language-selector">
      <span class="language-flag">{{ currentFlag }}</span>
      <span v-if="showText" class="language-text">{{ currentName }}</span>
      <el-icon class="language-arrow">
        <ArrowDown />
      </el-icon>
    </div>
    
    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item 
          v-for="locale in supportedLocales" 
          :key="locale.code"
          :command="locale.code"
          :class="{ 'is-active': locale.code === currentLocale }"
        >
          <div class="language-option">
            <span class="language-option-flag">{{ locale.flag }}</span>
            <span class="language-option-text">{{ locale.name }}</span>
            <el-icon v-if="locale.code === currentLocale" class="language-option-check">
              <Check />
            </el-icon>
          </div>
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ArrowDown, Check } from '@element-plus/icons-vue'
import { 
  useI18n, 
  supportedLocales, 
  getLocaleName, 
  getLocaleFlag,
  switchLocale,
  type SupportedLocale 
} from '@/locales'

// 组件属性
interface Props {
  /** 显示文本 */
  showText?: boolean
  /** 大小 */
  size?: 'small' | 'default' | 'large'
  /** 变体 */
  variant?: 'default' | 'button' | 'minimal'
}

const props = withDefaults(defineProps<Props>(), {
  showText: true,
  size: 'default',
  variant: 'default'
})

// 国际化
const { locale } = useI18n()

// 计算属性
const currentLocale = computed(() => locale.value as SupportedLocale)

const currentName = computed(() => getLocaleName(currentLocale.value))

const currentFlag = computed(() => getLocaleFlag(currentLocale.value))

const dropdownClasses = computed(() => [
  'language-selector-dropdown',
  `language-selector-dropdown--${props.size}`,
  `language-selector-dropdown--${props.variant}`
])

// 方法
const handleCommand = (command: SupportedLocale) => {
  if (command !== currentLocale.value) {
    switchLocale(command)
  }
}
</script>

<style scoped>
.language-selector {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--border-radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
  color: var(--color-text-primary);
  background-color: transparent;
}

.language-selector:hover {
  background-color: var(--color-interactive-hover);
}

.language-flag {
  font-size: 16px;
  line-height: 1;
}

.language-text {
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  white-space: nowrap;
}

.language-arrow {
  font-size: 12px;
  color: var(--color-text-tertiary);
  transition: transform var(--transition-fast);
}

.language-selector-dropdown.is-opened .language-arrow {
  transform: rotate(180deg);
}

/* 大小变体 */
.language-selector-dropdown--small .language-selector {
  padding: var(--spacing-1) var(--spacing-2);
}

.language-selector-dropdown--small .language-flag {
  font-size: 14px;
}

.language-selector-dropdown--small .language-text {
  font-size: var(--font-size-xs);
}

.language-selector-dropdown--large .language-selector {
  padding: var(--spacing-3) var(--spacing-4);
}

.language-selector-dropdown--large .language-flag {
  font-size: 18px;
}

.language-selector-dropdown--large .language-text {
  font-size: var(--font-size-base);
}

/* 样式变体 */
.language-selector-dropdown--button .language-selector {
  border: 1px solid var(--color-border-primary);
  background-color: var(--color-bg-primary);
}

.language-selector-dropdown--button .language-selector:hover {
  border-color: var(--color-brand-primary);
  background-color: var(--color-interactive-hover);
}

.language-selector-dropdown--minimal .language-selector {
  padding: var(--spacing-1);
}

.language-selector-dropdown--minimal .language-text {
  display: none;
}

/* 下拉选项 */
.language-option {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  width: 100%;
  min-width: 120px;
}

.language-option-flag {
  font-size: 16px;
  line-height: 1;
}

.language-option-text {
  flex: 1;
  font-size: var(--font-size-sm);
}

.language-option-check {
  font-size: 14px;
  color: var(--color-brand-primary);
}

/* 激活状态 */
:deep(.el-dropdown-menu__item.is-active) {
  background-color: var(--color-brand-primary);
  color: var(--color-white);
}

:deep(.el-dropdown-menu__item.is-active) .language-option-check {
  color: var(--color-white);
}

/* 响应式设计 */
@media (max-width: 768px) {
  .language-selector-dropdown--default .language-text {
    display: none;
  }
  
  .language-option {
    min-width: 100px;
  }
}

/* 深色主题适配 */
.theme-dark .language-selector-dropdown--button .language-selector {
  border-color: var(--color-border-secondary);
  background-color: var(--color-bg-secondary);
}

.theme-dark .language-selector-dropdown--button .language-selector:hover {
  border-color: var(--color-brand-primary);
  background-color: var(--color-interactive-hover);
}
</style>