//! # 测试框架模块
//!
//! 提供测试工具、fixtures 和测试辅助函数

#[cfg(any(test, feature = "testing"))]
pub mod fixtures;
#[cfg(any(test, feature = "testing"))]
pub mod helpers;
#[cfg(any(test, feature = "testing"))]
pub mod mocks;

#[cfg(any(test, feature = "testing"))]
pub use fixtures::*;
#[cfg(any(test, feature = "testing"))]
pub use helpers::*;
#[cfg(any(test, feature = "testing"))]
pub use mocks::*;