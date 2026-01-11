//! # 数据驱动字段提取器
//!
//! 基于数据库配置的通用字段提取器，支持JSONPath查询、数学表达式和条件判断

use crate::error::Result;
use crate::{
    ldebug,
    logging::{LogComponent, LogStage},
    types::TokenCount,
};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

/// `严格校验 token_mappings_json 是否符合字段提取器定义`
///
/// 说明：
/// - 采集层在运行时会尽量“容错”（失败则回退默认值），避免因为配置错误导致请求失败。
/// - 管理端在新增/编辑时需要“严格校验”，将无效配置拦截在写入数据库之前。
pub fn validate_token_mappings_value(value: &Value) -> Result<()> {
    validate_token_mappings_value_inner(value, 0)
}

fn validate_token_mappings_value_inner(value: &Value, depth: usize) -> Result<()> {
    const MAX_DEPTH: usize = 8;
    if depth > MAX_DEPTH {
        return Err(crate::error::conversion::ConversionError::message(
            "token_mappings_json 嵌套过深（fallback 链过长）",
        )
        .into());
    }

    let obj = value.as_object().ok_or_else(|| {
        crate::error::conversion::ConversionError::message(
            "token_mappings_json 必须是对象（field_name -> mapping）",
        )
    })?;

    if obj.is_empty() {
        return Err(crate::error::conversion::ConversionError::message(
            "token_mappings_json 不能为空对象",
        )
        .into());
    }

    for (field_name, mapping_cfg) in obj {
        if field_name.trim().is_empty() {
            return Err(crate::error::conversion::ConversionError::message(
                "token_mappings_json 的字段名不能为空",
            )
            .into());
        }
        validate_token_mapping_config(field_name, mapping_cfg, depth)?;
    }

    Ok(())
}

fn validate_token_mapping_config(field_name: &str, config: &Value, depth: usize) -> Result<()> {
    let mapping_type = config.get("type").and_then(Value::as_str).ok_or_else(|| {
        crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: 缺少或非法的 type 字段"
        ))
    })?;

    validate_token_mapping_type(field_name, config, mapping_type)?;
    validate_token_mapping_fallback(field_name, config, depth)
}

fn validate_token_mapping_type(field_name: &str, config: &Value, mapping_type: &str) -> Result<()> {
    match mapping_type {
        "direct" => validate_token_mapping_direct(field_name, config),
        "expression" => validate_token_mapping_expression(field_name, config),
        "default" => validate_token_mapping_default(field_name, config),
        "conditional" => validate_token_mapping_conditional(field_name, config),
        "fallback" => validate_token_mapping_fallback_paths(field_name, config),
        other => Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: 未知的 mapping type: {other}"
        ))
        .into()),
    }
}

fn validate_token_mapping_direct(field_name: &str, config: &Value) -> Result<()> {
    let path = config.get("path").and_then(Value::as_str).unwrap_or("");
    if path.trim().is_empty() {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: direct 类型必须提供非空 path"
        ))
        .into());
    }
    Ok(())
}

fn validate_token_mapping_expression(field_name: &str, config: &Value) -> Result<()> {
    let formula = config.get("formula").and_then(Value::as_str).unwrap_or("");
    if formula.trim().is_empty() {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: expression 类型必须提供非空 formula"
        ))
        .into());
    }
    Ok(())
}

fn validate_token_mapping_default(field_name: &str, config: &Value) -> Result<()> {
    if !config.as_object().is_some_and(|m| m.contains_key("value")) {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: default 类型必须提供 value"
        ))
        .into());
    }
    Ok(())
}

fn validate_token_mapping_conditional(field_name: &str, config: &Value) -> Result<()> {
    let condition = config
        .get("condition")
        .and_then(Value::as_str)
        .unwrap_or("");
    let true_value = config
        .get("true_value")
        .and_then(Value::as_str)
        .unwrap_or("");
    if condition.trim().is_empty() {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional 类型必须提供非空 condition"
        ))
        .into());
    }
    if true_value.trim().is_empty() {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional 类型必须提供非空 true_value"
        ))
        .into());
    }
    if !config
        .as_object()
        .is_some_and(|m| m.contains_key("false_value"))
    {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional 类型必须提供 false_value"
        ))
        .into());
    }
    validate_condition_syntax(field_name, condition)
}

