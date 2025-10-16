//! # API密钥认证策略
//!
//! `实现管理端OAuth风格的API密钥认证逻辑`
//! `集成共享的ApiKeyManager进行数据库验证`

use super::traits::{AuthStrategy, OAuthTokenResult};
use crate::auth::{ApiKeyManager, types::AuthType};
use crate::error::Result;
use crate::{
    logging::{LogComponent, LogStage},
    lwarn,
};
use async_trait::async_trait;
use std::sync::Arc;

/// API密钥认证策略
///
/// 管理端使用，提供OAuth风格的API密钥认证，包含完整的权限验证
pub struct ApiKeyStrategy {
    /// 认证头名称（默认：Authorization）
    pub header_name: String,
    /// 值格式（默认：Bearer {key}）
    pub value_format: String,
    /// 共享的API密钥管理器（可选，用于实际验证）
    pub api_key_manager: Option<Arc<ApiKeyManager>>,
}

impl Default for ApiKeyStrategy {
    fn default() -> Self {
        Self {
            header_name: "Authorization".to_string(),
            value_format: "Bearer {key}".to_string(),
            api_key_manager: None,
        }
    }
}

impl ApiKeyStrategy {
    /// 创建新的API密钥认证策略
    #[must_use]
    pub fn new(header_name: &str, value_format: &str) -> Self {
        Self {
            header_name: header_name.to_string(),
            value_format: value_format.to_string(),
            api_key_manager: None,
        }
    }

    /// 创建带有API密钥管理器的认证策略
    ///
    /// 用于实际的数据库验证，适用于生产环境
    #[must_use]
    pub fn with_manager(
        header_name: &str,
        value_format: &str,
        api_key_manager: Arc<ApiKeyManager>,
    ) -> Self {
        Self {
            header_name: header_name.to_string(),
            value_format: value_format.to_string(),
            api_key_manager: Some(api_key_manager),
        }
    }

    /// 从配置创建策略
    pub fn from_config(config: &serde_json::Value) -> Result<Self> {
        let header_name = config
            .get("header_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Authorization");

        let value_format = config
            .get("value_format")
            .and_then(|v| v.as_str())
            .unwrap_or("Bearer {key}");

        Ok(Self::new(header_name, value_format))
    }

    /// 设置API密钥管理器
    ///
    /// 允许后续设置管理器用于实际验证
    pub fn set_api_key_manager(&mut self, manager: Arc<ApiKeyManager>) {
        self.api_key_manager = Some(manager);
    }

    /// 格式化认证头值
    #[must_use]
    pub fn format_header_value(&self, api_key: &str) -> String {
        self.value_format.replace("{key}", api_key)
    }

    /// 从认证头值提取API密钥
    pub fn extract_api_key(&self, header_value: &str) -> Option<String> {
        // 如果格式是 "Bearer {key}"
        if self.value_format.starts_with("Bearer ") {
            header_value
                .strip_prefix("Bearer ")
                .map(std::string::ToString::to_string)
        }
        // 如果格式是 "{key}"
        else if self.value_format == "{key}" {
            Some(header_value.to_string())
        }
        // 其他格式需要更复杂的解析
        else {
            // 简单实现：假设{key}在最后
            self.value_format.strip_suffix("{key}").and_then(|prefix| {
                header_value
                    .strip_prefix(prefix)
                    .map(std::string::ToString::to_string)
            })
        }
    }
}

#[async_trait]
impl AuthStrategy for ApiKeyStrategy {
    fn auth_type(&self) -> AuthType {
        AuthType::ApiKey
    }

