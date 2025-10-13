//! # 数据驱动字段提取器
//!
//! 基于数据库配置的通用字段提取器，支持JSONPath查询、数学表达式和条件判断

use crate::{
    ldebug,
    logging::{LogComponent, LogStage},
};
use anyhow::{Result, anyhow};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

/// Token字段映射类型
#[derive(Debug, Clone, PartialEq)]
pub enum TokenMapping {
    /// 直接映射字段路径
    Direct {
        path: String,
        fallback: Option<Box<TokenMapping>>,
    },
    /// 数学表达式计算
    Expression {
        formula: String,
        fallback: Option<Box<TokenMapping>>,
    },
    /// 固定默认值
    Default {
        value: Value,
        fallback: Option<Box<TokenMapping>>,
    },
    /// 条件判断映射
    Conditional {
        condition: String,
        true_value: String,
        false_value: Value,
        fallback: Option<Box<TokenMapping>>,
    },
    /// Fallback路径列表（保持向后兼容）
    Fallback {
        paths: Vec<String>,
        fallback: Option<Box<TokenMapping>>,
    },
}

impl TokenMapping {
    /// `从JSON配置解析Token映射`
    pub fn from_json(config: &Value) -> Result<Self> {
        let mapping_type = config
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing or invalid 'type' field"))?;

        match mapping_type {
            "direct" => Self::parse_direct_mapping(config),
            "expression" => Self::parse_expression_mapping(config),
            "default" => Self::parse_default_mapping(config),
            "conditional" => Self::parse_conditional_mapping(config),
            "fallback" => Self::parse_fallback_mapping(config),
            _ => Err(anyhow!("Unknown mapping type: {mapping_type}")),
        }
    }

    fn parse_direct_mapping(config: &Value) -> Result<Self> {
        let path = Self::require_string(config, "path", "Missing 'path' field for direct mapping")?;

        Ok(Self::Direct {
            path: path.to_string(),
            fallback: Self::parse_optional_fallback(config)?,
        })
    }

    fn parse_expression_mapping(config: &Value) -> Result<Self> {
        let formula = Self::require_string(
            config,
            "formula",
            "Missing 'formula' field for expression mapping",
        )?;

        Ok(Self::Expression {
            formula: formula.to_string(),
            fallback: Self::parse_optional_fallback(config)?,
        })
    }

    fn parse_default_mapping(config: &Value) -> Result<Self> {
        let value = config
            .get("value")
            .cloned()
            .ok_or_else(|| anyhow!("Missing 'value' field for default mapping"))?;

        Ok(Self::Default {
            value,
            fallback: Self::parse_optional_fallback(config)?,
        })
    }

    fn parse_conditional_mapping(config: &Value) -> Result<Self> {
        let condition = Self::require_string(
            config,
            "condition",
            "Missing 'condition' field for conditional mapping",
        )?;
        let true_value = Self::require_string(
            config,
            "true_value",
            "Missing 'true_value' field for conditional mapping",
        )?;
        let false_value = config
            .get("false_value")
            .cloned()
            .ok_or_else(|| anyhow!("Missing 'false_value' field for conditional mapping"))?;

        Ok(Self::Conditional {
            condition: condition.to_string(),
            true_value: true_value.to_string(),
            false_value,
            fallback: Self::parse_optional_fallback(config)?,
        })
    }

    fn parse_fallback_mapping(config: &Value) -> Result<Self> {
        let paths = config
            .get("paths")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Missing or invalid 'paths' field for fallback mapping"))?;

        let mut collected_paths = Vec::new();
        for path in paths {
            if let Some(path_str) = path.as_str() {
                collected_paths.push(path_str.to_string());
            }
        }

        Ok(Self::Fallback {
            paths: collected_paths,
            fallback: Self::parse_optional_fallback(config)?,
        })
    }

    fn parse_optional_fallback(config: &Value) -> Result<Option<Box<Self>>> {
        match config.get("fallback") {
            Some(fallback_config) if !fallback_config.is_null() => {
                Ok(Some(Box::new(Self::from_json(fallback_config)?)))
            }
            _ => Ok(None),
        }
    }

    fn require_string<'a>(config: &'a Value, field: &str, err: &str) -> Result<&'a str> {
        config
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("{err}"))
    }
}

/// Token字段映射配置（新格式）
#[derive(Debug, Clone)]
pub struct TokenMappingConfig {
    /// Token字段映射 `field_name` -> `TokenMapping`
    pub token_mappings: HashMap<String, TokenMapping>,
}

