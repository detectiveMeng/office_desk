//! 领域数据模型：纯数据结构，不依赖 GPUI / SQLite。
//! 字段口径与产品设计说明 v0.1 一一对应。

use chrono::{NaiveDate, NaiveDateTime};

/// 任务状态：待办 / 已完成
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TodoStatus {
    Todo,
    Done,
}

/// 项目状态：进行中 / 卡住了 / 已完成 / 暂停。
/// 「卡住了」由卡点字段自动驱动，不手动选择。
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ProjectStatus {
    Active,
    Blocked,
    Done,
    Paused,
}

/// 顺延流水：每次顺延追加一条 {from, to, at, auto}
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct PostponeEvent {
    pub from: Option<NaiveDate>,
    pub to: NaiveDate,
    pub at: NaiveDateTime,
    pub auto: bool,
}

/// 卡点流水：用于周复盘「卡点汇总」
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct BlockerEvent {
    pub text: String,
    pub start_at: NaiveDate,
    pub end_at: Option<NaiveDate>, // None = 仍未解除
}

/// 任务
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    /// 优先级 0..3，0 最高（对应 P0 紧急重要）
    pub priority: u8,
    /// 当前截止日，None = 待安排
    pub date: Option<NaiveDate>,
    /// 首次设定截止日，逾期判定依据，永不改
    pub original_date: Option<NaiveDate>,
    pub project_id: Option<String>,
    pub status: TodoStatus,
    pub done_at: Option<NaiveDateTime>,
    pub note: String,
    /// 顺延次数，系统维护
    pub postpone_count: u32,
    pub postpone_log: Vec<PostponeEvent>,
    pub created_at: NaiveDateTime,
}

/// 项目
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub current: String,
    pub next: String,
    pub blocker: String,
    pub status: ProjectStatus,
    pub updated_at: NaiveDateTime,
    pub blocker_log: Vec<BlockerEvent>,
}

/// 灵感
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Note {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// 周复盘记录（仅手写总结部分落库，图表数据实时派生）
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct WeeklyReview {
    pub week_start: NaiveDate,
    pub good: String,
    pub watch: String,
    pub focus: String,
    pub updated_at: Option<NaiveDateTime>,
}

/// 内存镜像：四类实体 + 元数据
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct DataSnapshot {
    pub todos: Vec<Todo>,
    pub projects: Vec<Project>,
    pub notes: Vec<Note>,
    pub reviews: Vec<WeeklyReview>,
    /// 上次自动顺延运行的日期，避免同日重复执行
    pub last_run_date: Option<NaiveDate>,
    pub seeded_at: Option<NaiveDateTime>,
}

impl DataSnapshot {
    pub fn new() -> Self {
        Self {
            todos: Vec::new(),
            projects: Vec::new(),
            notes: Vec::new(),
            reviews: Vec::new(),
            last_run_date: None,
            seeded_at: None,
        }
    }
}