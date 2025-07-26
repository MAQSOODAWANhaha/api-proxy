# 设计系统文档

## 概述

本项目采用统一的设计系统，基于设计令牌（Design Tokens）构建，提供一致的视觉体验和开发效率。

## 核心特性

- 🎨 **统一的设计令牌** - 颜色、字体、间距等全局一致
- 🌓 **主题切换** - 支持浅色/深色/自动主题
- 📱 **响应式设计** - 适配各种屏幕尺寸
- 🔧 **Element Plus 集成** - 无缝整合组件库
- 💪 **TypeScript 支持** - 完整的类型定义
- 🚀 **组合式 API** - Vue 3 现代开发体验

## 文档结构

```
src/styles/
├── design-tokens.ts    # 设计令牌定义
├── theme.ts           # 主题系统核心
├── globals.css        # 全局样式
└── README.md          # 本文档

src/composables/
└── useDesignSystem.ts # 设计系统组合式函数

src/components/ui/
└── ThemeToggle.vue    # 主题切换组件
```

## 快速开始

### 1. 使用设计令牌

```typescript
import { useDesignSystem } from '@/composables/useDesignSystem'

const { colors, spacing, typography, utils } = useDesignSystem()

// 直接使用令牌
const primaryColor = colors.primary[500]
const mediumSpacing = spacing[4]

// 使用工具函数
const color = utils.color('primary.500', 0.8) // 带透明度
const space = utils.spacing(4)
```

### 2. 使用主题系统

```vue
<template>
  <div :class="['container', { 'dark-theme': isDark }]">
    <button @click="toggleTheme">
      切换主题
    </button>
  </div>
</template>

<script setup>
import { useTheme } from '@/styles/theme'

const { theme, isDark, toggleTheme } = useTheme()
</script>

<style scoped>
.container {
  background-color: var(--color-bg-primary);
  color: var(--color-text-primary);
}
</style>
```

### 3. 使用颜色工具

```typescript
import { useColors } from '@/composables/useDesignSystem'

const colors = useColors()

// 语义化颜色
const primaryColor = colors.semantic.primary.value
const successColor = colors.semantic.success.value

// 主题色
const bgColor = colors.background.primary.value
const textColor = colors.text.primary.value
```

### 4. 使用响应式工具

```typescript
import { useResponsive } from '@/composables/useDesignSystem'

const responsive = useResponsive()

// 检查屏幕大小
const isMobile = responsive.isMobile.value
const isDesktop = responsive.isDesktop.value

// 响应式值
const columns = responsive.value({
  xs: 1,
  sm: 2,
  md: 3,
  lg: 4
})
```

## 设计令牌

### 颜色系统

```typescript
// 主色调
colors.primary[500]  // #0ea5e9
colors.primary[600]  // #0284c7

// 状态色
colors.success[500]  // #22c55e
colors.warning[500]  // #f59e0b
colors.error[500]    // #ef4444
colors.info[500]     // #3b82f6

// 中性色
colors.neutral[50]   // #fafafa
colors.neutral[900]  // #171717
```

### 字体系统

```typescript
// 字体大小
typography.fontSize.xs    // 0.75rem (12px)
typography.fontSize.sm    // 0.875rem (14px)
typography.fontSize.base  // 1rem (16px)
typography.fontSize.lg    // 1.125rem (18px)

// 字体权重
typography.fontWeight.normal    // 400
typography.fontWeight.medium    // 500
typography.fontWeight.semibold  // 600
typography.fontWeight.bold      // 700
```

### 间距系统

```typescript
// 间距令牌
spacing[1]   // 0.25rem (4px)
spacing[2]   // 0.5rem (8px)
spacing[4]   // 1rem (16px)
spacing[6]   // 1.5rem (24px)
spacing[8]   // 2rem (32px)
```

### 圆角系统

```typescript
borderRadius.sm     // 0.125rem (2px)
borderRadius.base   // 0.25rem (4px)
borderRadius.md     // 0.375rem (6px)
borderRadius.lg     // 0.5rem (8px)
borderRadius.full   // 9999px
```

## 主题系统

### 主题模式

- `light` - 浅色主题
- `dark` - 深色主题  
- `auto` - 跟随系统设置

### CSS 变量

设计系统会自动生成 CSS 变量，可直接在样式中使用：

```css
.example {
  /* 背景色 */
  background-color: var(--color-bg-primary);
  
  /* 文本色 */
  color: var(--color-text-primary);
  
  /* 品牌色 */
  border-color: var(--color-brand-primary);
  
  /* 间距 */
  padding: var(--spacing-4);
  
  /* 圆角 */
  border-radius: var(--border-radius-md);
  
  /* 阴影 */
  box-shadow: var(--box-shadow-sm);
}
```

