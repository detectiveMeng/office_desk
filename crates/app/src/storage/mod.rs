//! 存储层：SQLite 建库、迁移、读写与备份。
//! 对外只暴露 `Storage`，内部按 schema / repo / backup 分工。

pub mod backup;
pub mod repo;
pub mod schema;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use office_desk_core::domain::types::DataSnapshot;
use rusqlite::Connection;

/// 绿色版标记文件：与 exe 同目录存在时，数据写到 exe 旁的 `data` 目录。
pub const PORTABLE_FLAG: &str = "portable.flag";

/// 解析数据目录：
/// - exe 同目录有 `portable.flag` → `<exe 目录>\data`（绿色版，数据跟程序走）
/// - 否则 → `%APPDATA%\office_desk`
pub fn data_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let appdata = std::env::var("APPDATA").ok();
    resolve_data_dir(exe_dir.as_deref(), appdata.as_deref())
}

fn resolve_data_dir(exe_dir: Option<&Path>, appdata: Option<&str>) -> PathBuf {
    if let Some(dir) = exe_dir {
        if dir.join(PORTABLE_FLAG).is_file() {
            return dir.join("data");
        }
    }
    match appdata {
        Some(appdata) => Path::new(appdata).join("office_desk"),
        None => PathBuf::from("."),
    }
}

/// 数据库文件：数据目录下的 `office_desk.db`
pub fn db_path() -> PathBuf {
    data_dir().join("office_desk.db")
}

/// 单个 SQLite 连接 + 内存镜像的落库封装。
pub struct Storage {
    conn: Connection,
    path: Option<PathBuf>,
}

impl Storage {
    /// 打开默认位置数据库（自动建目录、建表、迁移）。
    pub fn open_default() -> Result<Self> {
        let path = db_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("创建数据目录失败: {}", dir.display()))?;
        }
        Self::open(&path)
    }

    /// 打开指定路径数据库。
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open(&path)
            .with_context(|| format!("打开数据库失败: {}", path.display()))?;
        let mut storage = Self {
            conn,
            path: Some(path),
        };
        storage.init()?;
        Ok(storage)
    }

    /// 内存数据库，仅供测试使用。
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let mut storage = Self { conn, path: None };
        storage.init()?;
        Ok(storage)
    }

    fn init(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nPRAGMA synchronous = NORMAL;",
        )?;
        schema::migrate(&mut self.conn)
    }

    /// 全量载入内存镜像。
    pub fn load(&self) -> Result<DataSnapshot> {
        repo::load(&self.conn)
    }

    /// 全量回写（单事务）。个人数据量级下每次改动整库重写最简单可靠。
    pub fn save(&mut self, snapshot: &DataSnapshot) -> Result<()> {
        repo::save(&mut self.conn, snapshot)
    }

    /// 数据库完整性检查。
    pub fn integrity_ok(&self) -> bool {
        backup::integrity_ok(&self.conn)
    }

    /// 备份到指定文件（`VACUUM INTO`，在线一致性备份）。
    pub fn backup_to(&self, target: impl AsRef<Path>) -> Result<()> {
        backup::backup_to(&self.conn, target)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use office_desk_core::domain::types::*;

    fn dt(y: i32, m: u32, d: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(9, 30, 0)
            .unwrap()
    }

    fn sample() -> DataSnapshot {
        let mut s = DataSnapshot::new();
        s.projects.push(Project {
            id: "p1".into(),
            name: "Q3 增长方案".into(),
            current: "梳理漏斗".into(),
            next: "复盘数据".into(),
            blocker: "等法务确认".into(),
            status: ProjectStatus::Blocked,
            updated_at: dt(2026, 10, 20),
            blocker_log: vec![BlockerEvent {
                text: "等法务确认".into(),
                start_at: NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
                end_at: None,
            }],
        });
        s.todos.push(Todo {
            id: "t1".into(),
            title: "写方案".into(),
            priority: 0,
            date: NaiveDate::from_ymd_opt(2026, 10, 21),
            original_date: NaiveDate::from_ymd_opt(2026, 10, 19),
            project_id: Some("p1".into()),
            status: TodoStatus::Todo,
            done_at: None,
            note: "备注".into(),
            postpone_count: 1,
            postpone_log: vec![PostponeEvent {
                from: NaiveDate::from_ymd_opt(2026, 10, 19),
                to: NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(),
                at: dt(2026, 10, 21),
                auto: true,
            }],
            created_at: dt(2026, 10, 18),
        });
        s.todos.push(Todo {
            id: "t2".into(),
            title: "无日期任务".into(),
            priority: 3,
            date: None,
            original_date: None,
            project_id: None,
            status: TodoStatus::Done,
            done_at: Some(dt(2026, 10, 20)),
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at: dt(2026, 10, 17),
        });
        s.notes.push(Note {
            id: "n1".into(),
            content: "灵感一条".into(),
            tags: vec!["增长".into(), "想法".into()],
            pinned: true,
            created_at: dt(2026, 10, 20),
            updated_at: dt(2026, 10, 21),
        });
        s.reviews.push(WeeklyReview {
            week_start: NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
            good: "推进顺利".into(),
            watch: "别拖".into(),
            focus: "聚焦主线".into(),
            updated_at: Some(dt(2026, 10, 24)),
        });
        s.last_run_date = NaiveDate::from_ymd_opt(2026, 10, 21);
        s.seeded_at = Some(dt(2026, 10, 1));
        s
    }

    #[test]
    fn roundtrip_snapshot() {
        let mut st = Storage::open_memory().unwrap();
        let original = sample();
        st.save(&original).unwrap();
        let loaded = st.load().unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn save_twice_is_idempotent() {
        let mut st = Storage::open_memory().unwrap();
        let data = sample();
        st.save(&data).unwrap();
        st.save(&data).unwrap();

        let loaded = st.load().unwrap();
        assert_eq!(loaded.todos.len(), 2);
        assert_eq!(loaded.todos[0].postpone_log.len(), 1); // 流水不重复累积
        assert_eq!(loaded.projects[0].blocker_log.len(), 1);
    }

    #[test]
    fn empty_db_loads_empty_snapshot() {
        let st = Storage::open_memory().unwrap();
        let loaded = st.load().unwrap();
        assert_eq!(loaded, DataSnapshot::new());
    }

    #[test]
    fn portable_flag_switches_data_dir() {
        let dir = std::env::temp_dir().join(format!(
            "office_desk_portable_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // 没有标记文件：走 APPDATA
        assert_eq!(
            resolve_data_dir(Some(&dir), Some("C:\\appdata")),
            Path::new("C:\\appdata").join("office_desk")
        );

        // 放了标记文件：数据落在 exe 同级的 data 目录
        std::fs::write(dir.join(PORTABLE_FLAG), b"").unwrap();
        assert_eq!(
            resolve_data_dir(Some(&dir), Some("C:\\appdata")),
            dir.join("data")
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