fn validate_token_mapping_fallback_paths(field_name: &str, config: &Value) -> Result<()> {
    let paths = config
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            crate::error::conversion::ConversionError::message(format!(
                "token_mappings_json.{field_name}: fallback 类型必须提供 paths 数组"
            ))
        })?;
    if paths.is_empty() {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: fallback 类型的 paths 不能为空数组"
        ))
        .into());
    }
    for (i, p) in paths.iter().enumerate() {
        let s = p.as_str().unwrap_or("").trim();
        if s.is_empty() {
            return Err(crate::error::conversion::ConversionError::message(format!(
                "token_mappings_json.{field_name}: fallback.paths[{i}] 必须是非空字符串"
            ))
            .into());
        }
    }
    Ok(())
}

fn validate_token_mapping_fallback(field_name: &str, config: &Value, depth: usize) -> Result<()> {
    let Some(fallback_cfg) = config.get("fallback") else {
        return Ok(());
    };
    if fallback_cfg.is_null() {
        return Ok(());
    }

    validate_token_mappings_value_inner(&serde_json::json!({ field_name: fallback_cfg }), depth + 1)
}

fn validate_condition_syntax(field_name: &str, condition: &str) -> Result<()> {
    // 当前实现只支持："{path} > 0" / "1 < 2" / "{path} == 0" 这种三段式条件
    let tokens: Vec<&str> = condition.split_whitespace().collect();
    if tokens.len() != 3 {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional.condition 仅支持三段式表达式，例如 \"{{usage.total_tokens}} > 0\""
        ))
        .into());
    }
    let left = tokens[0];
    let op = tokens[1];
    let right = tokens[2];

    if op != ">" && op != "<" && op != "==" {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional.condition 操作符仅支持 > / < / =="
        ))
        .into());
    }

    // left: number 或 {path}
    let left_ok = if left.starts_with('{') && left.ends_with('}') {
        left.len() > 2
    } else {
        left.parse::<f64>().is_ok()
    };
    if !left_ok {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional.condition 左侧必须是数字或 {{path}}"
        ))
        .into());
    }

    if right.parse::<f64>().is_err() {
        return Err(crate::error::conversion::ConversionError::message(format!(
            "token_mappings_json.{field_name}: conditional.condition 右侧必须是数字"
        ))
        .into());
    }

    Ok(())
}

/// `严格校验 model_extraction_json 是否符合字段提取器定义`
pub fn validate_model_extraction_value(value: &Value) -> Result<()> {
    let obj = value.as_object().ok_or_else(|| {
        crate::error::conversion::ConversionError::message("model_extraction_json 必须是对象")
    })?;

    let rules = obj
        .get("extraction_rules")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            crate::error::conversion::ConversionError::message(
                "model_extraction_json.extraction_rules 必须是数组",
            )
        })?;

    if rules.is_empty()
        && obj
            .get("fallback_model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(crate::error::conversion::ConversionError::message(
            "model_extraction_json 至少需要提供 extraction_rules 或 fallback_model",
        )
        .into());
    }

    for (i, item) in rules.iter().enumerate() {
        let item_obj = item.as_object().ok_or_else(|| {
            crate::error::conversion::ConversionError::message(format!(
                "model_extraction_json.extraction_rules[{i}] 必须是对象"
            ))
        })?;

        let rule_type = item_obj
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if rule_type.is_empty() {
            return Err(crate::error::conversion::ConversionError::message(format!(
                "model_extraction_json.extraction_rules[{i}] 缺少 type"
            ))
            .into());
        }

        match rule_type.as_str() {
            "body_json" => {
                let path = item_obj
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if path.is_empty() {
                    return Err(crate::error::conversion::ConversionError::message(format!(
                        "model_extraction_json.extraction_rules[{i}]: body_json 必须提供非空 path"
                    ))
                    .into());
                }
            }
            "url_regex" => {
                let pattern = item_obj
                    .get("pattern")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if pattern.is_empty() {
                    return Err(crate::error::conversion::ConversionError::message(format!(
                        "model_extraction_json.extraction_rules[{i}]: url_regex 必须提供非空 pattern"
                    ))
                    .into());
                }
                Regex::new(pattern).map_err(|e| {
                    crate::error::conversion::ConversionError::message(format!(
                        "model_extraction_json.extraction_rules[{i}]: url_regex.pattern 非法: {e}"
                    ))
                })?;
            }
            "query_param" => {
                let name = item_obj
                    .get("parameter")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if name.is_empty() {
                    return Err(crate::error::conversion::ConversionError::message(format!(
                        "model_extraction_json.extraction_rules[{i}]: query_param 必须提供非空 parameter"
                    ))
                    .into());
                }
            }
            other => {
                return Err(crate::error::conversion::ConversionError::message(format!(
                    "model_extraction_json.extraction_rules[{i}]: 未知的 type: {other}"
                ))
                .into());
            }
        }
    }

    Ok(())
}

