# OAuth Provider Trait 重构方案

## 目标
- 统一 Provider trait 的方法命名，采用 `build_*` / `customize_*` 格式，覆盖授权、令牌交换、刷新、撤销四个阶段。
- 调用方不再散落字符串匹配，而是通过 trait 挂钩获取 Provider 特殊逻辑。
- Provider 模块专注构建请求 payload，Auth 模块 orchestrate 整体流程。

## 新的 Trait 设计
```rust
pub trait OauthProvider: Send + Sync + fmt::Debug {
    fn provider_type(&self) -> ProviderType;

    fn build_authorization_request(
        &self,
        request: &mut AuthorizationRequest<'_>,
        session: &oauth_client_sessions::Model,
        config: &OAuthProviderConfig,
    );

    fn build_token_request(
        &self,
        context: TokenExchangeContext<'_>,
    ) -> TokenRequestPayload;

    fn build_refresh_request(
        &self,
        context: TokenRefreshContext<'_>,
    ) -> TokenRequestPayload;

    fn build_revoke_request(
        &self,
        context: TokenRevokeContext<'_>,
    ) -> Option<TokenRequestPayload>;
}
```
- `AuthorizationRequest`、`TokenExchangeContext`、`TokenRefreshContext`、`TokenRevokeContext`、`TokenRequestPayload` 新建在 `provider/request.rs`，提供 builder/辅助方法。
- `TokenRequestPayload` 同时携带目标 URL 与表单字段，调用方不再关心 token/revoke 请求应提交到哪个端点。
- 默认实现：授权请求为空；token/refresh/revoke 返回标准 form（或 `None`）。

## 调用层改造
- `provider::build_authorize_url` 创建 `AuthorizationRequest`，交给 provider 的 `build_authorization_request` 后统一编码。
- `ApiKeyOAuthRefreshService`：
  - 授权码交换：构造 `TokenExchangeContext`，调用 `build_token_request` 获取 payload 并发送。
  - Token 刷新：同理调用 `build_refresh_request`。
  - Token 撤销：调用 `build_revoke_request`，若返回 payload & endpoint 则发送，否则返回 Ok。
- 删除 `token_extra_params`、`add_provider_specific_params` 等旧逻辑。

## Provider 实现
- OpenAI：override `build_token_request` / `build_refresh_request` 以禁止额外参数，`build_revoke_request` 返回必要字段。
- Gemini：在 `build_authorization_request` 注入 `access_type` / `prompt`；token builder 包含额外参数。
- Anthropic：在 refresh/token builder 中写入 `client_secret = code_verifier`。
- Standard：作为回退 ProviderType::Custom，完全复用默认实现，保证新增 provider 至少具备标准 OAuth 行为。

## 测试
- 更新 `tests/claude_oauth_test.rs`、`tests/oauth_provider_test.rs`，直接调用新的 builder 验证参数。

## 验证步骤
1. `cargo fmt`
2. `cargo build`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo test`
```
