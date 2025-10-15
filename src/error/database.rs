use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("数据库连接失败: {0}")]
    Connection(String),

    #[error("查询执行失败: {0}")]
    Query(#[from] sea_orm::DbErr),

    #[error("记录未找到: {0}")]
    NotFound(String),

    #[error("事务错误: {0}")]
    Transaction(String),
}
