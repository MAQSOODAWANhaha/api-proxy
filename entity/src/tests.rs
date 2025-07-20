//! # 实体定义测试
//!
//! 测试所有 Sea-ORM 实体定义的正确性

#[cfg(test)]
mod tests {
    use crate::{
        users, user_sessions, user_audit_logs, provider_types,
        user_provider_keys, user_service_apis, api_health_status,
        request_statistics, daily_statistics
    };
    use sea_orm::Set;

    #[tokio::test]
    async fn test_entity_creation() {
        // 测试实体可以正常创建
        let user = users::ActiveModel {
            username: Set("test_user".to_string()),
            email: Set("test@example.com".to_string()),
            password_hash: Set("hash123".to_string()),
            salt: Set("salt123".to_string()),
            is_active: Set(true),
            is_admin: Set(false),
            ..Default::default()
        };

        assert_eq!(user.username.as_ref(), "test_user");
        assert_eq!(user.email.as_ref(), "test@example.com");
        assert_eq!(user.is_active.as_ref(), &true);
    }

    #[tokio::test]
    async fn test_provider_type_creation() {
        // 测试 AI 服务提供商类型实体
        let provider = provider_types::ActiveModel {
            name: Set("openai".to_string()),
            display_name: Set("OpenAI ChatGPT".to_string()),
            base_url: Set("api.openai.com".to_string()),
            api_format: Set("openai".to_string()),
            default_model: Set(Some("gpt-3.5-turbo".to_string())),
            is_active: Set(true),
            ..Default::default()
        };

        assert_eq!(provider.name.as_ref(), "openai");
        assert_eq!(provider.display_name.as_ref(), "OpenAI ChatGPT");
        assert_eq!(provider.is_active.as_ref(), &true);
    }

    #[tokio::test]
    async fn test_user_provider_key_creation() {
        // 测试用户内部代理商API密钥池实体
        let user_key = user_provider_keys::ActiveModel {
            user_id: Set(1),
            provider_type_id: Set(1),
            api_key: Set("sk-test123".to_string()),
            name: Set("我的OpenAI密钥".to_string()),
            weight: Set(Some(1)),
            is_active: Set(true),
            ..Default::default()
        };

        assert_eq!(user_key.user_id.as_ref(), &1);
        assert_eq!(user_key.api_key.as_ref(), "sk-test123");
        assert_eq!(user_key.name.as_ref(), "我的OpenAI密钥");
    }

    #[tokio::test]
    async fn test_user_service_api_creation() {
        // 测试用户对外服务API密钥实体
        let service_api = user_service_apis::ActiveModel {
            user_id: Set(1),
            provider_type_id: Set(1),
            api_key: Set("proxy-key-123".to_string()),
            api_secret: Set("secret-456".to_string()),
            name: Set(Some("我的代理API".to_string())),
            scheduling_strategy: Set(Some("round_robin".to_string())),
            is_active: Set(true),
            ..Default::default()
        };

        assert_eq!(service_api.user_id.as_ref(), &1);
        assert_eq!(service_api.api_key.as_ref(), "proxy-key-123");
        assert_eq!(service_api.scheduling_strategy.as_ref(), &Some("round_robin".to_string()));
    }

    #[tokio::test]
    async fn test_request_statistics_creation() {
        // 测试请求统计实体
        let stats = request_statistics::ActiveModel {
            user_service_api_id: Set(1),
            user_provider_key_id: Set(Some(1)),
            request_id: Set(Some("req-123".to_string())),
            method: Set("POST".to_string()),
            path: Set(Some("/v1/chat/completions".to_string())),
            status_code: Set(Some(200)),
            response_time_ms: Set(Some(150)),
            tokens_total: Set(Some(100)),
            ..Default::default()
        };

        assert_eq!(stats.user_service_api_id.as_ref(), &1);
        assert_eq!(stats.method.as_ref(), "POST");
        assert_eq!(stats.status_code.as_ref(), &Some(200));
        assert_eq!(stats.response_time_ms.as_ref(), &Some(150));
    }

    #[test]
    fn test_all_entities_compile() {
        // 确保所有实体都能编译通过
        println!("✅ 所有实体定义编译通过");
        println!("- Users: {}", std::any::type_name::<users::Entity>());
        println!("- UserSessions: {}", std::any::type_name::<user_sessions::Entity>());
        println!("- UserAuditLogs: {}", std::any::type_name::<user_audit_logs::Entity>());
        println!("- ProviderTypes: {}", std::any::type_name::<provider_types::Entity>());
        println!("- UserProviderKeys: {}", std::any::type_name::<user_provider_keys::Entity>());
        println!("- UserServiceApis: {}", std::any::type_name::<user_service_apis::Entity>());
        println!("- ApiHealthStatus: {}", std::any::type_name::<api_health_status::Entity>());
        println!("- RequestStatistics: {}", std::any::type_name::<request_statistics::Entity>());
        println!("- DailyStatistics: {}", std::any::type_name::<daily_statistics::Entity>());
    }
}