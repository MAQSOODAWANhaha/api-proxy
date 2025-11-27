use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManagementError {
    #[error("OAuth key {key_id} 不存在 (user_id={user_id})")]
    ProviderKeyNotFound { key_id: i32, user_id: String },

    #[error("OAuth key {key_id} 类型错误: 期望 {expected}, 实际 {actual}")]
    InvalidKeyAuthType {
        key_id: i32,
        expected: String,
        actual: String,
    },

    #[error("OAuth key {key_id} 缺少 session_id")]
    MissingOAuthSessionId { key_id: i32 },

    #[error("OAuth 会话 {session_id} 未找到或未授权 (user_id={user_id})")]
    OAuthSessionNotFound { session_id: String, user_id: String },

    #[error("OAuth 会话 {session_id} 缺少 access_token")]
    OAuthSessionTokenMissing { session_id: String },

    #[error("Management task `{task}` 未注册")]
    MissingTask { task: &'static str },

    #[error("System metrics collection failed")]
    MetricsUnavailable,
}
