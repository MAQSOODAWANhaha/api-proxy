use serde::Serialize;

/// 分页参数
#[derive(Debug, Clone, Copy)]
pub struct PaginationParams {
    /// 当前页码（>= 1）
    pub page: u64,
    /// 每页条数（>= 1）
    pub limit: u64,
}

impl PaginationParams {
    /// 根据可选参数创建分页配置，并应用默认值与上限。
    #[must_use]
    pub fn new(page: Option<u64>, limit: Option<u64>, default_limit: u64, max_limit: u64) -> Self {
        let page = page.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(default_limit).clamp(1, max_limit);
        Self { page, limit }
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        (self.page - 1) * self.limit
    }
}

/// 标准分页信息
#[derive(Debug, Clone, Serialize)]
pub struct PaginationInfo {
    pub page: u64,
    pub limit: u64,
    pub total: u64,
    pub pages: u64,
}

impl PaginationInfo {
    #[must_use]
    pub const fn new(page: u64, limit: u64, total: u64, pages: u64) -> Self {
        Self {
            page,
            limit,
            total,
            pages,
        }
    }
}

impl From<PaginationInfo> for crate::management::response::Pagination {
    fn from(value: PaginationInfo) -> Self {
        Self {
            page: value.page,
            limit: value.limit,
            total: value.total,
            pages: value.pages,
        }
    }
}

/// 根据总数和分页参数计算分页信息。
#[must_use]
pub const fn build_page(total: u64, params: PaginationParams) -> PaginationInfo {
    let pages = if total == 0 {
        0
    } else {
        total.div_ceil(params.limit)
    };
    PaginationInfo::new(params.page, params.limit, total, pages)
}
