use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Database connection failed: {0}")]
    Connection(String),

    #[error("Query execution failed: {0}")]
    Query(#[from] sea_orm::DbErr),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("Transaction error: {0}")]
    Transaction(String),
}
