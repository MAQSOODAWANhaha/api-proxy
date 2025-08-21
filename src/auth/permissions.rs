//! # 权限管理
//!
//! 定义系统中的权限和角色

use serde::{Deserialize, Serialize};
use std::fmt;

/// 权限枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // === 代理服务权限 ===
    /// 基础 API 使用权限
    UseApi,
    /// 使用 OpenAI API
    UseOpenAI,
    /// 使用 Anthropic API  
    UseAnthropic,
    /// 使用 Google Gemini API
    UseGemini,
    /// 使用所有 AI 提供商
    UseAllProviders,

    // === 管理权限 ===
    /// 查看用户信息
    ViewUsers,
    /// 管理用户
    ManageUsers,
    /// 查看 API 密钥
    ViewApiKeys,
    /// 管理 API 密钥
    ManageApiKeys,
    /// 查看统计信息
    ViewStatistics,
    /// 管理统计信息
    ManageStatistics,

    // === 系统权限 ===
    /// 查看系统健康状态
    ViewHealth,
    /// 管理系统配置
    ManageConfig,
    /// 管理服务器
    ManageServer,
    /// 查看日志
    ViewLogs,
    /// 管理日志
    ManageLogs,

    // === 特殊权限 ===
    /// 超级管理员权限
    SuperAdmin,
    /// 内部服务调用
    InternalService,
}

impl Permission {
    /// 获取权限的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::UseApi => "use_api",
            Permission::UseOpenAI => "use_openai",
            Permission::UseAnthropic => "use_anthropic",
            Permission::UseGemini => "use_gemini",
            Permission::UseAllProviders => "use_all_providers",
            Permission::ViewUsers => "view_users",
            Permission::ManageUsers => "manage_users",
            Permission::ViewApiKeys => "view_api_keys",
            Permission::ManageApiKeys => "manage_api_keys",
            Permission::ViewStatistics => "view_statistics",
            Permission::ManageStatistics => "manage_statistics",
            Permission::ViewHealth => "view_health",
            Permission::ManageConfig => "manage_config",
            Permission::ManageServer => "manage_server",
            Permission::ViewLogs => "view_logs",
            Permission::ManageLogs => "manage_logs",
            Permission::SuperAdmin => "super_admin",
            Permission::InternalService => "internal_service",
        }
    }

    /// 从字符串解析权限
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "use_api" => Some(Permission::UseApi),
            "use_openai" => Some(Permission::UseOpenAI),
            "use_anthropic" => Some(Permission::UseAnthropic),
            "use_gemini" => Some(Permission::UseGemini),
            "use_all_providers" => Some(Permission::UseAllProviders),
            "view_users" => Some(Permission::ViewUsers),
            "manage_users" => Some(Permission::ManageUsers),
            "view_api_keys" => Some(Permission::ViewApiKeys),
            "manage_api_keys" => Some(Permission::ManageApiKeys),
            "view_statistics" => Some(Permission::ViewStatistics),
            "manage_statistics" => Some(Permission::ManageStatistics),
            "view_health" => Some(Permission::ViewHealth),
            "manage_config" => Some(Permission::ManageConfig),
            "manage_server" => Some(Permission::ManageServer),
            "view_logs" => Some(Permission::ViewLogs),
            "manage_logs" => Some(Permission::ManageLogs),
            "super_admin" => Some(Permission::SuperAdmin),
            "internal_service" => Some(Permission::InternalService),
            _ => None,
        }
    }

    /// 获取所有权限列表
    pub fn all() -> Vec<Permission> {
        vec![
            Permission::UseApi,
            Permission::UseOpenAI,
            Permission::UseAnthropic,
            Permission::UseGemini,
            Permission::UseAllProviders,
            Permission::ViewUsers,
            Permission::ManageUsers,
            Permission::ViewApiKeys,
            Permission::ManageApiKeys,
            Permission::ViewStatistics,
            Permission::ManageStatistics,
            Permission::ViewHealth,
            Permission::ManageConfig,
            Permission::ManageServer,
            Permission::ViewLogs,
            Permission::ManageLogs,
            Permission::SuperAdmin,
            Permission::InternalService,
        ]
    }

    /// 获取权限的描述
    pub fn description(&self) -> &'static str {
        match self {
            Permission::UseApi => "基础 API 使用权限",
            Permission::UseOpenAI => "使用 OpenAI API",
            Permission::UseAnthropic => "使用 Anthropic Claude API",
            Permission::UseGemini => "使用 Google Gemini API",
            Permission::UseAllProviders => "使用所有 AI 提供商",
            Permission::ViewUsers => "查看用户信息",
            Permission::ManageUsers => "管理用户账户",
            Permission::ViewApiKeys => "查看 API 密钥",
            Permission::ManageApiKeys => "管理 API 密钥",
            Permission::ViewStatistics => "查看使用统计",
            Permission::ManageStatistics => "管理统计数据",
            Permission::ViewHealth => "查看系统健康状态",
            Permission::ManageConfig => "管理系统配置",
            Permission::ManageServer => "管理服务器",
            Permission::ViewLogs => "查看系统日志",
            Permission::ManageLogs => "管理系统日志",
            Permission::SuperAdmin => "超级管理员权限",
            Permission::InternalService => "内部服务调用权限",
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 角色定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// 超级管理员
    SuperAdmin,
    /// 管理员
    Admin,
    /// 用户管理员
    UserAdmin,
    /// 普通用户
    User,
    /// 只读用户
    ReadOnly,
    /// API 专用用户
    ApiOnly,
    /// 内部服务
    InternalService,
}

