use std::fmt;

use super::domain::{RequestCount, TimeoutSeconds, TokenCount};

#[derive(Debug)]
pub enum ConversionError {
    NegativeValue {
        field: &'static str,
        value: i64,
    },
    Overflow {
        field: &'static str,
    },
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeValue { field, value } => {
                write!(f, "field `{field}` expected non-negative value, got {value}")
            }
            Self::Overflow { field } => write!(f, "field `{field}` overflowed target type"),
        }
    }
}

impl std::error::Error for ConversionError {}

#[must_use]
pub fn timeout_from_i32(value: Option<i32>, fallback: TimeoutSeconds) -> TimeoutSeconds {
    value
        .map(|v| u64::try_from(v).unwrap_or(fallback.as_secs()))
        .map_or(fallback, TimeoutSeconds::new)
}

pub fn request_count_from_i64(value: i64, field: &'static str) -> Result<RequestCount, ConversionError> {
    if value < 0 {
        return Err(ConversionError::NegativeValue { field, value });
    }
    u64::try_from(value)
        .map_err(|_| ConversionError::Overflow { field })
}

pub fn token_count_from_i64(value: i64, field: &'static str) -> Result<TokenCount, ConversionError> {
    request_count_from_i64(value, field)
}

pub fn option_token_count_from_i64(
    value: Option<i64>,
    field: &'static str,
) -> Result<Option<TokenCount>, ConversionError> {
    value
        .map(|v| token_count_from_i64(v, field))
        .transpose()
}

const F64_EXACT_INTEGER_MAX: u64 = 1u64 << f64::MANTISSA_DIGITS;

/// 将无符号整数比值转换为浮点表示。
/// 对于监控与统计数据，`u64` 值通常远小于 `2^52`，因此在 `f64` 中仍能保持足够精度；
/// 超出时采用降采样（同时右移）保持比值稳定。
#[must_use]
pub fn ratio_as_f64(numerator: u64, denominator: u64) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    if numerator == 0 {
        return Some(0.0);
    }

    let mut num = numerator;
    let mut den = denominator;

    // 将数值缩放到 f64 精度范围内，避免极端情况下的精度损失
    while num > F64_EXACT_INTEGER_MAX || den > F64_EXACT_INTEGER_MAX {
        num >>= 1;
        den >>= 1;
        if den == 0 {
            return None;
        }
    }

    #[allow(clippy::cast_precision_loss)] // 缩放后数值已位于安全范围内
    Some(num as f64 / den as f64)
}

/// 以百分比形式返回比值表示。
#[must_use]
pub fn ratio_as_percentage(numerator: u64, denominator: u64) -> f64 {
    ratio_as_f64(numerator, denominator).map_or(0.0, |ratio| ratio * 100.0)
}
