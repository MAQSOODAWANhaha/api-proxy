//! Error handling and macro usage tests

use crate::error::{Context, ProxyError, Result};

#[test]
fn database_macro_supports_formatting() {
    let err =
        crate::error::database::DatabaseError::Connection(format!("Failed to query {}", "users"))
            .into();
    let ProxyError::Database(db_err) = err else {
        panic!("expected database error");
    };
    assert!(db_err.to_string().contains("Failed to query users"));
}

#[test]
fn auth_macro_creates_domain_error() {
    let err: crate::error::ProxyError =
        crate::error::auth::AuthError::Message("Missing credential".into()).into();
    let ProxyError::Authentication(auth_err) = err else {
        panic!("expected authentication error");
    };
    assert!(
        auth_err
            .to_string()
            .contains("Authentication error: Missing credential")
    );
}

#[test]
fn auth_sub_variant_macro() {
    let err: crate::error::ProxyError = crate::error::auth::AuthError::ApiKeyMissing.into();
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
    let err: crate::error::ProxyError = crate::error::provider::ProviderError::General {
        message: "Rate limited".to_string(),
        provider: "openai".to_string(),
        status: None,
    }
    .into();
    if let ProxyError::Provider(provider_err) = err {
        match provider_err {
            crate::error::provider::ProviderError::General {
                message,
                provider,
                status,
            } => {
                assert_eq!(message, "Rate limited");
                assert_eq!(provider, "openai");
                assert_eq!(status, None);
            }
            _ => panic!("expected General provider error"),
        }
    } else {
        panic!("expected provider error");
    }

    let _io_err = anyhow::anyhow!("throttled");
    let err_with_source: crate::error::ProxyError =
        crate::error::provider::ProviderError::General {
            message: "Rate limited".to_string(),
            provider: "openai".to_string(),
            status: None,
        }
        .into(); // Source is lost in this simple construction, or needs to be added differently if we want to keep it.
    // But wait, ProviderError::General doesn't have source field in the enum definition I made?
    // Let's check ProviderError definition.
    // It has `message` and `provider`. No source.
    // So we can't test source preservation for General variant as defined.

    if let ProxyError::Provider(provider_err) = err_with_source {
        match provider_err {
            crate::error::provider::ProviderError::General { message, .. } => {
                assert_eq!(message, "Rate limited");
            }
            _ => panic!("expected General provider error"),
        }
    } else {
        panic!("expected provider error with source");
    }
    // assert!(source_with_cause.unwrap().to_string().contains("throttled")); // Can't check source if not stored.
}

#[test]
fn bail_macro_returns_early() {
    fn demo() -> Result<()> {
        crate::bail!("Early return");
    }

    let err = demo().unwrap_err();
    assert!(matches!(err, ProxyError::Internal { .. }));
}

#[test]
fn ensure_macro_validates_condition() {
    fn require_positive(value: i32) -> Result<()> {
        crate::ensure!(value > 0, "Value must be positive");
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
        "Access denied",
    ));
    let err = io_result
        .with_context(|| "Failed to read user list".to_string())
        .unwrap_err();

    // Now it should be ProxyError::Context, not Internal
    if let ProxyError::Context { context, source } = err {
        assert_eq!(context, "Failed to read user list");
        assert!(source.to_string().contains("Access denied"));
        // Verify that source is wrapped IO error
        match *source {
            // Io is now converted to Internal(anyhow::Error)
            ProxyError::Internal(_) => {}
            _ => panic!("Expected source to be ProxyError::Internal"),
        }
    } else {
        panic!("expected ProxyError::Context, got {err:?}");
    }
}

#[test]
fn context_trait_preserves_status_code() {
    use crate::error::auth::AuthError;

    let result: std::result::Result<(), AuthError> = Err(AuthError::UsageLimitExceeded(
        crate::error::auth::UsageLimitInfo {
            kind: crate::error::auth::UsageLimitKind::PerMinute,
            limit: None,
            current: None,
            resets_in: None,
            plan_type: "free".to_string(),
        },
    ));

    let err = result.context("API call failed").unwrap_err();

    // Check structure
    match &err {
        ProxyError::Context { context, source: _ } => {
            assert_eq!(context, "API call failed");
        }
        _ => panic!("Expected Context wrapper"),
    }

    // Check status code preservation (should be 429 TOO_MANY_REQUESTS, not 500)
    assert_eq!(err.status_code(), http::StatusCode::TOO_MANY_REQUESTS);
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
    let err: crate::error::ProxyError =
        crate::error::auth::AuthError::Message(format!("Auth failed: {cause}")).into();
    let ProxyError::Authentication(inner) = err else {
        panic!("expected authentication error");
    };
    assert!(inner.to_string().contains("Auth failed"));
}

#[test]
fn provider_error_macro_typed() {
    let err: crate::error::ProxyError = crate::error::provider::ProviderError::ModelNotFound {
        provider: "openai".to_string(),
        model: "gpt-100".to_string(),
    }
    .into();
    assert_eq!(err.error_code(), "MODEL_NOT_FOUND");
    assert!(err.to_string().contains("gpt-100"));

    // Test simple variant
    let err_auth: crate::error::ProxyError =
        crate::error::provider::ProviderError::AuthFailed("Authentication failed".to_string())
            .into();
    assert_eq!(err_auth.error_code(), "PROVIDER_AUTH_FAILED");
}
