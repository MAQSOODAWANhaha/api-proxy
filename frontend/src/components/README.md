# 组件库文档

## 概述

本项目提供了一套功能完整、设计统一的Vue 3组件库，基于TypeScript开发，完全支持设计系统和主题切换。

## 特性

- 🎨 **设计系统集成** - 基于统一的设计令牌
- 🌓 **主题支持** - 完整的浅色/深色主题切换
- 📱 **响应式设计** - 适配各种屏幕尺寸
- 💪 **TypeScript** - 完整的类型定义
- 🚀 **Vue 3** - 基于Composition API
- 🔧 **Element Plus兼容** - 无缝整合现有组件
- ♿ **无障碍友好** - 符合WCAG标准

## 快速开始

### 安装使用

```typescript
// 导入单个组件
import { Card, Button, Badge } from '@/components/ui'

// 导入所有组件
import * as UI from '@/components/ui'
```

### 基础示例

```vue
<template>
  <PageContainer 
    title="仪表板" 
    description="系统概览和关键指标"
    :breadcrumb="breadcrumb"
  >
    <Grid :cols="{ xs: 1, md: 2, lg: 3 }" :gap="4">
      <GridItem>
        <Card title="用户统计" hoverable>
          <template #extra>
            <Badge :count="5" type="danger" />
          </template>
          <p>当前在线用户: 1,234</p>
        </Card>
      </GridItem>
      
      <GridItem>
        <Card title="系统状态">
          <div class="flex gap-2">
            <Tag type="success">运行中</Tag>
            <Tag type="info">版本 1.0.0</Tag>
          </div>
        </Card>
      </GridItem>
    </Grid>
  </PageContainer>
</template>
```

## 组件文档

### 基础组件

#### Card 卡片

用于内容分组的容器组件。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| title | string | - | 卡片标题 |
| subtitle | string | - | 卡片副标题 |
| variant | 'default' \| 'outlined' \| 'elevated' \| 'filled' | 'default' | 卡片变种 |
| size | 'sm' \| 'md' \| 'lg' | 'md' | 卡片大小 |
| hoverable | boolean | false | 是否可悬停 |
| clickable | boolean | false | 是否可点击 |
| shadow | boolean | true | 是否有阴影 |
| padding | 'none' \| 'sm' \| 'md' \| 'lg' | 'md' | 内边距 |
| loading | boolean | false | 加载状态 |

**插槽**

| 插槽 | 说明 |
|------|------|
| default | 卡片内容 |
| header | 自定义头部 |
| extra | 头部额外内容 |
| footer | 底部内容 |

**事件**

| 事件 | 参数 | 说明 |
|------|------|------|
| click | MouseEvent | 点击事件（仅在clickable时触发） |

**示例**

```vue
<template>
  <!-- 基础卡片 -->
  <Card title="基础卡片">
    <p>这是卡片内容</p>
  </Card>
  
  <!-- 可交互卡片 -->
  <Card 
    title="可点击卡片" 
    hoverable 
    clickable 
    @click="handleClick"
  >
    <p>点击我试试</p>
  </Card>
  
  <!-- 自定义头部 -->
  <Card>
    <template #header>
      <div class="flex justify-between items-center">
        <h3>自定义头部</h3>
        <Button size="sm">操作</Button>
      </div>
    </template>
    <p>内容区域</p>
  </Card>
</template>
```

#### Button 按钮

触发操作的基础组件。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| type | 'primary' \| 'success' \| 'warning' \| 'danger' \| 'info' \| 'default' | 'default' | 按钮类型 |
| variant | 'filled' \| 'outlined' \| 'text' \| 'ghost' | 'filled' | 按钮变种 |
| size | 'xs' \| 'sm' \| 'md' \| 'lg' \| 'xl' | 'md' | 按钮大小 |
| disabled | boolean | false | 是否禁用 |
| loading | boolean | false | 加载状态 |
| block | boolean | false | 块级按钮 |
| circle | boolean | false | 圆形按钮 |
| round | boolean | false | 圆角按钮 |

**示例**

```vue
<template>
  <!-- 按钮类型 -->
  <Button type="primary">主要按钮</Button>
  <Button type="success">成功按钮</Button>
  <Button type="warning">警告按钮</Button>
  <Button type="danger">危险按钮</Button>
  
  <!-- 按钮变种 -->
  <Button type="primary" variant="filled">填充</Button>
  <Button type="primary" variant="outlined">边框</Button>
  <Button type="primary" variant="text">文本</Button>
  <Button type="primary" variant="ghost">幽灵</Button>
  
  <!-- 按钮大小 -->
  <Button size="xs">超小</Button>
  <Button size="sm">小</Button>
  <Button size="md">中等</Button>
  <Button size="lg">大</Button>
  <Button size="xl">超大</Button>
  
  <!-- 按钮状态 -->
  <Button loading>加载中</Button>
  <Button disabled>禁用</Button>
  
  <!-- 特殊形状 -->
  <Button circle :icon="PlusIcon" />
  <Button round>圆角按钮</Button>
  <Button block>块级按钮</Button>
</template>
```

