//! # 服务层共享工具
//!
//! 提供分页、时间范围、统计指标等辅助方法，避免在各域服务中重复实现。
//! 推荐通过 `crate::management::services` 根模块的再导出进行访问。

pub mod metrics;
pub mod pagination;
pub mod response;
pub mod time_range;

pub use pagination::{PaginationInfo, PaginationParams, build_page};
pub use response::ServiceResponse;
pub use time_range::{TimeRangeBounds, TimeRangeDefault, resolve_range};

#[cfg(test)]
mod tests;
