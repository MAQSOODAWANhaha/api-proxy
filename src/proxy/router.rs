//! # 智能路由系统
//!
//! 实现高级路由功能，包括路径匹配、服务发现、负载均衡策略

use crate::config::AppConfig;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    /// 路径模式 (支持正则表达式)
    pub pattern: String,
    /// 编译后的正则表达式
    #[serde(skip)]
    pub regex: Option<Regex>,
    /// 目标服务提供商
    pub provider: String,
    /// 优先级 (数字越大优先级越高)
    pub priority: u32,
    /// 是否启用
    pub enabled: bool,
    /// HTTP 方法限制 (None 表示允许所有方法)
    pub methods: Option<Vec<String>>,
    /// 请求头匹配条件
    pub headers: Option<HashMap<String, String>>,
    /// 路由权重 (用于负载均衡)
    pub weight: u32,
    /// 描述
    pub description: String,
}

impl RouteRule {
    /// 创建新的路由规则
    pub fn new(pattern: &str, provider: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(Self {
            pattern: pattern.to_string(),
            regex: Some(regex),
            provider: provider.to_string(),
            priority: 100,
            enabled: true,
            methods: None,
            headers: None,
            weight: 100,
            description: format!("Route to {}", provider),
        })
    }

    /// 检查请求是否匹配此路由规则
    pub fn matches(&self, path: &str, method: &str, headers: &HashMap<String, String>) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查路径匹配
        if let Some(ref regex) = self.regex {
            if !regex.is_match(path) {
                return false;
            }
        }

        // 检查 HTTP 方法
        if let Some(ref allowed_methods) = self.methods {
            if !allowed_methods
                .iter()
                .any(|m| m.eq_ignore_ascii_case(method))
            {
                return false;
            }
        }

