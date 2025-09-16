//! 轻量级请求处理管道（Pipeline）骨架
//!
//! 目标：将代理准备流程按职责拆分为一组可组合步骤（认证 → 限流 → 提供商配置 → 选 key）。
//! 本模块提供最小可用接口与两个示例步骤（认证与速率限制占位）。
//! 先行落地骨架，不改变现有调用路径，便于后续渐进迁移与单元测试。

use std::sync::Arc;

use pingora_proxy::Session;

use crate::error::ProxyError;
use crate::proxy::{AuthenticationService, ProxyContext, RequestHandler};
use crate::auth::types::AuthType;

/// 步骤执行结果
pub enum StepResult {
    /// 继续执行后续步骤
    Continue,
    /// 终止管道并返回错误
    Break(ProxyError),
}

/// 管道步骤 trait：每个步骤只做一件事
#[async_trait::async_trait]
pub trait ProxyStep: Send + Sync {
    async fn run(&self, session: &mut Session, ctx: &mut ProxyContext) -> StepResult;
}

/// 处理管道：顺序执行步骤，遇错提前返回
pub struct ProxyPipeline {
    steps: Vec<Arc<dyn ProxyStep>>,
}

impl ProxyPipeline {
    pub fn new(steps: Vec<Arc<dyn ProxyStep>>) -> Self {
        Self { steps }
    }

    pub async fn execute(&self, session: &mut Session, ctx: &mut ProxyContext) -> Result<(), ProxyError> {
        for step in &self.steps {
            match step.run(session, ctx).await {
                StepResult::Continue => continue,
                StepResult::Break(err) => return Err(err),
            }
        }
        Ok(())
    }
}

/// 构建器：便于在服务启动时声明式组合步骤
pub struct PipelineBuilder {
    steps: Vec<Arc<dyn ProxyStep>>,
}

impl PipelineBuilder {
    pub fn new() -> Self { Self { steps: Vec::new() } }
    pub fn step(mut self, s: Arc<dyn ProxyStep>) -> Self { self.steps.push(s); self }
    pub fn build(self) -> ProxyPipeline { ProxyPipeline::new(self.steps) }
}

// ---------------- 示例步骤：认证 ----------------

pub struct AuthenticationStep {
    auth_service: Arc<AuthenticationService>,
}

impl AuthenticationStep {
    pub fn new(auth_service: Arc<AuthenticationService>) -> Self { Self { auth_service } }
}

#[async_trait::async_trait]
impl ProxyStep for AuthenticationStep {
    async fn run(&self, session: &mut Session, ctx: &mut ProxyContext) -> StepResult {
        let req_id = ctx.request_id.clone();
        match self.auth_service.authenticate_entry_api(session, &req_id).await {
            Ok(user_api) => {
                // 仅设置 user_service_api，其余配置交给后续步骤
                ctx.user_service_api = Some(user_api);
                StepResult::Continue
            }
            Err(e) => StepResult::Break(e),
        }
    }
}

// ---------------- 示例步骤：速率限制（占位） ----------------

/// 以闭包形式注入速率限制检查，避免直接依赖现有 RequestHandler 方法。
pub struct RateLimitStep<F>
where
    F: Send + Sync + 'static + Fn(i32) -> bool,
{
    /// 从 ctx 中获取 user_service_api.id 后进行检查：返回 true 表示通过
    checker: F,
}

impl<F> RateLimitStep<F>
where
    F: Send + Sync + 'static + Fn(i32) -> bool,
{
    pub fn new(checker: F) -> Self { Self { checker } }
}

#[async_trait::async_trait]
impl<F> ProxyStep for RateLimitStep<F>
where
    F: Send + Sync + 'static + Fn(i32) -> bool,
{
    async fn run(&self, _session: &mut Session, ctx: &mut ProxyContext) -> StepResult {
        let Some(api) = ctx.user_service_api.as_ref() else {
            return StepResult::Break(ProxyError::internal("user_service_api not set"));
        };

        if (self.checker)(api.id) {
            StepResult::Continue
        } else {
            StepResult::Break(ProxyError::rate_limit("rate limited"))
        }
    }
}

// （删除）开始追踪与聚合步骤：追踪副作用已统一由 ProxyService 处理

// ---------------- 步骤：速率限制 ----------------

pub struct RateLimitStepReal {
    handler: Arc<RequestHandler>,
}

impl RateLimitStepReal {
    pub fn new(handler: Arc<RequestHandler>) -> Self { Self { handler } }
}

