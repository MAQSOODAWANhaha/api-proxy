//! # 时区转换类型和工具
//!
//! 提供时区相关的类型转换和工具函数

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

/// 时区上下文，用于在请求中传递时区信息
#[derive(Debug, Clone)]
pub struct TimezoneContext {
    pub timezone: Tz,
}

/// 一个将本地时间安全转换为UTC时间的工具 Trait
pub trait ConvertToUtc {
    /// 接受一个时区作为参数，返回一个UTC的DateTime
    fn to_utc(&self, tz: &Tz) -> Option<DateTime<Utc>>;
}

// 为 NaiveDateTime 实现这个 Trait
impl ConvertToUtc for NaiveDateTime {
    fn to_utc(&self, tz: &Tz) -> Option<DateTime<Utc>> {
        // 使用 .single() 来安全处理夏令时切换等边界情况
        // 如果本地时间存在歧义或不存在，它会返回 None
        tz.from_local_datetime(self)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
    }
}

// 为 Option<NaiveDateTime> 实现，方便直接调用
impl ConvertToUtc for Option<NaiveDateTime> {
    fn to_utc(&self, tz: &Tz) -> Option<DateTime<Utc>> {
        // 如果 self 是 Some，则调用 NaiveDateTime 的 to_utc 方法
        self.as_ref().and_then(|naive_dt| naive_dt.to_utc(tz))
    }
}

/// 时区工具函数
pub mod timezone_utils {
    use super::{DateTime, NaiveDateTime, TimeZone, Tz, Utc};
    use chrono::Offset;

    /// 验证时区字符串是否有效
    #[must_use]
    pub fn is_valid_timezone(timezone_str: &str) -> bool {
        timezone_str.parse::<Tz>().is_ok()
    }

    /// 解析时区字符串，失败时返回UTC
    #[must_use]
    pub fn parse_timezone_safe(timezone_str: &str) -> Tz {
        timezone_str.parse::<Tz>().unwrap_or(Tz::UTC)
    }

    /// 获取当前时区的UTC偏移量（分钟）
    #[must_use]
    pub fn get_timezone_offset(timezone: &Tz) -> i32 {
        let now = Utc::now();
        let local_time = timezone.from_utc_datetime(&now.naive_utc());
        local_time.offset().fix().local_minus_utc()
    }

    /// 安全地转换字符串为 `NaiveDateTime`
    #[must_use]
    pub fn parse_naive_datetime_safe(datetime_str: &str) -> Option<NaiveDateTime> {
        // 尝试多种常见格式
        let datetime_formats = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S"];

        // 先尝试带时间的格式
        for format in &datetime_formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(datetime_str, format) {
                return Some(dt);
            }
        }

        // 如果失败了，尝试只有日期的格式，使用午夜时间
        if let Ok(date) = chrono::NaiveDate::parse_from_str(datetime_str, "%Y-%m-%d") {
            return date.and_hms_opt(0, 0, 0);
        }

        None
    }

    /// 将 `DateTime<Utc>` 格式化为字符串
    #[must_use]
    pub fn format_utc_to_string(dt: &DateTime<Utc>) -> String {
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    /// 获取时区的友好显示名称
    #[must_use]
    pub fn get_timezone_display_name(timezone: &Tz) -> String {
        match timezone.name() {
            "UTC" => "UTC (协调世界时)".to_string(),
            "Asia/Shanghai" => "Asia/Shanghai (中国标准时间)".to_string(),
            "America/New_York" => "America/New_York (美国东部时间)".to_string(),
            "Europe/London" => "Europe/London (格林威治标准时间)".to_string(),
            _ => format!("{} ({})", timezone.name(), timezone.name()),
        }
    }

    /// 获取常见时区列表
    #[must_use]
    pub fn get_common_timezones() -> Vec<&'static str> {
        vec![
            "UTC",
            "America/New_York",
            "America/Los_Angeles",
            "America/Chicago",
            "Europe/London",
            "Europe/Paris",
            "Europe/Berlin",
            "Asia/Shanghai",
            "Asia/Tokyo",
            "Asia/Hong_Kong",
            "Asia/Singapore",
            "Australia/Sydney",
        ]
    }

    // ===== 响应时间转换工具函数 =====

    /// 将UTC时间转换为用户时区的格式化字符串
    /// 用于API响应中的时间字段转换
    #[must_use]
    pub fn format_utc_for_response(utc_dt: &DateTime<Utc>, timezone: &Tz) -> String {
        utc_dt
            .with_timezone(timezone)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }

    /// 将可选的UTC时间转换为用户时区的格式化字符串
    /// 用于处理可能为空的时间字段
    #[must_use]
    pub fn format_option_utc_for_response(
        utc_dt: Option<&DateTime<Utc>>,
        timezone: &Tz,
    ) -> Option<String> {
        utc_dt.map(|dt| format_utc_for_response(dt, timezone))
    }

    /// 将NaiveDateTime（假设为UTC）转换为用户时区的格式化字符串
    #[must_use]
    pub fn format_naive_utc_for_response(naive_dt: &NaiveDateTime, timezone: &Tz) -> String {
        let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(*naive_dt, Utc);
        format_utc_for_response(&utc_dt, timezone)
    }

    /// 将可选的`NaiveDateTime`转换为用户时区的格式化字符串
    #[must_use]
    pub fn format_option_naive_utc_for_response(
        naive_dt: Option<&NaiveDateTime>,
        timezone: &Tz,
    ) -> Option<String> {
        naive_dt.map(|dt| format_naive_utc_for_response(dt, timezone))
    }

    /// 将UTC时间转换为RFC3339格式（保持UTC时区标识）
    /// 用于需要保持时间标识的场景
    #[must_use]
    pub fn format_utc_to_rfc3339(utc_dt: &DateTime<Utc>) -> String {
        utc_dt.to_rfc3339()
    }

    /// 将可选的UTC时间转换为RFC3339格式
    #[must_use]
    pub fn format_option_utc_to_rfc3339(utc_dt: Option<&DateTime<Utc>>) -> Option<String> {
        utc_dt.map(format_utc_to_rfc3339)
    }

    /// 批量转换响应结构体中的时间字段
    /// 这是一个通用工具，具体的响应结构需要实现相应的转换逻辑
    pub trait TimezoneResponseFormatter {
        /// 将时间字段转换为指定时区的字符串表示
        #[must_use]
      fn format_times_for_timezone(&self, timezone: &Tz) -> Self;
    }
}

