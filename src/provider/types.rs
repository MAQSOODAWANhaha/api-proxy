use crate::error::{AuthResult, ProxyError};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderType {
    OpenAI,
    Gemini,
    Anthropic,
    Custom(String),
}

impl ProviderType {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::OpenAI => "openai",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic",
            Self::Custom(name) => name.as_str(),
        }
    }

    #[must_use]
    pub const fn db_name(&self) -> &str {
        self.as_str()
    }

    fn normalize(input: &str) -> &str {
        input.split(':').next().unwrap_or(input)
    }

    pub fn parse(name: &str) -> AuthResult<Self> {
        let normalized = Self::normalize(name);
        match normalized {
            "openai" | "chatgpt" => Ok(Self::OpenAI),
            "google" | "gemini" => Ok(Self::Gemini),
            "anthropic" | "claude" => Ok(Self::Anthropic),
            other => Ok(Self::Custom(other.to_string())),
        }
    }
}

impl FromStr for ProviderType {
    type Err = ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

pub fn provider_type_from_name(provider_name: &str) -> AuthResult<ProviderType> {
    ProviderType::parse(provider_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_provider_aliases() {
        assert_eq!(ProviderType::parse("openai").unwrap(), ProviderType::OpenAI);
        assert_eq!(
            ProviderType::parse("claude").unwrap(),
            ProviderType::Anthropic
        );
        assert_eq!(
            ProviderType::parse("gemini:oauth").unwrap(),
            ProviderType::Gemini
        );
        assert_eq!(
            ProviderType::parse("some-new-provider").unwrap(),
            ProviderType::Custom("some-new-provider".to_string())
        );
    }
}
