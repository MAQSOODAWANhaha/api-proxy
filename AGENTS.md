## Rust 代码开发规范

**沟通语言**：使用中文进行对话和代码注释。

**代码质量要求**：每次代码修改后，必须按以下顺序完成所有检查：

1. **编译检查**：确保代码能够成功编译通过

   ```bash
   cargo build
   ```

2. **代码格式化**：使用 `cargo fmt` 自动格式化代码

   ```bash
   cargo fmt
   ```

3. **静态分析**：运行 `cargo clippy` 检查代码质量和潜在问题

   ```bash
   cargo clippy --all-targets -- -D warnings
   ```

4. **单元测试**：执行 `cargo test` 确保所有测试通过
   ```bash
   cargo test
   ```

**注意**：以上所有步骤都必须通过，不允许提交未通过检查的代码。

**Lint 规则**：禁止随意添加 `#[allow(...)]` 规避静态分析告警；只有在充分论证并经负责人确认后，才可以在最小范围内添加 `#[allow]`，且必须在代码附近注明理由。

## 前端代码开发规范（web/）

**代码质量要求**：每次前端代码修改后，必须按以下顺序完成所有检查：

1. **Lint 检查**：

   ```bash
   cd web
   npm run lint
   ```

2. **编译构建**：确保前端可正常编译

   ```bash
   cd web
   npm run build
   ```

**注意**：以上所有步骤都必须通过，不允许提交未通过检查的代码。

## 错误处理最佳实践（参见 `src/error/`）

- **统一返回类型**：所有可能失败的接口都应返回 `crate::error::Result<T>`，避免混用 `anyhow::Result` 等其他别名。
- **Typed Error 优先**：各领域通过 `#[derive(thiserror::Error)]` 定义错误枚举并实现 `#[from]`，再由 `ProxyError` 统一承载；若依赖第三方错误，优先实现 `From`/`Into<ProxyError>`，不要复活 `ProxyError::internal` 辅助函数。
- **上下文增强**：使用 `.context("...")` 或 `.with_context(|| ...)`（`anyhow::Context` 风格）只描述“当前动作”，保证原始状态码/错误类型得以保留并被包装成 `ProxyError::Context`。
- **快速返回**：条件判断失败时使用 `ensure!`，需要立即返回时使用 `bail!`，既减少样板代码，也保证错误栈统一。
- **稳定错误码**：新增错误变体时记得在 `ProxyError::error_code`/`status_code` 中维护对应的对外编号与状态码，保持 API 行为稳定。
- **最小化 Internal**：确实无法建模的异常可落到 `ProxyError::Internal(anyhow::Error)`，但必须通过 `?` 自动转换，禁止手工拼字符串。

## 日志记录最佳实践（参见 `src/logging.rs`）

- **统一宏**：业务日志统一使用 `linfo!`、`ldebug!`、`lwarn!`、`lerror!`，避免直接调用 `tracing::*`，确保字段一致。
- **基础字段**：日志必须包含 `request_id`、`stage`、`component`、`operation`、`message` 五个核心字段，可额外附加结构化键值对（如 `error = %err`）。
- **阶段 & 组件选择**：根据上下文选用合适的 `LogStage`、`LogComponent`，缺省时优先选择更精确的枚举值，方便后续检索与分析。
- **错误日志**：捕获 `ProxyError` 时调用 `error.log()` 输出结构化错误信息，同时再追加必要的业务字段，避免重复字符串拼接。
- **初始化配置**：如需调整日志级别或输出格式，应修改 `init_logging` 入口，保持统一的订阅器与过滤器配置。
