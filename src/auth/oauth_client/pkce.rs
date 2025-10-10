//! # PKCE (Proof Key for Code Exchange) 安全机制
//!
//! 实现RFC 7636定义的PKCE扩展，为OAuth 2.0公共客户端提供额外的安全保护
//! `PKCE通过Code Verifier和Code Challenge机制防止授权码拦截攻击`
//!
//! ## 核心原理
//! 1. 生成随机的Code Verifier（43-128个字符）
//! 2. `通过SHA256哈希生成Code Challenge`
//! 3. 授权请求时发送Code Challenge
//! 4. 令牌交换时发送Code Verifier进行验证

use base64::engine::{Engine, general_purpose::URL_SAFE_NO_PAD};
use rand::{Rng, distributions::Alphanumeric};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// PKCE Code Verifier长度范围
const MIN_CODE_VERIFIER_LENGTH: usize = 43;
const MAX_CODE_VERIFIER_LENGTH: usize = 128;
const DEFAULT_CODE_VERIFIER_LENGTH: usize = 64;

/// PKCE错误类型
#[derive(Debug, thiserror::Error)]
pub enum PkceError {
    #[error("Invalid code verifier length: {0}. Must be between {1} and {2}")]
    InvalidVerifierLength(usize, usize, usize),

    #[error("Invalid code verifier format: contains non-ASCII characters")]
    InvalidVerifierFormat,

    #[error("Code challenge verification failed")]
    VerificationFailed,

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

/// PKCE Code Verifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkceVerifier {
    value: String,
}

impl PkceVerifier {
    #[must_use]
    pub fn new() -> Self {
        Self::with_length(DEFAULT_CODE_VERIFIER_LENGTH)
    }

    #[must_use]
            pub fn with_length(length: usize) -> Self {        assert!(
            (MIN_CODE_VERIFIER_LENGTH..=MAX_CODE_VERIFIER_LENGTH).contains(&length),
            "Code verifier length must be between {MIN_CODE_VERIFIER_LENGTH} and {MAX_CODE_VERIFIER_LENGTH}"
        );

        let mut rng = rand::thread_rng();
        let verifier: String = (0..length)
            .map(|_| rng.sample(Alphanumeric) as char)
            .collect();

        Self { value: verifier }
    }

    /// 从现有字符串创建Code Verifier
    pub fn from_string(value: String) -> Result<Self, PkceError> {
        Self::validate_verifier(&value)?;
        Ok(Self { value })
    }

    /// 获取Code Verifier的字符串值
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// 获取Code Verifier的字符串值（消费自身）
    #[must_use]
    pub fn into_string(self) -> String {
        self.value
    }

    /// 生成对应的Code Challenge
    #[must_use]
    pub fn create_challenge(&self) -> PkceChallenge {
        PkceChallenge::from_verifier(self)
    }

    /// 验证Code Verifier格式
    fn validate_verifier(verifier: &str) -> Result<(), PkceError> {
        let len = verifier.len();
        if !(MIN_CODE_VERIFIER_LENGTH..=MAX_CODE_VERIFIER_LENGTH).contains(&len) {
            return Err(PkceError::InvalidVerifierLength(
                len,
                MIN_CODE_VERIFIER_LENGTH,
                MAX_CODE_VERIFIER_LENGTH,
            ));
        }

        // 验证字符集：只能包含[A-Z] [a-z] [0-9] - . _ ~
        if !verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
        {
            return Err(PkceError::InvalidVerifierFormat);
        }

        Ok(())
    }
}

impl Default for PkceVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// PKCE Code Challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkceChallenge {
    value: String,
    method: ChallengeMethod,
}

/// Code Challenge方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ChallengeMethod {
    /// SHA256哈希方法（推荐）
    #[serde(rename = "S256")]
    #[default]
    S256,
    /// 明文方法（不推荐，仅在不支持SHA256时使用）
    #[serde(rename = "plain")]
    Plain,
}

impl ChallengeMethod {
    /// 转换为字符串
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::S256 => "S256",
            Self::Plain => "plain",
        }
    }
}



impl PkceChallenge {
    /// 从`Code Verifier`生成`Code Challenge`（使用SHA256）
    #[must_use]
    pub fn from_verifier(verifier: &PkceVerifier) -> Self {
        Self::from_verifier_with_method(verifier, ChallengeMethod::S256)
    }

    /// 从`Code Verifier`生成指定方法的`Code Challenge`
    #[must_use]
    pub fn from_verifier_with_method(verifier: &PkceVerifier, method: ChallengeMethod) -> Self {
        let value = match method {
            ChallengeMethod::S256 => {
                let mut hasher = Sha256::new();
                hasher.update(verifier.as_str().as_bytes());
                let hash = hasher.finalize();
                URL_SAFE_NO_PAD.encode(hash)
            }
            ChallengeMethod::Plain => verifier.as_str().to_string(),
        };

        Self { value, method }
    }

    #[must_use]
    /// 获取Code Challenge的字符串值
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// 获取Challenge方法
    #[must_use]
    pub const fn method(&self) -> ChallengeMethod {
        self.method
    }

    /// 获取Challenge方法的字符串表示
    #[must_use]
    pub fn method_str(&self) -> &'static str {
        self.method.as_str()
    }

    /// 验证`Code Verifier`是否匹配此`Challenge`
    pub fn verify(&self, verifier: &PkceVerifier) -> Result<bool, PkceError> {
        let expected_challenge = Self::from_verifier_with_method(verifier, self.method);
        Ok(self.value == expected_challenge.value)
    }
}

