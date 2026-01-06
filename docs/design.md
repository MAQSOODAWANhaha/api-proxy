# 全局 UI 设计标准（Admin Dashboard）

本项目管理端 UI 以「数据密集（Data‑Dense）+ 极简（Minimal）+ 一致性优先」为原则。

**目标**：所有页面/组件看起来来自同一个设计系统；同类数据使用同一套排版与颜色规则；减少视觉噪音（尤其是阴影、花哨的胶囊背景、随机色值）。

---

## 1. 设计原则

1. **一致性优先**：同一种语义（如“次级信息/ID/状态/路径/Key”）在任何页面都必须呈现一致。
2. **无阴影主 UI**：卡片、表格容器、弹窗、浮层默认 **不使用阴影**（避免层级噪音）。
3. **唯一阴影例外**：仅 `key / url / path` 这类“代码样式”的信息允许使用轻微阴影（用于可读性与视觉锚点）。
4. **轻边框 + 轻底色**：层级主要通过 `border`、`bg`、`spacing` 表达，而不是 `shadow`。
5. **高可读性**：正文对比度稳定；次级信息统一使用灰度；不依赖颜色表达含义（状态同时提供文本）。

---

## 2. 设计 Tokens（建议值）

> 以 Tailwind 的 slate/neutral 系为主，匹配数据面板风格。

### 2.1 颜色（语义）

- **Surface**（卡片/弹窗背景）：`bg-white`
- **Page Background**：`hsl(var(--background))`（现有 `shadcn.css`）
- **Border**：`border-slate-200`（暗色：`border-slate-800`）
- **Text / Primary**：`text-slate-700`（暗色：`text-slate-200`）
- **Text / Secondary（次级说明、ID、时间）**：`text-slate-500`（暗色：`text-slate-400`）
- **Header（表头）**：`bg-slate-50` + `text-slate-500`

### 2.2 圆角

- **基础圆角**：`rounded-2xl`（容器/弹窗）
- **小圆角**：`rounded-md`（输入/按钮）
- **代码块/路径**：`rounded`

### 2.3 阴影（强约束）

- 默认：**不使用** `shadow-*`
- 例外：仅代码信息使用轻阴影（`shadow-sm`）
- 同样禁止使用 `shadow-[...]` 这类自定义阴影（包括用阴影模拟描边）

---

## 3. 排版标准（Typography）

### 3.1 表格（Table）

以 `@/components/ui/table` 的默认样式为准：

- 表头（`TableHead`）：`text-xs`、`font-medium`、`text-slate-500`
- 表格正文（`TableCell`）：`text-sm`、`text-slate-700`
- 行 hover：`hover:bg-slate-50`

**禁止**：在页面里为表格重复定义 `thead/row/cell` 的基础排版颜色与 padding。

### 3.2 次级信息（Subtext）

用于：ID、创建时间、更新时间、辅助说明、次要标签。

- 统一类：`table-subtext`
- 字号：`text-xs`
- 颜色：`text-slate-500`

---

## 4. 组件标准（Component Standards）

## 4.1 表格容器

- 表格必须使用 `@/components/ui/table`（禁止直接 `<table>`）
- 表格外层统一使用 `DataTableShell`（圆角 + 边框，无阴影）

相关文件：
- `web/src/components/ui/table.tsx`
- `web/src/components/common/DataTableShell.tsx`

## 4.2 表格内的“语义样式类”（强制）

在表格内展示以下语义时，必须使用对应类：

- **key / url / path**：使用 `table-code`（唯一允许阴影）
  - 用于：API Key、Base URL、请求路径、Request ID（如用 code 展示）
- **次级信息**：使用 `table-subtext`
  - 用于：ID、创建时间、更新时间、说明文字
- **状态**：使用 `table-status-*`（无阴影）
  - `table-status-success` / `table-status-warning` / `table-status-danger` / `table-status-muted`
- **普通标签/文本**：使用 `table-tag`（纯文本风格）
  - 用于：服务商名、认证类型、权重等“标签化信息”，但不使用胶囊背景

以上类定义位置：
- `web/src/shadcn.css`

## 4.3 弹窗（Dialog / AlertDialog / Sheet）

统一原则：
- Overlay：`bg-black/50`
- Content：`bg-white` + `border` + `rounded-2xl` + **无阴影**
- Footer 按钮间距统一 `gap-2`（移动端至少 8px）

相关文件：
- `web/src/components/ui/dialog.tsx`
- `web/src/components/ui/alert-dialog.tsx`
- `web/src/components/ui/sheet.tsx`

## 4.4 浮层组件（Select / Popover / Menubar / DropdownMenu / ContextMenu / HoverCard / Tooltip / Toast）

统一原则：浮层以边框表达层级，默认无阴影（含图表 Tooltip）。
业务自定义下拉（例如 `MultiSelect`）也必须遵守同一规则：仅 `border + bg` 表达层级。

相关文件：
- `web/src/components/ui/select.tsx`
- `web/src/components/ui/popover.tsx`
- `web/src/components/ui/menubar.tsx`
- `web/src/components/ui/dropdown-menu.tsx`
- `web/src/components/ui/context-menu.tsx`
- `web/src/components/ui/hover-card.tsx`
- `web/src/components/ui/toast.tsx`
- `web/src/components/ui/sonner.tsx`
- `web/src/components/ui/chart.tsx`

## 4.5 Card / StatCard / SectionCard

统一原则：卡片容器无阴影，通过边框与间距表达层次。

相关文件：
- `web/src/components/ui/card.tsx`
- `web/src/lib/cardStyles.ts`
- `web/src/components/common/StatCard.tsx`
- `web/src/components/common/SectionCard.tsx`

## 4.6 导航（Sidebar）

统一原则：侧栏与菜单项默认无阴影；激活态通过 `bg + outline/border + text` 区分即可。

相关文件：
- `web/src/components/layout/Sidebar.tsx`
- `web/src/components/ui/sidebar.tsx`

---

## 5. 反例（Don’t）

- 不要在不同页面用不同的 `text-neutral-*` / `text-slate-*` 来表达同一种次级信息
- 不要为“服务商/认证类型/权重”随意套胶囊背景（除非它是“状态”）
- 不要在容器与弹窗上堆叠阴影（全站阴影会变成噪音）
- 不要用 `hover:scale-*` 做 hover（会造成布局抖动与风格不统一），优先使用 `hover:bg-*` / `hover:text-*` / `border` 变化

---

## 6. 交付检查（必做）

每次 UI 调整后必须执行：

```bash
cd web
npm run lint
npm run build
```
