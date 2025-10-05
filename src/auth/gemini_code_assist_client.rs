//! # Gemini Code Assist API客户端
//!
//! 实现Google Gemini Code Assist API的调用逻辑
//! 支持loadCodeAssist和onboardUser接口，用于自动获取project_id

use crate::error::ProxyError;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Gemini Code Assist API基础URL
const GEMINI_CODE_ASSIST_BASE_URL: &str = "https://cloudcode-pa.googleapis.com";

/// API请求超时时间（秒）
const GEMINI_REQUEST_TIMEOUT_SECONDS: u64 = 30;

/// 最大重试次数
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// 重试基础延迟（毫秒）
const RETRY_BASE_DELAY_MS: u64 = 1000;

/// 重试最大延迟（毫秒）
const RETRY_MAX_DELAY_MS: u64 = 10000;

/// 客户端元数据结构
#[derive(Debug, Serialize, Clone)]
#[allow(non_snake_case)]
pub struct ClientMetadata {
    pub ideType: String,
    pub platform: String,
    pub pluginType: String,
    pub duetProject: Option<String>,
}

impl ClientMetadata {
    /// 创建没有 project_id 的客户端元数据
    pub fn new() -> Self {
        Self {
            ideType: "IDE_UNSPECIFIED".to_string(),
            platform: "PLATFORM_UNSPECIFIED".to_string(),
            pluginType: "GEMINI".to_string(),
            duetProject: None,
        }
    }

    /// 创建带有 project_id 的客户端元数据
    pub fn with_project(project_id: &str) -> Self {
        Self {
            ideType: "IDE_UNSPECIFIED".to_string(),
            platform: "PLATFORM_UNSPECIFIED".to_string(),
            pluginType: "GEMINI".to_string(),
            duetProject: Some(project_id.to_string()),
        }
    }
}

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: MAX_RETRY_ATTEMPTS,
            base_delay_ms: RETRY_BASE_DELAY_MS,
            max_delay_ms: RETRY_MAX_DELAY_MS,
        }
    }
}

impl RetryConfig {
    pub fn new(max_attempts: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
        }
    }

    /// 计算指数退避延迟
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms =
            (self.base_delay_ms * 2u64.pow(attempt.saturating_sub(1))).min(self.max_delay_ms);
        Duration::from_millis(delay_ms)
    }
}

/// Code Assist API响应结构
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct LoadCodeAssistResponse {
    #[serde(default)]
    pub cloudaicompanionProject: Option<String>,
    pub currentTier: Option<CurrentTier>,
}

/// 当前层级信息（简化版）
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct CurrentTier {
    pub id: String,
}

