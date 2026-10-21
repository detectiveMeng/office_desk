//! 任务领域逻辑：逾期判定、顺延、排序、分组。全部为纯函数。

use chrono::{NaiveDate, NaiveDateTime};

use crate::domain::types::*;

/// 任务归属的五个分区
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bucket {
    Done,
    Overdue,
    Today,
    Future,
    Someday,
}

impl Bucket {
    fn rank(self) -> u8 {
        match self {
            Bucket::Overdue => 0,
            Bucket::Today => 1,
            Bucket::Future => 2,
            Bucket::Someday => 3,
            Bucket::Done => 4,
        }
    }
}

/// 逾期判定唯一口径：先看终态/无日期/未来，再看 original_date 是否早于今天。
pub fn bucket(t: &Todo, today: NaiveDate) -> Bucket {
    if t.status == TodoStatus::Done {
        return Bucket::Done;
    }
    let Some(date) = t.date else {
        return Bucket::Someday;
    };
    if date > today {
        return Bucket::Future;
    }
    if let Some(od) = t.original_date {
        if od < today {
            return Bucket::Overdue;
        }
    }
    Bucket::Today
}

/// 逾期天数：today - original_date，仅逾期任务有意义
pub fn overdue_days(t: &Todo, today: NaiveDate) -> u32 {
    match t.original_date {
        Some(od) if bucket(t, today) == Bucket::Overdue => (today - od).num_days().max(0) as u32,
        _ => 0,
    }
}

/// 是否「反复顺延」：顺延次数 ≥ 2 且仍未完成
pub fn is_repeated(t: &Todo) -> bool {
    t.status == TodoStatus::Todo && t.postpone_count >= 2
}

/// 今日清单排序：逾期 → 优先级(0..3) → 原定日期 → 创建时间
pub fn sort_for_today(todos: &mut [Todo], today: NaiveDate) {
    todos.sort_by(|a, b| {
        bucket(a, today)
            .rank()
            .cmp(&bucket(b, today).rank())
            .then_with(|| a.priority.cmp(&b.priority))
            .then_with(|| a.original_date.cmp(&b.original_date))
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
}

/// 未来区分组：有日期按日期升序，无日期(None)排最后；组内按优先级。
pub fn group_future(todos: &[Todo], today: NaiveDate) -> Vec<(Option<NaiveDate>, Vec<&Todo>)> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<Option<NaiveDate>, Vec<&Todo>> = BTreeMap::new();
    for t in todos {
        if t.status != TodoStatus::Todo {
            continue;
        }
        match t.date {
            Some(d) if d > today => groups.entry(Some(d)).or_default().push(t),
            None => groups.entry(None).or_default().push(t),
            _ => {}
        }
    }
    for v in groups.values_mut() {
        v.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.created_at.cmp(&b.created_at)));
    }
    let mut out: Vec<_> = groups.into_iter().collect();
    out.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    out
}

/// 自动顺延（Y2T）：跨天把 date<today 的任务带到今天，记流水；original_date 不变。
/// 返回是否有改动（true 时调用方需落库）。
pub fn run_auto_rollover(s: &mut DataSnapshot, today: NaiveDate, now: NaiveDateTime) -> bool {
    if s.last_run_date == Some(today) {
        return false;
    }
    let mut changed = false;
    for t in s.todos.iter_mut() {
        if t.status != TodoStatus::Todo {
            continue;
        }
        if let Some(d) = t.date {
            if d < today {
                t.postpone_log.push(PostponeEvent {
                    from: Some(d),
                    to: today,
                    at: now,
                    auto: true,
                });
                t.postpone_count += 1;
                t.date = Some(today);
                changed = true;
            }
        }
    }
    s.last_run_date = Some(today);
    changed
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
        priority: u8,
        date: Option<NaiveDate>,
        original: Option<NaiveDate>,
        status: TodoStatus,
    ) -> Todo {
        Todo {
            id: id.into(),
            title: id.into(),
            priority,
            date,
            original_date: original,
            project_id: None,
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

    #[test]
    fn bucket_rules() {
        let today = day(2026, 10, 20);
        let yesterday = day(2026, 10, 19);
        let tomorrow = day(2026, 10, 21);

        assert_eq!(
            bucket(&mk("done", 0, Some(yesterday), Some(yesterday), TodoStatus::Done), today),
            Bucket::Done
        );
        assert_eq!(bucket(&mk("someday", 0, None, None, TodoStatus::Todo), today), Bucket::Someday);
        assert_eq!(
            bucket(&mk("future", 0, Some(tomorrow), Some(tomorrow), TodoStatus::Todo), today),
            Bucket::Future
        );
        assert_eq!(
            bucket(&mk("overdue", 0, Some(today), Some(yesterday), TodoStatus::Todo), today),
            Bucket::Overdue
        );
        assert_eq!(
            bucket(&mk("today", 0, Some(today), Some(today), TodoStatus::Todo), today),
            Bucket::Today
        );
    }

    #[test]
    fn overdue_days_calc() {
        let today = day(2026, 10, 20);
        let five_ago = day(2026, 10, 15);
        assert_eq!(overdue_days(&mk("x", 0, Some(today), Some(five_ago), TodoStatus::Todo), today), 5);
        assert_eq!(overdue_days(&mk("y", 0, Some(today), Some(today), TodoStatus::Todo), today), 0);
    }

    #[test]
    fn auto_rollover_moves_once() {
        let today = day(2026, 10, 20);
        let yesterday = day(2026, 10, 19);
        let now = dt(2026, 10, 20);

        let mut s = DataSnapshot::new();
        s.todos.push(mk("old", 1, Some(yesterday), Some(yesterday), TodoStatus::Todo));
        s.todos.push(mk("future", 1, Some(today), Some(today), TodoStatus::Todo));

        assert!(run_auto_rollover(&mut s, today, now));
        assert_eq!(s.todos[0].date, Some(today));
        assert_eq!(s.todos[0].postpone_count, 1);
        assert_eq!(s.todos[0].original_date, Some(yesterday)); // 原定日期不变
        assert_eq!(s.last_run_date, Some(today));

        assert!(!run_auto_rollover(&mut s, today, now)); // 同日不重复
        assert_eq!(s.todos[0].postpone_count, 1);
    }

    #[test]
    fn sort_puts_overdue_first() {
        let today = day(2026, 10, 20);
        let yesterday = day(2026, 10, 19);
        let tomorrow = day(2026, 10, 21);
        let mut v = vec![
            mk("f", 0, Some(tomorrow), Some(tomorrow), TodoStatus::Todo),
            mk("o", 2, Some(today), Some(yesterday), TodoStatus::Todo),
            mk("t", 1, Some(today), Some(today), TodoStatus::Todo),
        ];
        sort_for_today(&mut v, today);
        let ids: Vec<&str> = v.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["o", "t", "f"]);
    }
}