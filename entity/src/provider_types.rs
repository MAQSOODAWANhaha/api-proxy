//! # AI 服务提供商类型实体定义
//!
//! AI 服务提供商类型表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// AI 服务提供商类型实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "provider_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub api_format: String,
    pub default_model: Option<String>,
    pub max_tokens: Option<i32>,
    pub rate_limit: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub health_check_path: Option<String>,
    pub is_active: bool,
    pub config_json: Option<String>,           // JSON 字符串
    pub token_mappings_json: Option<String>,   // Token字段映射配置
    pub model_extraction_json: Option<String>, // 模型提取规则配置
    // 认证配置字段
    pub supported_auth_types: String,          // 支持的认证类型列表 (JSON数组)
    pub auth_header_format: String,            // 认证头格式模板
    pub auth_configs_json: Option<String>,     // 认证配置详情 (JSON对象)
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_provider_keys::Entity")]
    UserProviderKeys,
    #[sea_orm(has_many = "super::user_service_apis::Entity")]
    UserServiceApis,
}

impl Related<super::user_provider_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserProviderKeys.def()
    }
}

impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApis.def()
    }
}


impl ActiveModelBehavior for ActiveModel {}

/// OAuth配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub authorize_url: String,
    pub token_url: String,
    pub redirect_uri: Option<String>,
    pub scopes: String,
    pub pkce_required: bool,
    // 额外参数
    pub access_type: Option<String>,
    pub prompt: Option<String>,
    pub project_id: Option<String>,
    pub response_type: Option<String>,
    pub grant_type: Option<String>,
    // 通用额外参数支持
    #[serde(default)]
    pub extra_params: Option<std::collections::HashMap<String, String>>,
}

/// OAuth配置解析方法
impl Model {
    /// 获取支持的认证类型列表
    pub fn get_supported_auth_types(&self) -> Result<Vec<String>, serde_json::Error> {
        serde_json::from_str(&self.supported_auth_types)
    }

    /// 检查是否支持指定的认证类型
    pub fn supports_auth_type(&self, auth_type: &str) -> bool {
        self.get_supported_auth_types()
            .map(|types| types.contains(&auth_type.to_string()))
            .unwrap_or(false)
    }

    /// 从AuthConfigsJson中提取特定认证类型的配置
    pub fn get_auth_config(&self, auth_type: &str) -> Result<Option<serde_json::Value>, serde_json::Error> {
        if let Some(ref configs_json) = self.auth_configs_json {
            let configs: serde_json::Map<String, serde_json::Value> = serde_json::from_str(configs_json)?;
            Ok(configs.get(auth_type).cloned())
        } else {
            Ok(None)
        }
    }

    /// 获取OAuth配置
    pub fn get_oauth_config(&self, oauth_type: &str) -> Result<Option<OAuthConfig>, serde_json::Error> {
        if let Some(config_value) = self.get_auth_config(oauth_type)? {
            let oauth_config: OAuthConfig = serde_json::from_value(config_value)?;
            Ok(Some(oauth_config))
        } else {
            Ok(None)
        }
    }

    /// 获取统一OAuth配置
    pub fn get_oauth_config_unified(&self) -> Result<Option<OAuthConfig>, serde_json::Error> {
        self.get_oauth_config("oauth")
    }

    /// 获取所有OAuth配置类型
    pub fn get_oauth_types(&self) -> Vec<String> {
        let oauth_types = vec!["oauth"];
        oauth_types.into_iter()
            .filter(|&auth_type| self.supports_auth_type(auth_type))
            .map(|s| s.to_string())
            .collect()
    }

