//! # 数据驱动字段提取器
//!
//! 基于数据库配置的通用字段提取器，支持JSONPath查询、数学表达式和条件判断

use anyhow::{Result, anyhow};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use tracing::debug;

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
    /// 从JSON配置解析Token映射
    pub fn from_json(config: &Value) -> Result<Self> {
        let mapping_type = config
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'type' field"))?;

        match mapping_type {
            "direct" => {
                let path = config
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' field for direct mapping"))?;

                // 解析可选的fallback
                let fallback = if let Some(fallback_config) = config.get("fallback") {
                    if fallback_config.is_null() {
                        None
                    } else {
                        Some(Box::new(Self::from_json(fallback_config)?))
                    }
                } else {
                    None
                };

                Ok(TokenMapping::Direct {
                    path: path.to_string(),
                    fallback,
                })
            }
            "expression" => {
                let formula = config
                    .get("formula")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'formula' field for expression mapping"))?;

                // 解析可选的fallback
                let fallback = if let Some(fallback_config) = config.get("fallback") {
                    if fallback_config.is_null() {
                        None
                    } else {
                        Some(Box::new(Self::from_json(fallback_config)?))
                    }
                } else {
                    None
                };

                Ok(TokenMapping::Expression {
                    formula: formula.to_string(),
                    fallback,
                })
            }
            "default" => {
                let value = config
                    .get("value")
                    .cloned()
                    .ok_or_else(|| anyhow!("Missing 'value' field for default mapping"))?;

                // 解析可选的fallback
                let fallback = if let Some(fallback_config) = config.get("fallback") {
                    if fallback_config.is_null() {
                        None
                    } else {
                        Some(Box::new(Self::from_json(fallback_config)?))
                    }
                } else {
                    None
                };

                Ok(TokenMapping::Default { value, fallback })
            }
            "conditional" => {
                let condition = config
                    .get("condition")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'condition' field for conditional mapping"))?;
                let true_value = config
                    .get("true_value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'true_value' field for conditional mapping"))?;
                let false_value = config.get("false_value").cloned().ok_or_else(|| {
                    anyhow!("Missing 'false_value' field for conditional mapping")
                })?;

                // 解析可选的fallback
                let fallback = if let Some(fallback_config) = config.get("fallback") {
                    if fallback_config.is_null() {
                        None
                    } else {
                        Some(Box::new(Self::from_json(fallback_config)?))
                    }
                } else {
                    None
                };

                Ok(TokenMapping::Conditional {
                    condition: condition.to_string(),
                    true_value: true_value.to_string(),
                    false_value,
                    fallback,
                })
            }
            "fallback" => {
                let paths = config
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        anyhow!("Missing or invalid 'paths' field for fallback mapping")
                    })?;

                let mut path_strings = Vec::new();
                for path in paths {
                    if let Some(path_str) = path.as_str() {
                        path_strings.push(path_str.to_string());
                    }
                }

                // 解析可选的fallback
                let fallback = if let Some(fallback_config) = config.get("fallback") {
                    if fallback_config.is_null() {
                        None
                    } else {
                        Some(Box::new(Self::from_json(fallback_config)?))
                    }
                } else {
                    None
                };

                Ok(TokenMapping::Fallback {
                    paths: path_strings,
                    fallback,
                })
            }
            _ => Err(anyhow!("Unknown mapping type: {}", mapping_type)),
        }
    }
}

/// 数值转换类型
#[derive(Debug, Clone, PartialEq)]
pub enum TransformRule {
    /// 乘法转换
    Multiply(f64),
    /// 除法转换
    Divide(f64),
    /// 固定值
    Fixed(Value),
    /// 无转换
    None,
}

impl TransformRule {
    /// 从字符串解析转换规则
    pub fn from_str(rule: &str) -> Self {
        let parts: Vec<&str> = rule.split(':').collect();
        match parts.as_slice() {
            ["multiply", value] => {
                if let Ok(num) = value.parse::<f64>() {
                    TransformRule::Multiply(num)
                } else {
                    TransformRule::None
                }
            }
            ["divide", value] => {
                if let Ok(num) = value.parse::<f64>() {
                    TransformRule::Divide(num)
                } else {
                    TransformRule::None
                }
            }
            ["fixed", value] => {
                if let Ok(num) = value.parse::<f64>() {
                    TransformRule::Fixed(Value::Number(serde_json::Number::from_f64(num).unwrap()))
                } else {
                    TransformRule::Fixed(Value::String(value.to_string()))
                }
            }
            _ => TransformRule::None,
        }
    }