impl Role {
    /// 获取角色的权限列表
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            Role::SuperAdmin => Permission::all(),
            Role::Admin => vec![
                Permission::UseAllProviders,
                Permission::ViewUsers,
                Permission::ManageUsers,
                Permission::ViewApiKeys,
                Permission::ManageApiKeys,
                Permission::ViewStatistics,
                Permission::ManageStatistics,
                Permission::ViewHealth,
                Permission::ManageConfig,
                Permission::ViewLogs,
            ],
            Role::UserAdmin => vec![
                Permission::UseAllProviders,
                Permission::ViewUsers,
                Permission::ManageUsers,
                Permission::ViewApiKeys,
                Permission::ViewStatistics,
                Permission::ViewHealth,
            ],
            Role::User => vec![
                Permission::UseOpenAI,
                Permission::UseAnthropic,
                Permission::UseGemini,
                Permission::ViewApiKeys,
                Permission::ViewStatistics,
                Permission::ViewHealth,
            ],
            Role::ReadOnly => vec![
                Permission::ViewApiKeys,
                Permission::ViewStatistics,
                Permission::ViewHealth,
            ],
            Role::ApiOnly => vec![
                Permission::UseOpenAI,
                Permission::UseAnthropic,
                Permission::UseGemini,
            ],
            Role::InternalService => vec![
                Permission::InternalService,
                Permission::UseAllProviders,
                Permission::ViewHealth,
            ],
        }
    }

    /// 获取角色的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::SuperAdmin => "super_admin",
            Role::Admin => "admin",
            Role::UserAdmin => "user_admin",
            Role::User => "user",
            Role::ReadOnly => "read_only",
            Role::ApiOnly => "api_only",
            Role::InternalService => "internal_service",
        }
    }

    /// 从字符串解析角色
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "super_admin" => Some(Role::SuperAdmin),
            "admin" => Some(Role::Admin),
            "user_admin" => Some(Role::UserAdmin),
            "user" => Some(Role::User),
            "read_only" => Some(Role::ReadOnly),
            "api_only" => Some(Role::ApiOnly),
            "internal_service" => Some(Role::InternalService),
            _ => None,
        }
    }

    /// 获取角色的描述
    pub fn description(&self) -> &'static str {
        match self {
            Role::SuperAdmin => "超级管理员 - 拥有所有权限",
            Role::Admin => "管理员 - 拥有大部分管理权限",
            Role::UserAdmin => "用户管理员 - 负责用户管理",
            Role::User => "普通用户 - 使用 AI 服务",
            Role::ReadOnly => "只读用户 - 只能查看信息",
            Role::ApiOnly => "API 专用用户 - 只能调用 AI API",
            Role::InternalService => "内部服务 - 系统内部调用",
        }
    }

    /// 检查角色是否有特定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions().contains(permission)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 权限检查器
