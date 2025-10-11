//! # 用户角色定义
//!
//! 定义系统中的基本用户角色

use serde::{Deserialize, Serialize};
use std::fmt;

/// 用户角色枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserRole {
    /// 管理员
    Admin,
    /// 普通用户
    RegularUser,
}

impl UserRole {
    /// 获取角色的字符串表示
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::RegularUser => "regular_user",
        }
    }

    /// 从字符串解析角色
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "regular_user" => Some(Self::RegularUser),
            _ => None,
        }
    }

    /// 检查是否为管理员
    #[must_use]
    pub const fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }

    /// 获取角色的描述
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Admin => "管理员 - 拥有所有管理权限",
            Self::RegularUser => "普通用户 - 使用 AI 服务",
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Invalid user role: {s}"))
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_string_conversion() {
        let role = UserRole::Admin;
        assert_eq!(role.as_str(), "admin");
        assert_eq!(UserRole::parse("admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("regular_user"), Some(UserRole::RegularUser));
    }

    #[test]
    fn test_is_admin() {
        assert!(UserRole::Admin.is_admin());
        assert!(!UserRole::RegularUser.is_admin());
    }
}
