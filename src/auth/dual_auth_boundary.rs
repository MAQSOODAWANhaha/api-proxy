//! # 双认证机制边界控制
//!
//! 强化管理端(9090)和代理端(8080)的认证边界，防止认证方式混用

use crate::auth::{AuthMethod, AuthResult};
use crate::error::{ProxyError, Result};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lwarn};
use std::collections::HashSet;

/// 端口类型定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortType {
    /// 管理端口 (9090)
    Management = 9090,
    /// 代理端口 (8080)  
    Proxy = 8080,
}

impl PortType {
    /// 从端口号创建端口类型
    pub fn from_port(port: u16) -> Option<Self> {
        match port {
            9090 => Some(Self::Management),
            8080 => Some(Self::Proxy),
            _ => None,
        }
    }

    /// 获取端口号
    pub fn port(&self) -> u16 {
        *self as u16
    }

    /// 获取端口描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::Management => "Management Port",
            Self::Proxy => "Proxy Port",
        }
    }
}

/// 认证边界验证规则
#[derive(Debug, Clone)]
pub struct AuthBoundaryRule {
    /// 端口类型
    pub port_type: PortType,
    /// 允许的认证方法
    pub allowed_methods: HashSet<AuthMethod>,
    /// 禁止的认证方法
    pub forbidden_methods: HashSet<AuthMethod>,
    /// 是否启用严格模式
    pub strict_mode: bool,
}

impl AuthBoundaryRule {
    /// 创建管理端认证规则
    pub fn management_port() -> Self {
        let mut allowed_methods = HashSet::new();
        allowed_methods.insert(AuthMethod::Jwt);
        allowed_methods.insert(AuthMethod::BasicAuth);
        allowed_methods.insert(AuthMethod::OAuth);
        allowed_methods.insert(AuthMethod::Internal); // 允许内部调用

        let mut forbidden_methods = HashSet::new();
        forbidden_methods.insert(AuthMethod::ApiKey); // 管理端禁止直接API密钥认证

        Self {
            port_type: PortType::Management,
            allowed_methods,
            forbidden_methods,
            strict_mode: true,
        }
    }

    /// 创建代理端认证规则
    pub fn proxy_port() -> Self {
        let mut allowed_methods = HashSet::new();
        allowed_methods.insert(AuthMethod::ApiKey); // 代理端只允许API密钥认证

        let mut forbidden_methods = HashSet::new();
        forbidden_methods.insert(AuthMethod::Jwt);
        forbidden_methods.insert(AuthMethod::BasicAuth);
        forbidden_methods.insert(AuthMethod::OAuth);

        Self {
            port_type: PortType::Proxy,
            allowed_methods,
            forbidden_methods,
            strict_mode: true,
        }
    }

    /// 验证认证方法是否被允许
    pub fn is_method_allowed(&self, method: &AuthMethod) -> bool {
        if self.forbidden_methods.contains(method) {
            return false;
        }

        if self.strict_mode {
            self.allowed_methods.contains(method)
        } else {
            !self.forbidden_methods.contains(method)
        }
    }

    /// 获取违规说明
    pub fn get_violation_reason(&self, method: &AuthMethod) -> String {
        if self.forbidden_methods.contains(method) {
            format!(
                "{:?} authentication is explicitly forbidden on {} (port {})",
                method,
                self.port_type.description(),
                self.port_type.port()
            )
        } else if !self.allowed_methods.contains(method) {
            format!(
                "{:?} authentication is not allowed on {} (port {}). Allowed methods: {:?}",
                method,
                self.port_type.description(),
                self.port_type.port(),
                self.allowed_methods
            )
        } else {
            "No violation".to_string()
        }
    }
}

/// 双认证边界控制器
///
/// 负责强化双端口认证机制的边界控制，防止认证方式混用
pub struct DualAuthBoundaryController {
    /// 管理端认证规则
    management_rule: AuthBoundaryRule,
    /// 代理端认证规则
    proxy_rule: AuthBoundaryRule,
    /// 是否启用边界检查
    enabled: bool,
    /// 违规计数器
    violation_count: std::sync::atomic::AtomicU64,
}