#[derive(Debug, Clone)]
pub struct PermissionChecker {
    /// 用户权限列表
    permissions: Vec<Permission>,
}

impl PermissionChecker {
    /// 创建权限检查器
    pub fn new(permissions: Vec<Permission>) -> Self {
        Self { permissions }
    }

    /// 从角色创建权限检查器
    pub fn from_role(role: &Role) -> Self {
        Self::new(role.permissions())
    }

    /// 检查是否有指定权限
    pub fn has(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission) || self.permissions.contains(&Permission::SuperAdmin)
    }

    /// 检查是否有任意权限
    pub fn has_any(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has(p))
    }

    /// 检查是否有所有权限
    pub fn has_all(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has(p))
    }

    /// 检查路径权限
    pub fn can_access_path(&self, path: &str, method: &str) -> bool {
        // 健康检查端点
        if path == "/health" {
            return self.has(&Permission::ViewHealth);
        }

        // API 路径权限检查
        if path.starts_with("/api/") {
            return match path {
                p if p.starts_with("/api/users") => match method {
                    "GET" => self.has(&Permission::ViewUsers),
                    "POST" | "PUT" | "DELETE" => self.has(&Permission::ManageUsers),
                    _ => false,
                },
                p if p.starts_with("/api/keys") => match method {
                    "GET" => self.has(&Permission::ViewApiKeys),
                    "POST" | "PUT" | "DELETE" => self.has(&Permission::ManageApiKeys),
                    _ => false,
                },
                p if p.starts_with("/api/stats") => match method {
                    "GET" => self.has(&Permission::ViewStatistics),
                    "POST" | "PUT" | "DELETE" => self.has(&Permission::ManageStatistics),
                    _ => false,
                },
                p if p.starts_with("/api/config") => self.has(&Permission::ManageConfig),
                _ => self.has(&Permission::SuperAdmin),
            };
        }

        // AI 服务路径权限检查
        if path.starts_with("/v1/") {
            return self.has_any(&[Permission::UseOpenAI, Permission::UseAllProviders]);
        }

        if path.contains("anthropic") {
            return self.has_any(&[Permission::UseAnthropic, Permission::UseAllProviders]);
        }

        if path.contains("gemini") || path.contains("google") {
            return self.has_any(&[Permission::UseGemini, Permission::UseAllProviders]);
        }

        // 默认拒绝访问
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_string_conversion() {
        let permission = Permission::UseOpenAI;
        assert_eq!(permission.as_str(), "use_openai");
        assert_eq!(
            Permission::from_str("use_openai"),
            Some(Permission::UseOpenAI)
        );
    }

    #[test]
    fn test_role_permissions() {
        let admin = Role::Admin;
        assert!(admin.has_permission(&Permission::ManageUsers));
        assert!(admin.has_permission(&Permission::ViewHealth));
        assert!(!admin.has_permission(&Permission::SuperAdmin));

        let user = Role::User;
        assert!(user.has_permission(&Permission::UseOpenAI));
        assert!(!user.has_permission(&Permission::ManageUsers));
    }

    #[test]
    fn test_permission_checker() {
        let checker = PermissionChecker::from_role(&Role::Admin);
        assert!(checker.has(&Permission::ManageUsers));
        assert!(checker.has_any(&[Permission::ViewUsers, Permission::ManageConfig]));
        assert!(!checker.has(&Permission::SuperAdmin));
    }

    #[test]
    fn test_path_permission_checking() {
        let admin_checker = PermissionChecker::from_role(&Role::Admin);
        assert!(admin_checker.can_access_path("/api/users", "GET"));
        assert!(admin_checker.can_access_path("/api/users", "POST"));
        assert!(admin_checker.can_access_path("/v1/chat/completions", "POST"));

        let user_checker = PermissionChecker::from_role(&Role::User);
        assert!(!user_checker.can_access_path("/api/users", "POST"));
        assert!(user_checker.can_access_path("/v1/chat/completions", "POST"));
        assert!(user_checker.can_access_path("/health", "GET"));
    }
}