/// Token字段映射类型
#[derive(Debug, Clone, PartialEq)]
pub enum TokenMapping {
    /// 直接映射字段路径
    Direct {
        path: String,
        fallback: Option<Box<Self>>,
    },
    /// 数学表达式计算
    Expression {
        formula: String,
        fallback: Option<Box<Self>>,
    },
    /// 固定默认值
    Default {
        value: Value,
        fallback: Option<Box<Self>>,
    },
    /// 条件判断映射
    Conditional {
        condition: String,
        true_value: String,
        false_value: Value,
        fallback: Option<Box<Self>>,
    },
    /// Fallback路径列表（保持向后兼容）
    Fallback {
        paths: Vec<String>,
        fallback: Option<Box<Self>>,
    },
}

impl TokenMapping {
    /// `从JSON配置解析Token映射`
    pub fn from_json(config: &Value) -> Result<Self> {
        let mapping_type = config.get("type").and_then(Value::as_str).ok_or_else(|| {
            crate::error::ProxyError::from(crate::error::conversion::ConversionError::Message(
                "Invalid token mappings configuration: missing or invalid 'type' field".to_string(),
            ))
        })?;

        match mapping_type {
            "direct" => Self::parse_direct_mapping(config),
            "expression" => Self::parse_expression_mapping(config),
            "default" => Self::parse_default_mapping(config),
            "conditional" => Self::parse_conditional_mapping(config),
            "fallback" => Self::parse_fallback_mapping(config),
            _ => Err(crate::error::ProxyError::from(
                crate::error::conversion::ConversionError::Message(format!(
                    "Unknown mapping type: {mapping_type}"
                )),
            )),
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
        let value = config.get("value").cloned().ok_or_else(|| {
            crate::error::ProxyError::from(crate::error::conversion::ConversionError::Message(
                "Missing 'value' field for default mapping".to_string(),
            ))
        })?;

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
        let false_value = config.get("false_value").cloned().ok_or_else(|| {
            crate::error::ProxyError::from(crate::error::conversion::ConversionError::Message(
                "Missing 'false_value' field for conditional mapping".to_string(),
            ))
        })?;

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
            .ok_or_else(|| {
                crate::error::ProxyError::from(crate::error::conversion::ConversionError::Message(
                    "Missing or invalid 'paths' field for fallback mapping".to_string(),
                ))
            })?;

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
        config.get(field).and_then(Value::as_str).ok_or_else(|| {
            crate::error::ProxyError::from(crate::error::conversion::ConversionError::Message(
                err.to_string(),
            ))
        })
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
        let config_value: Value = match serde_json::from_str(json_str) {
            Ok(value) => value,
            Err(e) => {
                return Err(crate::error::conversion::ConversionError::Message(format!(
                    "Invalid token mappings JSON: {e}"
                ))
                .into());
            }
        };
        let mut token_mappings = HashMap::new();

        // 解析每个token字段映射
        for (field_name, mapping_config) in config_value.as_object().ok_or_else(|| {
            crate::error::ProxyError::from(crate::error::conversion::ConversionError::Message(
                "Invalid token mappings JSON format".to_string(),
            ))
        })? {
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

    /// 提取token数量（使用统一TokenCount类型）
    #[must_use]
    pub fn extract_token_count(&self, response: &Value, field_name: &str) -> Option<TokenCount> {
        self.extract_token_field(response, field_name)
            .and_then(|v| match v {
                Value::Number(n) => n
                    .as_u64()
                    .or_else(|| n.as_f64().and_then(Self::float_to_u64)),
                Value::String(s) => s.parse::<TokenCount>().ok(),
                _ => None,
            })
    }

    fn float_to_u64(value: f64) -> Option<u64> {
        if !value.is_finite() {
            return None;
        }

        let rounded = value.round();
        if (rounded - value).abs() > f64::EPSILON {
            return None;
        }

        format!("{rounded:.0}").parse::<u64>().ok()
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
                    let num = token.parse::<f64>().ok().unwrap_or_else(|| {
                        json_path_lookup(value, token).map_or(0.0, |v| match v {
                            Value::Number(n) => n.as_f64().unwrap_or(0.0),
                            Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                            _ => 0.0,
                        })
                    });
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
        let v: Value = match serde_json::from_str(json_str) {
            Ok(value) => value,
            Err(e) => {
                return Err(crate::error::conversion::ConversionError::Message(format!(
                    "Invalid token mappings JSON: {e}"
                ))
                .into());
            }
        };
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
