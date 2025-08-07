//! # 提供商解析服务
//! 
//! 负责解析提供商名称到ProviderId的映射，替换所有硬编码逻辑

use std::sync::Arc;
use std::collections::HashMap;
use sea_orm::{DatabaseConnection, EntityTrait, ColumnTrait, QueryFilter};
use crate::error::{ProxyError, Result};
use crate::proxy::upstream::ProviderId;
use entity::provider_types::{self, Entity as ProviderTypes};

/// 提供商解析服务
pub struct ProviderResolver {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 提供商名称到ID的缓存映射
    cache: tokio::sync::RwLock<HashMap<String, ProviderId>>,
}

impl ProviderResolver {
    /// 创建新的提供商解析服务
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
    
    /// 根据提供商名称解析为ProviderId
    /// 
    /// 支持以下查询方式：
    /// - 精确匹配name字段 (如：openai, gemini, claude)
    /// - 不区分大小写匹配display_name字段 (如：OpenAI, Gemini, Claude)
    pub async fn resolve_provider(&self, provider_name: &str) -> Result<ProviderId> {
        let normalized_name = provider_name.to_lowercase();
        
        // 首先检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(provider_id) = cache.get(&normalized_name) {
                return Ok(*provider_id);
            }
        }
        
        // 如果缓存中没有，从数据库查询
        let provider_id = self.fetch_from_database(&normalized_name).await?;
        
        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(normalized_name, provider_id);
        }
        
        Ok(provider_id)
    }
    
    /// 从数据库获取提供商ID
    async fn fetch_from_database(&self, normalized_name: &str) -> Result<ProviderId> {
        // 尝试精确匹配name字段
        if let Some(provider) = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(normalized_name))
            .filter(provider_types::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::internal(format!("Database query error: {}", e)))?
        {
            return Ok(ProviderId::from_database_id(provider.id));
        }
        
        // 获取所有活跃提供商进行灵活匹配
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::internal(format!("Database query error: {}", e)))?;
            
        // 尝试多种匹配策略
        for provider in &providers {
            // 1. display_name精确匹配（不区分大小写）
            if provider.display_name.to_lowercase() == normalized_name {
                return Ok(ProviderId::from_database_id(provider.id));
            }
            
            // 2. display_name中包含查询词（处理"OpenAI ChatGPT" -> "openai"的情况）
            let display_lower = provider.display_name.to_lowercase();
            if display_lower.contains(&normalized_name) && normalized_name.len() >= 3 {
                return Ok(ProviderId::from_database_id(provider.id));
            }
            
            // 3. 查询词包含display_name中的关键词（处理"google gemini" -> "gemini"的情况）
            let display_words: Vec<&str> = display_lower.split_whitespace().collect();
            for word in display_words {
                if word.len() >= 3 && normalized_name.contains(word) {
                    return Ok(ProviderId::from_database_id(provider.id));
                }
            }
        }
        
        // 获取所有活跃提供商的名称用于错误消息
        let available_providers = providers.iter()
            .map(|p| format!("{} ({})", p.display_name, p.name))
            .collect::<Vec<_>>()
            .join(", ");
        
        Err(ProxyError::config(format!(
            "Unknown or inactive provider: '{}'. Available providers: {}", 
            normalized_name, available_providers
        )))
    }
    
    /// 刷新缓存，重新从数据库加载所有提供商
    pub async fn refresh_cache(&self) -> Result<()> {
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::internal(format!("Database query error: {}", e)))?;
            
        let mut cache = self.cache.write().await;
        cache.clear();
        
        for provider in &providers {
            let provider_id = ProviderId::from_database_id(provider.id);
            
            // 缓存name字段（内部标识符）
            cache.insert(provider.name.to_lowercase(), provider_id);
            
            // 缓存display_name的各种变体
            let display_lower = provider.display_name.to_lowercase();
            cache.insert(display_lower.clone(), provider_id);
            
            // 缓存display_name中的单词（如"google"、"gemini"等）
            for word in display_lower.split_whitespace() {
                if word.len() >= 3 {  // 只缓存长度>=3的词避免过度匹配
                    cache.insert(word.to_string(), provider_id);
                }
            }
        }
        
        tracing::info!("Provider cache refreshed with {} entries", cache.len());
        Ok(())
    }
    
    /// 获取所有可用的提供商
    pub async fn get_all_providers(&self) -> Result<Vec<(String, ProviderId)>> {
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::internal(format!("Database query error: {}", e)))?;
            
        Ok(providers.into_iter()
            .map(|p| (p.name, ProviderId::from_database_id(p.id)))
            .collect())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_provider_resolver_creation() {
        // 基本的单元测试占位符
        // 实际测试需要数据库连接，应该在集成测试中进行
        assert!(true);
    }
}