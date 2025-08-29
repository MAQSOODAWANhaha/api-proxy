//! # 服务商识别器
//!
//! 根据请求路径自动识别对应的AI服务提供商类型

use anyhow::Result;
use pingora_proxy::Session;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::cache::UnifiedCacheManager;
use crate::error::ProxyError;

/// 服务商识别器
///
/// 负责根据请求路径识别具体的provider_type，支持缓存以提高性能
pub struct ProviderResolver {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 缓存管理器
    cache: Arc<UnifiedCacheManager>,
    /// 路径映射规则缓存
    path_mappings: tokio::sync::RwLock<HashMap<String, entity::provider_types::Model>>,
}

impl ProviderResolver {
    /// 创建新的服务商识别器
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<UnifiedCacheManager>) -> Self {
        Self {
            db,
            cache,
            path_mappings: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// 根据请求路径识别provider类型
    ///
    /// 支持的路径格式：
    /// - `/openai/v1/chat/completions` -> openai
    /// - `/gemini/v1beta/models/gemini-2.5-flash:generateContent` -> gemini
    /// - `/anthropic/v1/messages` -> anthropic
    /// - `/custom_gemini/v1/models/gemini-2.5-flash:generateContent` -> custom_gemini
    pub async fn resolve_from_request(
        &self,
        session: &Session,
    ) -> Result<entity::provider_types::Model, ProxyError> {
        let req_header = session.req_header();
        let path = req_header.uri.path();

        debug!(path = %path, "Resolving provider from request path");

        // 从路径中提取provider名称
        let provider_name = self.extract_provider_name(path)?;

        debug!(provider_name = %provider_name, "Extracted provider name from path");

        // 尝试从缓存获取
        if let Some(provider) = self.get_cached_provider(&provider_name).await {
            debug!(provider_id = provider.id, provider_name = %provider.name, "Found provider in cache");
            return Ok(provider);
        }

        // 从数据库查询
        let provider = self.query_provider_from_db(&provider_name).await?;

        // 缓存结果
        self.cache_provider(&provider_name, &provider).await;

        debug!(
            provider_id = provider.id,
            provider_name = %provider.name,
            auth_type = %provider.auth_type,
            auth_format = %provider.auth_header_format,
            "Resolved provider from database"
        );

        Ok(provider)
    }

    /// 从路径中提取provider名称
    ///
    /// 路径格式: /{provider_name}/{version}/{endpoint}
    /// 例如: /gemini/v1beta/models/... -> gemini
    fn extract_provider_name(&self, path: &str) -> Result<String, ProxyError> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        if parts.is_empty() || parts[0].is_empty() {
            return Err(ProxyError::authentication(
                "Invalid request path: missing provider",
            ));
        }

        let provider_name = parts[0].to_lowercase();

        // 验证provider名称格式
        if !self.is_valid_provider_name(&provider_name) {
            return Err(ProxyError::authentication(&format!(
                "Invalid provider name: {}",
                provider_name
            )));
        }

        Ok(provider_name)
    }

    /// 验证provider名称格式
    fn is_valid_provider_name(&self, name: &str) -> bool {
        // 允许字母、数字、下划线，长度2-50
        name.len() >= 2
            && name.len() <= 50
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// 从缓存获取provider信息
    async fn get_cached_provider(
        &self,
        provider_name: &str,
    ) -> Option<entity::provider_types::Model> {
        // 首先尝试内存缓存
        {
            let mappings = self.path_mappings.read().await;
            if let Some(provider) = mappings.get(provider_name) {
                return Some(provider.clone());
            }
        }

        // 然后尝试Redis缓存
        let cache_key = format!("provider:name:{}", provider_name);
        if let Ok(Some(cached_json)) = self.cache.get::<String>(&cache_key).await {
            if let Ok(provider) =
                serde_json::from_str::<entity::provider_types::Model>(&cached_json)
            {
                // 同时更新内存缓存
                {
                    let mut mappings = self.path_mappings.write().await;
                    mappings.insert(provider_name.to_string(), provider.clone());
                }
                return Some(provider);
            }
        }

        None
    }

    /// 从数据库查询provider信息
    async fn query_provider_from_db(
        &self,
        provider_name: &str,
    ) -> Result<entity::provider_types::Model, ProxyError> {
        use entity::provider_types::{Column, Entity};
        use sea_orm::{ColumnTrait, QueryFilter};

        let provider = Entity::find()
            .filter(Column::Name.eq(provider_name))
            .filter(Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::database(&format!("Failed to query provider: {}", e)))?
            .ok_or_else(|| {
                ProxyError::authentication(&format!("Provider not found: {}", provider_name))
            })?;

        Ok(provider)
    }

    /// 缓存provider信息
    async fn cache_provider(&self, provider_name: &str, provider: &entity::provider_types::Model) {
        // 更新内存缓存
        {
            let mut mappings = self.path_mappings.write().await;
            mappings.insert(provider_name.to_string(), provider.clone());
        }

        // 更新Redis缓存（5分钟TTL）
        let cache_key = format!("provider:name:{}", provider_name);
        if let Ok(provider_json) = serde_json::to_string(provider) {
            if let Err(e) = self
                .cache
                .set(
                    &cache_key,
                    provider_json,
                    Some(std::time::Duration::from_secs(300)),
                )
                .await
            {
                warn!(error = ?e, cache_key = %cache_key, "Failed to cache provider info");
            }
        }
    }

    /// 清除缓存
    ///
    /// 用于配置更新后强制重新加载
    pub async fn clear_cache(&self) {
        // 清除内存缓存
        {
            let mut mappings = self.path_mappings.write().await;
            mappings.clear();
        }

        // 清除Redis缓存（使用通配符模式）
        // TODO: 实现通配符删除逻辑，暂时跳过
        // if let Err(e) = self.cache.delete_pattern("provider:name:*").await {
        //     warn!(error = ?e, "Failed to clear provider cache");
        // }
        warn!("Provider cache pattern deletion not implemented yet");
    }

    /// 预热缓存
    ///
    /// 启动时加载所有活跃的provider到缓存中
    pub async fn warmup_cache(&self) -> Result<(), ProxyError> {
        use entity::provider_types::{Column, Entity};
        use sea_orm::{ColumnTrait, QueryFilter};

        debug!("Starting provider cache warmup");

        let providers = Entity::find()
            .filter(Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                ProxyError::database(&format!("Failed to load providers for cache warmup: {}", e))
            })?;

        let mut count = 0;
        {
            let mut mappings = self.path_mappings.write().await;
            for provider in providers {
                mappings.insert(provider.name.clone(), provider);
                count += 1;
            }
        }

        debug!(count = count, "Provider cache warmup completed");
        Ok(())
    }

    /// 获取所有支持的provider名称
    pub async fn get_supported_providers(&self) -> Vec<String> {
        let mappings = self.path_mappings.read().await;
        mappings.keys().cloned().collect()
    }
}
