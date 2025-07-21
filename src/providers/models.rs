//! # AI 模型枚举定义
//!
//! 定义各个AI服务提供商支持的模型类型，使用枚举而非硬编码字符串

use serde::{Deserialize, Serialize};
use std::fmt;

/// OpenAI 模型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenAIModel {
    /// GPT-4 最新版本
    #[serde(rename = "gpt-4")]
    Gpt4,
    /// GPT-4 Turbo
    #[serde(rename = "gpt-4-turbo")]
    Gpt4Turbo,
    /// GPT-4 Turbo Preview
    #[serde(rename = "gpt-4-turbo-preview")]
    Gpt4TurboPreview,
    /// GPT-3.5 Turbo
    #[serde(rename = "gpt-3.5-turbo")]
    Gpt35Turbo,
    /// GPT-3.5 Turbo 16K 上下文
    #[serde(rename = "gpt-3.5-turbo-16k")]
    Gpt35Turbo16k,
    /// Text Davinci 003
    #[serde(rename = "text-davinci-003")]
    TextDavinci003,
    /// Text Davinci 002
    #[serde(rename = "text-davinci-002")]
    TextDavinci002,
    /// Code Davinci 002
    #[serde(rename = "code-davinci-002")]
    CodeDavinci002,
}

impl fmt::Display for OpenAIModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model_str = match self {
            Self::Gpt4 => "gpt-4",
            Self::Gpt4Turbo => "gpt-4-turbo",
            Self::Gpt4TurboPreview => "gpt-4-turbo-preview",
            Self::Gpt35Turbo => "gpt-3.5-turbo",
            Self::Gpt35Turbo16k => "gpt-3.5-turbo-16k",
            Self::TextDavinci003 => "text-davinci-003",
            Self::TextDavinci002 => "text-davinci-002",
            Self::CodeDavinci002 => "code-davinci-002",
        };
        write!(f, "{}", model_str)
    }
}

impl OpenAIModel {
    /// 获取所有支持的模型
    pub fn all() -> Vec<Self> {
        vec![
            Self::Gpt4,
            Self::Gpt4Turbo,
            Self::Gpt4TurboPreview,
            Self::Gpt35Turbo,
            Self::Gpt35Turbo16k,
            Self::TextDavinci003,
            Self::TextDavinci002,
            Self::CodeDavinci002,
        ]
    }

    /// 获取默认模型
    pub fn default() -> Self {
        Self::Gpt35Turbo
    }

    /// 从字符串解析模型
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gpt-4" => Some(Self::Gpt4),
            "gpt-4-turbo" => Some(Self::Gpt4Turbo),
            "gpt-4-turbo-preview" => Some(Self::Gpt4TurboPreview),
            "gpt-3.5-turbo" => Some(Self::Gpt35Turbo),
            "gpt-3.5-turbo-16k" => Some(Self::Gpt35Turbo16k),
            "text-davinci-003" => Some(Self::TextDavinci003),
            "text-davinci-002" => Some(Self::TextDavinci002),
            "code-davinci-002" => Some(Self::CodeDavinci002),
            _ => None,
        }
    }
}

/// Google Gemini 模型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiModel {
    /// Gemini 1.5 Pro 最新版本
    #[serde(rename = "gemini-1.5-pro-latest")]
    Gemini15ProLatest,
    /// Gemini 1.5 Flash 最新版本
    #[serde(rename = "gemini-1.5-flash-latest")]
    Gemini15FlashLatest,
    /// Gemini 1.0 Pro 最新版本
    #[serde(rename = "gemini-1.0-pro-latest")]
    Gemini10ProLatest,
    /// Gemini Pro
    #[serde(rename = "gemini-pro")]
    GeminiPro,
    /// Gemini Pro Vision
    #[serde(rename = "gemini-pro-vision")]
    GeminiProVision,
}

impl fmt::Display for GeminiModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model_str = match self {
            Self::Gemini15ProLatest => "gemini-1.5-pro-latest",
            Self::Gemini15FlashLatest => "gemini-1.5-flash-latest",
            Self::Gemini10ProLatest => "gemini-1.0-pro-latest",
            Self::GeminiPro => "gemini-pro",
            Self::GeminiProVision => "gemini-pro-vision",
        };
        write!(f, "{}", model_str)
    }
}

impl GeminiModel {
    /// 获取所有支持的模型
    pub fn all() -> Vec<Self> {
        vec![
            Self::Gemini15ProLatest,
            Self::Gemini15FlashLatest,
            Self::Gemini10ProLatest,
            Self::GeminiPro,
            Self::GeminiProVision,
        ]
    }

    /// 获取默认模型
    pub fn default() -> Self {
        Self::Gemini15ProLatest
    }

    /// 从字符串解析模型
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gemini-1.5-pro-latest" => Some(Self::Gemini15ProLatest),
            "gemini-1.5-flash-latest" => Some(Self::Gemini15FlashLatest),
            "gemini-1.0-pro-latest" => Some(Self::Gemini10ProLatest),
            "gemini-pro" => Some(Self::GeminiPro),
            "gemini-pro-vision" => Some(Self::GeminiProVision),
            _ => None,
        }
    }
}

