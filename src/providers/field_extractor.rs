//! # 数据驱动字段提取器
//!
//! 基于数据库配置的通用字段提取器，支持JSONPath查询、数学表达式和条件判断

use anyhow::{Result, anyhow};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, info, warn};

/// Token字段映射类型
#[derive(Debug, Clone, PartialEq)]
pub enum TokenMapping {
    /// 直接映射字段路径
    Direct { path: String },
    /// 数学表达式计算
    Expression {
        formula: String,
        fallback: Option<String>,
    },
    /// 固定默认值
    Default { value: Value },
    /// 条件判断映射
    Conditional {
        condition: String,
        true_value: String,
        false_value: Value,
    },
    /// Fallback路径列表
    Fallback { paths: Vec<String> },
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
                Ok(TokenMapping::Direct {
                    path: path.to_string(),
                })
            }
            "expression" => {
                let formula = config
                    .get("formula")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'formula' field for expression mapping"))?;
                let fallback = config
                    .get("fallback")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Ok(TokenMapping::Expression {
                    formula: formula.to_string(),
                    fallback,
                })
            }
            "default" => {
                let value = config
                    .get("value")
                    .ok_or_else(|| anyhow!("Missing 'value' field for default mapping"))?;
                Ok(TokenMapping::Default {
                    value: value.clone(),
                })
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
                let false_value = config.get("false_value").ok_or_else(|| {
                    anyhow!("Missing 'false_value' field for conditional mapping")
                })?;
                Ok(TokenMapping::Conditional {
                    condition: condition.to_string(),
                    true_value: true_value.to_string(),
                    false_value: false_value.clone(),
                })
            }
            "fallback" => {
                let paths = config
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        anyhow!("Missing or invalid 'paths' field for fallback mapping")
                    })?;
                let path_strings: Result<Vec<String>> = paths
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| anyhow!("Invalid path in fallback list"))
                            .map(|s| s.to_string())
                    })
                    .collect();
                Ok(TokenMapping::Fallback {
                    paths: path_strings?,
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
                Value::Number(n) => {
                    if let Some(num) = n.as_f64() {
                        Value::Number(
                            serde_json::Number::from_f64(num * factor).unwrap_or(n.clone()),
                        )
                    } else {
                        value.clone()
                    }
                }
                _ => value.clone(),
            },
            TransformRule::Divide(divisor) => match value {
                Value::Number(n) => {
                    if let Some(num) = n.as_f64() {
                        if *divisor != 0.0 {
                            Value::Number(
                                serde_json::Number::from_f64(num / divisor).unwrap_or(n.clone()),
                            )
                        } else {
                            value.clone()
                        }
                    } else {
                        value.clone()
                    }
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
    /// 转换规则配置 field_name -> transform_rule
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

    /// 提取Token字段值
    pub fn extract_token_field(&self, response: &Value, field_name: &str) -> Option<Value> {
        if let Some(mapping) = self.config.token_mappings.get(field_name) {
            self.extract_by_mapping(response, mapping)
        } else {
            debug!(field_name = %field_name, "Token field mapping not found");
            None
        }
    }

    /// 根据映射规则提取值
    fn extract_by_mapping(&self, response: &Value, mapping: &TokenMapping) -> Option<Value> {
        match mapping {
            TokenMapping::Direct { path } => self.extract_by_path(response, path),
            TokenMapping::Expression { formula, fallback } => {
                // 尝试计算表达式
                if let Some(result) = self.evaluate_expression(response, formula) {
                    Some(result)
                } else if let Some(fallback_path) = fallback {
                    // 使用fallback路径
                    self.extract_by_path(response, fallback_path)
                } else {
                    None
                }
            }
            TokenMapping::Default { value } => Some(value.clone()),
            TokenMapping::Conditional {
                condition,
                true_value,
                false_value,
            } => {
                if self.evaluate_condition(response, condition) {
                    self.extract_by_path(response, true_value)
                } else {
                    Some(false_value.clone())
                }
            }
            TokenMapping::Fallback { paths } => {
                // 按顺序尝试每个路径
                for path in paths {
                    if let Some(value) = self.extract_by_path(response, path) {
                        return Some(value);
                    }
                }
                None
            }
        }
    }

    /// 计算数学表达式
    fn evaluate_expression(&self, response: &Value, formula: &str) -> Option<Value> {
        // 辅助函数：将JSON值转换为数字
        let to_number = |val: &Value| -> Option<f64> {
            match val {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.parse::<f64>().ok(),
                _ => None,
            }
        };

        // 解析简单的加法表达式：usageMetadata.promptTokenCount + usageMetadata.candidatesTokenCount
        let parts: Vec<&str> = formula.split('+').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            debug!("Evaluating expression: {} + {}", parts[0], parts[1]);

            let left_val = self.extract_by_path(response, parts[0]);
            let right_val = self.extract_by_path(response, parts[1]);

            debug!("Left value: {:?}, Right value: {:?}", left_val, right_val);

            if let (Some(left_val), Some(right_val)) = (left_val, right_val) {
                let left_num = to_number(&left_val);
                let right_num = to_number(&right_val);

                debug!("Left number: {:?}, Right number: {:?}", left_num, right_num);

                if let (Some(left_num), Some(right_num)) = (left_num, right_num) {
                    let result = left_num + right_num;
                    debug!("Expression result: {}", result);
                    return serde_json::Number::from_f64(result).map(Value::Number);
                }
            }
        }

        // 解析减法表达式
        let parts: Vec<&str> = formula.split('-').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            let left_val = self.extract_by_path(response, parts[0])?;
            let right_val = self.extract_by_path(response, parts[1])?;

            let left_num = to_number(&left_val)?;
            let right_num = to_number(&right_val)?;

            let result = left_num - right_num;
            return serde_json::Number::from_f64(result).map(Value::Number);
        }

        // 解析乘法表达式
        let parts: Vec<&str> = formula.split('*').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            let left_val = self.extract_by_path(response, parts[0])?;
            let right_val = self.extract_by_path(response, parts[1])?;

            let left_num = to_number(&left_val)?;
            let right_num = to_number(&right_val)?;

            let result = left_num * right_num;
            return serde_json::Number::from_f64(result).map(Value::Number);
        }

        // 解析除法表达式
        let parts: Vec<&str> = formula.split('/').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            let left_val = self.extract_by_path(response, parts[0])?;
            let right_val = self.extract_by_path(response, parts[1])?;

            let left_num = to_number(&left_val)?;
            let right_num = to_number(&right_val)?;

            if right_num != 0.0 {
                let result = left_num / right_num;
                return serde_json::Number::from_f64(result).map(Value::Number);
            }
        }

        // 在实际线上环境中，很多提供商/路由并不会提供表达式涉及的字段，属于正常情况。
        // 记录为 info，避免误报错误，同时方便定位配置缺失场景；上层会做 0/回退处理。
        info!(
            formula = %formula,
            "Expression not applicable (missing inputs); using fallback/zero"
        );
        None
    }

    /// 计算条件表达式
    fn evaluate_condition(&self, response: &Value, condition: &str) -> bool {
        // 解析exists(path)条件
        if condition.starts_with("exists(") && condition.ends_with(")") {
            let path = &condition[7..condition.len() - 1];
            return self.extract_by_path(response, path).is_some();
        }

        // 其他条件类型可以在这里扩展
        false
    }

    /// 根据路径提取值，支持JSONPath语法和数组索引
    fn extract_by_path(&self, data: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data;

        for (i, part) in parts.iter().enumerate() {
            // 检查是否是数组索引访问，如 choices[0] 或 0
            if let Some((field_name, index_str)) = self.parse_array_access(part) {
                // 处理 field_name[index] 格式
                if let Some(array_field) = current.get(field_name) {
                    if let Some(array) = array_field.as_array() {
                        if let Ok(index) = index_str.parse::<usize>() {
                            if let Some(element) = array.get(index) {
                                current = element;
                                continue;
                            } else {
                                warn!(
                                    path = %path,
                                    field_name = %field_name,
                                    index = %index_str,
                                    array_len = array.len(),
                                    "Array index out of bounds"
                                );
                                return None;
                            }
                        } else {
                            warn!(
                                path = %path,
                                index_str = %index_str,
                                "Invalid array index format"
                            );
                            return None;
                        }
                    } else {
                        warn!(
                            path = %path,
                            field_name = %field_name,
                            actual_type = ?array_field,
                            "Field is not an array"
                        );
                        return None;
                    }
                } else {
                    warn!(
                        path = %path,
                        field_name = %field_name,
                        available_fields = ?current.as_object().map(|obj| obj.keys().collect::<Vec<_>>()),
                        "Array field not found"
                    );
                    return None;
                }
            } else if let Ok(index) = part.parse::<usize>() {
                // 处理纯数字索引，如 usage.0.usageMetadata 中的 "0"
                if let Some(array) = current.as_array() {
                    if let Some(element) = array.get(index) {
                        current = element;
                        continue;
                    } else {
                        warn!(
                            path = %path,
                            index = %part,
                            array_len = array.len(),
                            part_index = i,
                            "Array index out of bounds for numeric index"
                        );
                        return None;
                    }
                } else {
                    warn!(
                        path = %path,
                        index = %part,
                        actual_type = ?current,
                        part_index = i,
                        "Field is not an array for numeric index"
                    );
                    return None;
                }
            } else {
                // 普通字段访问
                if let Some(next_value) = current.get(part) {
                    current = next_value;
                } else {
                    warn!(
                        path = %path,
                        missing_field = %part,
                        available_fields = ?current.as_object().map(|obj| obj.keys().collect::<Vec<_>>()),
                        part_index = i,
                        "Field not found in path"
                    );
                    return None;
                }
            }
        }

        debug!(
            path = %path,
            extracted_value = ?current,
            "Successfully extracted value from path"
        );
        Some(current.clone())
    }

    /// 解析数组访问语法，返回 (field_name, index_str) 或 None
    fn parse_array_access<'a>(&self, part: &'a str) -> Option<(&'a str, &'a str)> {
        if part.contains('[') && part.ends_with(']') {
            let bracket_pos = part.find('[')?;
            let field_name = &part[..bracket_pos];
            let index_str = &part[bracket_pos + 1..part.len() - 1];
            Some((field_name, index_str))
        } else {
            None
        }
    }

    /// 提取u32类型Token字段
    pub fn extract_token_u32(&self, response: &Value, field_name: &str) -> Option<u32> {
        let value = self.extract_token_field(response, field_name)?;
        match value {
            Value::Number(n) => {
                // 尝试作为整数
                if let Some(u) = n.as_u64() {
                    Some(u as u32)
                } else if let Some(f) = n.as_f64() {
                    // 如果是浮点数，则四舍五入为整数
                    Some(f.round() as u32)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 提取i32类型Token字段
    pub fn extract_token_i32(&self, response: &Value, field_name: &str) -> Option<i32> {
        let value = self.extract_token_field(response, field_name)?;
        match value {
            Value::Number(n) => {
                // 尝试作为整数
                if let Some(i) = n.as_i64() {
                    Some(i as i32)
                } else if let Some(f) = n.as_f64() {
                    // 如果是浮点数，则四舍五入为整数
                    Some(f.round() as i32)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// 通用字段提取器
#[derive(Debug, Clone)]
pub struct FieldExtractor {
    config: FieldMappingConfig,
}

impl FieldExtractor {
    /// 创建新的字段提取器
    pub fn new(config: FieldMappingConfig) -> Self {
        Self { config }
    }

    /// 从JSON配置字符串创建
    pub fn from_json_config(json_str: &str) -> Result<Self> {
        let config = FieldMappingConfig::from_json(json_str)?;
        Ok(Self::new(config))
    }

    /// 提取字段值
    pub fn extract_field(&self, response: &Value, field_name: &str) -> Option<Value> {
        // 1. 尝试从映射配置中获取路径
        if let Some(json_path) = self.config.field_mappings.get(field_name) {
            if let Some(mut value) = self.extract_by_path(response, json_path) {
                // 2. 应用转换规则
                if let Some(transform_rule) = self.config.transformations.get(field_name) {
                    value = transform_rule.apply(&value);
                }
                return Some(value);
            }
        }

        // 3. 尝试获取默认值
        if let Some(default_val) = self.config.default_values.get(field_name) {
            debug!(field_name = %field_name, "Using default value");
            return Some(default_val.clone());
        }

        debug!(field_name = %field_name, "Field not found and no default value");
        None
    }

    /// 根据路径提取值，支持JSONPath语法和数组索引
    fn extract_by_path(&self, data: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data;

        for (i, part) in parts.iter().enumerate() {
            // 检查是否是数组索引访问，如 choices[0] 或 0
            if let Some((field_name, index_str)) = self.parse_array_access(part) {
                // 处理 field_name[index] 格式
                if let Some(array_field) = current.get(field_name) {
                    if let Some(array) = array_field.as_array() {
                        if let Ok(index) = index_str.parse::<usize>() {
                            if let Some(element) = array.get(index) {
                                current = element;
                                continue;
                            } else {
                                warn!(
                                    path = %path,
                                    field_name = %field_name,
                                    index = %index_str,
                                    array_len = array.len(),
                                    "Array index out of bounds"
                                );
                                return None;
                            }
                        } else {
                            warn!(
                                path = %path,
                                index_str = %index_str,
                                "Invalid array index format"
                            );
                            return None;
                        }
                    } else {
                        warn!(
                            path = %path,
                            field_name = %field_name,
                            actual_type = ?array_field,
                            "Field is not an array"
                        );
                        return None;
                    }
                } else {
                    warn!(
                        path = %path,
                        field_name = %field_name,
                        available_fields = ?current.as_object().map(|obj| obj.keys().collect::<Vec<_>>()),
                        "Array field not found"
                    );
                    return None;
                }
            } else if let Ok(index) = part.parse::<usize>() {
                // 处理纯数字索引，如 usage.0.usageMetadata 中的 "0"
                if let Some(array) = current.as_array() {
                    if let Some(element) = array.get(index) {
                        current = element;
                        continue;
                    } else {
                        warn!(
                            path = %path,
                            index = %part,
                            array_len = array.len(),
                            part_index = i,
                            "Array index out of bounds for numeric index"
                        );
                        return None;
                    }
                } else {
                    warn!(
                        path = %path,
                        index = %part,
                        actual_type = ?current,
                        part_index = i,
                        "Field is not an array for numeric index"
                    );
                    return None;
                }
            } else {
                // 普通字段访问
                if let Some(next_value) = current.get(part) {
                    current = next_value;
                } else {
                    warn!(
                        path = %path,
                        missing_field = %part,
                        available_fields = ?current.as_object().map(|obj| obj.keys().collect::<Vec<_>>()),
                        part_index = i,
                        "Field not found in path"
                    );
                    return None;
                }
            }
        }

        debug!(
            path = %path,
            extracted_value = ?current,
            "Successfully extracted value from path"
        );
        Some(current.clone())
    }

    /// 解析数组访问语法，返回 (field_name, index_str) 或 None
    fn parse_array_access<'a>(&self, part: &'a str) -> Option<(&'a str, &'a str)> {
        if part.contains('[') && part.ends_with(']') {
            let bracket_pos = part.find('[')?;
            let field_name = &part[..bracket_pos];
            let index_str = &part[bracket_pos + 1..part.len() - 1];
            Some((field_name, index_str))
        } else {
            None
        }
    }

    /// 提取u32类型字段
    pub fn extract_u32(&self, response: &Value, field_name: &str) -> Option<u32> {
        self.extract_field(response, field_name)?
            .as_u64()
            .map(|v| v as u32)
    }

    /// 提取i32类型字段
    pub fn extract_i32(&self, response: &Value, field_name: &str) -> Option<i32> {
        self.extract_field(response, field_name)?
            .as_i64()
            .map(|v| v as i32)
    }

    /// 提取f64类型字段
    pub fn extract_f64(&self, response: &Value, field_name: &str) -> Option<f64> {
        self.extract_field(response, field_name)?.as_f64()
    }

    /// 提取字符串类型字段
    pub fn extract_string(&self, response: &Value, field_name: &str) -> Option<String> {
        self.extract_field(response, field_name)?
            .as_str()
            .map(|s| s.to_string())
    }

    /// 提取布尔类型字段
    pub fn extract_bool(&self, response: &Value, field_name: &str) -> Option<bool> {
        self.extract_field(response, field_name)?.as_bool()
    }

    /// 获取所有配置的字段名
    pub fn get_configured_fields(&self) -> Vec<String> {
        self.config.field_mappings.keys().cloned().collect()
    }

    /// 验证配置是否有效
    pub fn validate_config(&self, sample_response: &Value) -> Vec<String> {
        let mut errors = Vec::new();

        for (field_name, json_path) in &self.config.field_mappings {
            if self.extract_by_path(sample_response, json_path).is_none() {
                // 如果有默认值就不算错误
                if !self.config.default_values.contains_key(field_name) {
                    errors.push(format!(
                        "Field '{}' with path '{}' not found in sample response",
                        field_name, json_path
                    ));
                }
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_token_direct_mapping() {
        let config = TokenMappingConfig {
            token_mappings: [
                (
                    "tokens_prompt".to_string(),
                    TokenMapping::Direct {
                        path: "usageMetadata.promptTokenCount".to_string(),
                    },
                ),
                (
                    "tokens_completion".to_string(),
                    TokenMapping::Direct {
                        path: "usageMetadata.candidatesTokenCount".to_string(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };

        let extractor = TokenFieldExtractor::new(config);

        let response = json!({
            "usageMetadata": {
                "promptTokenCount": 4,
                "candidatesTokenCount": 1548,
                "totalTokenCount": 3256
            }
        });

        assert_eq!(
            extractor.extract_token_u32(&response, "tokens_prompt"),
            Some(4)
        );
        assert_eq!(
            extractor.extract_token_u32(&response, "tokens_completion"),
            Some(1548)
        );
    }

    #[test]
    fn test_token_expression_mapping() {
        let config = TokenMappingConfig {
            token_mappings: [(
                "tokens_total".to_string(),
                TokenMapping::Expression {
                    formula: "usageMetadata.promptTokenCount + usageMetadata.candidatesTokenCount"
                        .to_string(),
                    fallback: Some("usageMetadata.totalTokenCount".to_string()),
                },
            )]
            .into_iter()
            .collect(),
        };

        let extractor = TokenFieldExtractor::new(config);

        let response = json!({
            "usageMetadata": {
                "promptTokenCount": 4,
                "candidatesTokenCount": 1548,
                "totalTokenCount": 3256
            }
        });

        // 表达式计算应该得到4 + 1548 = 1552
        assert_eq!(
            extractor.extract_token_u32(&response, "tokens_total"),
            Some(1552)
        );
    }

    #[test]
    fn test_token_expression_fallback() {
        let config = TokenMappingConfig {
            token_mappings: [(
                "tokens_total".to_string(),
                TokenMapping::Expression {
                    formula: "usageMetadata.missingField + usageMetadata.anotherMissingField"
                        .to_string(),
                    fallback: Some("usageMetadata.totalTokenCount".to_string()),
                },
            )]
            .into_iter()
            .collect(),
        };

        let extractor = TokenFieldExtractor::new(config);

        let response = json!({
            "usageMetadata": {
                "promptTokenCount": 4,
                "candidatesTokenCount": 1548,
                "totalTokenCount": 3256
            }
        });

        // 表达式失败，应该使用fallback值
        assert_eq!(
            extractor.extract_token_u32(&response, "tokens_total"),
            Some(3256)
        );
    }

    #[test]
    fn test_token_default_mapping() {
        let config = TokenMappingConfig {
            token_mappings: [(
                "cache_create_tokens".to_string(),
                TokenMapping::Default { value: json!(0) },
            )]
            .into_iter()
            .collect(),
        };

        let extractor = TokenFieldExtractor::new(config);

        let response = json!({});

        assert_eq!(
            extractor.extract_token_u32(&response, "cache_create_tokens"),
            Some(0)
        );
    }

    #[test]
    fn test_token_conditional_mapping() {
        let config = TokenMappingConfig {
            token_mappings: [(
                "cache_read_tokens".to_string(),
                TokenMapping::Conditional {
                    condition: "exists(usageMetadata.thoughtsTokenCount)".to_string(),
                    true_value: "usageMetadata.thoughtsTokenCount".to_string(),
                    false_value: json!(0),
                },
            )]
            .into_iter()
            .collect(),
        };

        let extractor = TokenFieldExtractor::new(config);

        // 测试条件为真的情况
        let response_with_thoughts = json!({
            "usageMetadata": {
                "thoughtsTokenCount": 1704
            }
        });

        assert_eq!(
            extractor.extract_token_u32(&response_with_thoughts, "cache_read_tokens"),
            Some(1704)
        );

        // 测试条件为假的情况
        let response_without_thoughts = json!({
            "usageMetadata": {
                "promptTokenCount": 4
            }
        });

        assert_eq!(
            extractor.extract_token_u32(&response_without_thoughts, "cache_read_tokens"),
            Some(0)
        );
    }

    #[test]
    fn test_token_fallback_mapping() {
        let config = TokenMappingConfig {
            token_mappings: [(
                "cache_create_tokens".to_string(),
                TokenMapping::Fallback {
                    paths: vec![
                        "usage.prompt_tokens_details.cached_tokens".to_string(),
                        "usage.cached_tokens".to_string(),
                        "0".to_string(),
                    ],
                },
            )]
            .into_iter()
            .collect(),
        };

        let extractor = TokenFieldExtractor::new(config);

        // 测试第一个路径存在的情况
        let response1 = json!({
            "usage": {
                "prompt_tokens_details": {
                    "cached_tokens": 42
                }
            }
        });

        assert_eq!(
            extractor.extract_token_u32(&response1, "cache_create_tokens"),
            Some(42)
        );

        // 测试第二个路径存在的情况
        let response2 = json!({
            "usage": {
                "cached_tokens": 24
            }
        });

        assert_eq!(
            extractor.extract_token_u32(&response2, "cache_create_tokens"),
            Some(24)
        );
    }

    #[test]
    fn test_token_config_from_json() {
        let json_config = r#"{
            "tokens_prompt": {
                "type": "direct",
                "path": "usageMetadata.promptTokenCount"
            },
            "tokens_total": {
                "type": "expression",
                "formula": "usageMetadata.promptTokenCount + usageMetadata.candidatesTokenCount",
                "fallback": "usageMetadata.totalTokenCount"
            },
            "cache_create_tokens": {
                "type": "default",
                "value": 0
            }
        }"#;

        let config = TokenMappingConfig::from_json(json_config).unwrap();
        let extractor = TokenFieldExtractor::new(config);

        let response = json!({
            "usageMetadata": {
                "promptTokenCount": 4,
                "candidatesTokenCount": 1548,
                "totalTokenCount": 3256
            }
        });

        assert_eq!(
            extractor.extract_token_u32(&response, "tokens_prompt"),
            Some(4)
        );
        assert_eq!(
            extractor.extract_token_u32(&response, "tokens_total"),
            Some(1552)
        ); // 4 + 1548
        assert_eq!(
            extractor.extract_token_u32(&response, "cache_create_tokens"),
            Some(0)
        );
    }

    #[test]
    fn test_simple_field_extraction() {
        let config = FieldMappingConfig {
            field_mappings: [
                (
                    "input_tokens".to_string(),
                    "usage.prompt_tokens".to_string(),
                ),
                ("model_name".to_string(), "model".to_string()),
            ]
            .into_iter()
            .collect(),
            default_values: HashMap::new(),
            transformations: HashMap::new(),
        };

        let extractor = FieldExtractor::new(config);

        let response = json!({
            "model": "gpt-4",
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        });

        assert_eq!(extractor.extract_u32(&response, "input_tokens"), Some(100));
        assert_eq!(
            extractor.extract_string(&response, "model_name"),
            Some("gpt-4".to_string())
        );
    }

    #[test]
    fn test_array_index_extraction() {
        let config = FieldMappingConfig {
            field_mappings: [(
                "content".to_string(),
                "choices[0].message.content".to_string(),
            )]
            .into_iter()
            .collect(),
            default_values: HashMap::new(),
            transformations: HashMap::new(),
        };

        let extractor = FieldExtractor::new(config);

        let response = json!({
            "choices": [{
                "message": {
                    "content": "Hello, world!"
                }
            }]
        });

        assert_eq!(
            extractor.extract_string(&response, "content"),
            Some("Hello, world!".to_string())
        );
    }

    #[test]
    fn test_default_values() {
        let config = FieldMappingConfig {
            field_mappings: HashMap::new(),
            default_values: [
                ("cache_tokens".to_string(), json!(0)),
                ("currency".to_string(), json!("USD")),
            ]
            .into_iter()
            .collect(),
            transformations: HashMap::new(),
        };

        let extractor = FieldExtractor::new(config);
        let response = json!({});

        assert_eq!(extractor.extract_u32(&response, "cache_tokens"), Some(0));
        assert_eq!(
            extractor.extract_string(&response, "currency"),
            Some("USD".to_string())
        );
    }

    #[test]
    fn test_transformations() {
        let config = FieldMappingConfig {
            field_mappings: [("cost".to_string(), "billing.total".to_string())]
                .into_iter()
                .collect(),
            default_values: HashMap::new(),
            transformations: [("cost".to_string(), TransformRule::Divide(1000.0))]
                .into_iter()
                .collect(),
        };

        let extractor = FieldExtractor::new(config);

        let response = json!({
            "billing": {
                "total": 5000.0
            }
        });

        assert_eq!(extractor.extract_f64(&response, "cost"), Some(5.0));
    }

    #[test]
    fn test_config_serialization() {
        let config = FieldMappingConfig {
            field_mappings: [(
                "input_tokens".to_string(),
                "usage.prompt_tokens".to_string(),
            )]
            .into_iter()
            .collect(),
            default_values: [("cache_tokens".to_string(), json!(0))]
                .into_iter()
                .collect(),
            transformations: [("cost".to_string(), TransformRule::Divide(1000.0))]
                .into_iter()
                .collect(),
        };

        let json_str = config.to_json().unwrap();
        let parsed_config = FieldMappingConfig::from_json(&json_str).unwrap();

        assert_eq!(config.field_mappings, parsed_config.field_mappings);
        assert_eq!(config.default_values, parsed_config.default_values);
    }
}

/// 模型提取规则类型
#[derive(Debug, Clone, PartialEq)]
pub enum ModelExtractionRule {
    /// 从URL路径使用正则表达式提取
    UrlRegex { pattern: String, priority: u8 },
    /// 从请求body的JSON字段提取
    BodyJson { path: String, priority: u8 },
    /// 从query参数提取
    QueryParam { parameter: String, priority: u8 },
}

/// 模型提取配置
#[derive(Debug, Clone)]
pub struct ModelExtractionConfig {
    pub extraction_rules: Vec<ModelExtractionRule>,
    pub fallback_model: String,
}

impl ModelExtractionConfig {
    /// 从JSON字符串解析配置
    pub fn from_json(json_str: &str) -> Result<Self> {
        let json: Value = serde_json::from_str(json_str)?;

        let fallback_model = json
            .get("fallback_model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let mut extraction_rules = Vec::new();

        if let Some(rules) = json.get("extraction_rules").and_then(|r| r.as_array()) {
            for rule in rules {
                if let Some(rule_type) = rule.get("type").and_then(|t| t.as_str()) {
                    let priority = rule.get("priority").and_then(|p| p.as_u64()).unwrap_or(0) as u8;

                    match rule_type {
                        "url_regex" => {
                            if let Some(pattern) = rule.get("pattern").and_then(|p| p.as_str()) {
                                extraction_rules.push(ModelExtractionRule::UrlRegex {
                                    pattern: pattern.to_string(),
                                    priority,
                                });
                            }
                        }
                        "body_json" => {
                            if let Some(path) = rule.get("path").and_then(|p| p.as_str()) {
                                extraction_rules.push(ModelExtractionRule::BodyJson {
                                    path: path.to_string(),
                                    priority,
                                });
                            }
                        }
                        "query_param" => {
                            if let Some(parameter) = rule.get("parameter").and_then(|p| p.as_str())
                            {
                                extraction_rules.push(ModelExtractionRule::QueryParam {
                                    parameter: parameter.to_string(),
                                    priority,
                                });
                            }
                        }
                        _ => {
                            warn!("Unknown model extraction rule type: {}", rule_type);
                        }
                    }
                }
            }
        }

        // 按优先级排序（优先级低的数字在前）
        extraction_rules.sort_by_key(|rule| match rule {
            ModelExtractionRule::UrlRegex { priority, .. } => *priority,
            ModelExtractionRule::BodyJson { priority, .. } => *priority,
            ModelExtractionRule::QueryParam { priority, .. } => *priority,
        });

        Ok(ModelExtractionConfig {
            extraction_rules,
            fallback_model,
        })
    }
}

/// 模型提取器
#[derive(Debug)]
pub struct ModelExtractor {
    config: ModelExtractionConfig,
    regex_cache: Mutex<HashMap<String, Regex>>,
}

impl ModelExtractor {
    /// 从JSON配置创建模型提取器
    pub fn from_json_config(json_str: &str) -> Result<Self> {
        let config = ModelExtractionConfig::from_json(json_str)?;
        Ok(Self {
            config,
            regex_cache: Mutex::new(HashMap::new()),
        })
    }

    /// 提取模型名称
    pub fn extract_model_name(
        &self,
        url_path: &str,
        request_body: Option<&Value>,
        query_params: &HashMap<String, String>,
    ) -> String {
        for rule in &self.config.extraction_rules {
            match rule {
                ModelExtractionRule::UrlRegex { pattern, .. } => {
                    if let Some(model) = self.extract_from_url(url_path, pattern) {
                        debug!("Extracted model from URL: {}", model);
                        return model;
                    }
                }
                ModelExtractionRule::BodyJson { path, .. } => {
                    if let Some(body) = request_body {
                        if let Some(model) = self.extract_from_json(body, path) {
                            debug!("Extracted model from body JSON: {}", model);
                            return model;
                        }
                    }
                }
                ModelExtractionRule::QueryParam { parameter, .. } => {
                    if let Some(model) = query_params.get(parameter) {
                        debug!("Extracted model from query param: {}", model);
                        return model.clone();
                    }
                }
            }
        }

        debug!("Using fallback model: {}", self.config.fallback_model);
        self.config.fallback_model.clone()
    }

    /// 从URL路径提取模型名
    fn extract_from_url(&self, url_path: &str, pattern: &str) -> Option<String> {
        // 首先尝试从缓存中获取regex
        {
            let cache = self.regex_cache.lock().ok()?;
            if let Some(regex) = cache.get(pattern) {
                return regex
                    .captures(url_path)
                    .and_then(|captures| captures.get(1))
                    .map(|m| m.as_str().to_string());
            }
        }

        // 如果缓存中没有，创建新的regex并缓存
        match Regex::new(pattern) {
            Ok(new_regex) => {
                let result = new_regex
                    .captures(url_path)
                    .and_then(|captures| captures.get(1))
                    .map(|m| m.as_str().to_string());

                // 将新regex存入缓存
                if let Ok(mut cache) = self.regex_cache.lock() {
                    cache.insert(pattern.to_string(), new_regex);
                }
                result
            }
            Err(e) => {
                warn!("Invalid regex pattern '{}': {}", pattern, e);
                None
            }
        }
    }

    /// 从JSON中提取模型名
    fn extract_from_json(&self, json: &Value, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = json;

        for part in parts {
            current = current.get(part)?;
        }

        current.as_str().map(|s| s.to_string())
    }
}

#[cfg(test)]
mod model_extractor_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_model_extraction_from_url() {
        let config_json = r#"
        {
            "extraction_rules": [
                {
                    "type": "url_regex",
                    "pattern": "/v1beta/models/([^:]+):generateContent",
                    "priority": 1,
                    "description": "从URL路径提取模型名"
                }
            ],
            "fallback_model": "gemini-pro"
        }
        "#;

        let extractor = ModelExtractor::from_json_config(config_json).unwrap();
        let model = extractor.extract_model_name(
            "/v1beta/models/gemini-pro-vision:generateContent",
            None,
            &HashMap::new(),
        );

        assert_eq!(model, "gemini-pro-vision");
    }

    #[test]
    fn test_model_extraction_from_body() {
        let config_json = r#"
        {
            "extraction_rules": [
                {
                    "type": "body_json",
                    "path": "model",
                    "priority": 1,
                    "description": "从请求body提取模型名"
                }
            ],
            "fallback_model": "default-model"
        }
        "#;

        let extractor = ModelExtractor::from_json_config(config_json).unwrap();
        let body = json!({"model": "gpt-4", "messages": []});
        let model =
            extractor.extract_model_name("/v1/chat/completions", Some(&body), &HashMap::new());

        assert_eq!(model, "gpt-4");
    }

    #[test]
    fn test_model_extraction_from_query() {
        let config_json = r#"
        {
            "extraction_rules": [
                {
                    "type": "query_param",
                    "parameter": "model",
                    "priority": 1,
                    "description": "从query参数提取模型名"
                }
            ],
            "fallback_model": "default-model"
        }
        "#;

        let extractor = ModelExtractor::from_json_config(config_json).unwrap();
        let mut query_params = HashMap::new();
        query_params.insert("model".to_string(), "claude-3-sonnet".to_string());

        let model = extractor.extract_model_name("/v1/messages", None, &query_params);

        assert_eq!(model, "claude-3-sonnet");
    }

    #[test]
    fn test_model_extraction_priority() {
        let config_json = r#"
        {
            "extraction_rules": [
                {
                    "type": "query_param",
                    "parameter": "model",
                    "priority": 3,
                    "description": "从query参数提取模型名"
                },
                {
                    "type": "body_json",
                    "path": "model",
                    "priority": 2,
                    "description": "从请求body提取模型名"
                },
                {
                    "type": "url_regex",
                    "pattern": "/v1beta/models/([^:]+):generateContent",
                    "priority": 1,
                    "description": "从URL路径提取模型名"
                }
            ],
            "fallback_model": "fallback-model"
        }
        "#;

        let extractor = ModelExtractor::from_json_config(config_json).unwrap();
        let body = json!({"model": "gpt-4"});
        let mut query_params = HashMap::new();
        query_params.insert("model".to_string(), "claude-3".to_string());

        // URL优先级最高，应该返回URL中的模型
        let model = extractor.extract_model_name(
            "/v1beta/models/gemini-pro:generateContent",
            Some(&body),
            &query_params,
        );

        assert_eq!(model, "gemini-pro");
    }

    #[test]
    fn test_model_extraction_fallback() {
        let config_json = r#"
        {
            "extraction_rules": [
                {
                    "type": "body_json",
                    "path": "model",
                    "priority": 1,
                    "description": "从请求body提取模型名"
                }
            ],
            "fallback_model": "fallback-model"
        }
        "#;

        let extractor = ModelExtractor::from_json_config(config_json).unwrap();
        let body = json!({"messages": []}); // 没有model字段

        let model =
            extractor.extract_model_name("/v1/chat/completions", Some(&body), &HashMap::new());

        assert_eq!(model, "fallback-model");
    }
}
