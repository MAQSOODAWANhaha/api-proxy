//! # 性能基准测试
//!
//! 使用 Criterion 进行性能基准测试

use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub use cache_benchmarks::*;
pub use database_benchmarks::*;

criterion_group!(benches, cache_benchmark, database_benchmark);
criterion_main!(benches);