/// Anthropic Claude 模型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaudeModel {
    /// Claude 3 Opus
    #[serde(rename = "claude-3-opus-20240229")]
    Claude3Opus20240229,
    /// Claude 3 Sonnet
    #[serde(rename = "claude-3-sonnet-20240229")]
    Claude3Sonnet20240229,
    /// Claude 3 Haiku
    #[serde(rename = "claude-3-haiku-20240307")]
    Claude3Haiku20240307,
    /// Claude 3.5 Sonnet
    #[serde(rename = "claude-3-5-sonnet-20241022")]
    Claude35Sonnet20241022,
    /// Claude 3.5 Haiku
    #[serde(rename = "claude-3-5-haiku-20241022")]
    Claude35Haiku20241022,
}

impl fmt::Display for ClaudeModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model_str = match self {
            Self::Claude3Opus20240229 => "claude-3-opus-20240229",
            Self::Claude3Sonnet20240229 => "claude-3-sonnet-20240229",
            Self::Claude3Haiku20240307 => "claude-3-haiku-20240307",
            Self::Claude35Sonnet20241022 => "claude-3-5-sonnet-20241022",
            Self::Claude35Haiku20241022 => "claude-3-5-haiku-20241022",
        };
        write!(f, "{}", model_str)
    }
}

impl ClaudeModel {
    /// 获取所有支持的模型
    pub fn all() -> Vec<Self> {
        vec![
            Self::Claude3Opus20240229,
            Self::Claude3Sonnet20240229,
            Self::Claude3Haiku20240307,
            Self::Claude35Sonnet20241022,
            Self::Claude35Haiku20241022,
        ]
    }

    /// 获取默认模型
    pub fn default() -> Self {
        Self::Claude3Sonnet20240229
    }

    /// 从字符串解析模型
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude-3-opus-20240229" => Some(Self::Claude3Opus20240229),
            "claude-3-sonnet-20240229" => Some(Self::Claude3Sonnet20240229),
            "claude-3-haiku-20240307" => Some(Self::Claude3Haiku20240307),
            "claude-3-5-sonnet-20241022" => Some(Self::Claude35Sonnet20241022),
            "claude-3-5-haiku-20241022" => Some(Self::Claude35Haiku20241022),
            _ => None,
        }
    }
}

/// 统一的AI模型枚举，包含所有提供商的模型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "provider", content = "model")]
pub enum AIModel {
    /// OpenAI 模型
    OpenAI(OpenAIModel),
    /// Google Gemini 模型
    Gemini(GeminiModel),
    /// Anthropic Claude 模型
    Claude(ClaudeModel),
}

impl fmt::Display for AIModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenAI(model) => write!(f, "{}", model),
            Self::Gemini(model) => write!(f, "{}", model),
            Self::Claude(model) => write!(f, "{}", model),
        }
    }
}

impl AIModel {
    /// 从字符串和提供商类型解析模型
    pub fn from_str(provider: &str, model: &str) -> Option<Self> {
        match provider.to_lowercase().as_str() {
            "openai" => OpenAIModel::from_str(model).map(Self::OpenAI),
            "gemini" | "google" => GeminiModel::from_str(model).map(Self::Gemini),
            "claude" | "anthropic" => ClaudeModel::from_str(model).map(Self::Claude),
            _ => None,
        }
    }

    /// 获取提供商名称
    pub fn provider(&self) -> &'static str {
        match self {
            Self::OpenAI(_) => "openai",
            Self::Gemini(_) => "gemini",
            Self::Claude(_) => "claude",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_model_display() {
        assert_eq!(OpenAIModel::Gpt4.to_string(), "gpt-4");
        assert_eq!(OpenAIModel::Gpt35Turbo.to_string(), "gpt-3.5-turbo");
    }

    #[test]
    fn test_openai_model_from_str() {
        assert_eq!(OpenAIModel::from_str("gpt-4"), Some(OpenAIModel::Gpt4));
        assert_eq!(OpenAIModel::from_str("unknown"), None);
    }

    #[test]
    fn test_gemini_model_display() {
        assert_eq!(GeminiModel::Gemini15ProLatest.to_string(), "gemini-1.5-pro-latest");
        assert_eq!(GeminiModel::GeminiPro.to_string(), "gemini-pro");
    }

    #[test]
    fn test_claude_model_display() {
        assert_eq!(ClaudeModel::Claude3Sonnet20240229.to_string(), "claude-3-sonnet-20240229");
        assert_eq!(ClaudeModel::Claude3Opus20240229.to_string(), "claude-3-opus-20240229");
    }

    #[test]
    fn test_ai_model_unified() {
        let openai_model = AIModel::OpenAI(OpenAIModel::Gpt4);
        assert_eq!(openai_model.to_string(), "gpt-4");
        assert_eq!(openai_model.provider(), "openai");

        let gemini_model = AIModel::Gemini(GeminiModel::GeminiPro);
        assert_eq!(gemini_model.to_string(), "gemini-pro");
        assert_eq!(gemini_model.provider(), "gemini");
    }

    #[test]
    fn test_model_serialization() {
        let model = OpenAIModel::Gpt4;
        let json = serde_json::to_string(&model).unwrap();
        assert_eq!(json, "\"gpt-4\"");

        let deserialized: OpenAIModel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, model);
    }

    #[test]
    fn test_all_models() {
        assert!(!OpenAIModel::all().is_empty());
        assert!(!GeminiModel::all().is_empty());
        assert!(!ClaudeModel::all().is_empty());
    }

    #[test]
    fn test_default_models() {
        assert_eq!(OpenAIModel::default(), OpenAIModel::Gpt35Turbo);
        assert_eq!(GeminiModel::default(), GeminiModel::Gemini15ProLatest);
        assert_eq!(ClaudeModel::default(), ClaudeModel::Claude3Sonnet20240229);
    }
}