impl TokenMappingConfig {
    /// `从token_mappings_json解析配置`
    pub fn from_json(json_str: &str) -> Result<Self> {
        let config_value: Value = serde_json::from_str(json_str)?;
        let mut token_mappings = HashMap::new();

        // 解析每个token字段映射
        for (field_name, mapping_config) in config_value
            .as_object()
            .ok_or_else(|| anyhow!("Invalid token mappings JSON format"))?
        {
            let mapping = TokenMapping::from_json(mapping_config)?;
            token_mappings.insert(field_name.clone(), mapping);
        }

        Ok(Self { token_mappings })
    }
}

/// Token字段提取器（新版本）
#[derive(Debug, Clone)]
pub struct TokenFieldExtractor {
    config: TokenMappingConfig,
}

impl TokenFieldExtractor {
    /// 创建新的Token字段提取器
    #[must_use]
    pub const fn new(config: TokenMappingConfig) -> Self {
        Self { config }
    }

    /// 尝试fallback逻辑
    fn try_fallback(&self, response: &Value, fallback: Option<&TokenMapping>) -> Option<Value> {
        fallback.map_or_else(
            || {
                ldebug!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Statistics,
                    "no_fallback",
                    "No fallback available"
                );
                None
            },
            |fallback_mapping| {
                ldebug!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Statistics,
                    "fallback_extraction",
                    "Attempting fallback extraction"
                );
                self.extract_by_mapping(response, fallback_mapping)
            },
        )
    }

    /// 提取Token字段值
    #[must_use]
    pub fn extract_token_field(&self, response: &Value, field_name: &str) -> Option<Value> {
        self.config.token_mappings.get(field_name).map_or_else(
            || {
                ldebug!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Statistics,
                    "mapping_not_found",
                    "Token field mapping not found",
                    field_name = %field_name
                );
                None
            },
            |mapping| self.extract_by_mapping(response, mapping),
        )
    }

    /// 提取u32 token值
    #[must_use]
    pub fn extract_token_u32(&self, response: &Value, field_name: &str) -> Option<u32> {
        self.extract_token_field(response, field_name)
            .and_then(|v| match v {
                Value::Number(n) => n
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .or_else(|| n.as_f64().and_then(Self::float_to_u32)),
                Value::String(s) => s.parse::<u32>().ok(),
                _ => None,
            })
    }

    fn float_to_u32(value: f64) -> Option<u32> {
        if !value.is_finite() {
            return None;
        }

        let rounded = value.round();
        if (rounded - value).abs() > f64::EPSILON {
            return None;
        }

        if !(0.0..=f64::from(u32::MAX)).contains(&rounded) {
            return None;
        }

        format!("{rounded:.0}").parse::<u32>().ok()
    }

    fn extract_by_mapping(&self, response: &Value, mapping: &TokenMapping) -> Option<Value> {
        match mapping {
            TokenMapping::Direct { path, fallback } => {
                let result = json_path_lookup(response, path);
                result.or_else(|| self.try_fallback(response, fallback.as_deref()))
            }
            TokenMapping::Expression { formula, fallback } => {
                let result = Self::evaluate_expression(response, formula);
                result.or_else(|| self.try_fallback(response, fallback.as_deref()))
            }
            TokenMapping::Default { value, fallback } => Some(
                self.try_fallback(response, fallback.as_deref())
                    .unwrap_or_else(|| value.clone()),
            ),
            TokenMapping::Conditional {
                condition,
                true_value,
                false_value,
                fallback,
            } => {
                let result = if Self::evaluate_condition(response, condition) {
                    json_path_lookup(response, true_value)
                        .or_else(|| Some(Value::String(true_value.clone())))
                } else {
                    Some(false_value.clone())
                };
                result.or_else(|| self.try_fallback(response, fallback.as_deref()))
            }
            TokenMapping::Fallback { paths, fallback } => {
                for path in paths {
                    if let Some(result) = json_path_lookup(response, path) {
                        return Some(result);
                    }
                }
                self.try_fallback(response, fallback.as_deref())
            }
        }
    }

    /// 评估简单算术表达式（包含路径替换）
    fn evaluate_expression(value: &Value, formula: &str) -> Option<Value> {
        // 替换 {path} 为实际数值
        let re = Regex::new(r"\{([^}]+)\}").ok()?;
        let mut eval_str = formula.to_string();
        for cap in re.captures_iter(formula) {
            if let Some(path) = cap.get(1) {
                let path_str = path.as_str();
                if let Some(v) = json_path_lookup(value, path_str) {
                    let num = match v {
                        Value::Number(n) => n.as_f64().unwrap_or(0.0),
                        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                        _ => 0.0,
                    };
                    eval_str = eval_str.replace(&format!("{{{path_str}}}"), &num.to_string());
                }
            }
        }

        // 仅支持 + 和 - 的简单表达式；支持直接路径令牌与数字
        let mut total = 0.0;
        let mut sign = 1.0;
        for token in eval_str.split_whitespace() {
            match token {
                "+" => sign = 1.0,
                "-" => sign = -1.0,
                _ => {
                    let num = token.parse::<f64>().ok().map_or_else(
                        || {
                            json_path_lookup(value, token).map_or(0.0, |v| match v {
                                Value::Number(n) => n.as_f64().unwrap_or(0.0),
                                Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                                _ => 0.0,
                            })
                        },
                        |n| n,
                    );
                    total += sign * num;
                }
            }
        }

        let number =
            serde_json::Number::from_f64(total).unwrap_or_else(|| serde_json::Number::from(0));
        Some(Value::Number(number))
    }

    /// 评估条件表达式（非常简化，仅示例用途）
    fn evaluate_condition(value: &Value, condition: &str) -> bool {
        // 示例："{usage.prompt_tokens} > 0"
        let parts: Vec<&str> = condition.split_whitespace().collect();
        if parts.len() != 3 {
            return false;
        }

        let left = parts[0];
        let op = parts[1];
        let right = parts[2];

        let left_val = if left.starts_with('{') && left.ends_with('}') {
            let path = &left[1..left.len() - 1];
            json_path_lookup(value, path).map_or(0.0, |v| v.as_f64().unwrap_or(0.0))
        } else {
            left.parse::<f64>().unwrap_or(0.0)
        };

        let right_val = right.parse::<f64>().unwrap_or(0.0);

        match op {
            ">" => left_val > right_val,
            "<" => left_val < right_val,
            "==" => (left_val - right_val).abs() < f64::EPSILON,
            _ => false,
        }
    }
}

