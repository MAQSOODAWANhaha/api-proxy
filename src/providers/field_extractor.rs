//! # 数据驱动字段提取器
//!
//! 基于数据库配置的通用字段提取器，支持JSONPath查询和数值转换

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;
use tracing::debug;

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

    /// 根据路径提取值，支持JSONPath语法
    fn extract_by_path(&self, data: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data;

        for part in parts {
            // 处理数组索引 如 choices[0]
            if part.contains('[') && part.ends_with(']') {
                let bracket_pos = part.find('[').unwrap();
                let field_name = &part[..bracket_pos];
                let index_str = &part[bracket_pos + 1..part.len() - 1];

                // 先获取数组字段
                if let Some(array_field) = current.get(field_name) {
                    if let Some(array) = array_field.as_array() {
                        if let Ok(index) = index_str.parse::<usize>() {
                            if let Some(element) = array.get(index) {
                                current = element;
                                continue;
                            }
                        }
                    }
                }
                return None;
            } else {
                // 普通字段访问
                if let Some(next_value) = current.get(part) {
                    current = next_value;
                } else {
                    return None;
                }
            }
        }

        Some(current.clone())
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