#### Badge 徽章

用于显示数量或状态的小标记。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| count | number | - | 显示数量 |
| max | number | 99 | 最大显示数量 |
| type | 'primary' \| 'success' \| 'warning' \| 'danger' \| 'info' \| 'default' | 'danger' | 徽章类型 |
| size | 'xs' \| 'sm' \| 'md' \| 'lg' | 'md' | 徽章大小 |
| dot | boolean | false | 显示为点 |
| hidden | boolean | false | 是否隐藏 |

**示例**

```vue
<template>
  <!-- 数字徽章 -->
  <Badge :count="5">
    <Button>消息</Button>
  </Badge>
  
  <!-- 点徽章 -->
  <Badge dot type="success">
    <Button>在线状态</Button>
  </Badge>
  
  <!-- 超出最大值 -->
  <Badge :count="100" :max="99">
    <Button>通知</Button>
  </Badge>
  
  <!-- 独立使用 -->
  <Badge :count="5" type="primary" />
  <Badge dot type="warning" />
</template>
```

#### Tag 标签

用于标记和分类的标签组件。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| type | 'primary' \| 'success' \| 'warning' \| 'danger' \| 'info' \| 'default' | 'default' | 标签类型 |
| variant | 'filled' \| 'outlined' \| 'light' \| 'ghost' | 'filled' | 标签变种 |
| size | 'xs' \| 'sm' \| 'md' \| 'lg' | 'md' | 标签大小 |
| closable | boolean | false | 是否可关闭 |
| clickable | boolean | false | 是否可点击 |
| round | boolean | false | 圆角标签 |

**事件**

| 事件 | 参数 | 说明 |
|------|------|------|
| close | - | 关闭事件 |
| click | MouseEvent | 点击事件 |

**示例**

```vue
<template>
  <!-- 基础标签 -->
  <Tag>默认标签</Tag>
  <Tag type="primary">主要标签</Tag>
  <Tag type="success">成功标签</Tag>
  
  <!-- 标签变种 -->
  <Tag variant="filled">填充</Tag>
  <Tag variant="outlined">边框</Tag>
  <Tag variant="light">浅色</Tag>
  
  <!-- 可关闭标签 -->
  <Tag closable @close="handleClose">可关闭</Tag>
  
  <!-- 可点击标签 -->
  <Tag clickable @click="handleClick">可点击</Tag>
  
  <!-- 自定义颜色 -->
  <Tag color="#87d068">自定义颜色</Tag>
</template>
```

#### Loading 加载

用于页面和区块的加载状态。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| visible | boolean | true | 是否显示 |
| text | string | - | 加载文本 |
| spinner | 'default' \| 'dots' \| 'pulse' \| 'bounce' \| 'wave' | 'default' | 加载器类型 |
| size | 'xs' \| 'sm' \| 'md' \| 'lg' \| 'xl' | 'md' | 大小 |
| overlay | boolean | false | 显示遮罩 |
| fullscreen | boolean | false | 全屏显示 |
| centered | boolean | true | 居中对齐 |

**示例**

```vue
<template>
  <!-- 基础加载 -->
  <Loading text="加载中..." />
  
  <!-- 不同类型 -->
  <Loading spinner="dots" />
  <Loading spinner="pulse" />
  <Loading spinner="bounce" />
  <Loading spinner="wave" />
  
  <!-- 遮罩加载 -->
  <div style="position: relative; height: 200px;">
    <Loading overlay :visible="loading" text="数据加载中..." />
    <p>这里是内容</p>
  </div>
  
  <!-- 全屏加载 -->
  <Loading fullscreen :visible="loading" text="页面加载中..." />
</template>
```

### 布局组件

#### PageContainer 页面容器

页面级别的容器组件，提供标题、面包屑等功能。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| title | string | - | 页面标题 |
| description | string | - | 页面描述 |
| breadcrumb | BreadcrumbItem[] | - | 面包屑数据 |
| size | 'sm' \| 'md' \| 'lg' \| 'xl' \| 'full' | 'lg' | 容器大小 |
| fluid | boolean | false | 流式布局 |
| padded | boolean | true | 是否有内边距 |
| centered | boolean | false | 是否居中 |

**插槽**

| 插槽 | 说明 |
|------|------|
| default | 页面内容 |
| header | 自定义头部 |
| extra | 头部额外内容 |
| footer | 页面底部 |

**示例**

```vue
<template>
  <PageContainer 
    title="用户管理" 
    description="管理系统用户信息"
    :breadcrumb="[
      { title: '首页', path: '/' },
      { title: '用户管理' }
    ]"
  >
    <template #extra>
      <Button type="primary">新增用户</Button>
    </template>
    
    <!-- 页面内容 -->
    <div>用户列表...</div>
  </PageContainer>
</template>
```

#### Grid 网格系统

响应式网格布局系统。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| cols | ResponsiveValue\<number\> | 12 | 列数 |
| gap | ResponsiveValue\<number \| string\> | 4 | 间距 |
| autoFit | boolean | false | 自动填充 |
| minItemWidth | string | '250px' | 最小项宽度 |
| justify | string | 'start' | 对齐方式 |
| align | string | 'stretch' | 垂直对齐 |
| dense | boolean | false | 密集布局 |