### 主题切换组件

```vue
<template>
  <!-- 简单切换按钮 -->
  <ThemeToggle mode="simple" />
  
  <!-- 下拉选择器 -->
  <ThemeToggle mode="dropdown" />
</template>

<script setup>
import ThemeToggle from '@/components/ui/ThemeToggle.vue'
</script>
```

## 响应式断点

```typescript
// 断点定义
breakpoints.xs   // 0px
breakpoints.sm   // 576px
breakpoints.md   // 768px
breakpoints.lg   // 992px
breakpoints.xl   // 1200px
breakpoints['2xl'] // 1400px
```

### 媒体查询

```typescript
const { utils } = useDesignSystem()

// 生成媒体查询
const mobileQuery = utils.mediaQuery('md', 'max') // @media (max-width: 768px)
const desktopQuery = utils.mediaQuery('lg', 'min') // @media (min-width: 992px)
```

## 动画系统

```typescript
// 动画时长
animation.duration.fast     // 150ms
animation.duration.normal   // 300ms
animation.duration.slow     // 500ms

// 缓动函数
animation.easing.ease       // ease
animation.easing.easeInOut  // ease-in-out
animation.easing.bounceOut  // cubic-bezier(0.175, 0.885, 0.32, 1.275)
```

### 过渡动画工具

```typescript
const { utils } = useDesignSystem()

// 生成过渡
const transition = utils.transition(['color', 'background-color'], 'fast', 'ease')
// 结果: "color 150ms ease, background-color 150ms ease"
```

## 最佳实践

### 1. 统一使用设计令牌

❌ **不推荐**
```css
.button {
  background-color: #0ea5e9;
  padding: 12px 20px;
  border-radius: 6px;
}
```

✅ **推荐**
```css
.button {
  background-color: var(--color-brand-primary);
  padding: var(--spacing-3) var(--spacing-5);
  border-radius: var(--border-radius-md);
}
```

### 2. 使用语义化颜色

❌ **不推荐**
```typescript
const redColor = colors.error[500]
```

✅ **推荐**
```typescript
const errorColor = colors.semantic.error.value
// 或
const errorColor = utils.themeColor('status.error')
```

### 3. 响应式设计

```vue
<template>
  <div class="grid">
    <div v-for="item in items" :key="item.id" class="grid-item">
      {{ item }}
    </div>
  </div>
</template>

<style scoped>
.grid {
  display: grid;
  gap: var(--spacing-4);
  grid-template-columns: repeat(1, 1fr);
}

@media (min-width: 576px) {
  .grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 768px) {
  .grid {
    grid-template-columns: repeat(3, 1fr);
  }
}
```

### 4. 主题兼容

```vue
<style scoped>
.card {
  background-color: var(--color-bg-elevated);
  border: var(--border-width-1) solid var(--color-border-primary);
  transition: all var(--transition-normal);
}

.card:hover {
  border-color: var(--color-brand-primary);
  box-shadow: var(--box-shadow-md);
}

/* 特殊的深色主题样式 */
.theme-dark .card {
  background-color: var(--color-bg-tertiary);
}
```

## 扩展指南

### 添加新的设计令牌

1. 在 `design-tokens.ts` 中添加新令牌
2. 在 `theme.ts` 中添加主题映射
3. 在 `globals.css` 中添加 CSS 变量
4. 更新组合式函数

### 自定义主题颜色

```typescript
// 扩展主题定义
const customTheme: Theme = {
  ...lightTheme,
  colors: {
    ...lightTheme.colors,
    brand: {
      primary: '#your-color',
      secondary: '#your-secondary-color',
      accent: '#your-accent-color',
    }
  }
}
```

## 故障排除

### 常见问题

1. **CSS 变量未生效**
   - 确保已导入 `globals.css`
   - 检查主题系统是否正确初始化

2. **主题切换不生效**
   - 确保使用了 `useTheme` 组合式函数
   - 检查浏览器控制台是否有错误

3. **响应式断点异常**
   - 确认浏览器窗口大小
   - 检查 CSS 媒体查询语法

### 调试工具

```typescript
// 调试当前主题
import { currentTheme } from '@/styles/theme'
console.log('Current theme:', currentTheme.value)

// 调试设计令牌
import { useDesignSystem } from '@/composables/useDesignSystem'
const ds = useDesignSystem()
console.log('Design system:', ds)
```

## 更新日志

- **v1.0.0** - 初始版本，包含基础设计令牌和主题系统
- 支持浅色/深色主题切换
- 集成 Element Plus 组件库
- 提供完整的 TypeScript 类型支持