    /// 验证OAuth配置的完整性
    pub fn validate_oauth_config(&self, oauth_type: &str) -> Result<bool, String> {
        match self.get_oauth_config(oauth_type) {
            Ok(Some(config)) => {
                // 检查必需字段
                if config.client_id.is_empty() {
                    return Err("client_id is required".to_string());
                }
                if config.authorize_url.is_empty() {
                    return Err("authorize_url is required".to_string());
                }
                if config.token_url.is_empty() {
                    return Err("token_url is required".to_string());
                }
                
                // 验证URL格式
                if let Err(e) = url::Url::parse(&config.authorize_url) {
                    return Err(format!("Invalid authorize_url: {}", e));
                }
                if let Err(e) = url::Url::parse(&config.token_url) {
                    return Err(format!("Invalid token_url: {}", e));
                }
                
                // 验证redirect_uri（如果存在）
                if let Some(ref redirect_uri) = config.redirect_uri {
                    if !redirect_uri.is_empty() {
                        if let Err(e) = url::Url::parse(redirect_uri) {
                            return Err(format!("Invalid redirect_uri: {}", e));
                        }
                    }
                }
                
                // 验证scopes格式
                if config.scopes.is_empty() {
                    return Err("At least one scope is required".to_string());
                }
                
                // 对于公共客户端（没有client_secret），验证PKCE要求
                if config.client_secret.is_none() && !config.pkce_required {
                    return Err("PKCE is required for public clients (no client_secret)".to_string());
                }
                
                Ok(true)
            }
            Ok(None) => Err(format!("OAuth config for '{}' not found", oauth_type)),
            Err(e) => Err(format!("Failed to parse OAuth config: {}", e)),
        }
    }

    /// 验证所有支持的OAuth配置
    pub fn validate_all_oauth_configs(&self) -> Result<Vec<(String, bool)>, String> {
        let oauth_types = self.get_oauth_types();
        let mut results = Vec::new();
        
        for oauth_type in oauth_types {
            match self.validate_oauth_config(&oauth_type) {
                Ok(is_valid) => {
                    results.push((oauth_type, is_valid));
                }
                Err(e) => {
                    return Err(format!("Validation failed for {}: {}", oauth_type, e));
                }
            }
        }
        
        Ok(results)
    }

    /// 检查OAuth配置是否为公共客户端
    pub fn is_public_oauth_client(&self, oauth_type: &str) -> Result<bool, serde_json::Error> {
        if let Some(config) = self.get_oauth_config(oauth_type)? {
            Ok(config.client_secret.is_none())
        } else {
            Ok(false)
        }
    }

    /// 获取OAuth配置的安全等级
    pub fn get_oauth_security_level(&self, oauth_type: &str) -> Result<String, serde_json::Error> {
        if let Some(config) = self.get_oauth_config(oauth_type)? {
            let security_level = match (config.client_secret.is_some(), config.pkce_required) {
                (true, true) => "HIGH", // 机密客户端 + PKCE
                (true, false) => "MEDIUM", // 机密客户端，无PKCE
                (false, true) => "MEDIUM", // 公共客户端 + PKCE
                (false, false) => "LOW", // 公共客户端，无PKCE
            };
            Ok(security_level.to_string())
        } else {
            Ok("UNKNOWN".to_string())
        }
    }

    /// 替换动态参数
    pub fn replace_dynamic_params(&self, oauth_config: &mut OAuthConfig, params: &std::collections::HashMap<String, String>) {
        // 替换project_id等动态参数
        if let Some(ref mut project_id) = oauth_config.project_id {
            if project_id == "{dynamic_project_id}" {
                if let Some(actual_project_id) = params.get("project_id") {
                    *project_id = actual_project_id.clone();
                }
            }
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: 0,
            name: "unknown".to_string(),
            display_name: "Unknown Provider".to_string(),
            base_url: "".to_string(),
            api_format: "".to_string(),
            default_model: None,
            max_tokens: None,
            rate_limit: None,
            timeout_seconds: None,
            health_check_path: None,
            is_active: false,
            config_json: None,
            token_mappings_json: None,
            model_extraction_json: None,
            supported_auth_types: "[\"api_key\"]".to_string(),
            auth_header_format: "Authorization: Bearer {key}".to_string(),
            auth_configs_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }
}
