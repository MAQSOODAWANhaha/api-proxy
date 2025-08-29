//! # 代理端认证模块
//!
//! 专门处理Pingora代理服务的数据库驱动认证逻辑
//! 与management.rs的JWT认证完全独立，各司其职

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use std::time::Duration;

use crate::auth::{ApiKeyManager, AuthUtils};
use crate::cache::UnifiedCacheManager;
use crate::error::ProxyError;
use entity::{
    user_service_apis::{self, Entity as UserServiceApis},
};

/// 代理端认证器
/// 
/// 专门处理代理端的API密钥认证，使用数据库驱动的认证逻辑
/// 包含缓存优化和完整的验证流程，集成共享的API密钥工具
pub struct ProxyAuthenticator {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 统一缓存管理器
    cache: Arc<UnifiedCacheManager>,
    /// 共享的API密钥数据库工具
    api_key_manager: Arc<ApiKeyManager>,
}

/// 代理端认证结果
/// 
/// 包含认证成功后的用户信息和服务商信息
#[derive(Debug, Clone)]
pub struct ProxyAuthResult {
    /// 用户服务API信息
    pub user_api: user_service_apis::Model,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型ID
    pub provider_type_id: i32,
}

impl ProxyAuthenticator {
    /// 创建新的代理端认证器
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        auth_config: Arc<crate::auth::types::AuthConfig>,
    ) -> Self {
        let api_key_manager = Arc::new(ApiKeyManager::new(db.clone(), auth_config));
        
        Self {
            db,
            cache,
            api_key_manager,
        }
    }

    /// 认证API密钥
    /// 
    /// 这是代理端认证的核心方法，负责：
    /// 1. 缓存查询优化
    /// 2. 数据库验证
    /// 3. 过期检查
    /// 4. 活跃状态验证
    /// 
    /// # 参数
    /// - `api_key`: 要验证的API密钥
    /// 
    /// # 返回
    /// - `Ok(ProxyAuthResult)`: 认证成功，包含用户信息
    /// - `Err(ProxyError)`: 认证失败或系统错误
    pub async fn authenticate_api_key(
        &self,
        api_key: &str,
    ) -> Result<ProxyAuthResult, ProxyError> {
        let cache_key = format!("user_service_api:{}", api_key);

        // 首先检查缓存
        if let Ok(Some(user_api)) = self
            .cache
            .provider()
            .get::<user_service_apis::Model>(&cache_key)
            .await
        {
            tracing::debug!(
                api_key_preview = %AuthUtils::sanitize_api_key(api_key),
                user_id = user_api.user_id,
                provider_type_id = user_api.provider_type_id,
                "Found API key in cache"
            );
            
            return Ok(ProxyAuthResult {
                user_id: user_api.user_id,
                provider_type_id: user_api.provider_type_id,
                user_api,
            });
        }

        // 从数据库查询
        let user_api = UserServiceApis::find()
            .filter(user_service_apis::Column::ApiKey.eq(api_key))
            .filter(user_service_apis::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::authentication("Invalid API key"))?;

        // 检查API密钥是否过期
        if let Some(expires_at) = user_api.expires_at {
            if expires_at < Utc::now().naive_utc() {
                tracing::warn!(
                    api_key_preview = %AuthUtils::sanitize_api_key(api_key),
                    user_id = user_api.user_id,
                    expires_at = %expires_at,
                    "API key has expired"
                );
                return Err(ProxyError::authentication("API key expired"));
            }
        }

        // 缓存结果（5分钟TTL）
        let _ = self
            .cache
            .provider()
            .set(&cache_key, &user_api, Some(Duration::from_secs(300)))
            .await;

        tracing::debug!(
            api_key_preview = %AuthUtils::sanitize_api_key(api_key),
            user_id = user_api.user_id,
            provider_type_id = user_api.provider_type_id,
            "API key authenticated from database"
        );

        Ok(ProxyAuthResult {
            user_id: user_api.user_id,
            provider_type_id: user_api.provider_type_id,
            user_api,
        })
    }

    /// 验证API密钥基本格式
    /// 
    /// 在进行数据库查询前的快速格式验证，使用共享的API密钥工具
    /// 
    /// # 参数
    /// - `api_key`: 要验证的API密钥
    /// 
    /// # 返回
    /// - `true`: 格式有效
    /// - `false`: 格式无效
    pub fn validate_api_key_format(&self, api_key: &str) -> bool {
        // 使用共享的ApiKeyManager进行格式验证
        self.api_key_manager.validate_api_key_format(api_key)
    }

    /// 清理API密钥缓存
    /// 
    /// 当API密钥被禁用或删除时，清理对应的缓存
    /// 同时清理共享API密钥管理器的缓存
    /// 
    /// # 参数
    /// - `api_key`: 要清理缓存的API密钥
    pub async fn invalidate_cache(&self, api_key: &str) -> Result<(), ProxyError> {
        let cache_key = format!("user_service_api:{}", api_key);
        
        // 清理代理端缓存
        self.cache
            .provider()
            .delete(&cache_key)
            .await
            .map_err(|e| ProxyError::internal(format!("Cache invalidation failed: {}", e)))?;
        
        // 清理共享API密钥管理器的缓存
        self.api_key_manager.invalidate_api_key_cache(api_key).await;
        
        tracing::debug!(
            api_key_preview = %AuthUtils::sanitize_api_key(api_key),
            "API key cache invalidated from both proxy and shared manager"
        );
        
        Ok(())
    }

    /// 访问共享的API密钥管理器
    /// 
    /// 提供对内部AI提供商密钥管理的访问
    /// 这与代理端的用户服务API认证是互补的功能
    pub fn api_key_manager(&self) -> &Arc<ApiKeyManager> {
        &self.api_key_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_key_format() {
        // 这个测试不需要数据库连接和缓存，可以测试基本的格式验证逻辑
        // 有效格式
        assert!(AuthUtils::is_valid_api_key_format("sk-1234567890abcdef12345"));
        
        // 无效格式
        assert!(!AuthUtils::is_valid_api_key_format("invalid-key"));
        assert!(!AuthUtils::is_valid_api_key_format("sk-short"));
        assert!(!AuthUtils::is_valid_api_key_format("ak-1234567890abcdef12345"));
    }

    // TODO: 添加完整的集成测试，需要设置测试数据库和缓存环境
}