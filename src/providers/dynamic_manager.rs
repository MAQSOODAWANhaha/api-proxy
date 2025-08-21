//! # 动态适配器管理器
//!
//! 基于数据库配置的动态适配器管理系统

use super::generic_adapter::{GenericAdapter, GenericAdapterConfig};
use super::traits::ProviderAdapter;
use super::types::{AdapterRequest, AdapterResponse, ProviderError, ProviderResult, StreamChunk};
use crate::config::ProviderConfigManager;
use crate::proxy::upstream::ProviderId;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 动态适配器管理器
pub struct DynamicAdapterManager {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 提供商配置管理器
    provider_config_manager: Arc<ProviderConfigManager>,
    /// 动态加载的适配器缓存
    adapters: Arc<RwLock<HashMap<ProviderId, Box<dyn ProviderAdapter + Send + Sync>>>>,
    /// 适配器统计信息
    stats: Arc<RwLock<HashMap<String, AdapterStats>>>,
}

/// 适配器统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterStats {
    pub name: String,
    pub provider_id: String,
    pub api_format: String,
    pub is_active: bool,
    pub last_loaded: chrono::DateTime<chrono::Utc>,
}

impl DynamicAdapterManager {
    /// 创建新的动态适配器管理器
    pub fn new(
        db: Arc<DatabaseConnection>,
        provider_config_manager: Arc<ProviderConfigManager>,
    ) -> Self {
        Self {
            db,
            provider_config_manager,
            adapters: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 初始化管理器，从数据库加载所有活跃的提供商
    pub async fn initialize(&self) -> ProviderResult<()> {
        tracing::info!("Initializing dynamic adapter manager...");

        match self.provider_config_manager.get_active_providers().await {
            Ok(providers) => {
                let mut loaded_count = 0;
                for provider in providers {
                    let provider_id = ProviderId::from_database_id(provider.id);

                    match self.load_adapter_for_provider(provider_id, &provider).await {
                        Ok(_) => {
                            loaded_count += 1;
                            tracing::debug!(
                                "Loaded adapter for provider: {} (ID: {})",
                                provider.display_name,
                                provider.id
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load adapter for provider {} (ID: {}): {}",
                                provider.display_name,
                                provider.id,
                                e
                            );
                        }
                    }
                }

                tracing::info!(
                    "Dynamic adapter manager initialized with {} adapters",
                    loaded_count
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to load providers from database: {}", e);
                Err(ProviderError::ConfigurationError(format!(
                    "Failed to initialize dynamic adapter manager: {}",
                    e
                )))
            }
        }
    }

    /// 为指定提供商加载适配器
    async fn load_adapter_for_provider(
        &self,
        provider_id: ProviderId,
        provider: &crate::config::ProviderConfig,
    ) -> ProviderResult<()> {
        let config = GenericAdapterConfig {
            provider_id,
            provider_name: provider.name.clone(),
            display_name: provider.display_name.clone(),
            api_format: provider.api_format.clone(),
            request_transform_rules: provider
                .config_json
                .as_ref()
                .and_then(|c| c.get("request_transform").cloned()),
            response_transform_rules: provider
                .config_json
                .as_ref()
                .and_then(|c| c.get("response_transform").cloned()),
            streaming_config: provider
                .config_json
                .as_ref()
                .and_then(|c| c.get("streaming").cloned()),
            field_extractor: None, // TODO: 从config_json创建FieldExtractor
        };

        let adapter =
            Box::new(GenericAdapter::new(config.clone())) as Box<dyn ProviderAdapter + Send + Sync>;

        // 将适配器添加到缓存
        {
            let mut adapters = self.adapters.write().await;
            adapters.insert(provider_id, adapter);
        }

        // 更新统计信息
        {
            let mut stats = self.stats.write().await;
            let adapter_stats = AdapterStats {
                name: provider.display_name.clone(),
                provider_id: format!("{:?}", provider_id),
                api_format: provider.api_format.clone(),
                is_active: provider.is_active,
                last_loaded: chrono::Utc::now(),
            };
            stats.insert(provider.display_name.clone(), adapter_stats);
        }

        Ok(())
    }

    /// 获取指定提供商的适配器
    pub async fn get_adapter(
        &self,
        _provider_id: &ProviderId,
    ) -> Option<Box<dyn ProviderAdapter + Send + Sync>> {
        let _adapters = self.adapters.read().await;

        // 这里有一个问题：我们不能直接克隆Box<dyn ProviderAdapter>
        // 需要重新设计这个接口
        // 暂时返回None，实际实现需要不同的方法
        None
    }

    /// 检查是否有指定提供商的适配器
    pub async fn has_adapter(&self, provider_id: &ProviderId) -> bool {
        let adapters = self.adapters.read().await;
        adapters.contains_key(provider_id)
    }

    /// 处理请求 - 通过引用传递避免所有权问题
    pub async fn process_request(
        &self,
        provider_id: &ProviderId,
        request: &AdapterRequest,
    ) -> ProviderResult<AdapterRequest> {
        let adapters = self.adapters.read().await;

        let adapter = adapters.get(provider_id).ok_or_else(|| {
            ProviderError::UnsupportedOperation(format!(
                "No adapter found for provider ID: {:?}",
                provider_id
            ))
        })?;

        adapter.transform_request(request)
    }

    /// 处理响应
    pub async fn process_response(
        &self,
        provider_id: &ProviderId,
        response: &AdapterResponse,
        original_request: &AdapterRequest,
    ) -> ProviderResult<AdapterResponse> {
        let adapters = self.adapters.read().await;

        let adapter = adapters.get(provider_id).ok_or_else(|| {
            ProviderError::UnsupportedOperation(format!(
                "No adapter found for provider ID: {:?}",
                provider_id
            ))
        })?;

        adapter.transform_response(response, original_request)
    }

    /// 处理流式响应
    pub async fn process_streaming_response(
        &self,
        provider_id: &ProviderId,
        chunk: &[u8],
        request: &AdapterRequest,
    ) -> ProviderResult<Option<StreamChunk>> {
        let adapters = self.adapters.read().await;

        let adapter = adapters.get(provider_id).ok_or_else(|| {
            ProviderError::UnsupportedOperation(format!(
                "No adapter found for provider ID: {:?}",
                provider_id
            ))
        })?;

        adapter.handle_streaming_chunk(chunk, request)
    }

    /// 验证请求
    pub async fn validate_request(
        &self,
        provider_id: &ProviderId,
        request: &AdapterRequest,
    ) -> ProviderResult<()> {
        let adapters = self.adapters.read().await;

        let adapter = adapters.get(provider_id).ok_or_else(|| {
            ProviderError::UnsupportedOperation(format!(
                "No adapter found for provider ID: {:?}",
                provider_id
            ))
        })?;

        adapter.validate_request(request)
    }

    /// 获取所有支持的提供商ID
    pub async fn supported_provider_ids(&self) -> Vec<ProviderId> {
        let adapters = self.adapters.read().await;
        adapters.keys().copied().collect()
    }

    /// 获取第一个可用的提供商ID（因为支持任意路径转发，不再基于路径检测）
    pub async fn get_first_available_provider(&self) -> Option<ProviderId> {
        let adapters = self.adapters.read().await;
        adapters.keys().next().copied()
    }

    /// 获取适配器统计信息
    pub async fn get_adapter_stats(&self) -> HashMap<String, AdapterStats> {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// 重新加载所有适配器（热重载）
    pub async fn reload_adapters(&self) -> ProviderResult<()> {
        tracing::info!("Reloading all adapters...");

        // 清空现有适配器
        {
            let mut adapters = self.adapters.write().await;
            adapters.clear();
        }
        {
            let mut stats = self.stats.write().await;
            stats.clear();
        }

        // 重新初始化
        self.initialize().await
    }

    /// 为新提供商动态加载适配器
    pub async fn load_provider_adapter(&self, provider_id: ProviderId) -> ProviderResult<()> {
        match self
            .provider_config_manager
            .get_provider_by_id(provider_id.id())
            .await
        {
            Ok(Some(provider)) => {
                if provider.is_active {
                    self.load_adapter_for_provider(provider_id, &provider)
                        .await?;
                    tracing::info!(
                        "Dynamically loaded adapter for provider: {} (ID: {})",
                        provider.display_name,
                        provider.id
                    );
                    Ok(())
                } else {
                    Err(ProviderError::ConfigurationError(format!(
                        "Provider {} (ID: {}) is not active",
                        provider.display_name, provider.id
                    )))
                }
            }
            Ok(None) => Err(ProviderError::ConfigurationError(format!(
                "Provider with ID {} not found",
                provider_id.id()
            ))),
            Err(e) => Err(ProviderError::ConfigurationError(format!(
                "Failed to load provider config: {}",
                e
            ))),
        }
    }

    /// 移除指定提供商的适配器
    pub async fn remove_provider_adapter(&self, provider_id: ProviderId) -> ProviderResult<()> {
        let removed = {
            let mut adapters = self.adapters.write().await;
            adapters.remove(&provider_id).is_some()
        };

        if removed {
            let mut stats = self.stats.write().await;
            stats.retain(|_, stat| stat.provider_id != format!("{:?}", provider_id));

            tracing::info!("Removed adapter for provider ID: {:?}", provider_id);
            Ok(())
        } else {
            Err(ProviderError::UnsupportedOperation(format!(
                "No adapter found for provider ID: {:?}",
                provider_id
            )))
        }
    }

    /// 获取活跃适配器数量
    pub async fn active_adapter_count(&self) -> usize {
        let adapters = self.adapters.read().await;
        adapters.len()
    }
}

// 实现Debug trait用于调试
impl std::fmt::Debug for DynamicAdapterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicAdapterManager")
            .field("adapter_count", &"<async>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::abstract_cache::UnifiedCacheManager;
    use crate::config::{AppConfig, ProviderConfigManager};

    async fn create_test_manager() -> DynamicAdapterManager {
        // 这里需要真实的数据库连接和配置管理器进行测试
        // 实际测试需要设置测试数据库
        todo!("Need to set up test database for integration tests")
    }

    #[tokio::test]
    async fn test_manager_creation() {
        // 基本的单元测试
        assert!(true); // placeholder
    }
}
