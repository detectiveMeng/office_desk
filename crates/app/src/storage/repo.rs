//! 快照 ↔ SQLite 读写。日期存 `YYYY-MM-DD`，时间存 `YYYY-MM-DD HH:MM:SS`。

use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveDateTime};
use office_desk_core::domain::types::*;
use rusqlite::{params, Connection};

use super::schema;

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn fmt_dt(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").with_context(|| format!("非法日期: {s}"))
}

fn parse_dt(s: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .with_context(|| format!("非法时间: {s}"))
}

fn opt_date(raw: Option<String>) -> Result<Option<NaiveDate>> {
    raw.as_deref().map(parse_date).transpose()
}

fn opt_dt(raw: Option<String>) -> Result<Option<NaiveDateTime>> {
    raw.as_deref().map(parse_dt).transpose()
}

fn parse_todo_status(s: &str) -> TodoStatus {
    if s == "done" {
        TodoStatus::Done
    } else {
        TodoStatus::Todo
    }
}

fn todo_status_str(s: TodoStatus) -> &'static str {
    match s {
        TodoStatus::Todo => "todo",
        TodoStatus::Done => "done",
    }
}

fn parse_project_status(s: &str) -> ProjectStatus {
    match s {
        "blocked" => ProjectStatus::Blocked,
        "done" => ProjectStatus::Done,
        "paused" => ProjectStatus::Paused,
        _ => ProjectStatus::Active,
    }
}

fn project_status_str(s: ProjectStatus) -> &'static str {
    match s {
        ProjectStatus::Active => "active",
        ProjectStatus::Blocked => "blocked",
        ProjectStatus::Done => "done",
        ProjectStatus::Paused => "paused",
    }
}

/// 全量载入内存镜像。
pub fn load(conn: &Connection) -> Result<DataSnapshot> {
    let mut snap = DataSnapshot::new();

    // 顺延流水：按 todo_id 归组
    let mut postpone: HashMap<String, Vec<PostponeEvent>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT todo_id, from_date, to_date, at, auto FROM postpone_log ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
            ))
        })?;
        for row in rows {
            let (todo_id, from, to, at, auto) = row?;
            let (Some(to), Some(at)) = (to, at) else {
                continue; // 残缺流水直接跳过，不让整库读取失败
            };
            postpone.entry(todo_id).or_default().push(PostponeEvent {
                from: opt_date(from)?,
                to: parse_date(&to)?,
                at: parse_dt(&at)?,
                auto: auto.unwrap_or(0) != 0,
            });
        }
    }

    {
        let mut stmt = conn.prepare(
            "SELECT id, title, priority, date, original_date, project_id, status, done_at,
                    note, postpone_count, created_at FROM todos",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, String>(10)?,
            ))
        })?;
        for row in rows {
            let (id, title, priority, date, original, project_id, status, done_at, note, count, created_at) =
                row?;
            snap.todos.push(Todo {
                postpone_log: postpone.remove(&id).unwrap_or_default(),
                id,
                title,
                priority: priority.clamp(0, 3) as u8,
                date: opt_date(date)?,
                original_date: opt_date(original)?,
                project_id,
                status: parse_todo_status(&status),
                done_at: opt_dt(done_at)?,
                note,
                postpone_count: count.max(0) as u32,
                created_at: parse_dt(&created_at)?,
            });
        }
    }

    // 卡点流水
    let mut blockers: HashMap<String, Vec<BlockerEvent>> = HashMap::new();
    {
        let mut stmt =
            conn.prepare("SELECT project_id, text, start_at, end_at FROM blocker_log ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        for row in rows {
            let (project_id, text, start_at, end_at) = row?;
            let Some(start_at) = start_at else { continue };
            blockers.entry(project_id).or_default().push(BlockerEvent {
                text: text.unwrap_or_default(),
                start_at: parse_date(&start_at)?,
                end_at: opt_date(end_at)?,
            });
        }
    }

    {
        let mut stmt = conn.prepare(
            "SELECT id, name, goal, current, next, blocker, status, updated_at FROM projects",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        for row in rows {
            let (id, name, goal, current, next, blocker, status, updated_at) = row?;
            let blocker_log = blockers.remove(&id).unwrap_or_default();
            snap.projects.push(Project {
                id,
                name,
                goal,
                current,
                next,
                blocker,
                status: parse_project_status(&status),
                updated_at: parse_dt(&updated_at)?,
                blocker_log,
            });
        }
    }

    {
        let mut stmt =
            conn.prepare("SELECT id, content, tags, pinned, created_at, updated_at FROM notes")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        for row in rows {
            let (id, content, tags, pinned, created_at, updated_at) = row?;
            snap.notes.push(Note {
                id,
                content,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                pinned: pinned != 0,
                created_at: opt_dt(created_at)?.unwrap_or_else(|| chrono::Local::now().naive_local()),
                updated_at: opt_dt(updated_at)?.unwrap_or_else(|| chrono::Local::now().naive_local()),
            });
        }
    }

    {
        let mut stmt =
            conn.prepare("SELECT week_start, good, watch, focus, updated_at FROM weekly_reviews")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (week_start, good, watch, focus, updated_at) = row?;
            snap.reviews.push(WeeklyReview {
                week_start: parse_date(&week_start)?,
                good: good.unwrap_or_default(),
                watch: watch.unwrap_or_default(),
                focus: focus.unwrap_or_default(),
                updated_at: opt_dt(updated_at)?,
            });
        }
    }

    snap.last_run_date = opt_date(schema::get_meta(conn, "last_run_date")?)?;
    snap.seeded_at = opt_dt(schema::get_meta(conn, "seeded_at")?)?;

    Ok(snap)
}