/// 模型提取器（从请求URL/Body/Query中提取模型名）
#[derive(Debug, Clone)]
pub struct ModelExtractor {
    rules: Vec<ModelRule>,
    fallback_model: Option<String>,
}

#[derive(Debug, Clone)]
enum ModelRule {
    BodyJson { path: String, priority: i32 },
    UrlRegex { pattern: Regex, priority: i32 },
    QueryParam { name: String, priority: i32 },
}

impl ModelExtractor {
    pub fn from_json_config(json_str: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(json_str)?;
        let mut rules = Vec::new();
        if let Some(arr) = v.get("extraction_rules").and_then(|x| x.as_array()) {
            for item in arr {
                let r#type = item.get("type").and_then(|x| x.as_str()).unwrap_or("");
                let prio = item
                    .get("priority")
                    .and_then(sea_orm::JsonValue::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
                    .unwrap_or(0);
                match r#type {
                    "body_json" => {
                        if let Some(path) = item.get("path").and_then(|x| x.as_str()) {
                            rules.push(ModelRule::BodyJson {
                                path: path.to_string(),
                                priority: prio,
                            });
                        }
                    }
                    "url_regex" => {
                        if let Some(pattern) = item.get("pattern").and_then(|x| x.as_str())
                            && let Ok(re) = Regex::new(pattern)
                        {
                            rules.push(ModelRule::UrlRegex {
                                pattern: re,
                                priority: prio,
                            });
                        }
                    }
                    "query_param" => {
                        if let Some(name) = item.get("parameter").and_then(|x| x.as_str()) {
                            rules.push(ModelRule::QueryParam {
                                name: name.to_string(),
                                priority: prio,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        // priority 小的优先
        rules.sort_by_key(|r| match r {
            ModelRule::BodyJson { priority, .. }
            | ModelRule::UrlRegex { priority, .. }
            | ModelRule::QueryParam { priority, .. } => *priority,
        });
        let fallback_model = v
            .get("fallback_model")
            .and_then(|x| x.as_str())
            .map(std::string::ToString::to_string);
        Ok(Self {
            rules,
            fallback_model,
        })
    }

    #[must_use]
    pub fn extract_model_name(
        &self,
        url_path: &str,
        body_json: Option<&Value>,
        query_params: &std::collections::HashMap<String, String>,
    ) -> String {
        for rule in &self.rules {
            match rule {
                ModelRule::BodyJson { path, .. } => {
                    if let Some(val) = body_json.and_then(|v| json_path_lookup(v, path))
                        && let Some(s) = val.as_str()
                    {
                        return s.to_string();
                    }
                }
                ModelRule::UrlRegex { pattern, .. } => {
                    if let Some(cap) = pattern.captures(url_path)
                        && let Some(m) = cap.get(1)
                    {
                        return m.as_str().to_string();
                    }
                }
                ModelRule::QueryParam { name, .. } => {
                    if let Some(v) = query_params.get(name) {
                        return v.clone();
                    }
                }
            }
        }
        self.fallback_model
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

fn json_path_lookup(v: &Value, path: &str) -> Option<Value> {
    let mut cur = v;
    for seg in path.split('.') {
        if let Ok(idx) = seg.parse::<usize>() {
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    Some(cur.clone())
}
