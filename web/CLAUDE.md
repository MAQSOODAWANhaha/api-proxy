# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 常用命令

### 开发和构建
- `npm run dev` - 启动开发服务器（带热重载）
- `npm run build` - 生产环境构建

### 项目架构

这是一个基于 React + TypeScript 的单页应用，使用现代前端技术栈构建：

#### 核心技术栈
- **构建工具**: ESBuild（自定义构建脚本）
- **前端框架**: React 18 + TypeScript
- **路由**: React Router 7（HashRouter）
- **状态管理**: Zustand
- **UI组件**: Radix UI + shadcn/ui
- **样式**: Tailwind CSS + CSS Variables
- **主题**: next-themes（支持亮/暗模式）
- **图表**: Recharts
- **表单**: React Hook Form + Zod

#### 项目结构说明

```
src/
├── components/           # 可复用组件
│   ├── ui/              # shadcn/ui 基础组件
│   ├── common/          # 通用业务组件
│   ├── dashboard/       # 仪表板特定组件
│   ├── provider/        # 提供商相关组件
│   └── layout/          # 布局组件
├── layouts/             # 页面布局
├── pages/               # 页面组件
│   ├── dashboard/       # 仪表板页面
│   └── api/             # API 相关页面
├── store/               # Zustand 状态管理
├── hooks/               # 自定义 React Hooks
└── lib/                 # 工具函数
```

#### 架构特点

**路由架构**:
- 使用 HashRouter 进行客户端路由
- ProtectedRoute 组件提供路由守卫
- DashboardLayout 为主要布局（侧栏+顶栏+内容区）

**状态管理**:
- 使用 Zustand 进行轻量级状态管理
- `store/auth.ts` 管理认证状态
- `store/ui.ts` 管理UI状态

**组件系统**:
- 基于 shadcn/ui 的设计系统
- 采用 CSS Variables + Tailwind 的主题系统
- 组件采用 Radix UI 作为无头组件基础

**样式系统**:
- Tailwind CSS 作为主要样式解决方案
- CSS Variables 定义主题颜色
- 支持亮/暗模式切换

#### 开发注意事项

- 路径别名: `@/*` 映射到 `./src/*`
- 使用 TypeScript 严格模式
- 组件文件使用 `.tsx` 扩展名
- 工具函数放置在 `lib/utils.ts`
- 新增页面需要在 `App.tsx` 中配置路由
- 主题颜色通过 CSS Variables 定义在 `shadcn.css` 中

## UI / 设计规范

- 全局 UI 设计标准见 `docs/design.md`（Tokens、组件标准、语义样式类、反例、交付检查）
- `src/shadcn.css` 是主题变量与“语义样式类”（如 `table-code` / `table-subtext` / `table-status-*`）的权威定义位置
- 组件与页面优先复用 `src/components/ui/` 与 `src/components/common/` 的既有实现，避免在页面里重复定义基础样式
- 容器/弹窗/浮层默认不使用阴影；仅 `key/url/path` 等代码样式信息允许轻微阴影（见 `docs/design.md`）