/// 全量回写（单事务）：先清后灌，保证内存镜像与库一致。
pub fn save(conn: &mut Connection, s: &DataSnapshot) -> Result<()> {
    let tx = conn.transaction()?;

    tx.execute_batch(
        "DELETE FROM postpone_log;
         DELETE FROM blocker_log;
         DELETE FROM todos;
         DELETE FROM projects;
         DELETE FROM notes;
         DELETE FROM weekly_reviews;",
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO projects(id, name, goal, current, next, blocker, status, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for p in &s.projects {
            stmt.execute(params![
                p.id,
                p.name,
                p.goal,
                p.current,
                p.next,
                p.blocker,
                project_status_str(p.status),
                fmt_dt(p.updated_at),
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO todos(id, title, priority, date, original_date, project_id, status,
                               done_at, note, postpone_count, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;
        for t in &s.todos {
            stmt.execute(params![
                t.id,
                t.title,
                t.priority as i64,
                t.date.map(fmt_date),
                t.original_date.map(fmt_date),
                t.project_id,
                todo_status_str(t.status),
                t.done_at.map(fmt_dt),
                t.note,
                t.postpone_count as i64,
                fmt_dt(t.created_at),
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO postpone_log(todo_id, from_date, to_date, at, auto)
             VALUES(?1, ?2, ?3, ?4, ?5)",
        )?;
        for t in &s.todos {
            for e in &t.postpone_log {
                stmt.execute(params![
                    t.id,
                    e.from.map(fmt_date),
                    fmt_date(e.to),
                    fmt_dt(e.at),
                    e.auto as i64,
                ])?;
            }
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO blocker_log(project_id, text, start_at, end_at) VALUES(?1, ?2, ?3, ?4)",
        )?;
        for p in &s.projects {
            for e in &p.blocker_log {
                stmt.execute(params![
                    p.id,
                    e.text,
                    fmt_date(e.start_at),
                    e.end_at.map(fmt_date),
                ])?;
            }
        }
    }

    {
        let mut stmt =
            tx.prepare("INSERT INTO notes(id, content, tags, pinned, created_at, updated_at)
                        VALUES(?1, ?2, ?3, ?4, ?5, ?6)")?;
        for n in &s.notes {
            let tags = serde_json::to_string(&n.tags)?;
            stmt.execute(params![
                n.id,
                n.content,
                tags,
                n.pinned as i64,
                fmt_dt(n.created_at),
                fmt_dt(n.updated_at),
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO weekly_reviews(week_start, good, watch, focus, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5)",
        )?;
        for r in &s.reviews {
            stmt.execute(params![
                fmt_date(r.week_start),
                r.good,
                r.watch,
                r.focus,
                r.updated_at.map(fmt_dt),
            ])?;
        }
    }

    set_meta(&tx, "last_run_date", s.last_run_date.map(fmt_date))?;
    set_meta(&tx, "seeded_at", s.seeded_at.map(fmt_dt))?;

    tx.commit()?;
    Ok(())
}

/// meta 写入：Some 时 upsert，None 时删除，避免残留旧值。
fn set_meta(conn: &Connection, key: &str, value: Option<String>) -> Result<()> {
    match value {
        Some(v) => {
            conn.execute(
                "INSERT INTO meta(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?;
        }
        None => {
            conn.execute("DELETE FROM meta WHERE key = ?1", [key])?;
        }
    }
    Ok(())
}
