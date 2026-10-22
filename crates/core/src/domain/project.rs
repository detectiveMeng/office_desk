//! 项目领域逻辑：进度、状态联动。纯函数。

use chrono::NaiveDate;

use crate::domain::todo::{bucket, Bucket};
use crate::domain::types::*;

/// 项目进度快照
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub total: usize,
    pub done: usize,
    pub overdue: usize,
    /// 完成率；0 关联任务时为 None（前端显示「暂无关联待办」，不显示 0%）
    pub rate: Option<f64>,
}

/// 项目下关联的任务
pub fn linked_todos<'a>(p: &Project, todos: &'a [Todo]) -> Vec<&'a Todo> {
    todos
        .iter()
        .filter(|t| t.project_id.as_deref() == Some(p.id.as_str()))
        .collect()
}

/// 项目进度：完成 / 总数 / 逾期数 / 完成率
pub fn progress(p: &Project, todos: &[Todo], today: NaiveDate) -> Progress {
    let rel = linked_todos(p, todos);
    let done = rel.iter().filter(|t| t.status == TodoStatus::Done).count();
    let overdue = rel
        .iter()
        .filter(|t| bucket(t, today) == Bucket::Overdue)
        .count();
    let total = rel.len();
    let rate = if total > 0 {
        Some(done as f64 / total as f64)
    } else {
        None
    };
    Progress {
        total,
        done,
        overdue,
        rate,
    }
}

/// 卡点驱动状态：卡点空 → 进行中，非空 → 卡住了；已完成/暂停为终态，不被自动覆盖。
pub fn sync_status(p: &Project) -> ProjectStatus {
    match p.status {
        ProjectStatus::Done | ProjectStatus::Paused => p.status,
        _ => {
            if p.blocker.trim().is_empty() {
                ProjectStatus::Active
            } else {
                ProjectStatus::Blocked
            }
        }
    }
}

/// 项目列表排序：卡住 → 有逾期 → 其余；同组内按最近更新，已完成永远排最后。
pub fn sort_projects(projects: &mut [Project], todos: &[Todo], today: NaiveDate) {
    projects.sort_by(|a, b| {
        let rank = |p: &Project| match sync_status(p) {
            ProjectStatus::Done => 3,
            ProjectStatus::Blocked => 0,
            _ if progress(p, todos, today).overdue > 0 => 1,
            _ => 2,
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
    fn dt(y: i32, m: u32, d: u32) -> chrono::NaiveDateTime {
        day(y, m, d).and_hms_opt(9, 0, 0).unwrap()
    }

    fn mk(
        id: &str,
        project: Option<&str>,
        status: TodoStatus,
        date: Option<NaiveDate>,
        original: Option<NaiveDate>,
    ) -> Todo {
        Todo {
            id: id.into(),
            title: id.into(),
            priority: 1,
            date,
            original_date: original,
            project_id: project.map(|s| s.into()),
            status,
            done_at: if status == TodoStatus::Done {
                Some(dt(2026, 10, 20))
            } else {
                None
            },
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at: dt(2026, 10, 1),
        }
    }

    fn proj(id: &str, blocker: &str, status: ProjectStatus) -> Project {
        Project {
            id: id.into(),
            name: id.into(),
            current: String::new(),
            next: String::new(),
            blocker: blocker.into(),
            status,
            updated_at: dt(2026, 10, 20),
            blocker_log: Vec::new(),
        }
    }

    #[test]
    fn progress_and_rate() {
        let p = proj("p1", "", ProjectStatus::Active);
        let today = day(2026, 10, 20);
        let yesterday = day(2026, 10, 19);
        let todos = vec![
            mk("a", Some("p1"), TodoStatus::Done, Some(today), Some(today)),
            mk("b", Some("p1"), TodoStatus::Todo, Some(today), Some(yesterday)),
            mk("c", None, TodoStatus::Todo, Some(today), Some(today)),
        ];
        let prog = progress(&p, &todos, today);
        assert_eq!(prog.total, 2);
        assert_eq!(prog.done, 1);
        assert_eq!(prog.overdue, 1);
        assert!((prog.rate.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn zero_related_is_none() {
        let p = proj("p1", "", ProjectStatus::Active);
        let prog = progress(&p, &[], day(2026, 10, 20));
        assert_eq!(prog.total, 0);
        assert_eq!(prog.rate, None);
    }

    #[test]
    fn sort_orders_blocked_overdue_then_others_done_last() {
        let today = day(2026, 10, 20);
        let yesterday = day(2026, 10, 19);
        let mut projects = vec![
            proj("done", "", ProjectStatus::Done),
            proj("normal", "", ProjectStatus::Active),
            proj("blocked", "等确认", ProjectStatus::Active),
            proj("overdue", "", ProjectStatus::Active),
        ];
        let todos = vec![mk(
            "t",
            Some("overdue"),
            TodoStatus::Todo,
            Some(today),
            Some(yesterday),
        )];

        sort_projects(&mut projects, &todos, today);

        let ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["blocked", "overdue", "normal", "done"]);
    }

    #[test]
    fn blocker_drives_status() {
        let p = proj("p", "", ProjectStatus::Active);
        assert_eq!(sync_status(&p), ProjectStatus::Active);

        let blocked = proj("p", "等法务确认", ProjectStatus::Active);
        assert_eq!(sync_status(&blocked), ProjectStatus::Blocked);

        let done = proj("p", "等法务确认", ProjectStatus::Done);
        assert_eq!(sync_status(&done), ProjectStatus::Done); // 终态不被覆盖
    }
}