use super::{PaginationParams, ServiceResponse, TimeRangeDefault, build_page, resolve_range};
use crate::management::response::Pagination;
use crate::types::timezone_utils;
use chrono::{Duration, NaiveDate, Utc};
use chrono_tz::Asia::Shanghai;

#[test]
fn pagination_params_apply_defaults_and_limits() {
    let params = PaginationParams::new(Some(0), Some(200), 20, 100);
    assert_eq!(params.page, 1, "page 应回退到最小值 1");
    assert_eq!(params.limit, 100, "limit 应被限制在最大值内");
    assert_eq!(params.offset(), 0, "第一页 offset 应为 0");
}

#[test]
fn build_page_computes_pages_and_into_response() {
    let params = PaginationParams::new(Some(2), Some(15), 20, 50);
    let info = build_page(95, params);

    assert_eq!(info.page, 2);
    assert_eq!(info.limit, 15);
    assert_eq!(info.total, 95);
    assert_eq!(info.pages, 7);

    let response: Pagination = info.into();
    assert_eq!(response.page, 2);
    assert_eq!(response.limit, 15);
    assert_eq!(response.total, 95);
    assert_eq!(response.pages, 7);
}

#[test]
fn service_response_supports_message() {
    let response = ServiceResponse::with_message("payload", "ok");
    assert_eq!(response.data, "payload");
    assert_eq!(response.message.as_deref(), Some("ok"));
}

#[test]
fn resolve_range_supports_today_keyword() {
    let bounds = resolve_range(
        Some("today"),
        None,
        None,
        &Shanghai,
        TimeRangeDefault::LastDays(1),
    )
    .expect("today 关键字应解析成功");

    let duration = bounds.end - bounds.start;
    assert_eq!(duration, Duration::hours(24));
    let now = Utc::now();
    assert!(
        bounds.start <= now && bounds.end > now,
        "today 范围应覆盖到当前时刻"
    );
}

#[test]
fn resolve_range_custom_validates_inputs() {
    let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
    let end_date = NaiveDate::from_ymd_opt(2024, 1, 2).expect("valid date");
    let start = start_date
        .and_hms_opt(9, 0, 0)
        .expect("valid start datetime");
    let end = end_date.and_hms_opt(10, 0, 0).expect("valid end datetime");

    let bounds = resolve_range(
        Some("custom"),
        Some(start),
        Some(end),
        &Shanghai,
        TimeRangeDefault::LastDays(1),
    )
    .expect("自定义时间范围应解析成功");
    let (expected_start, expected_end) =
        timezone_utils::convert_range_to_utc(&start, &end, &Shanghai)
            .expect("convert_range_to_utc 应成功");
    assert_eq!(bounds.start, expected_start);
    assert_eq!(bounds.end, expected_end);

    let missing_end = resolve_range(
        Some("custom"),
        Some(start),
        None,
        &Shanghai,
        TimeRangeDefault::LastDays(1),
    );
    assert!(missing_end.is_err(), "缺少结束时间应返回错误");
}

#[test]
fn resolve_range_keyword_duration() {
    let bounds = resolve_range(
        Some("6h"),
        None,
        None,
        &Shanghai,
        TimeRangeDefault::LastDays(1),
    )
    .expect("6h 关键字应解析成功");

    let duration = bounds.end - bounds.start;
    assert_eq!(duration, Duration::hours(6));
}