/// PKCE参数对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkceParams {
    pub verifier: PkceVerifier,
    pub challenge: PkceChallenge,
}

impl PkceParams {
    #[must_use]
    pub fn new() -> Self {
        let verifier = PkceVerifier::new();
        let challenge = verifier.create_challenge();
        Self {
            verifier,
            challenge,
        }
    }

    /// 生成指定长度的PKCE参数对
    pub fn with_length(length: usize) -> Self {
        let verifier = PkceVerifier::with_length(length);
        let challenge = verifier.create_challenge();
        Self {
            verifier,
            challenge,
        }
    }

    /// 验证`Code Verifier`是否匹配`Challenge`
    pub fn verify(&self) -> Result<bool, PkceError> {
        self.challenge.verify(&self.verifier)
    }

    /// 获取用于授权请求的参数
    #[must_use]
    pub fn authorization_params(&self) -> Vec<(&'static str, String)> {
        vec![
            ("code_challenge", self.challenge.as_str().to_string()),
            (
                "code_challenge_method",
                self.challenge.method_str().to_string(),
            ),
        ]
    }

    /// 获取用于令牌交换的参数
    #[must_use]
    pub fn token_params(&self) -> Vec<(&'static str, String)> {
        vec![("code_verifier", self.verifier.as_str().to_string())]
    }
}

impl Default for PkceParams {
    fn default() -> Self {
        Self::new()
    }
}

/// PKCE工具函数
pub struct PkceUtils;

impl PkceUtils {
    /// 生成符合RFC 7636规范的随机Code Verifier
    #[must_use]
    pub fn generate_code_verifier() -> String {
        PkceVerifier::new().into_string()
    }

    /// 生成指定长度的Code Verifier
    pub fn generate_code_verifier_with_length(length: usize) -> Result<String, PkceError> {
        if !(MIN_CODE_VERIFIER_LENGTH..=MAX_CODE_VERIFIER_LENGTH).contains(&length) {
            return Err(PkceError::InvalidVerifierLength(
                length,
                MIN_CODE_VERIFIER_LENGTH,
                MAX_CODE_VERIFIER_LENGTH,
            ));
        }
        Ok(PkceVerifier::with_length(length).into_string())
    }

    /// 从`Code Verifier`生成`Code Challenge`（SHA256方法）
    pub fn generate_code_challenge(code_verifier: &str) -> Result<String, PkceError> {
        let verifier = PkceVerifier::from_string(code_verifier.to_string())?;
        Ok(verifier.create_challenge().as_str().to_string())
    }

    /// 验证`Code Verifier`和`Code Challenge`是否匹配
    pub fn verify_challenge(
        code_verifier: &str,
        code_challenge: &str,
        method: Option<ChallengeMethod>,
    ) -> Result<bool, PkceError> {
        let verifier = PkceVerifier::from_string(code_verifier.to_string())?;
        let challenge =
            PkceChallenge::from_verifier_with_method(&verifier, method.unwrap_or_default());
        Ok(challenge.as_str() == code_challenge)
    }

    /// 检查Code Verifier是否符合规范
    #[must_use]
    pub fn is_valid_code_verifier(code_verifier: &str) -> bool {
        PkceVerifier::validate_verifier(code_verifier).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_verifier_generation() {
        let verifier = PkceVerifier::new();
        assert!(verifier.as_str().len() >= MIN_CODE_VERIFIER_LENGTH);
        assert!(verifier.as_str().len() <= MAX_CODE_VERIFIER_LENGTH);
        assert!(PkceUtils::is_valid_code_verifier(verifier.as_str()));
    }

    #[test]
    fn test_code_challenge_generation() {
        let verifier = PkceVerifier::new();
        let challenge = verifier.create_challenge();

        assert_eq!(challenge.method(), ChallengeMethod::S256);
        assert!(!challenge.as_str().is_empty());
    }

    #[test]
    fn test_pkce_verification() {
        let params = PkceParams::new();
        assert!(params.verify().unwrap());
    }

    #[test]
    fn test_challenge_verification() {
        let verifier = PkceVerifier::new();
        let challenge = verifier.create_challenge();

        assert!(challenge.verify(&verifier).unwrap());

        let wrong_verifier = PkceVerifier::new();
        assert!(!challenge.verify(&wrong_verifier).unwrap());
    }

    #[test]
    fn test_invalid_verifier_length() {
        let result = PkceVerifier::from_string("short".to_string());
        assert!(matches!(
            result,
            Err(PkceError::InvalidVerifierLength(_, _, _))
        ));
    }

    #[test]
    fn test_authorization_params() {
        let params = PkceParams::new();
        let auth_params = params.authorization_params();

        assert_eq!(auth_params.len(), 2);
        assert_eq!(auth_params[0].0, "code_challenge");
        assert_eq!(auth_params[1].0, "code_challenge_method");
        assert_eq!(auth_params[1].1, "S256");
    }

    #[test]
    fn test_token_params() {
        let params = PkceParams::new();
        let token_params = params.token_params();

        assert_eq!(token_params.len(), 1);
        assert_eq!(token_params[0].0, "code_verifier");
    }

    #[test]
    fn test_utils_functions() {
        let verifier = PkceUtils::generate_code_verifier();
        assert!(PkceUtils::is_valid_code_verifier(&verifier));

        let challenge = PkceUtils::generate_code_challenge(&verifier).unwrap();
        assert!(!challenge.is_empty());

        assert!(
            PkceUtils::verify_challenge(&verifier, &challenge, Some(ChallengeMethod::S256))
                .unwrap()
        );
    }
}
