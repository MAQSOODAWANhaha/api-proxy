use sea_orm_migration::prelude::*;
use std::env;

#[tokio::main]
async fn main() {
    // 如果没有设置 DATABASE_URL 环境变量，则默认设置为 data/dev.db
    if env::var("DATABASE_URL").is_err() {
        let db_path = if env::current_dir().unwrap().ends_with("migration") {
            "../data/dev.db"
        } else {
            "data/dev.db"
        };
        unsafe {
            env::set_var("DATABASE_URL", format!("sqlite://{}", db_path));
        }
    }
    cli::run_cli(migration::Migrator).await;
}