/// Cloud AI Companion项目信息（onboardUser响应使用）
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct CloudAiCompanionProject {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

/// onboardUser响应结构
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct OnboardUserResponse {
    pub cloudaicompanionProject: CloudAiCompanionProject,
    pub status: String,
}

/// Gemini Code Assist API客户端
#[derive(Debug, Clone)]
pub struct GeminiCodeAssistClient {
    http_client: Client,
    base_url: String,
    retry_config: RetryConfig,
}

impl GeminiCodeAssistClient {
    /// 创建新的Code Assist客户端
    pub fn new() -> Self {
        Self::with_base_url(GEMINI_CODE_ASSIST_BASE_URL)
    }

    /// 使用自定义base URL创建客户端（主要用于测试）
    pub fn with_base_url(base_url: &str) -> Self {
        Self::with_config(base_url, RetryConfig::default())
    }

    /// 使用完整配置创建客户端
    pub fn with_config(base_url: &str, retry_config: RetryConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(GEMINI_REQUEST_TIMEOUT_SECONDS))
            .build()
            .unwrap_or_default();

        Self {
            http_client: client,
            base_url: base_url.to_string(),
            retry_config,
        }
    }

    /// 带重试机制的HTTP请求
    async fn execute_with_retry<F, Fut, R>(
        &self,
        operation: &str,
        mut request_fn: F,
    ) -> Result<R, ProxyError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<R, ProxyError>>,
    {
        let mut last_error = None;

        for attempt in 1..=self.retry_config.max_attempts {
            let start_time = Instant::now();

            match request_fn().await {
                Ok(response) => {
                    let duration = start_time.elapsed();
                    ldebug!(
                        "system",
                        LogStage::ExternalApi,
                        LogComponent::GeminiClient,
                        "retry_success",
                        &format!("{}在第{}次尝试中成功完成，耗时: {:?}", operation, attempt, duration)
                    );
                    return Ok(response);
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    lwarn!(
                        "system",
                        LogStage::ExternalApi,
                        LogComponent::GeminiClient,
                        "retry_fail",
                        &format!("{}第{}次尝试失败，耗时: {:?}，错误: {}", operation, attempt, duration, e)
                    );

                    // 如果是最后一次尝试，保存错误并返回
                    if attempt == self.retry_config.max_attempts {
                        last_error = Some(e);
                        break;
                    }

                    // 检查是否可重试的错误类型
                    let should_retry = match &e {
                        ProxyError::GeminiCodeAssistError { message } => {
                            // 检查错误消息中是否包含可重试的状态码
                            [408, 429, 500, 502, 503, 504]
                                .iter()
                                .any(|&code| message.contains(&code.to_string()))
                        }
                        ProxyError::Network { .. } => true,
                        ProxyError::ConnectionTimeout { .. }
                        | ProxyError::ReadTimeout { .. }
                        | ProxyError::WriteTimeout { .. } => true,
                        _ => false,
                    };

                    if !should_retry {
                        ldebug!(
                            "system",
                            LogStage::ExternalApi,
                            LogComponent::GeminiClient,
                            "non_retryable_error",
                            &format!("错误不可重试，立即返回: {}", e)
                        );
                        return Err(e);
                    }

                    // 计算延迟时间
                    let delay = self.retry_config.calculate_delay(attempt);
                    linfo!(
                        "system",
                        LogStage::ExternalApi,
                        LogComponent::GeminiClient,
                        "retrying",
                        &format!("{}将在{:?}后进行第{}次重试", operation, delay, attempt + 1)
                    );

                    tokio::time::sleep(delay).await;
                }
            }
        }

        // 所有尝试都失败了，返回最后一个错误
        if let Some(error) = last_error {
            Err(error)
        } else {
            Err(ProxyError::gemini_code_assist(format!(
                "{}所有重试尝试都失败了",
                operation
            )))
        }
    }

    /// 调用loadCodeAssist API
    ///
    /// # 参数
    /// * `access_token` - OAuth访问令牌
    /// * `project_id` - 可选的项目ID，如果提供则检查指定项目
    /// * `_client_metadata` - 客户端元数据，包含平台和IDE信息（当前未使用）
    pub async fn load_code_assist(
        &self,
        access_token: &str,
        project_id: Option<&str>,
        _client_metadata: Option<&ClientMetadata>,
    ) -> Result<LoadCodeAssistResponse, ProxyError> {
        let operation_name = "loadCodeAssist";

        self.execute_with_retry(operation_name, || async {
            let mut request_body = serde_json::Map::new();

            // 添加客户端元数据
            let metadata = if let Some(pid) = project_id {
                ClientMetadata::with_project(pid)
            } else {
                ClientMetadata::new()
            };
            request_body.insert(
                "metadata".to_string(),
                serde_json::to_value(metadata).map_err(|e| {
                    ProxyError::gemini_code_assist(format!(
                        "Failed to serialize client metadata: {}",
                        e
                    ))
                })?,
            );

            // 如果有project_id，添加到请求中
            if let Some(pid) = project_id {
                request_body.insert(
                    "cloudaicompanionProject".to_string(),
                    serde_json::Value::String(pid.to_string()),
                );
                ldebug!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "load_code_assist_with_project",
                    &format!("调用loadCodeAssist with project_id: {}", pid)
                );
            } else {
                ldebug!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "load_code_assist_no_project",
                    "调用loadCodeAssist without project_id"
                );
            }

            let url = format!("{}/v1internal:loadCodeAssist", self.base_url);
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "load_code_assist_request",
                &format!("发送loadCodeAssist请求到: {}", url)
            );

            // 打印请求参数
            let request_json = serde_json::to_string(&request_body).map_err(|e| {
                ProxyError::gemini_code_assist(format!("Failed to serialize request body: {}", e))
            })?;
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "load_code_assist_params",
                &format!("loadCodeAssist请求参数: {}", request_json)
            );

            let response = self
                .http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await?;

            ldebug!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "load_code_assist_status",
                &format!("loadCodeAssist响应状态: {}", response.status())
            );

            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                lerror!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "load_code_assist_fail",
                    &format!("loadCodeAssist API失败: status={}, response={}", status, error_text)
                );
                return Err(ProxyError::gemini_code_assist(format!(
                    "loadCodeAssist API failed: {} - {}",
                    status, error_text
                )));
            }

            let response_body = response.text().await?;
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "load_code_assist_response",
                &format!("loadCodeAssist响应体: {}", response_body)
            );

            let response_data: LoadCodeAssistResponse = serde_json::from_str(&response_body)
                .map_err(|e| {
                    ProxyError::gemini_code_assist(format!(
                        "Failed to parse loadCodeAssist response: {}",
                        e
                    ))
                })?;

            let tier_id = self.get_tier_id_from_load_response(&response_data);
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "load_code_assist_ok",
                &format!("loadCodeAssist调用成功: has_project={}, tier_id={}",
                response_data.cloudaicompanionProject.is_some(),
                tier_id)
            );

            Ok(response_data)
        })
        .await
    }

    /// 调用onboardUser API
    ///
    /// # 参数
    /// * `access_token` - OAuth访问令牌
    /// * `project_id` - 可选的项目ID，免费层通常不携带
    /// * `tier_id` - tier ID，从loadCodeAssist响应中获取
    /// * `_client_metadata` - 客户端元数据，包含平台和IDE信息（当前未使用）
    pub async fn onboard_user(
        &self,
        access_token: &str,
        project_id: Option<&str>,
        tier_id: Option<&str>,
        _client_metadata: Option<&ClientMetadata>,
    ) -> Result<OnboardUserResponse, ProxyError> {
        let operation_name = "onboardUser";

        self.execute_with_retry(operation_name, || async {
            let mut request_body = serde_json::Map::new();

            // 添加tierId（必需参数）
            if let Some(tid) = tier_id {
                request_body.insert(
                    "tierId".to_string(),
                    serde_json::Value::String(tid.to_string()),
                );
            } else {
                // 如果没有tierId，使用默认值
                request_body.insert(
                    "tierId".to_string(),
                    serde_json::Value::String("FREE".to_string()),
                );
            }

            // 添加客户端元数据
            let metadata = if let Some(pid) = project_id {
                ClientMetadata::with_project(pid)
            } else {
                ClientMetadata::new()
            };
            request_body.insert(
                "metadata".to_string(),
                serde_json::to_value(metadata).map_err(|e| {
                    ProxyError::gemini_code_assist(format!(
                        "Failed to serialize client metadata: {}",
                        e
                    ))
                })?,
            );

            ldebug!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "onboard_user_call",
                &format!("调用onboardUser with tier_id: {:?}, project_id: {:?}", tier_id, project_id)
            );

            let url = format!("{}/v1internal:onboardUser", self.base_url);
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "onboard_user_request",
                &format!("发送onboardUser请求到: {}", url)
            );

            // 打印请求参数
            let request_json = serde_json::to_string(&request_body).map_err(|e| {
                ProxyError::gemini_code_assist(format!("Failed to serialize request body: {}", e))
            })?;
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "onboard_user_params",
                &format!("onboardUser请求参数: {}", request_json)
            );

            let response = self
                .http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await?;

            ldebug!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "onboard_user_status",
                &format!("onboardUser响应状态: {}", response.status())
            );

            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                lerror!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "onboard_user_fail",
                    &format!("onboardUser API失败: status={}, response={}", status, error_text)
                );
                return Err(ProxyError::gemini_code_assist(format!(
                    "onboardUser API failed: {} - {}",
                    status, error_text
                )));
            }

            let response_body = response.text().await?;
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "onboard_user_response",
                &format!("onboardUser响应体: {}", response_body)
            );

            let response_data: OnboardUserResponse =
                serde_json::from_str(&response_body).map_err(|e| {
                    ProxyError::gemini_code_assist(format!(
                        "Failed to parse onboardUser response: {}",
                        e
                    ))
                })?;

            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "onboard_user_ok",
                &format!("onboardUser调用成功: status={}, project_id={}, display_name={}",
                response_data.status,
                response_data.cloudaicompanionProject.id,
                response_data.cloudaicompanionProject.display_name)
            );

            Ok(response_data)
        })
        .await
    }

    /// 带重试机制的onboardUser调用
    ///
    /// 最多重试5次，使用指数退避算法
    pub async fn onboard_user_with_retry(
        &self,
        access_token: &str,
        project_id: Option<&str>,
        tier_id: Option<&str>,
        client_metadata: Option<&ClientMetadata>,
    ) -> Result<OnboardUserResponse, ProxyError> {
        const MAX_RETRIES: usize = 5;
        const BASE_DELAY_MS: u64 = 1000; // 1秒基础延迟

        let mut retry_count = 0;
        let mut last_error = None;

        while retry_count < MAX_RETRIES {
            match self
                .onboard_user(access_token, project_id, tier_id, client_metadata)
                .await
            {
                Ok(response) => {
                    if retry_count > 0 {
                        linfo!(
                            "system",
                            LogStage::ExternalApi,
                            LogComponent::GeminiClient,
                            "onboard_user_retry_ok",
                            &format!("onboardUser在第{}次重试后成功", retry_count)
                        );
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(format!("{}", e));
                    retry_count += 1;

                    if retry_count < MAX_RETRIES {
                        // 指数退避计算延迟时间
                        let delay_ms = BASE_DELAY_MS * 2u64.pow(retry_count as u32 - 1);
                        lwarn!(
                            "system",
                            LogStage::ExternalApi,
                            LogComponent::GeminiClient,
                            "onboard_user_retry_fail",
                            &format!("onboardUser第{}次调用失败，{}ms后进行第{}次重试. 错误: {}",
                            retry_count,
                            delay_ms,
                            retry_count + 1,
                            e)
                        );

                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    } else {
                        lerror!(
                            "system",
                            LogStage::ExternalApi,
                            LogComponent::GeminiClient,
                            "onboard_user_retry_giveup",
                            &format!("onboardUser重试{}次后仍然失败，最终错误: {}", MAX_RETRIES, e)
                        );
                    }
                }
            }
        }

        // 所有重试都失败了，返回最后一个错误
        Err(ProxyError::gemini_code_assist(last_error.unwrap_or_else(
            || "onboardUser重试失败，但没有具体的错误信息".to_string(),
        )))
    }

    /// 自动获取project_id（带重试机制）
    ///
    /// 1. 调用loadCodeAssist检查是否已有project
    /// 2. 如果没有cloudaicompanionProject，调用onboardUser初始化新项目（带重试）
    /// 3. 返回获取到的project_id
    pub async fn auto_get_project_id_with_retry(
        &self,
        access_token: &str,
    ) -> Result<Option<String>, ProxyError> {
        linfo!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "auto_get_project_id_start",
            "开始自动获取Gemini project_id（带重试）"
        );

        // Step 1: 调用loadCodeAssist (不携带project_id)
        ldebug!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "auto_get_project_id_step1",
            "Step 1: 调用loadCodeAssist检查现有项目"
        );
        let load_response = match self.load_code_assist(access_token, None, None).await {
            Ok(response) => response,
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "load_code_assist_fail",
                    &format!("loadCodeAssist调用失败: {}", e)
                );
                return Err(e);
            }
        };

        // 检查是否已有project
        if let Some(project_id) = load_response.cloudaicompanionProject {
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "project_id_from_load",
                &format!("通过loadCodeAssist获取到project_id: {}", project_id)
            );
            return Ok(Some(project_id));
        }

        // Step 2: 如果没有cloudaicompanionProject，调用onboardUser初始化项目（带重试）
        ldebug!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "auto_get_project_id_step2",
            "Step 2: loadCodeAssist未返回cloudaicompanionProject，调用onboardUser初始化（带重试）"
        );

        // 从loadCodeAssist响应中获取tierId
        let tier_id = self.get_tier_id_from_load_response(&load_response);
        ldebug!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "get_tier_id",
            &format!("从loadCodeAssist获取到tierId: {}", tier_id)
        );

        let onboard_response = match self
            .onboard_user_with_retry(access_token, None, Some(tier_id), None)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "onboard_user_retry_fail",
                    &format!("onboardUser重试调用失败: {}", e)
                );
                return Err(e);
            }
        };

        let project_id = Some(onboard_response.cloudaicompanionProject.id);
        linfo!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "project_id_from_onboard",
            &format!("通过onboardUser重试获取到project_id: {:?} (display_name: {})",
            project_id,
            onboard_response.cloudaicompanionProject.display_name)
        );

        Ok(project_id)
    }

    /// 从loadCodeAssist响应中获取tierId
    ///
    /// 参考JavaScript实现中的getOnboardTier逻辑
    fn get_tier_id_from_load_response<'a>(
        &self,
        load_response: &'a LoadCodeAssistResponse,
    ) -> &'a str {
        // 使用currentTier的id
        if let Some(current_tier) = &load_response.currentTier {
            return &current_tier.id;
        }

        // 默认返回FREE层级
        "FREE"
    }

    /// 自动获取project_id的完整流程
    ///
    /// 这个方法会依次尝试：
    /// 1. 调用loadCodeAssist（不携带project_id）检查是否有现有项目
    /// 2. 如果有cloudaicompanionProject，直接使用该值作为project_id
    /// 3. 如果没有cloudaicompanionProject，调用onboardUser初始化新项目
    /// 4. 返回获取到的project_id
    pub async fn auto_get_project_id(
        &self,
        access_token: &str,
    ) -> Result<Option<String>, ProxyError> {
        linfo!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "auto_get_project_id_start",
            "开始自动获取Gemini project_id"
        );

        // Step 1: 调用loadCodeAssist (不携带project_id)
        ldebug!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "auto_get_project_id_step1",
            "Step 1: 调用loadCodeAssist检查现有项目"
        );
        let load_response = match self.load_code_assist(access_token, None, None).await {
            Ok(response) => response,
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "load_code_assist_fail",
                    &format!("loadCodeAssist调用失败: {}", e)
                );
                return Err(e);
            }
        };

        // 检查是否已有project
        if let Some(project_id) = load_response.cloudaicompanionProject {
            linfo!(
                "system",
                LogStage::ExternalApi,
                LogComponent::GeminiClient,
                "project_id_from_load",
                &format!("通过loadCodeAssist获取到project_id: {}", project_id)
            );
            return Ok(Some(project_id));
        }

        // Step 2: 如果没有cloudaicompanionProject，调用onboardUser初始化项目
        ldebug!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "auto_get_project_id_step2",
            "Step 2: loadCodeAssist未返回cloudaicompanionProject，调用onboardUser初始化"
        );

        // 从loadCodeAssist响应中获取tierId
        let tier_id = self.get_tier_id_from_load_response(&load_response);
        ldebug!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "get_tier_id",
            &format!("从loadCodeAssist获取到tierId: {}", tier_id)
        );

        let onboard_response = match self
            .onboard_user(access_token, None, Some(tier_id), None)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::ExternalApi,
                    LogComponent::GeminiClient,
                    "onboard_user_fail",
                    &format!("onboardUser调用失败: {}", e)
                );
                return Err(e);
            }
        };

        let project_id = Some(onboard_response.cloudaicompanionProject.id);
        linfo!(
            "system",
            LogStage::ExternalApi,
            LogComponent::GeminiClient,
            "project_id_from_onboard",
            &format!("通过onboardUser获取到project_id: {:?} (display_name: {})",
            project_id,
            onboard_response.cloudaicompanionProject.display_name)
        );

        Ok(project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GeminiCodeAssistClient::new();
        assert!(!client.base_url.is_empty());
    }

    #[test]
    fn test_client_with_custom_base_url() {
        let client = GeminiCodeAssistClient::with_base_url("https://test.example.com");
        assert_eq!(client.base_url, "https://test.example.com");
    }

    #[test]
    fn test_client_with_custom_config() {
        let retry_config = RetryConfig::new(5, 2000, 30000);
        let client = GeminiCodeAssistClient::with_config("https://test.example.com", retry_config);
        assert_eq!(client.retry_config.max_attempts, 5);
        assert_eq!(client.retry_config.base_delay_ms, 2000);
    }

    #[test]
    fn test_client_metadata_new() {
        let metadata = ClientMetadata::new();
        assert_eq!(metadata.ideType, "IDE_UNSPECIFIED");
        assert_eq!(metadata.platform, "PLATFORM_UNSPECIFIED");
        assert_eq!(metadata.pluginType, "GEMINI");
        assert!(metadata.duetProject.is_none());
    }

    #[test]
    fn test_client_metadata_with_project() {
        let metadata = ClientMetadata::with_project("test-project");
        assert_eq!(metadata.ideType, "IDE_UNSPECIFIED");
        assert_eq!(metadata.platform, "PLATFORM_UNSPECIFIED");
        assert_eq!(metadata.pluginType, "GEMINI");
        assert_eq!(metadata.duetProject, Some("test-project".to_string()));
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, MAX_RETRY_ATTEMPTS);
        assert_eq!(config.base_delay_ms, RETRY_BASE_DELAY_MS);
        assert_eq!(config.max_delay_ms, RETRY_MAX_DELAY_MS);
    }

    #[test]
    fn test_retry_config_delay_calculation() {
        let config = RetryConfig::new(3, 1000, 8000);

        // 第一次重试: 1000ms
        assert_eq!(config.calculate_delay(1).as_millis(), 1000);

        // 第二次重试: 2000ms
        assert_eq!(config.calculate_delay(2).as_millis(), 2000);

        // 第三次重试: 4000ms (不会超过max_delay)
        assert_eq!(config.calculate_delay(3).as_millis(), 4000);

        // 测试上限
        let delay = config.calculate_delay(10);
        assert_eq!(delay.as_millis(), 8000);
    }

    #[test]
    fn test_retry_config_custom() {
        let config = RetryConfig::new(5, 500, 5000);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 5000);
    }

    #[tokio::test]
    async fn test_load_code_assist_request_structure() {
        // 这个测试需要mock服务器，在集成测试中实现
        let client = GeminiCodeAssistClient::new();

        // 由于需要真实的token，这里只测试请求结构构建
        // 实际的API调用在集成测试中测试
        assert!(client.base_url.contains("cloudcode-pa.googleapis.com"));
    }

    #[test]
    fn test_platform_detection() {
        let metadata = ClientMetadata::new();

        // 验证固定的平台值
        assert_eq!(metadata.platform, "PLATFORM_UNSPECIFIED");
        assert_eq!(metadata.ideType, "IDE_UNSPECIFIED");
        assert_eq!(metadata.pluginType, "GEMINI");
    }

    #[tokio::test]
    async fn test_auto_get_project_id() {
        let client = GeminiCodeAssistClient::new();

        // 测试方法在没有真实token时的行为
        let result = client.auto_get_project_id("fake_token").await;

        // 应该失败（因为没有真实的token）
        match result {
            Err(_) => {
                // 预期的行为，因为没有真实的token
            }
            Ok(_) => {
                panic!("应该失败，因为没有真实的token");
            }
        }
    }
}
