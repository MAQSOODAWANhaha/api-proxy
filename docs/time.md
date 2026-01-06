# API-Proxy 系统时区处理增强方案

## 问题分析

当前系统存在严重的时区处理缺失问题：
1. **前端缺失**: 所有API请求未携带时区信息
2. **后端缺失**: 中间件未解析时区，时间查询直接使用UTC
3. **数据展示**: 时间显示未考虑用户时区，导致用户体验差
4. **查询偏差**: 时间范围查询可能因时区差异导致数据不准确

## 核心思路

无需数据库迁移，通过前端携带时区信息，后端在请求处理过程中进行时区转换，确保时间查询的准确性。

- **数据库仍以UTC存储**：所有写入保持 `UTC`，仅在查询入库前将用户本地时间转换为 `UTC`，响应时再按用户时区格式化返回。
- **统一转换工具**：在 `src/types/timezone.rs` 中集中提供 `local_day_bounds`、`local_previous_day_bounds`、`local_date_label`、`local_date_window` 等辅助函数，避免各 Handler 内重复实现。

## 第一部分：前端改造 (Frontend)

### 目标
所有发往后端的API请求都必须携带用户的本地时区信息。

#### 1.1 获取并存储时区
- **位置**: `web/src/store/timezone.ts`（新建文件）
- **功能**: 应用启动时获取浏览器时区，存储在Zustand中
- **实现**: 使用 `Intl.DateTimeFormat().resolvedOptions().timeZone` 获取时区（例如: Asia/Shanghai）

#### 1.2 注入时区头
- **位置**: `web/src/lib/api.ts`
- **功能**: 修改ApiClient类，为所有请求添加 `X-Timezone` Header
- **实现**: 在 `buildHeaders` 方法中自动添加时区信息

#### 1.3 调整时间参数传递
- **时间范围选择器**: 传递本地时间字符串（不带时区），如 "2025-10-18 10:00:00"
- **后端解析**: 结合 `X-Timezone` Header正确解析这些时间

#### 1.4 调整时间数据显示
- **后端返回**: 统一返回UTC格式的ISO 8601字符串
- **前端转换**: 将UTC时间转换为用户本地时间显示
- **推荐库**: 使用 `date-fns-tz` 或 `dayjs` 库来处理时区转换

## 第二部分：后端中间件 (Backend Middleware)

### 目标
创建一个Axum中间件，用于解析 `X-Timezone` 头，并在请求上下文中提供时区信息。

#### 2.1 添加依赖
- **位置**: `Cargo.toml`
- **新增**: `chrono-tz = "0.10"`

#### 2.2 创建时区上下文
- **位置**: `src/management/middleware/timezone.rs`（新建）
- **结构定义**:
```rust
// src/management/middleware/timezone.rs
use chrono_tz::Tz;

pub struct TimezoneContext {
    pub timezone: Tz,
}
```

#### 2.3 创建时区中间件
- **功能**:
  - 读取 `X-Timezone` Header
  - 如果Header不存在或无效，则默认使用 UTC
  - 将解析后的 `chrono_tz::Tz` 实例存入Axum的请求 extensions 中
- **实现**: 定义一个 `TimeZoneLayer` 中间件

#### 2.4 注册中间件
- **位置**: `src/management/server.rs` 或路由模块
- **应用**: 将 `TimeZoneLayer` 应用到所有需要时区处理的管理API路由上

## 第三部分：后端业务逻辑改造 (Backend Handlers)

### 目标
调整所有与时间相关的查询和业务逻辑，以利用中间件提供的时区信息。

#### 3.1 修改查询结构体
- **文件**: `src/management/handlers/logs.rs`, `src/management/handlers/statistics.rs`
- **改动**: 将所有包含时间字段的查询参数结构体中的 `DateTime<Utc>` 修改为 `NaiveDateTime`

**示例**:
```rust
// 之前
pub start_time: Option<DateTime<Utc>>

// 之后
pub start_time: Option<NaiveDateTime>
```

#### 3.2 改造Handler函数
**每个需要处理时间的Handler中的转换逻辑**:

1. **从请求扩展获取时区上下文**:
```rust
let tz_ctx = request.extensions().get::<TimezoneContext>().unwrap();
```

2. **获取查询参数中的NaiveDateTime**:
```rust
let naive_start = query.start_time.unwrap();
```

3. **转换为UTC进行数据库查询**:
```rust
let utc_start = tz_ctx.timezone.from_local_datetime(&naive_start)
    .unwrap()
    .with_timezone(&Utc);
```

#### 3.3 重点改造文件和函数