**示例**

```vue
<template>
  <!-- 响应式网格 -->
  <Grid :cols="{ xs: 1, sm: 2, md: 3, lg: 4 }" :gap="4">
    <GridItem v-for="item in items" :key="item.id">
      <Card>{{ item.content }}</Card>
    </GridItem>
  </Grid>
  
  <!-- 自适应网格 -->
  <Grid auto-fit min-item-width="300px" :gap="6">
    <GridItem v-for="item in items" :key="item.id">
      <Card>{{ item.content }}</Card>
    </GridItem>
  </Grid>
  
  <!-- 自定义布局 -->
  <Grid :cols="6" :gap="4">
    <GridItem :span="2">
      <Card>占用2列</Card>
    </GridItem>
    <GridItem :span="4">
      <Card>占用4列</Card>
    </GridItem>
  </Grid>
</template>
```

#### GridItem 网格项

网格系统的子项组件。

**属性**

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| span | ResponsiveValue\<number\> | 1 | 占用列数 |
| offset | ResponsiveValue\<number\> | 0 | 列偏移 |
| rowSpan | ResponsiveValue\<number\> | - | 占用行数 |
| order | ResponsiveValue\<number\> | - | 显示顺序 |
| justify | string | 'stretch' | 自对齐 |
| align | string | 'stretch' | 自垂直对齐 |

## 预设配置

组件库提供了常用的预设配置：

```typescript
import { componentPresets } from '@/components/ui'

// 使用预设配置
const primaryButton = componentPresets.button.primary
const elevatedCard = componentPresets.card.elevated
const statusBadge = componentPresets.badge.status
```

## 自定义主题

组件完全支持设计系统的主题切换：

```vue
<template>
  <div>
    <!-- 主题切换器 -->
    <ThemeToggle mode="dropdown" />
    
    <!-- 组件会自动适应主题 -->
    <Card title="主题测试">
      <Button type="primary">按钮</Button>
      <Tag type="success">标签</Tag>
    </Card>
  </div>
</template>
```

## 响应式设计

所有组件都支持响应式设计：

```typescript
// 响应式属性值
const responsiveCols = {
  xs: 1,    // 手机
  sm: 2,    // 小平板
  md: 3,    // 平板
  lg: 4,    // 桌面
  xl: 5,    // 大屏幕
  '2xl': 6  // 超大屏幕
}
```

## 最佳实践

### 1. 组件选择

- **Card**: 用于内容分组和信息展示
- **Button**: 用于触发操作，注意选择合适的类型和变种
- **Badge**: 用于状态标记和数量提示
- **Tag**: 用于分类标记和过滤条件
- **Loading**: 用于异步操作的加载状态

### 2. 布局设计

```vue
<template>
  <!-- 页面级布局 -->
  <PageContainer title="页面标题">
    <!-- 卡片网格布局 -->
    <Grid :cols="{ xs: 1, md: 2, lg: 3 }" :gap="6">
      <GridItem v-for="item in items" :key="item.id">
        <Card hoverable>
          <!-- 卡片内容 -->
        </Card>
      </GridItem>
    </Grid>
  </PageContainer>
</template>
```

### 3. 主题适配

```vue
<style scoped>
.custom-component {
  /* 使用设计系统变量 */
  background-color: var(--color-bg-primary);
  color: var(--color-text-primary);
  border: var(--border-width-1) solid var(--color-border-primary);
  border-radius: var(--border-radius-md);
  padding: var(--spacing-4);
}
</style>
```

### 4. 响应式适配

```vue
<template>
  <!-- 响应式网格 -->
  <Grid 
    :cols="{ xs: 1, sm: 2, md: 3, lg: 4 }"
    :gap="{ xs: 3, md: 4, lg: 6 }"
  >
    <GridItem 
      v-for="item in items" 
      :key="item.id"
      :span="{ xs: 1, md: item.featured ? 2 : 1 }"
    >
      <Card>{{ item.content }}</Card>
    </GridItem>
  </Grid>
</template>
```

## 问题排除

### 常见问题

1. **组件样式不生效**
   - 确保已导入全局样式 `globals.css`
   - 检查是否正确使用设计令牌变量

2. **主题切换异常**
   - 确保已初始化主题管理器
   - 检查CSS变量是否正确应用

3. **响应式布局异常**
   - 检查响应式值格式是否正确
   - 确认断点配置是否匹配设计系统

### 调试技巧

```vue
<script setup>
import { useDesignSystem, useResponsive } from '@/composables/useDesignSystem'

// 调试设计系统
const ds = useDesignSystem()
console.log('Current theme:', ds.theme.value)

// 调试响应式
const responsive = useResponsive()
console.log('Current breakpoint:', responsive.current.value)
</script>
```

## 更新日志

- **v1.0.0** - 初始版本，包含基础UI组件和布局系统
- 完整的TypeScript类型支持
- 响应式设计和主题切换
- 无障碍友好的实现