//! # `OpenAI` JWT 解析器
//!
//! 专门用于解析 `OpenAI` `access_token` 中的用户信息
//! 从 JWT payload 中提取 `chatgpt_account_id` 等关键信息

use crate::auth::oauth_client::OAuthError;
use crate::logging::{LogComponent, LogStage};
use crate::{linfo, lwarn};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
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
pub struct JWTParser {
    /// 解码密钥（对于 `OpenAI` JWT，通常使用公开的验证密钥）
    decoding_key: DecodingKey,
    /// 验证配置
    validation: Validation,
}

impl JWTParser {
    /// 创建新的 JWT 解析器
    pub fn new() -> Result<Self, OAuthError> {
        // OpenAI 的 JWT 使用 RS256 算法签名。然而，由于我们在此处仅解码 payload 而不验证签名
        // (通过 insecure_disable_signature_validation)，我们面临一个技术选择。
        // 为了避免 `jsonwebtoken` 库因算法与密钥类型不匹配而抛出 `InvalidKeyFormat` 错误，
        // 我们在此处将验证算法“伪装”为 HS256，以匹配 `DecodingKey::from_secret` 的密钥格式。
        // 因为签名验证已被禁用，所以此处的算法选择仅为满足格式要求，不影响解码 payload 的能力。
        let decoding_key = DecodingKey::from_secret(&[]);

        let mut validation = Validation::new(Algorithm::HS256);
        // 禁用签名验证，因为我们只是解析 payload
        validation.insecure_disable_signature_validation();
        // 禁用过期验证，因为我们需要解析可能过期的 token
        validation.validate_exp = false;
        validation.validate_nbf = false;

        Ok(Self {
            decoding_key,
            validation,
        })
    }

    /// 从 `access_token` 中解析 `OpenAI` 用户信息
    pub fn extract_openai_info(
        &self,
        access_token: &str,
    ) -> Result<Option<OpenAIAuthInfo>, OAuthError> {
        // 解析 JWT token
        let token_data =
            decode::<OpenAIJWTPayload>(access_token, &self.decoding_key, &self.validation)
                .map_err(|e| OAuthError::InvalidToken(format!("JWT 解析失败: {e}")))?;

        // 提取 OpenAI 认证信息
        if let Some(openai_auth) = token_data.claims.openai_auth {
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

impl Default for JWTParser {
    fn default() -> Self {
        Self::new().expect("JWTParser 创建失败")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 JWT 解析器创建
    #[test]
    fn test_jwt_parser_creation() {
        let parser = JWTParser::new();
        assert!(parser.is_ok());
    }

    /// 测试无效 token 处理
    #[test]
    fn test_invalid_token_handling() {
        let parser = JWTParser::new().unwrap();
        let result = parser.extract_chatgpt_account_id("invalid_token");
        assert!(result.is_err());
    }

    /// 测试空 token 处理
    #[test]
    fn test_empty_token_handling() {
        let parser = JWTParser::new().unwrap();
        let result = parser.extract_chatgpt_account_id("");
        assert!(result.is_err());
    }

    /// 测试默认解析器
    #[test]
    fn test_default_parser() {
        let parser = JWTParser::default();
        let result = parser.extract_chatgpt_account_id("invalid_token");
        assert!(result.is_err());
    }
}