impl DualAuthBoundaryController {
    /// 创建新的双认证边界控制器
    pub fn new() -> Self {
        Self {
            management_rule: AuthBoundaryRule::management_port(),
            proxy_rule: AuthBoundaryRule::proxy_port(),
            enabled: true,
            violation_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 创建禁用边界检查的控制器（用于测试）
    pub fn disabled() -> Self {
        let mut controller = Self::new();
        controller.enabled = false;
        controller
    }

    /// 验证认证请求是否符合边界规则
    pub fn validate_auth_request(
        &self,
        port: u16,
        method: &AuthMethod,
        request_context: &AuthRequestContext,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let port_type = PortType::from_port(port)
            .ok_or_else(|| ProxyError::authentication(&format!("Unsupported port: {}", port)))?;

        let rule = match port_type {
            PortType::Management => &self.management_rule,
            PortType::Proxy => &self.proxy_rule,
        };

        if !rule.is_method_allowed(method) {
            let violation_reason = rule.get_violation_reason(method);

            // 记录违规
            self.record_violation(port_type, method, &violation_reason, request_context);

            return Err(ProxyError::authentication(&format!(
                "Authentication boundary violation: {}",
                violation_reason
            )));
        }

        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "boundary_validation_passed",
            "Authentication boundary validation passed",
            port = port,
            method = ?method,
            client_ip = request_context.client_ip.as_deref().unwrap_or("unknown")
        );

        Ok(())
    }

    /// 验证认证结果是否符合边界规则
    pub fn validate_auth_result(
        &self,
        port: u16,
        auth_result: &AuthResult,
        request_context: &AuthRequestContext,
    ) -> Result<()> {
        self.validate_auth_request(port, &auth_result.auth_method, request_context)
    }

    /// 记录边界违规
    fn record_violation(
        &self,
        port_type: PortType,
        method: &AuthMethod,
        reason: &str,
        context: &AuthRequestContext,
    ) {
        let count = self
            .violation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        lwarn!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "boundary_violation",
            "Authentication boundary violation detected",
            violation_id = count,
            port = port_type.port(),
            port_type = ?port_type,
            auth_method = ?method,
            client_ip = context.client_ip.as_deref().unwrap_or("unknown"),
            user_agent = context.user_agent.as_deref().unwrap_or("unknown"),
            path = context.path.as_deref().unwrap_or("unknown"),
            reason = reason
        );
    }

    /// 获取违规统计
    pub fn get_violation_stats(&self) -> BoundaryViolationStats {
        BoundaryViolationStats {
            total_violations: self
                .violation_count
                .load(std::sync::atomic::Ordering::Relaxed),
            management_port_violations: 0, // 可以扩展为详细统计
            proxy_port_violations: 0,
            enabled: self.enabled,
        }
    }

    /// 重置违规计数器
    pub fn reset_violation_count(&self) {
        self.violation_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
        ldebug!(
            "system",
            LogStage::Internal,
            LogComponent::Auth,
            "boundary_counter_reset",
            "Authentication boundary violation counter reset"
        );
    }

    /// 启用边界检查
    pub fn enable(&mut self) {
        self.enabled = true;
        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::Auth,
            "boundary_check_enabled",
            "Authentication boundary checking enabled"
        );
    }

    /// 禁用边界检查
    pub fn disable(&mut self) {
        self.enabled = false;
        lwarn!(
            "system",
            LogStage::Configuration,
            LogComponent::Auth,
            "boundary_check_disabled",
            "Authentication boundary checking disabled - this should only be used for testing"
        );
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 获取端口的认证规则
    pub fn get_port_rules(&self, port: u16) -> Option<&AuthBoundaryRule> {
        match PortType::from_port(port) {
            Some(PortType::Management) => Some(&self.management_rule),
            Some(PortType::Proxy) => Some(&self.proxy_rule),
            None => None,
        }
    }

    /// 动态更新认证规则（慎用）
    pub fn update_rule(&mut self, port_type: PortType, rule: AuthBoundaryRule) {
        match port_type {
            PortType::Management => {
                self.management_rule = rule;
                ldebug!(
                    "system",
                    LogStage::Configuration,
                    LogComponent::Auth,
                    "mgmt_rule_updated",
                    "Updated management port authentication rule"
                );
            }
            PortType::Proxy => {
                self.proxy_rule = rule;
                ldebug!(
                    "system",
                    LogStage::Configuration,
                    LogComponent::Auth,
                    "proxy_rule_updated",
                    "Updated proxy port authentication rule"
                );
            }
        }
    }
}

/// 认证请求上下文
#[derive(Debug, Clone)]
pub struct AuthRequestContext {
    /// 客户端IP
    pub client_ip: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 请求路径
    pub path: Option<String>,
    /// 请求时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl AuthRequestContext {
    /// 创建新的认证请求上下文
    pub fn new() -> Self {
        Self {
            client_ip: None,
            user_agent: None,
            path: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 设置客户端IP
    pub fn with_client_ip(mut self, client_ip: String) -> Self {
        self.client_ip = Some(client_ip);
        self
    }

    /// 设置用户代理
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// 设置请求路径
    pub fn with_path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }
}

impl Default for AuthRequestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 边界违规统计
#[derive(Debug, Clone)]
pub struct BoundaryViolationStats {
    /// 总违规数
    pub total_violations: u64,
    /// 管理端口违规数
    pub management_port_violations: u64,
    /// 代理端口违规数
    pub proxy_port_violations: u64,
    /// 是否启用边界检查
    pub enabled: bool,
}

/// 默认的双认证边界控制器实例
static BOUNDARY_CONTROLLER: std::sync::LazyLock<std::sync::RwLock<DualAuthBoundaryController>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(DualAuthBoundaryController::new()));