    /// 应用转换规则
    pub fn apply(&self, value: &Value) -> Value {
        match self {
            TransformRule::Multiply(factor) => match value {
                Value::Number(num) => {
                    let v = num.as_f64().unwrap_or(0.0) * factor;
                    Value::Number(serde_json::Number::from_f64(v).unwrap())
                }
                _ => value.clone(),
            },
            TransformRule::Divide(divisor) => match value {
                Value::Number(num) => {
                    let v = num.as_f64().unwrap_or(0.0) / divisor;
                    Value::Number(serde_json::Number::from_f64(v).unwrap())
                }
                _ => value.clone(),
            },
            TransformRule::Fixed(fixed_val) => fixed_val.clone(),
            TransformRule::None => value.clone(),
        }
    }
}

/// Token字段映射配置（新格式）
#[derive(Debug, Clone)]
pub struct TokenMappingConfig {
    /// Token字段映射 field_name -> TokenMapping
    pub token_mappings: HashMap<String, TokenMapping>,
}

impl TokenMappingConfig {
    /// 从token_mappings_json解析配置
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

/// 字段映射配置（来自数据库）
#[derive(Debug, Clone)]
pub struct FieldMappingConfig {
    /// 字段路径映射 field_name -> json_path
    pub field_mappings: HashMap<String, String>,
    /// 默认值配置 field_name -> default_value  
    pub default_values: HashMap<String, Value>,
    /// 转换规则
    pub transformations: HashMap<String, TransformRule>,
}

impl FieldMappingConfig {
    /// 从JSON字符串解析配置
    pub fn from_json(json_str: &str) -> Result<Self> {
        let config_value: Value = serde_json::from_str(json_str)?;

        // 解析字段映射
        let field_mappings = if let Some(mappings) = config_value.get("field_mappings") {
            mappings
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        } else {
            HashMap::new()
        };

        // 解析默认值
        let default_values = if let Some(defaults) = config_value.get("default_values") {
            defaults
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            HashMap::new()
        };

        // 解析转换规则
        let transformations = if let Some(transforms) = config_value.get("transformations") {
            transforms
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), TransformRule::from_str(s))))
                .collect()
        } else {
            HashMap::new()
        };

        Ok(Self {
            field_mappings,
            default_values,
            transformations,
        })
    }

    /// 转换为JSON字符串
    pub fn to_json(&self) -> Result<String> {
        let mut config = serde_json::Map::new();

        // 字段映射
        let mappings: serde_json::Map<String, Value> = self
            .field_mappings
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        config.insert("field_mappings".to_string(), Value::Object(mappings));

        // 默认值
        let defaults = serde_json::Map::from_iter(self.default_values.clone());
        config.insert("default_values".to_string(), Value::Object(defaults));

        // 转换规则
        let transforms = serde_json::Map::from_iter(
            self.transformations
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(format!("{:?}", v)))),
        );
        config.insert("transformations".to_string(), Value::Object(transforms));

        serde_json::to_string_pretty(&Value::Object(config))
            .map_err(|e| anyhow!("Failed to serialize config: {}", e))
    }
}

/// Token字段提取器（新版本）
#[derive(Debug, Clone)]
pub struct TokenFieldExtractor {
    config: TokenMappingConfig,
}

impl TokenFieldExtractor {
    /// 创建新的Token字段提取器
    pub fn new(config: TokenMappingConfig) -> Self {
        Self { config }
    }

    /// 从JSON配置字符串创建
    pub fn from_json_config(json_str: &str) -> Result<Self> {
        let config = TokenMappingConfig::from_json(json_str)?;
        Ok(Self::new(config))
    }

    /// 尝试fallback逻辑
    fn try_fallback(
        &self,
        response: &Value,
        fallback: &Option<Box<TokenMapping>>,
    ) -> Option<Value> {
        if let Some(fallback_mapping) = fallback {
            debug!("Attempting fallback extraction");
            self.extract_by_mapping(response, fallback_mapping.as_ref())
        } else {
            debug!("No fallback available");
            None
        }
    }

    /// 提取Token字段值
    pub fn extract_token_field(&self, response: &Value, field_name: &str) -> Option<Value> {
        if let Some(mapping) = self.config.token_mappings.get(field_name) {
            self.extract_by_mapping(response, mapping)
        } else {
            debug!(field_name = %field_name, "Token field mapping not found");
            None
        }
    }

    /// 提取u32 token值
    pub fn extract_token_u32(&self, response: &Value, field_name: &str) -> Option<u32> {
        self.extract_token_field(response, field_name)
            .and_then(|v| match v {
                Value::Number(n) => n.as_u64().map(|v| v as u32),
                Value::String(s) => s.parse::<u32>().ok(),
                _ => None,
            })
    }

