//! 建表与迁移：schema_version 记在 meta 表，启动时按序补齐。

use anyhow::Result;
use rusqlite::Connection;

/// 迁移脚本，下标 + 1 即版本号。只追加，不改历史项。
pub const MIGRATIONS: &[&str] = &[V1, V2];

/// v1：完整初始表结构
const V1: &str = r#"
CREATE TABLE todos (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  priority       INTEGER NOT NULL DEFAULT 1,
  date           TEXT,
  original_date  TEXT,
  project_id     TEXT REFERENCES projects(id) ON DELETE SET NULL,
  status         TEXT NOT NULL DEFAULT 'todo',
  done_at        TEXT,
  note           TEXT NOT NULL DEFAULT '',
  postpone_count INTEGER NOT NULL DEFAULT 0,
  created_at     TEXT NOT NULL
);

CREATE TABLE postpone_log (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  todo_id   TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
  from_date TEXT,
  to_date   TEXT,
  at        TEXT,
  auto      INTEGER
);

CREATE TABLE projects (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  current    TEXT NOT NULL DEFAULT '',
  next       TEXT NOT NULL DEFAULT '',
  blocker    TEXT NOT NULL DEFAULT '',
  status     TEXT NOT NULL DEFAULT 'active',
  updated_at TEXT NOT NULL
);

CREATE TABLE blocker_log (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  text       TEXT,
  start_at   TEXT,
  end_at     TEXT
);

CREATE TABLE notes (
  id         TEXT PRIMARY KEY,
  content    TEXT NOT NULL,
  tags       TEXT NOT NULL DEFAULT '[]',
  pinned     INTEGER NOT NULL DEFAULT 0,
  created_at TEXT,
  updated_at TEXT
);

CREATE TABLE weekly_reviews (
  week_start TEXT PRIMARY KEY,
  good       TEXT,
  watch      TEXT,
  focus      TEXT,
  updated_at TEXT
);

CREATE INDEX idx_todos_date ON todos(date);
CREATE INDEX idx_todos_project ON todos(project_id);
CREATE INDEX idx_postpone_todo ON postpone_log(todo_id);
CREATE INDEX idx_blocker_project ON blocker_log(project_id);
"#;

/// v2：项目卡新增「目标 / 需求」内容
const V2: &str = r#"
ALTER TABLE projects ADD COLUMN goal TEXT NOT NULL DEFAULT '';
"#;

/// 执行未应用的迁移。
pub fn migrate(conn: &mut Connection) -> Result<()> {
    // meta 表先就位，才能读取当前版本
    conn.execute_batch("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);")?;

    let current: u32 = get_meta(conn, "schema_version")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        let version = (i + 1).to_string();
        tx.execute(
            "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&version],
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// 读取 meta 值。
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .ok()
        .flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(get_meta(&conn, "schema_version").unwrap().as_deref(), Some("2"));

        migrate(&mut conn).unwrap(); // 再次执行不应报错
        assert_eq!(get_meta(&conn, "schema_version").unwrap().as_deref(), Some("2"));
    }

    #[test]
    fn v1_database_gains_goal_column_with_empty_default() {
        let mut conn = Connection::open_in_memory().unwrap();
        // 模拟旧版本库：只有 v1 表结构，版本号停在 1
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
             INSERT INTO meta(key, value) VALUES('schema_version', '1');",
        )
        .unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute(
            "INSERT INTO projects(id, name, status, updated_at)
             VALUES('p1', '旧项目', 'active', '2026-10-01 09:00:00')",
            [],
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(get_meta(&conn, "schema_version").unwrap().as_deref(), Some("2"));
        let goal: String = conn
            .query_row("SELECT goal FROM projects WHERE id = 'p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(goal, "");
    }

    #[test]
    fn schema_creates_all_tables() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
                 AND name IN ('todos','postpone_log','projects','blocker_log','notes','weekly_reviews','meta')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 7);
    }
}
