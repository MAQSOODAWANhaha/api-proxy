## 前端 UI 设计规范（web/）

**权威来源**：前端 UI 设计标准以 `docs/design.md` 为准；本文件仅做“强约束摘要”，避免在页面/组件里出现风格漂移。

### 强约束（必须遵守）

- **无阴影主 UI**：卡片/表格容器/弹窗/浮层默认不使用 `shadow-*`。
- **唯一阴影例外**：仅 `key / url / path` 等“代码样式信息”允许轻微阴影（通常由 `table-code` 承担）。
- **表格与容器**：表格使用 `@/components/ui/table`；外层使用 `DataTableShell`；禁止页面里重复定义表头/行/单元格的基础排版与 padding。
- **语义样式类**：在表格内展示 key/url/path、次级信息、状态、标签时，必须使用 `docs/design.md` 规定的语义类（定义在 `web/src/shadcn.css`）。
- **禁止自定义阴影**：禁止 `shadow-[...]`（包括用阴影模拟描边）。

### 变更类型 → 必做动作

- **只改业务逻辑/数据交互**：不改 UI 结构与样式时，仍需保持现有组件/样式类不被破坏（尤其是 `DataTableShell` 与语义样式类）。
- **调整页面布局/组件结构**：优先复用 `web/src/components/common/` 既有组件；避免在页面内重复拼装“卡片/表格容器”的基础样式。
- **新增/调整 UI Tokens 或语义样式类**：
  - 同步更新 `docs/design.md`（语义、使用场景、反例）
  - 同步更新 `web/src/shadcn.css`（类定义与暗色适配）
- **移动端适配（`sm` 以下）**：涉及表格信息密度时，按 `docs/design.md` 的规则考虑“卡片列表替代表格”，并确保关键信息首屏可见。

### UI 交付自查（建议在提交前走一遍）

- **一致性**：次级信息统一用 `table-subtext`；状态统一用 `table-status-*`；key/url/path 用 `table-code`。
- **层级**：容器/弹窗/浮层不使用阴影；层级只用 `border + bg + spacing`。
- **可读性**：长文本/ID 支持截断或换行策略一致；表格列对齐稳定，不靠随机颜色传达语义。
- **暗色模式**：若涉及 `shadcn.css` 或自定义类，必须确认暗色下对比度与边框可用。

### 交付检查（必做）

```bash
cd web
npm run lint
npm run build
```
