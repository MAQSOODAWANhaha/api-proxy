//! # 配置加密模块
//!
//! 处理敏感配置信息的加密和解密

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::{Engine as _, engine::general_purpose};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 加密的配置值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedValue {
    /// Base64编码的加密数据
    pub data: String,
    /// Base64编码的随机数
    pub nonce: String,
}

/// 配置加密器
pub struct ConfigCrypto {
    cipher: Aes256Gcm,
}

impl ConfigCrypto {
    /// 创建新的配置加密器
    #[must_use]
    pub fn new(key: &[u8; 32]) -> Self {
        let key: [u8; 32] = *key;
        let key = key.into();
        let cipher = Aes256Gcm::new(&key);
        Self { cipher }
    }

    /// 从环境变量或默认值创建加密器
    pub fn from_env() -> crate::error::Result<Self> {
        let key_str = std::env::var("PROXY_CONFIG_KEY").unwrap_or_else(|_| {
            "default_key_please_change_in_production_environment_32bytes".to_string()
        });

        if key_str.len() != 64 {
            return Err(crate::error::ProxyError::config(
                "配置加密密钥必须是64个字符的十六进制字符串（32字节）",
            ));
        }

        let key_bytes = hex::decode(&key_str)
            .map_err(|e| crate::error::ProxyError::config_with_source("配置加密密钥格式错误", e))?;

        if key_bytes.len() != 32 {
            return Err(crate::error::ProxyError::config("配置加密密钥必须是32字节"));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        Ok(Self::new(&key))
    }

    /// 加密字符串
    pub fn encrypt(&self, plaintext: &str) -> crate::error::Result<EncryptedValue> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| {
                crate::error::ProxyError::config_with_source(
                    "配置加密失败",
                    anyhow::anyhow!("AES-GCM encryption failed: {e}"),
                )
            })?;

        Ok(EncryptedValue {
            data: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(nonce),
        })
    }

    /// 解密字符串
    pub fn decrypt(&self, encrypted: &EncryptedValue) -> crate::error::Result<String> {
        let ciphertext = general_purpose::STANDARD
            .decode(&encrypted.data)
            .map_err(|e| crate::error::ProxyError::config_with_source("加密数据格式错误", e))?;

        let nonce_bytes = general_purpose::STANDARD
            .decode(&encrypted.nonce)
            .map_err(|e| crate::error::ProxyError::config_with_source("加密随机数格式错误", e))?;

        if nonce_bytes.len() != 12 {
            return Err(crate::error::ProxyError::config("加密随机数长度错误"));
        }

        let nonce_bytes: [u8; 12] = nonce_bytes.try_into().unwrap();
        let nonce = nonce_bytes.into();

        let plaintext = self
            .cipher
            .decrypt(&nonce, ciphertext.as_ref())
            .map_err(|e| {
                crate::error::ProxyError::config_with_source(
                    "配置解密失败",
                    anyhow::anyhow!("AES-GCM decryption failed: {e}"),
                )
            })?;

        String::from_utf8(plaintext).map_err(|e| {
            crate::error::ProxyError::config_with_source("解密后的数据不是有效的UTF-8字符串", e)
        })
    }

    /// 生成新的加密密钥
    #[must_use]
    pub fn generate_key() -> String {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        hex::encode(key)
    }
}

/// 敏感配置字段定义
#[derive(Debug, Clone)]
pub struct SensitiveFields {
    /// 需要加密的字段路径
    fields: HashMap<String, bool>,
}

impl SensitiveFields {
    /// 创建新的敏感字段配置
    #[must_use]
    pub fn new() -> Self {
        let mut fields = HashMap::new();

        // 默认敏感字段
        fields.insert("database.password".to_string(), true);
        fields.insert("redis.password".to_string(), true);
        fields.insert("tls.private_key".to_string(), true);
        fields.insert("providers.*.api_key".to_string(), true);
        fields.insert("auth.jwt_secret".to_string(), true);

        Self { fields }
    }

    /// 检查字段是否敏感
    #[must_use]
    pub fn is_sensitive(&self, field_path: &str) -> bool {
        // 直接匹配
        if self.fields.contains_key(field_path) {
            return true;
        }

        // 通配符匹配
        for pattern in self.fields.keys() {
            if Self::matches_pattern(pattern, field_path) {
                return true;
            }
        }

        false
    }

    /// 模式匹配
    fn matches_pattern(pattern: &str, field_path: &str) -> bool {
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return field_path.starts_with(prefix) && field_path.ends_with(suffix);
            }
        }
        false
    }

    /// 添加敏感字段
    pub fn add_sensitive_field(&mut self, field_path: String) {
        self.fields.insert(field_path, true);
    }
}

impl Default for SensitiveFields {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_crypto_encrypt_decrypt() {
        let key = [0u8; 32];
        let crypto = ConfigCrypto::new(&key);

        let plaintext = "sensitive_api_key_12345";
        let encrypted = crypto.encrypt(plaintext).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_generate_key() {
        let key1 = ConfigCrypto::generate_key();
        let key2 = ConfigCrypto::generate_key();

        assert_eq!(key1.len(), 64); // 32 bytes in hex
        assert_eq!(key2.len(), 64);
        assert_ne!(key1, key2); // Should be different
    }

    #[test]
    fn test_sensitive_fields() {
        let fields = SensitiveFields::new();

        assert!(fields.is_sensitive("database.password"));
        assert!(fields.is_sensitive("providers.openai.api_key"));
        assert!(fields.is_sensitive("providers.gemini.api_key"));
        assert!(!fields.is_sensitive("server.port"));
        assert!(!fields.is_sensitive("database.url"));
    }

    #[test]
    fn test_add_sensitive_field() {
        let mut fields = SensitiveFields::new();
        fields.add_sensitive_field("custom.secret".to_string());

        assert!(fields.is_sensitive("custom.secret"));
    }
}