    fn extract_by_mapping(&self, response: &Value, mapping: &TokenMapping) -> Option<Value> {
        match mapping {
            TokenMapping::Direct { path, fallback } => {
                let result = self.json_path_lookup(response, path);
                result.or_else(|| self.try_fallback(response, fallback))
            }
            TokenMapping::Expression { formula, fallback } => {
                let result = self.evaluate_expression(response, formula);
                result.or_else(|| self.try_fallback(response, fallback))
            }
            TokenMapping::Default { value, fallback } => {
                if let Some(result) = self.try_fallback(response, fallback) {
                    Some(result)
                } else {
                    Some(value.clone())
                }
            }
            TokenMapping::Conditional {
                condition,
                true_value,
                false_value,
                fallback,
            } => {
                let result = if self.evaluate_condition(response, condition) {
                    self.json_path_lookup(response, true_value)
                        .or_else(|| Some(Value::String(true_value.clone())))
                } else {
                    Some(false_value.clone())
                };
                result.or_else(|| self.try_fallback(response, fallback))
            }
            TokenMapping::Fallback { paths, fallback } => {
                for path in paths {
                    if let Some(result) = self.json_path_lookup(response, path) {
                        return Some(result);
                    }
                }
                self.try_fallback(response, fallback)
            }
        }
    }

    /// 简化版 JSON 路径查询（支持 a.b[0].c）
    fn json_path_lookup(&self, value: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            if let Some((field, index)) = Self::parse_index(part) {
                current = current.get(field)?;
                current = current.get(index)?;
            } else if part.chars().all(|c| c.is_ascii_digit()) {
                // 直接的数字索引，如 data.0.model 中的 "0"
                let idx = part.parse::<usize>().ok()?;
                current = current.as_array()?.get(idx)?;
            } else {
                current = current.get(part)?;
            }
        }

        Some(current.clone())
    }

    fn parse_index(part: &str) -> Option<(&str, usize)> {
        if let Some(beg) = part.find('[') {
            if let Some(end) = part.find(']') {
                let field = &part[..beg];
                let idx = &part[beg + 1..end];
                if let Ok(i) = idx.parse::<usize>() {
                    return Some((field, i));
                }
            }
        }
        None
    }

    /// 评估简单算术表达式（包含路径替换）
    fn evaluate_expression(&self, value: &Value, formula: &str) -> Option<Value> {
        // 替换 {path} 为实际数值
        let re = Regex::new(r"\{([^}]+)\}").ok()?;
        let mut eval_str = formula.to_string();
        for cap in re.captures_iter(formula) {
            if let Some(path) = cap.get(1) {
                let path_str = path.as_str();
                if let Some(v) = self.json_path_lookup(value, path_str) {
                    let num = match v {
                        Value::Number(n) => n.as_f64().unwrap_or(0.0),
                        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                        _ => 0.0,
                    };
                    eval_str = eval_str.replace(&format!("{{{}}}", path_str), &num.to_string());
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
                    let num = if let Ok(n) = token.parse::<f64>() {
                        n
                    } else if let Some(v) = self.json_path_lookup(value, token) {
                        match v {
                            Value::Number(n) => n.as_f64().unwrap_or(0.0),
                            Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    total += sign * num;
                }
            }
        }

        let number = if (total.fract()).abs() < f64::EPSILON {
            serde_json::Number::from(total as i64)
        } else {
            serde_json::Number::from_f64(total).unwrap()
        };
        Some(Value::Number(number))
    }

    /// 评估条件表达式（非常简化，仅示例用途）
    fn evaluate_condition(&self, value: &Value, condition: &str) -> bool {
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
            if let Some(v) = self.json_path_lookup(value, path) {
                v.as_f64().unwrap_or(0.0)
            } else {
                0.0
            }
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

/// 兼容旧版通用字段提取器（可选）
#[derive(Debug, Clone)]
pub struct FieldExtractor {
    config: FieldMappingConfig,
}

impl FieldExtractor {
    pub fn new(config: FieldMappingConfig) -> Self {
        Self { config }
    }

    pub fn from_json_config(json_str: &str) -> Result<Self> {
        let config = FieldMappingConfig::from_json(json_str)?;
        Ok(Self::new(config))
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
                let prio = item.get("priority").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
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
                        if let Some(pattern) = item.get("pattern").and_then(|x| x.as_str()) {
                            if let Ok(re) = Regex::new(pattern) {
                                rules.push(ModelRule::UrlRegex {
                                    pattern: re,
                                    priority: prio,
                                });
                            }
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
            .map(|s| s.to_string());
        Ok(Self {
            rules,
            fallback_model,
        })
    }

    pub fn extract_model_name(
        &self,
        url_path: &str,
        body_json: Option<&Value>,
        query_params: &std::collections::HashMap<String, String>,
    ) -> String {
        for rule in &self.rules {
            match rule {
                ModelRule::BodyJson { path, .. } => {
                    if let Some(val) = body_json.and_then(|v| json_path_lookup(v, path)) {
                        if let Some(s) = val.as_str() {
                            return s.to_string();
                        }
                    }
                }
                ModelRule::UrlRegex { pattern, .. } => {
                    if let Some(cap) = pattern.captures(url_path) {
                        if let Some(m) = cap.get(1) {
                            return m.as_str().to_string();
                        }
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
