//! 错误处理与宏用法测试

use crate::error::{Context, ProxyError, Result};
use std::error::Error;

#[test]
fn internal_macro_creates_structured_error() {
    let err = crate::error!(Internal, "系统故障");
    match err {
        ProxyError::Internal { message, source } => {
            assert_eq!(message, "系统故障");
            assert!(source.is_none());
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn internal_macro_with_source_preserves_cause() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
    let err = crate::error!(Internal, "配置加载失败", io_err);
    match err {
        ProxyError::Internal { message, source } => {
            assert_eq!(message, "配置加载失败");
            let cause = source.expect("source should be recorded");
            assert!(
                cause.to_string().contains("missing file"),
                "unexpected source: {cause:?}"
            );
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn database_macro_supports_formatting() {
    let err = crate::error!(Database, format!("Failed to query {}", "users"));
    let ProxyError::Database(db_err) = err else {
        panic!("expected database error");
    };
    assert!(db_err.to_string().contains("Failed to query users"));
}

#[test]
fn auth_macro_creates_domain_error() {
    let err = crate::error!(Authentication, "缺少凭据");
    let ProxyError::Authentication(auth_err) = err else {
        panic!("expected authentication error");
    };
    assert_eq!(auth_err.to_string(), "Authentication error: 缺少凭据");
}

#[test]
fn auth_sub_variant_macro() {
    let err = crate::error!(Auth, ApiKeyMissing);
    let ProxyError::Authentication(auth_err) = err else {
        panic!("expected authentication error");
    };
    assert!(matches!(
        auth_err,
        crate::error::auth::AuthError::ApiKeyMissing
    ));
}

#[test]
fn provider_macro_records_provider_name() {
    let err = crate::error!(Provider, message = "Rate limited", provider = "openai");
    let ProxyError::Provider {
        message,
        provider,
        source,
    } = err
    else {
        panic!("expected provider error");
    };
    assert_eq!(message, "Rate limited");
    assert_eq!(provider, "openai");
    assert!(source.is_none());

    let io_err = std::io::Error::other("throttled");
    let err_with_source = crate::error!(
        Provider,
        message = "Rate limited",
        provider = "openai",
        source = io_err
    );
    let ProxyError::Provider {
        source: source_with_cause,
        ..
    } = err_with_source
    else {
        panic!("expected provider error with source");
    };
    assert!(source_with_cause.unwrap().to_string().contains("throttled"));
}

#[test]
fn bail_macro_returns_early() {
    fn demo() -> Result<()> {
        crate::bail!(Internal, "提前返回");
    }

    let err = demo().unwrap_err();
    assert!(matches!(err, ProxyError::Internal { .. }));
}

#[test]
fn ensure_macro_validates_condition() {
    fn require_positive(value: i32) -> Result<()> {
        crate::ensure!(value > 0, Internal, "值必须大于0");
        Ok(())
    }

    assert!(require_positive(3).is_ok());
    let err = require_positive(-1).unwrap_err();
    assert!(matches!(err, ProxyError::Internal { .. }));
}

#[test]
fn context_trait_wraps_error() {
    let io_result: std::result::Result<(), std::io::Error> = Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "拒绝访问",
    ));
    let err = io_result
        .with_context(|| "读取用户列表失败".to_string())
        .unwrap_err();
    assert!(matches!(err, ProxyError::Internal { .. }));
    assert!(err.to_string().contains("读取用户列表失败"));
    assert!(err.source().is_some());
}

#[test]
fn conversion_from_external_error() {
    let toml_err = toml::from_str::<toml::Value>("invalid = toml = syntax").unwrap_err();
    let proxy_err: ProxyError = toml_err.into();
    assert!(matches!(proxy_err, ProxyError::Config { .. }));
}

#[test]
fn authentication_with_source_helper_captures_cause() {
    let cause = std::io::Error::new(std::io::ErrorKind::InvalidData, "bad token");
    let err = crate::error!(Authentication, format!("认证失败: {}", cause));
    let ProxyError::Authentication(inner) = err else {
        panic!("expected authentication error");
    };
    assert!(inner.to_string().contains("认证失败"));
}
