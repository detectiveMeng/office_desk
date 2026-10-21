//! 领域层：纯函数 + 纯数据结构，不依赖 GPUI / SQLite。

pub mod types;
pub mod date;
pub mod todo;
pub mod project;
pub mod stats;

pub use types::*;