##### `src/management/handlers/logs.rs`
- `get_traces_list` - 日志列表查询
- `get_dashboard_stats` - 仪表板统计
- `get_analytics` - 分析数据

##### `src/management/handlers/statistics.rs`
- [done] `get_today_dashboard_cards` - 今日仪表板卡片（使用 `local_day_bounds` 计算本地0点、排除昨日数据）
- [done] `get_models_rate` - 模型使用占比（起止时间统一用 `[start, end)` 范围过滤）
- [done] `get_models_statistics` - 模型详细统计（同上）
- [done] `get_tokens_trend` - Token使用趋势（分组标签使用本地日期，时间戳返回本地RFC3339）
- [done] `parse_time_range` - 时间范围解析函数（today/custom 走统一能力，默认回退7天）

#### 3.4 全面审查文件
- `src/management/handlers/users.rs` - 用户相关时间查询（保持响应格式化）
- [done] `src/management/handlers/provider_keys.rs` - 提供商密钥时间查询（趋势窗口完全按本地日界限聚合）
- [done] `src/management/handlers/service_apis.rs` - 服务API时间查询（查询条件统一转为 `[start, end)` 的UTC范围）

检查所有涉及按时间筛选或排序的逻辑。

## 第四部分：前端库选择优化

### 4.1 推荐库对比
- **date-fns-tz**: 功能强大，API友好，但打包体积较大
- **dayjs**: 轻量级，插件化设计，打包体积小

### 4.2 选择建议
- **项目体积要求高**: 优先选择 dayjs + timezone插件
- **功能丰富性要求高**: 选择 date-fns-tz
- **当前项目建议**: dayjs，因为它对打包体积更友好

### 4.3 dayjs集成示例
```typescript
// web/src/lib/timezone.ts
import dayjs from 'dayjs'
import utc from 'dayjs/plugin/utc'
import timezone from 'dayjs/plugin/timezone'

dayjs.extend(utc)
dayjs.extend(timezone)

export const formatUTCtoLocal = (utcTime: string, userTimezone: string) => {
  return dayjs(utcTime).tz(userTimezone).format('YYYY-MM-DD HH:mm:ss')
}

export const formatLocalToISOString = (localTime: string) => {
  return dayjs(localTime).format('YYYY-MM-DD HH:mm:ss')
}
```

## 第五部分：ConvertToUtc Trait实现

### 5.1 设计思路
为了处理任意命名的时间字段，我们引入一个通用的 `ConvertToUtc` Trait，支持：
- `created_after`, `created_before`
- `updated_since`, `updated_until`
- `expires_at`, `last_login`
- 以及任何其他时间字段

### 5.2 ConvertToUtc Trait设计

#### 5.2.1 Trait实现
- **位置**: `src/utils/timezone.rs`（新建）
- **功能**: 提供通用的本地时间到UTC转换能力
```rust
// src/utils/timezone.rs

use chrono::{DateTime, Utc, NaiveDateTime};
use chrono_tz::Tz;

/// 一个将本地时间安全转换为UTC时间的工具 Trait
pub trait ConvertToUtc {
    /// 接受一个时区作为参数，返回一个UTC的DateTime
    fn to_utc(&self, tz: &Tz) -> Option<DateTime<Utc>>;
}

// 为 NaiveDateTime 实现这个 Trait
impl ConvertToUtc for NaiveDateTime {
    fn to_utc(&self, tz: &Tz) -> Option<DateTime<Utc>> {
        // 使用 .single() 来安全处理夏令时切换等边界情况
        // 如果本地时间存在歧义或不存在，它会返回 None
        tz.from_local_datetime(self).single().map(|dt| dt.with_timezone(&Utc))
    }
}

// 为 Option<NaiveDateTime> 实现，方便直接调用
impl ConvertToUtc for Option<NaiveDateTime> {
    fn to_utc(&self, tz: &Tz) -> Option<DateTime<Utc>> {
        // 如果 self 是 Some，则调用 NaiveDateTime 的 to_utc 方法
        self.as_ref().and_then(|naive_dt| naive_dt.to_utc(tz))
    }
}

// 注意：不建议为 Vec<NaiveDateTime> 实现 ConvertToUtc
// 如果需要批量转换，应该创建专门的方法，例如：
// pub trait ConvertToUtcBatch {
//     fn to_utc_batch(&self, tz: &Tz) -> Vec<Option<DateTime<Utc>>>;
// }
```

### 5.3 在Handler中的优雅使用

