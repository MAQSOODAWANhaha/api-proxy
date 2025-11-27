//! # `OpenAI` JWT 解析器
//!
//! 专门用于解析 `OpenAI` `access_token` 中的用户信息
//! 从 JWT payload 中提取 `chatgpt_account_id` 等关键信息

use crate::error::auth::OAuthError;
use crate::logging::{LogComponent, LogStage};
use crate::{linfo, lwarn};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `OpenAI` JWT 中的认证信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIAuthInfo {
    /// `ChatGPT` 账户 ID
    pub chatgpt_account_id: String,
    /// `ChatGPT` 计划类型
    pub chatgpt_plan_type: Option<String>,
    /// `ChatGPT` 用户 ID
    pub chatgpt_user_id: Option<String>,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 其他声明
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// `OpenAI` JWT Payload 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIJWTPayload {
    /// `OpenAI` 特定声明
    #[serde(rename = "https://api.openai.com/auth")]
    pub openai_auth: Option<OpenAIAuthInfo>,
    /// 其他声明
    #[serde(flatten)]
    pub other_claims: HashMap<String, serde_json::Value>,
}

/// JWT 解析器
pub struct OpenAI;

impl OpenAI {
    /// 从 `access_token` 中解析 `OpenAI` 用户信息
    pub fn extract_openai_info(
        &self,
        access_token: &str,
    ) -> Result<Option<OpenAIAuthInfo>, OAuthError> {
        let claims = Self::decode_payload(access_token)?;

        // 提取 OpenAI 认证信息
        if let Some(openai_auth) = claims.openai_auth {
            linfo!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "openai_jwt_parsed",
                "成功从 JWT 解析 OpenAI 用户信息",
                chatgpt_account_id = %openai_auth.chatgpt_account_id,
                chatgpt_plan_type = ?openai_auth.chatgpt_plan_type
            );
            Ok(Some(openai_auth))
        } else {
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "openai_jwt_missing_auth",
                "JWT 中未找到 OpenAI 认证信息"
            );
            Ok(None)
        }
    }

    fn decode_payload(access_token: &str) -> Result<OpenAIJWTPayload, OAuthError> {
        let mut segments = access_token.split('.');
        let header_segment = segments
            .next()
            .ok_or_else(|| OAuthError::InvalidToken("JWT 结构无效: 缺少 header 段".to_string()))?;
        let payload_segment = segments
            .next()
            .ok_or_else(|| OAuthError::InvalidToken("JWT 结构无效: 缺少 payload 段".to_string()))?;
        segments.next().ok_or_else(|| {
            OAuthError::InvalidToken("JWT 结构无效: 缺少 signature 段".to_string())
        })?;

        if segments.next().is_some() {
            return Err(OAuthError::InvalidToken(
                "JWT 结构无效: 包含多余的段".to_string(),
            ));
        }

        if header_segment.is_empty() {
            return Err(OAuthError::InvalidToken("JWT header 段为空".to_string()));
        }

        if payload_segment.is_empty() {
            return Err(OAuthError::InvalidToken("JWT payload 段为空".to_string()));
        }

        if let Err(e) = URL_SAFE_NO_PAD.decode(header_segment) {
            return Err(OAuthError::InvalidToken(format!(
                "JWT header Base64 解析失败: {e}"
            )));
        }

        let decoded_payload = match URL_SAFE_NO_PAD.decode(payload_segment) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(OAuthError::InvalidToken(format!(
                    "JWT payload Base64 解析失败: {e}"
                )));
            }
        };

        match serde_json::from_slice::<OpenAIJWTPayload>(&decoded_payload) {
            Ok(payload) => Ok(payload),
            Err(e) => Err(OAuthError::InvalidToken(format!(
                "JWT payload JSON 解析失败: {e}"
            ))),
        }
    }

    /// 从 `access_token` 中提取 `chatgpt_account_id`
    pub fn extract_chatgpt_account_id(
        &self,
        access_token: &str,
    ) -> Result<Option<String>, OAuthError> {
        match self.extract_openai_info(access_token) {
            Ok(Some(info)) => Ok(Some(info.chatgpt_account_id)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 验证 JWT 是否为 `OpenAI` token
    #[must_use]
    pub fn is_openai_token(&self, access_token: &str) -> bool {
        self.extract_openai_info(access_token)
            .map(|_| true)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::json;

    /// 测试 JWT 解析器创建
    #[test]
    fn test_jwt_parser_creation() {
        let parser = OpenAI;
        assert!(!parser.is_openai_token("invalid_token"));
    }

    /// 测试无效 token 处理
    #[test]
    fn test_invalid_token_handling() {
        let parser = OpenAI;
        let result = parser.extract_chatgpt_account_id("invalid_token");
        assert!(result.is_err());
    }

    /// 测试空 token 处理
    #[test]
    fn test_empty_token_handling() {
        let parser = OpenAI;
        let result = parser.extract_chatgpt_account_id("");
        assert!(result.is_err());
    }

    /// 测试默认解析器
    #[test]
    fn test_default_parser() {
        let parser = OpenAI;
        let result = parser.extract_chatgpt_account_id("invalid_token");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_chatgpt_account_id_success() {
        let parser = OpenAI;

        let header = json!({
            "alg": "RS256",
            "typ": "JWT"
        })
        .to_string();

        let payload = json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc_test_123"
            }
        })
        .to_string();

        let token = format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(header),
            URL_SAFE_NO_PAD.encode(payload),
            "signature"
        );

        let account_id = parser.extract_chatgpt_account_id(&token).unwrap();
        assert_eq!(account_id, Some("acc_test_123".to_string()));
    }
}
