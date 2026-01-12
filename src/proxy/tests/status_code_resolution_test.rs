//! # 状态码解析逻辑测试
//!
//! 测试各种连接失败场景下的状态码解析，确保修复的正确性

use crate::proxy::service::ProxyService;
use crate::proxy::context::ProxyContext;
use pingora_core::Error;
use pingora_core::ErrorType;
use std::time::Instant;

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的上下文
    fn create_test_context() -> ProxyContext {
        let mut ctx = ProxyContext::default();
        ctx.request_id = "test-request-123".to_string();
        ctx.start_time = Instant::now();
        ctx.response.details = Default::default();
        ctx
    }

    /// 创建测试用的Pingora错误
    fn create_test_error(error_type: ErrorType) -> Error {
        Error::new(error_type)
    }

    #[test]
    fn test_connection_failure_returns_502() {
        let ctx = create_test_context();

        // 测试连接关闭错误
        let error = create_test_error(ErrorType::ConnectionClosed);
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 502, "连接关闭应该返回502");
    }

    #[test]
    fn test_connection_timeout_returns_502() {
        let ctx = create_test_context();

        // 测试连接超时错误
        let error = create_test_error(ErrorType::ConnectTimedout);
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 502, "连接超时应该返回502");
    }

    #[test]
    fn test_read_timeout_returns_504() {
        let ctx = create_test_context();

        // 测试读取超时错误
        let error = create_test_error(ErrorType::ReadTimedout);
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 504, "读取超时应该返回504");
    }

    #[test]
    fn test_write_timeout_returns_504() {
        let ctx = create_test_context();

        // 测试写入超时错误
        let error = create_test_error(ErrorType::WriteTimedout);
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 504, "写入超时应该返回504");
    }

    #[test]
    fn test_connection_closed_variations() {
        let ctx = create_test_context();

        // 测试连接关闭错误的各种情况
        let error = create_test_error(ErrorType::ConnectionClosed);
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 502, "连接关闭应该返回502");
    }

    #[test]
    fn test_http_status_zero_returns_502() {
        let ctx = create_test_context();

        // 测试HTTP状态码为0的错误（连接中断）
        let error = create_test_error(ErrorType::HTTPStatus(0));
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 502, "HTTP状态码0应该返回502");
    }

    #[test]
    fn test_http_status_non_zero_returns_original() {
        let ctx = create_test_context();

        // 测试非零HTTP状态码
        let error = create_test_error(ErrorType::HTTPStatus(404));
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 404, "非零HTTP状态码应该原样返回");
    }

    #[test]
    fn test_custom_error_5xx_returns_code() {
        let ctx = create_test_context();

        // 测试5xx自定义错误
        let error = create_test_error(ErrorType::CustomCode("test".to_string(), 503));
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 503, "5xx自定义错误应该原样返回");
    }

    #[test]
    fn test_custom_error_4xx_fallback_to_502() {
        let ctx = create_test_context();

        // 测试4xx自定义错误（应该回退到502，因为不是连接错误）
        let error = create_test_error(ErrorType::CustomCode("test".to_string(), 400));
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 502, "4xx自定义错误应该返回502");
    }

    #[test]
    fn test_context_status_code_ignored_with_connection_error() {
        let mut ctx = create_test_context();
        ctx.response.details.status_code = Some(200); // 设置一个成功的状态码

        // 但是有连接错误
        let error = create_test_error(ErrorType::ConnectionFailed);
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        // 应该优先使用连接错误的502，而不是上下文中的200
        assert_eq!(status_code, 502, "连接错误时应该忽略上下文中的状态码");
    }

    #[test]
    fn test_context_status_code_used_without_error() {
        let mut ctx = create_test_context();
        ctx.response.details.status_code = Some(200);

        // 没有错误时使用上下文中的状态码
        let status_code = ProxyService::resolve_status_code(&ctx, None);

        assert_eq!(status_code, 200, "没有错误时应该使用上下文中的状态码");
    }

    #[test]
    fn test_no_error_no_status_code_returns_200() {
        let ctx = create_test_context();

        // 没有错误也没有状态码时默认200
        let status_code = ProxyService::resolve_status_code(&ctx, None);

        assert_eq!(status_code, 200, "没有错误和状态码时应该返回200");
    }

    #[test]
    fn test_unknown_error_returns_500() {
        let ctx = create_test_context();

        // 未知错误类型
        let error = create_test_error(ErrorType::Retry); // 这个不在连接失败列表中
        let status_code = ProxyService::resolve_status_code(&ctx, Some(&error));

        assert_eq!(status_code, 500, "未知错误应该返回500");
    }

    #[test]
    fn test_is_connection_failure_function() {
        // 测试连接失败检测函数

        // 连接关闭错误
        let connection_error = create_test_error(ErrorType::ConnectionClosed);
        assert!(ProxyService::is_connection_failure(Some(&connection_error)));

        // 超时错误
        let timeout_error = create_test_error(ErrorType::ReadTimedout);
        assert!(ProxyService::is_connection_failure(Some(&timeout_error)));

        // HTTP状态码0
        let http_zero_error = create_test_error(ErrorType::HTTPStatus(0));
        assert!(ProxyService::is_connection_failure(Some(&http_zero_error)));

        // 非连接失败错误
        let retry_error = create_test_error(ErrorType::Retry);
        assert!(!ProxyService::is_connection_failure(Some(&retry_error)));

        // 没有错误
        assert!(!ProxyService::is_connection_failure(None));
    }

    #[test]
    fn test_partial_response_error_detection() {
        let mut ctx = create_test_context();
        ctx.response.details.status_code = Some(200); // 有响应状态码

        // 有连接错误
        let error = create_test_error(ErrorType::ConnectionClosed);

        // 应该检测为部分响应错误
        assert!(ProxyService::is_partial_response_error(&ctx, Some(&error)));

        // 没有连接错误时
        assert!(!ProxyService::is_partial_response_error(&ctx, None));

        // 没有响应状态码时
        ctx.response.details.status_code = None;
        assert!(!ProxyService::is_partial_response_error(&ctx, Some(&error)));
    }
}