#[async_trait::async_trait]
impl ProxyStep for RateLimitStepReal {
    async fn run(&self, _session: &mut Session, ctx: &mut ProxyContext) -> StepResult {
        let Some(api) = ctx.user_service_api.as_ref() else {
            return StepResult::Break(ProxyError::internal("user_service_api not set"));
        };
        match self.handler.check_rate_limit(api).await {
            Ok(_) => StepResult::Continue,
            Err(e) => StepResult::Break(e),
        }
    }
}

// ---------------- 步骤：提供商配置与超时 ----------------

pub struct ProviderConfigStep {
    handler: Arc<RequestHandler>,
}

impl ProviderConfigStep {
    pub fn new(handler: Arc<RequestHandler>) -> Self { Self { handler } }
}

#[async_trait::async_trait]
impl ProxyStep for ProviderConfigStep {
    async fn run(&self, _session: &mut Session, ctx: &mut ProxyContext) -> StepResult {
        let Some(user_api) = ctx.user_service_api.as_ref() else {
            return StepResult::Break(ProxyError::internal("user_service_api not set"));
        };
        let provider_type = match self.handler.get_provider_type(user_api.provider_type_id).await {
            Ok(p) => p,
            Err(e) => { return StepResult::Break(e); }
        };

        ctx.provider_type = Some(provider_type.clone());
        ctx.selected_provider = Some(provider_type.name.clone());

        // 超时配置：用户 > 动态配置 > 默认
        let timeout = if let Some(user_timeout) = user_api.timeout_seconds {
            Some(user_timeout)
        } else if let Ok(Some(pc)) = self.handler.provider_config_manager()
            .get_provider_by_name(&provider_type.name).await {
            pc.timeout_seconds
        } else {
            provider_type.timeout_seconds
        };
        ctx.timeout_seconds = timeout;

        StepResult::Continue
    }
}

// ---------------- 步骤：API 密钥选择 ----------------

pub struct ApiKeySelectionStep {
    handler: Arc<RequestHandler>,
}

impl ApiKeySelectionStep {
    pub fn new(handler: Arc<RequestHandler>) -> Self { Self { handler } }
}

#[async_trait::async_trait]
impl ProxyStep for ApiKeySelectionStep {
    async fn run(&self, _session: &mut Session, ctx: &mut ProxyContext) -> StepResult {
        let Some(user_api) = ctx.user_service_api.as_ref() else {
            return StepResult::Break(ProxyError::internal("user_service_api not set"));
        };
        if ctx.provider_type.is_none() {
            return StepResult::Break(ProxyError::internal("provider_type not set"));
        }

        let selected_backend = match self.handler.select_api_key(user_api, &ctx.request_id).await {
            Ok(b) => b,
            Err(e) => { return StepResult::Break(e); }
        };
        ctx.selected_backend = Some(selected_backend.clone());

        StepResult::Continue
    }
}

// ---------------- 步骤：凭证解析 ----------------

/// 根据 ApiKeySelectionStep 的结果解析最终上游凭证
pub struct CredentialResolutionStep {
    handler: Arc<RequestHandler>,
}

impl CredentialResolutionStep {
    pub fn new(handler: Arc<RequestHandler>) -> Self { Self { handler } }
}

#[async_trait::async_trait]
impl ProxyStep for CredentialResolutionStep {
    async fn run(&self, _session: &mut Session, ctx: &mut ProxyContext) -> StepResult {
        let Some(backend) = ctx.selected_backend.as_ref() else {
            return StepResult::Break(ProxyError::internal("selected_backend not set"));
        };

        let auth_type = AuthType::from(backend.auth_type.as_str());
        match auth_type {
            AuthType::ApiKey => {
                ctx.resolved_credential = Some(crate::proxy::request_handler::ResolvedCredential::ApiKey(
                    backend.api_key.clone(),
                ));
                StepResult::Continue
            }
            AuthType::OAuth => {
                // backend.api_key 在 OAuth 模式下存 session_id
                match self
                    .handler
                    .resolve_oauth_access_token(&backend.api_key, &ctx.request_id)
                    .await
                {
                    Ok(token) => {
                        ctx.resolved_credential = Some(
                            crate::proxy::request_handler::ResolvedCredential::OAuthAccessToken(token),
                        );
                        StepResult::Continue
                    }
                    Err(e) => StepResult::Break(e),
                }
            }
            // 其余认证类型后续扩展；当前保持不支持，避免隐式行为
            other => StepResult::Break(ProxyError::internal(format!(
                "Unsupported auth type in CredentialResolutionStep: {}",
                other
            ))),
        }
    }
}

// 注意：此处不添加基于 unsafe 的单元测试，避免违反仓库禁止 unsafe 的约束