        // 检查请求头
        if let Some(ref required_headers) = self.headers {
            for (key, value) in required_headers {
                if let Some(header_value) = headers.get(key) {
                    if header_value != value {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        true
    }
}

/// 路由决策结果
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// 选中的提供商
    pub provider: String,
    /// 匹配的路由规则
    pub rule: RouteRule,
    /// 路由权重
    pub weight: u32,
    /// 路由说明
    pub reason: String,
}

/// 智能路由器
pub struct SmartRouter {
    /// 路由规则列表 (按优先级排序)
    rules: Vec<RouteRule>,
    /// 默认提供商
    default_provider: String,
    /// 配置
    config: Arc<AppConfig>,
}

impl SmartRouter {
    /// 创建新的智能路由器
    pub fn new(config: Arc<AppConfig>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut router = Self {
            rules: Vec::new(),
            default_provider: "OpenAI".to_string(),
            config,
        };

        // 加载默认路由规则
        router.load_default_rules()?;

        Ok(router)
    }

    /// 加载默认路由规则
    fn load_default_rules(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // OpenAI API 路由
        let openai_rules = vec![
            RouteRule::new(r"^/v1/chat/completions", "OpenAI")?,
            RouteRule::new(r"^/v1/completions", "OpenAI")?,
            RouteRule::new(r"^/v1/embeddings", "OpenAI")?,
            RouteRule::new(r"^/v1/images/generations", "OpenAI")?,
            RouteRule::new(r"^/v1/audio/transcriptions", "OpenAI")?,
            RouteRule::new(r"^/v1/models", "OpenAI")?,
            // 通用 OpenAI 路由 (优先级较低)
            {
                let mut rule = RouteRule::new(r"^/v1/.*", "OpenAI")?;
                rule.priority = 50;
                rule.description = "Generic OpenAI API endpoint".to_string();
                rule
            },
        ];

        // Anthropic Claude API 路由
        let anthropic_rules = vec![
            RouteRule::new(r"^/anthropic/v1/messages", "Anthropic")?,
            RouteRule::new(r"^/anthropic/v1/complete", "Anthropic")?,
            {
                let mut rule = RouteRule::new(r"^/anthropic/.*", "Anthropic")?;
                rule.priority = 50;
                rule.description = "Generic Anthropic API endpoint".to_string();
                rule
            },
        ];

        // Google Gemini API 路由
        let gemini_rules = vec![
            RouteRule::new(r"^/gemini/v1/models/.*/generateContent", "GoogleGemini")?,
            RouteRule::new(r"^/gemini/v1/models", "GoogleGemini")?,
            RouteRule::new(r"^/google/ai/.*", "GoogleGemini")?,
            {
                let mut rule = RouteRule::new(r"^/gemini/.*", "GoogleGemini")?;
                rule.priority = 50;
                rule.description = "Generic Google Gemini API endpoint".to_string();
                rule
            },
        ];

        // 管理 API 路由
        let management_rules = vec![
            {
                let mut rule = RouteRule::new(r"^/api/.*", "Management")?;
                rule.priority = 200; // 高优先级
                rule.description = "Management API endpoint".to_string();
                rule
            },
            {
                let mut rule = RouteRule::new(r"^/admin/.*", "Management")?;
                rule.priority = 200;
                rule.description = "Admin API endpoint".to_string();
                rule
            },
            {
                let mut rule = RouteRule::new(r"^/health", "Management")?;
                rule.priority = 250; // 最高优先级
                rule.methods = Some(vec!["GET".to_string(), "HEAD".to_string()]);
                rule.description = "Health check endpoint".to_string();
                rule
            },
        ];

        // 添加所有规则
        for rule in openai_rules {
            self.add_rule(rule);
        }
        for rule in anthropic_rules {
            self.add_rule(rule);
        }
        for rule in gemini_rules {
            self.add_rule(rule);
        }
        for rule in management_rules {
            self.add_rule(rule);
        }

        Ok(())
    }

    /// 添加路由规则
    pub fn add_rule(&mut self, rule: RouteRule) {
        self.rules.push(rule);
        // 按优先级排序 (高优先级在前)
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 路由请求
    pub fn route(
        &self,
        path: &str,
        method: &str,
        headers: &HashMap<String, String>,
    ) -> RouteDecision {
        // 遍历路由规则，找到第一个匹配的
        for rule in &self.rules {
            if rule.matches(path, method, headers) {
                return RouteDecision {
                    provider: rule.provider.clone(),
                    rule: rule.clone(),
                    weight: rule.weight,
                    reason: format!(
                        "Matched rule: {} (priority: {})",
                        rule.description, rule.priority
                    ),
                };
            }
        }

        // 如果没有匹配的规则，使用默认提供商
        RouteDecision {
            provider: self.default_provider.clone(),
            rule: RouteRule {
                pattern: ".*".to_string(),
                regex: None,
                provider: self.default_provider.clone(),
                priority: 0,
                enabled: true,
                methods: None,
                headers: None,
                weight: 100,
                description: "Default fallback rule".to_string(),
            },
            weight: 100,
            reason: "No matching rule found, using default provider".to_string(),
        }
    }

    /// 获取指定提供商的所有路由规则
    pub fn get_rules_for_provider(&self, provider: &str) -> Vec<&RouteRule> {
        self.rules
            .iter()
            .filter(|rule| rule.provider == provider)
            .collect()
    }

    /// 获取所有路由规则
    pub fn get_all_rules(&self) -> &Vec<RouteRule> {
        &self.rules
    }

    /// 启用/禁用路由规则
    pub fn toggle_rule(&mut self, pattern: &str, enabled: bool) -> bool {
        for rule in &mut self.rules {
            if rule.pattern == pattern {
                rule.enabled = enabled;
                return true;
            }
        }
        false
    }

    /// 获取路由统计信息
    pub fn get_statistics(&self) -> RouterStatistics {
        let total_rules = self.rules.len();
        let enabled_rules = self.rules.iter().filter(|r| r.enabled).count();
        let providers: std::collections::HashSet<String> =
            self.rules.iter().map(|r| r.provider.clone()).collect();

        RouterStatistics {
            total_rules,
            enabled_rules,
            disabled_rules: total_rules - enabled_rules,
            providers: providers.into_iter().collect(),
            default_provider: self.default_provider.clone(),
        }
    }
}

/// 路由器统计信息
#[derive(Debug, Serialize)]
pub struct RouterStatistics {
    /// 总规则数
    pub total_rules: usize,
    /// 启用的规则数
    pub enabled_rules: usize,
    /// 禁用的规则数
    pub disabled_rules: usize,
    /// 支持的提供商列表
    pub providers: Vec<String>,
    /// 默认提供商
    pub default_provider: String,
}