    async fn authenticate(&self, credentials: &serde_json::Value) -> Result<OAuthTokenResult> {
        let api_key = credentials
            .get("api_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error!(Config, "缺少api_key参数"))?;

        if api_key.is_empty() {
            return Err(crate::error!(Config, "API密钥不能为空"));
        }

        // 如果有API密钥管理器，使用它进行实际验证
        if let Some(manager) = &self.api_key_manager {
            match manager.validate_for_management(api_key).await {
                Ok(validation_result) => {
                    // 转换为OAuth风格的结果
                    let user_info = serde_json::json!({
                        "user_id": validation_result.api_key_info.user_id,
                        "api_key_id": validation_result.api_key_info.id,
                        "provider_type_id": validation_result.api_key_info.provider_type_id,
                        "permissions": validation_result.permissions
                    });

                    Ok(OAuthTokenResult {
                        access_token: api_key.to_string(),
                        refresh_token: None,
                        token_type: "ApiKey".to_string(),
                        expires_in: None, // API密钥通常不过期
                        scope: Some(
                            validation_result
                                .permissions
                                .into_iter()
                                .map(|p| format!("{p:?}"))
                                .collect::<Vec<_>>()
                                .join(" "),
                        ),
                        user_info: Some(user_info),
                    })
                }
                Err(e) => {
                    let error_message = e.to_string();
                    lwarn!(
                        "system",
                        LogStage::Authentication,
                        LogComponent::ApiKey,
                        "api_key_validation_failed",
                        "API key validation failed in management strategy",
                        api_key_preview = %&api_key[..std::cmp::min(8, api_key.len())],
                        error = %error_message
                    );
                    Err(e)
                }
            }
        } else {
            // 回退到基础格式验证（用于测试或没有管理器的场景）
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::ApiKey,
                "no_api_key_manager",
                "ApiKeyStrategy: 没有配置API密钥管理器，使用基础验证"
            );

            // 基础格式检查
            if !api_key.starts_with("sk-") || api_key.len() < 20 {
                crate::bail!(Auth, ApiKeyMalformed);
            }

            // 返回基础结果（仅用于开发/测试）
            Ok(OAuthTokenResult {
                access_token: api_key.to_string(),
                refresh_token: None,
                token_type: "ApiKey".to_string(),
                expires_in: None,
                scope: None,
                user_info: None,
            })
        }
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        // 验证必需的配置字段
        if let Some(header_name) = config.get("header_name")
            && !header_name.is_string()
        {
            return Err(crate::error!(Config, "header_name必须是字符串"));
        }

        if let Some(value_format) = config.get("value_format") {
            if !value_format.is_string() {
                return Err(crate::error!(Config, "value_format必须是字符串"));
            }

            let format_str = value_format.as_str().unwrap();
            if !format_str.contains("{key}") {
                return Err(crate::error!(
                    Config,
                    format!("value_format必须包含{{key}}占位符")
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_api_key_strategy_creation() {
        let strategy = ApiKeyStrategy::default();
        assert_eq!(strategy.header_name, "Authorization");
        assert_eq!(strategy.value_format, "Bearer {key}");

        let custom_strategy = ApiKeyStrategy::new("X-API-Key", "{key}");
        assert_eq!(custom_strategy.header_name, "X-API-Key");
        assert_eq!(custom_strategy.value_format, "{key}");
    }

    #[test]
    fn test_from_config() {
        let config = json!({
            "header_name": "X-API-Key",
            "value_format": "Key {key}"
        });

        let strategy = ApiKeyStrategy::from_config(&config).unwrap();
        assert_eq!(strategy.header_name, "X-API-Key");
        assert_eq!(strategy.value_format, "Key {key}");
    }

    #[test]
    fn test_format_header_value() {
        let strategy = ApiKeyStrategy::default();
        let formatted = strategy.format_header_value("sk-12345");
        assert_eq!(formatted, "Bearer sk-12345");

        let custom_strategy = ApiKeyStrategy::new("X-API-Key", "{key}");
        let formatted = custom_strategy.format_header_value("sk-12345");
        assert_eq!(formatted, "sk-12345");
    }

    #[test]
    fn test_extract_api_key() {
        let strategy = ApiKeyStrategy::default();
        let extracted = strategy.extract_api_key("Bearer sk-12345");
        assert_eq!(extracted, Some("sk-12345".to_string()));

        let custom_strategy = ApiKeyStrategy::new("X-API-Key", "{key}");
        let extracted = custom_strategy.extract_api_key("sk-12345");
        assert_eq!(extracted, Some("sk-12345".to_string()));

        let prefix_strategy = ApiKeyStrategy::new("X-API-Key", "Key {key}");
        let extracted = prefix_strategy.extract_api_key("Key sk-12345");
        assert_eq!(extracted, Some("sk-12345".to_string()));
    }

    #[tokio::test]
    async fn test_authenticate() {
        let strategy = ApiKeyStrategy::default();
        let credentials = json!({
            "api_key": "sk-1234567890abcdef1234"  // 符合长度要求
        });

        let result = strategy.authenticate(&credentials).await;
        match result {
            Ok(r) => {
                assert_eq!(r.access_token, "sk-1234567890abcdef1234");
                assert_eq!(r.token_type, "ApiKey");
                assert!(r.refresh_token.is_none());
            }
            Err(e) => {
                panic!("Authentication failed: {e:?}");
            }
        }
    }

    #[tokio::test]
    async fn test_authenticate_missing_key() {
        let strategy = ApiKeyStrategy::default();
        let credentials = json!({});

        let result = strategy.authenticate(&credentials).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config() {
        let strategy = ApiKeyStrategy::default();

        // 有效配置
        let valid_config = json!({
            "header_name": "X-API-Key",
            "value_format": "Bearer {key}"
        });
        assert!(strategy.validate_config(&valid_config).is_ok());

        // 无效配置：缺少{key}占位符
        let invalid_config = json!({
            "value_format": "Bearer token"
        });
        assert!(strategy.validate_config(&invalid_config).is_err());

        // 无效配置：类型错误
        let invalid_config = json!({
            "header_name": 123
        });
        assert!(strategy.validate_config(&invalid_config).is_err());
    }
}