#[cfg(test)]
mod tests {
    use super::timezone_utils;
    use super::{ConvertToUtc, NaiveDateTime, Tz};
    use chrono::NaiveDate;
    use chrono::Timelike;

    #[test]
    fn test_convert_to_utc_trait() {
        let tz = Tz::Asia__Shanghai;
        let naive_dt = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        let utc_dt = naive_dt.to_utc(&tz).unwrap();
        assert_eq!(utc_dt.hour(), 4); // 上海时间12点 = UTC 4点
    }

    #[test]
    fn test_option_convert_to_utc() {
        let tz = Tz::UTC;
        let some_dt = Some(
            NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
        );
        let none_dt: Option<NaiveDateTime> = None;

        let utc_some = some_dt.to_utc(&tz);
        let utc_none = none_dt.to_utc(&tz);

        assert!(utc_some.is_some());
        assert!(utc_none.is_none());
    }

    #[test]
    fn test_timezone_utils() {
        assert!(timezone_utils::is_valid_timezone("UTC"));
        assert!(timezone_utils::is_valid_timezone("Asia/Shanghai"));
        assert!(!timezone_utils::is_valid_timezone("Invalid/Timezone"));

        let tz = timezone_utils::parse_timezone_safe("Asia/Shanghai");
        assert_eq!(tz.name(), "Asia/Shanghai");

        let invalid_tz = timezone_utils::parse_timezone_safe("Invalid/Timezone");
        assert_eq!(invalid_tz.name(), "UTC");
    }

    #[test]
    fn test_parse_naive_datetime_safe() {
        assert!(timezone_utils::parse_naive_datetime_safe("2024-01-01 12:00:00").is_some());
        assert!(timezone_utils::parse_naive_datetime_safe("2024-01-01 12:00").is_some());
        assert!(timezone_utils::parse_naive_datetime_safe("2024-01-01").is_some());
        assert!(timezone_utils::parse_naive_datetime_safe("invalid").is_none());
    }

    #[test]
    fn test_response_time_converter() {
        use super::timezone_utils;
        use chrono::{DateTime, Utc};

        let tz = Tz::Asia__Shanghai;
        let utc_time = Utc::now();

        // 测试UTC时间转换
        let formatted = timezone_utils::format_utc_for_response(&utc_time, &tz);
        assert!(!formatted.is_empty());
        assert!(formatted.contains("2025") || formatted.contains("2024")); // 支持当前年份或2024年

        // 测试可选UTC时间转换
        let some_time = Some(&utc_time);
        let none_time: Option<&DateTime<Utc>> = None;

        assert!(timezone_utils::format_option_utc_for_response(some_time, &tz).is_some());
        assert!(timezone_utils::format_option_utc_for_response(none_time, &tz).is_none());

        // 测试RFC3339格式
        let rfc3339 = timezone_utils::format_utc_to_rfc3339(&utc_time);
        assert!(rfc3339.contains('Z') || rfc3339.contains("+00:00"));
    }
}
