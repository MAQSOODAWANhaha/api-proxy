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
        // OpenAI 使用 RS256 算法签名，我们需要使用公开的验证密钥
        // 对于 OpenAI 的 JWT，我们可以使用空的解码密钥进行无验证解析
        // 因为我们的目标只是提取 payload 中的信息
        let decoding_key = DecodingKey::from_secret(&[]);

        let mut validation = Validation::new(Algorithm::RS256);
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
