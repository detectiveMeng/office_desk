//! 周复盘统计：全部由已有数据派生，只读，不落库。

use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, NaiveDate, NaiveDateTime};

use crate::domain::date::in_week;
use crate::domain::project::{linked_todos, sync_status};
use crate::domain::todo::is_repeated;
use crate::domain::types::*;

/// 一周统计结果
#[derive(Clone, Debug)]
pub struct WeekStats {
    pub week_start: NaiveDate,
    /// 本周完成件数（doneAt ∈ 本周）
    pub done_count: u32,
    /// 分母 = 本周到期(date∈周) ∪ 本周完成(doneAt∈周)，去重
    pub denominator: u32,
    pub done_rate: Option<f64>,
    /// 本周内发生的顺延事件总数
    pub postpone_count: u32,
    /// 反复顺延(≥2)且未完成的任务数
    pub repeated_count: u32,
    /// 本周有任务完成或有字段更新的项目数
    pub active_projects: u32,
    pub blocked_projects: u32,
    /// 本周完成按优先级分布，[P0, P1, P2, P3]
    pub priority_count: [u32; 4],
    /// 本周完成按项目切分；None 表示「其他」（未挂项目或合并后超出 5 个）
    pub energy: Vec<(Option<String>, u32)>,
    /// 一周节奏：周一..周日 每日完成件数
    pub daily: [u32; 7],
}

impl WeekStats {
    pub fn new(week_start: NaiveDate) -> Self {
        Self {
            week_start,
            done_count: 0,
            denominator: 0,
            done_rate: None,
            postpone_count: 0,
            repeated_count: 0,
            active_projects: 0,
            blocked_projects: 0,
            priority_count: [0; 4],
            energy: Vec::new(),
            daily: [0; 7],
        }
    }
}

/// 计算指定周（以 week_start 为周一）的统计
pub fn week(s: &DataSnapshot, week_start: NaiveDate) -> WeekStats {
    let mut st = WeekStats::new(week_start);

    let done_in_week = |dt: &NaiveDateTime| in_week(dt.date(), week_start);

    // 本周完成
    let done_list: Vec<&Todo> = s
        .todos
        .iter()
        .filter(|t| t.status == TodoStatus::Done && t.done_at.as_ref().is_some_and(done_in_week))
        .collect();
    st.done_count = done_list.len() as u32;

    // 完成率分母
    let mut denom: HashSet<&str> = HashSet::new();
    for t in &s.todos {
        let done_wk = t.status == TodoStatus::Done && t.done_at.as_ref().is_some_and(done_in_week);
        let due_wk = t.date.map_or(false, |d| in_week(d, week_start));
        if done_wk || due_wk {
            denom.insert(t.id.as_str());
        }
    }
    st.denominator = denom.len() as u32;
    st.done_rate = if st.denominator > 0 {
        Some(st.done_count as f64 / st.denominator as f64)
    } else {
        None
    };

    // 顺延事件 + 反复顺延
    st.postpone_count = s
        .todos
        .iter()
        .flat_map(|t| &t.postpone_log)
        .filter(|e| in_week(e.at.date(), week_start))
        .count() as u32;
    st.repeated_count = s.todos.iter().filter(|t| is_repeated(t)).count() as u32;

    // 项目：本周推进 / 卡住
    st.active_projects = s
        .projects
        .iter()
        .filter(|p| {
            in_week(p.updated_at.date(), week_start)
                || linked_todos(p, &s.todos)
                    .iter()
                    .any(|t| t.done_at.as_ref().is_some_and(done_in_week))
        })
        .count() as u32;
    st.blocked_projects = s
        .projects
        .iter()
        .filter(|p| sync_status(p) == ProjectStatus::Blocked)
        .count() as u32;

    // 优先级分布
    for t in &done_list {
        st.priority_count[(t.priority.min(3)) as usize] += 1;
    }

    // 精力分布：按项目，最多 5 个 + 其他
    let mut energy: BTreeMap<Option<&str>, u32> = BTreeMap::new();
    for t in &done_list {
        *energy.entry(t.project_id.as_deref()).or_default() += 1;
    }
    let mut energy: Vec<(Option<String>, u32)> = energy
        .into_iter()
        .map(|(k, v)| (k.map(|s| s.to_string()), v))
        .collect();
    energy.sort_by(|a, b| b.1.cmp(&a.1));
    if energy.len() > 5 {
        let rest: u32 = energy[5..].iter().map(|e| e.1).sum();
        energy.truncate(5);
        match energy.iter_mut().find(|e| e.0.is_none()) {
            Some(slot) => slot.1 += rest,
            None => energy.push((None, rest)),
        }
    }
    st.energy = energy;

    // 一周节奏
    for t in &done_list {
        if let Some(dt) = t.done_at {
            let d = dt.date();
            if in_week(d, week_start) {
                st.daily[d.weekday().num_days_from_monday() as usize] += 1;
            }
        }
    }

    st
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
    fn dt(y: i32, m: u32, d: u32) -> NaiveDateTime {
        day(y, m, d).and_hms_opt(9, 0, 0).unwrap()
    }

    fn mk(
        id: &str,
        project: Option<&str>,
        status: TodoStatus,
        date: Option<NaiveDate>,
        done_at: Option<NaiveDateTime>,
        priority: u8,
    ) -> Todo {
        Todo {
            id: id.into(),
            title: id.into(),
            priority,
            date,
            original_date: date,
            project_id: project.map(|s| s.into()),
            status,
            done_at,
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at: dt(2026, 10, 1),
        }
    }

    #[test]
    fn week_aggregates() {
        let ws = day(2026, 10, 19); // 本周一
        let s = DataSnapshot {
            todos: vec![
                mk("a", Some("p1"), TodoStatus::Done, Some(day(2026, 10, 20)), Some(dt(2026, 10, 20)), 0),
                mk("b", Some("p1"), TodoStatus::Done, Some(day(2026, 10, 21)), Some(dt(2026, 10, 21)), 1),
                mk("c", None, TodoStatus::Todo, Some(day(2026, 10, 22)), None, 2),
                mk("d", None, TodoStatus::Done, Some(day(2026, 10, 12)), Some(dt(2026, 10, 12)), 0),
            ],
            ..DataSnapshot::new()
        };

        let st = week(&s, ws);

        assert_eq!(st.done_count, 2);
        assert_eq!(st.denominator, 3); // 完成 2 + 到期未完成 1
        assert!((st.done_rate.unwrap() - 2.0 / 3.0).abs() < 1e-9);

        assert_eq!(st.priority_count[0], 1);
        assert_eq!(st.priority_count[1], 1);

        assert_eq!(st.energy, vec![(Some("p1".to_string()), 2)]);

        let idx_20 = day(2026, 10, 20).weekday().num_days_from_monday() as usize;
        let idx_21 = day(2026, 10, 21).weekday().num_days_from_monday() as usize;
        assert_eq!(st.daily[idx_20], 1);
        assert_eq!(st.daily[idx_21], 1);
        assert_eq!(st.daily.iter().sum::<u32>(), 2);
    }
}