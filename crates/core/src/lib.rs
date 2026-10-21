//! 办公工作台 —— 库根。
//! 领域层纯函数逻辑放在 `domain` 模块，与 GPUI / SQLite 解耦，可被 `cargo test` 独立覆盖。

pub mod domain;