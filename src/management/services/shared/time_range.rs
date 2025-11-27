use std::ops::Range;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use chrono_tz::Tz;

use crate::ensure;
use crate::error::Result;
use crate::types::timezone_utils;

/// 通用时间范围
#[derive(Debug, Clone)]
pub struct TimeRangeBounds {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRangeBounds {
    #[must_use]
    pub const fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }
}

impl From<TimeRangeBounds> for Range<DateTime<Utc>> {
    fn from(bounds: TimeRangeBounds) -> Self {
        bounds.start..bounds.end
    }
}

/// 默认时间范围
#[derive(Debug, Clone, Copy)]
pub enum TimeRangeDefault {
    LastHours(i64),
    LastDays(i64),
}

/// 基于字符串与自定义时间解析时间范围。
pub fn resolve_range(
    keyword: Option<&str>,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
    timezone: &Tz,
    default: TimeRangeDefault,
) -> Result<TimeRangeBounds> {
    let now = Utc::now();
    match keyword {
        Some("today") => {
            let (start, end) =
                timezone_utils::local_day_bounds(&now, timezone).ok_or_else(|| {
                    crate::error::conversion::ConversionError::Message(format!(
                        "Failed to resolve local day bounds for timezone {timezone}"
                    ))
                })?;
            Ok(TimeRangeBounds::new(start, end))
        }
        Some("custom") => {
            let (start, end) = match (start, end) {
                (Some(start), Some(end)) => timezone_utils::convert_range_to_utc(
                    &start, &end, timezone,
                )
                .ok_or_else(|| {
                    crate::error::conversion::ConversionError::Message(
                        "Invalid custom datetime range".to_string(),
                    )
                })?,
                _ => {
                    return Err(crate::error::conversion::ConversionError::Message(
                        "Custom range requires both start and end datetime".to_string(),
                    )
                    .into());
                }
            };
            ensure!(
                start < end,
                crate::error::conversion::ConversionError::Message(
                    "Start datetime must be earlier than end datetime".to_string()
                )
            );
            Ok(TimeRangeBounds::new(start, end))
        }
        Some(keyword) => parse_duration_keyword(keyword).map_or_else(
            || {
                Err(crate::error::conversion::ConversionError::Message(format!(
                    "Unsupported time range keyword: {keyword}"
                ))
                .into())
            },
            |duration| Ok(TimeRangeBounds::new(now - duration, now)),
        ),
        None => {
            let duration = match default {
                TimeRangeDefault::LastHours(hours) => Duration::hours(hours),
                TimeRangeDefault::LastDays(days) => Duration::days(days),
            };
            Ok(TimeRangeBounds::new(now - duration, now))
        }
    }
}

/// 支持的关键字：
/// - `1h`/`6h`/`24h`
/// - `7d`/`30d`
/// - `7days`/`30days`
/// - `7hours`/`24hours`
fn parse_duration_keyword(keyword: &str) -> Option<Duration> {
    let keyword = keyword.trim().to_lowercase();

    if let Some(hours) = parse_numeric_suffix(&keyword, "h") {
        return Some(Duration::hours(hours));
    }
    if let Some(hours) = parse_numeric_suffix(&keyword, "hours") {
        return Some(Duration::hours(hours));
    }
    if let Some(days) = parse_numeric_suffix(&keyword, "d") {
        return Some(Duration::days(days));
    }
    if let Some(days) = parse_numeric_suffix(&keyword, "days") {
        return Some(Duration::days(days));
    }

    None
}

fn parse_numeric_suffix(keyword: &str, suffix: &str) -> Option<i64> {
    keyword
        .strip_suffix(suffix)
        .and_then(|digits| digits.parse::<i64>().ok())
        .filter(|value| *value > 0)
}