/// 获取全局边界控制器引用
pub fn get_boundary_controller() -> &'static std::sync::RwLock<DualAuthBoundaryController> {
    &BOUNDARY_CONTROLLER
}

/// 便捷函数：验证认证边界
pub fn validate_auth_boundary(
    port: u16,
    method: &AuthMethod,
    context: &AuthRequestContext,
) -> Result<()> {
    get_boundary_controller()
        .read()
        .unwrap()
        .validate_auth_request(port, method, context)
}

/// 便捷函数：获取违规统计
pub fn get_violation_stats() -> BoundaryViolationStats {
    get_boundary_controller()
        .read()
        .unwrap()
        .get_violation_stats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_type_creation() {
        assert_eq!(PortType::from_port(9090), Some(PortType::Management));
        assert_eq!(PortType::from_port(8080), Some(PortType::Proxy));
        assert_eq!(PortType::from_port(3000), None);
    }

    #[test]
    fn test_management_port_rules() {
        let rule = AuthBoundaryRule::management_port();
        assert!(rule.is_method_allowed(&AuthMethod::Jwt));
        assert!(rule.is_method_allowed(&AuthMethod::BasicAuth));
        assert!(rule.is_method_allowed(&AuthMethod::OAuth));
        assert!(!rule.is_method_allowed(&AuthMethod::ApiKey));
    }

    #[test]
    fn test_proxy_port_rules() {
        let rule = AuthBoundaryRule::proxy_port();
        assert!(rule.is_method_allowed(&AuthMethod::ApiKey));
        assert!(!rule.is_method_allowed(&AuthMethod::Jwt));
        assert!(!rule.is_method_allowed(&AuthMethod::BasicAuth));
        assert!(!rule.is_method_allowed(&AuthMethod::OAuth));
    }

    #[test]
    fn test_boundary_controller_validation() {
        let controller = DualAuthBoundaryController::new();
        let context = AuthRequestContext::new()
            .with_client_ip("127.0.0.1".to_string())
            .with_path("/api/test".to_string());

        // 管理端允许JWT
        assert!(
            controller
                .validate_auth_request(9090, &AuthMethod::Jwt, &context)
                .is_ok()
        );

        // 管理端禁止API密钥
        assert!(
            controller
                .validate_auth_request(9090, &AuthMethod::ApiKey, &context)
                .is_err()
        );

        // 代理端允许API密钥
        assert!(
            controller
                .validate_auth_request(8080, &AuthMethod::ApiKey, &context)
                .is_ok()
        );

        // 代理端禁止JWT
        assert!(
            controller
                .validate_auth_request(8080, &AuthMethod::Jwt, &context)
                .is_err()
        );
    }

    #[test]
    fn test_disabled_controller() {
        let controller = DualAuthBoundaryController::disabled();
        let context = AuthRequestContext::new();

        // 禁用时所有组合都应该通过
        assert!(
            controller
                .validate_auth_request(9090, &AuthMethod::ApiKey, &context)
                .is_ok()
        );
        assert!(
            controller
                .validate_auth_request(8080, &AuthMethod::Jwt, &context)
                .is_ok()
        );
    }

    #[test]
    fn test_violation_counting() {
        let controller = DualAuthBoundaryController::new();
        let context = AuthRequestContext::new();

        let initial_count = controller.get_violation_stats().total_violations;

        // 触发违规
        let _ = controller.validate_auth_request(9090, &AuthMethod::ApiKey, &context);
        let _ = controller.validate_auth_request(8080, &AuthMethod::Jwt, &context);

        let final_count = controller.get_violation_stats().total_violations;
        assert_eq!(final_count, initial_count + 2);
    }

    #[test]
    fn test_auth_request_context() {
        let context = AuthRequestContext::new()
            .with_client_ip("192.168.1.1".to_string())
            .with_user_agent("test-agent".to_string())
            .with_path("/api/endpoint".to_string());

        assert_eq!(context.client_ip.as_ref().unwrap(), "192.168.1.1");
        assert_eq!(context.user_agent.as_ref().unwrap(), "test-agent");
        assert_eq!(context.path.as_ref().unwrap(), "/api/endpoint");
    }

    #[test]
    fn test_global_boundary_controller() {
        let context = AuthRequestContext::new();

        // 测试全局控制器
        let result = validate_auth_boundary(9090, &AuthMethod::Jwt, &context);
        assert!(result.is_ok());

        let stats = get_violation_stats();
        assert!(stats.enabled);
    }
}