#### 5.3.1 Trait使用方法
```rust
// 在 handler 文件的顶部
use crate::utils::timezone::ConvertToUtc;

pub async fn get_traces_list(
    Extension(tz_ctx): Extension<Arc<TimezoneContext>>,
    Query(query): Query<LogsListQuery>, // 这里的时间字段依然是 NaiveDateTime
) -> impl IntoResponse {
    // 使用 Trait 进行转换，代码非常干净
    let start_time_utc = query.start_time.to_utc(&tz_ctx.timezone);
    let end_time_utc = query.end_time.to_utc(&tz_ctx.timezone);

    // 假设还有其他时间字段
    let created_after_utc = query.created_after.to_utc(&tz_ctx.timezone);
    let updated_since_utc = query.updated_since.to_utc(&tz_ctx.timezone);
    let expires_at_utc = query.expires_at.to_utc(&tz_ctx.timezone);

    // 在构建查询时使用转换后的UTC时间
    let mut select = ProxyTracing::find();
    if let Some(start) = start_time_utc {
        select = select.filter(proxy_tracing::Column::CreatedAt.gte(start));
    }
    if let Some(end) = end_time_utc {
        select = select.filter(proxy_tracing::Column::CreatedAt.lte(end));
    }
    if let Some(created_after) = created_after_utc {
        select = select.filter(proxy_tracing::Column::CreatedAt.gte(created_after));
    }
    // 其他查询条件...
}
```

### 5.4 方案优势

- **通用性**: 处理任意命名的时间字段
- **简洁性**: Handler业务逻辑更清晰
- **健壮性**: 安全处理夏令时和边界情况
- **可维护性**: 时区转换逻辑集中管理
- **类型安全**: 编译时确保正确性
- **性能**: 编译时确定转换逻辑，运行时开销最小

## 第六部分：错误处理

### 6.1 错误处理策略
- **时区解析错误**: 无效时区时默认使用UTC
- **时间转换错误**: 使用 `.single()` 安全处理边界情况
- **向后兼容**: 未提供时区时保持原有UTC行为
- **夏令时处理**: 自动处理夏令时切换的时间歧义

## 实施步骤

### 步骤1: 后端基础结构
1. 在 `Cargo.toml` 中添加 `chrono-tz = "0.8"` 依赖
2. 创建时区上下文结构体
3. 创建时区中间件

### 步骤2: 前端改造
1. 安装 dayjs + timezone 插件
2. 创建 `web/src/store/timezone.ts` 获取和存储时区
3. 创建 `web/src/lib/timezone.ts` 时间格式化工具函数
4. 修改 `web/src/lib/api.ts` 添加时区Header

### 步骤3: 后端数据结构改造
1. 修改查询参数结构体：`DateTime<Utc>` -> `NaiveDateTime`
2. 创建ConvertToUtc Trait并实现相关类型

### 步骤4: Handler函数改造
1. 在Handler中导入Trait：`use crate::utils::timezone::ConvertToUtc;`
2. 使用 `.to_utc(&tz_ctx.timezone)` 方法转换时间字段
3. 更新数据库查询代码

### 步骤5: 测试和验证
1. 测试各种时区场景
2. 验证夏令时切换和跨日期边界处理
3. 性能测试和错误处理验证

## 预期效果

- **用户体验提升**: 时间显示符合用户本地时区
- **数据准确性**: 时间范围查询更加准确，支持任意时间字段
- **系统稳定性**: 向后兼容，不影响现有功能，安全处理夏令时
- **性能影响**: 最小化，仅在请求处理时转换
- **国际化支持**: 为全球化部署奠定基础
- **代码质量**: 更简洁、可维护、类型安全的时间处理代码

## 注意事项

1. **向后兼容**: 未提供时区时使用UTC
2. **性能考虑**: 时区转换仅在请求处理时进行
3. **错误处理**: 妥善处理无效时区和时间转换错误
4. **测试覆盖**: 重点测试跨时区、夏令时切换等边界情况

## 相关文件清单

### 新建文件
- `web/src/store/timezone.ts` - 前端时区状态管理
- `web/src/lib/timezone.ts` - 前端时间格式化工具函数
- `src/management/middleware/timezone.rs` - 时区中间件
- `src/utils/timezone.rs` - ConvertToUtc Trait实现

### 修改文件
- `Cargo.toml` - 添加chrono-tz依赖
- `web/package.json` - 添加dayjs依赖
- `web/src/lib/api.ts` - 添加时区Header
- `src/management/middleware/mod.rs` - 导入时区中间件
- `src/management/server.rs` - 注册时区中间件
- `src/management/handlers/logs.rs` - 时间查询改造
- `src/management/handlers/statistics.rs` - 时间查询改造#
