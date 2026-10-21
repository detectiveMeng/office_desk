//! 首次启动种子数据：3 个项目、18 条任务、6 条灵感。
//! 口径回归基准：本周完成 11 · 完成率 84.6% · 逾期 2 · 卡住 1。
//!
//! 时间锚点取「今天」，这样任何一天打开都能看到同一组回归数字：
//! - 11 条本周完成铺在本周一~周日
//! - 2 条逾期：原定 anchor-3 / anchor-4，当前日期 = anchor，各顺延 2 次
//! - 3 条未来：anchor+7 / +8 / +9（永远落在下一周，不会污染本周完成率分母）
//! - 1 条无日期、1 条上周完成

use chrono::{Duration, NaiveDate, NaiveDateTime};
use office_desk_core::domain::date::start_of_week;
use office_desk_core::domain::types::*;

/// 生成种子快照。
pub fn seed(anchor: NaiveDate, now: NaiveDateTime) -> DataSnapshot {
    let mut s = DataSnapshot::new();
    let week_start = start_of_week(anchor);

    /* ---------------- 项目 ---------------- */
    s.projects.push(Project {
        id: "p-q3".into(),
        name: "Q3 增长方案".into(),
        current: "补充渠道漏斗与价格带".into(),
        next: "评审 11 月投放预算".into(),
        blocker: "等法务确认活动条款".into(),
        status: ProjectStatus::Blocked,
        updated_at: now,
        blocker_log: vec![BlockerEvent {
            text: "等法务确认活动条款".into(),
            start_at: anchor - Duration::days(3),
            end_at: None,
        }],
    });
    s.projects.push(Project {
        id: "p-home".into(),
        name: "首页改版".into(),
        current: "首屏视觉稿 v2 走查".into(),
        next: "埋点方案评审".into(),
        blocker: String::new(),
        status: ProjectStatus::Active,
        updated_at: now,
        blocker_log: Vec::new(),
    });
    s.projects.push(Project {
        id: "p-blog".into(),
        name: "个人博客迁移".into(),
        current: "旧文迁移与 301 跳转".into(),
        next: "配置 RSS 与站点地图".into(),
        blocker: String::new(),
        status: ProjectStatus::Active,
        updated_at: now,
        blocker_log: Vec::new(),
    });

    /* ---------------- 任务 ---------------- */
    let created_at = now - Duration::days(12);
    let mut done_days = [1u32, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6].into_iter();

    let mut push_done = |s: &mut DataSnapshot, title: &str, priority: u8, project: Option<&str>| {
        let idx = done_days.next().expect("种子完成日不足 11 个");
        let day = week_start + Duration::days(idx as i64);
        s.todos.push(Todo {
            id: format!("t-{}", s.todos.len() + 1),
            title: title.into(),
            priority,
            date: Some(day),
            original_date: Some(day),
            project_id: project.map(str::to_string),
            status: TodoStatus::Done,
            done_at: Some(day.and_hms_opt(10, 0, 0).expect("合法时间")),
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at,
        });
    };

    // Q3 增长方案：3 完成 + 1 逾期 + 1 未来
    push_done(&mut s, "梳理 Q3 渠道数据", 0, Some("p-q3"));
    push_done(&mut s, "输出增长方案初稿", 0, Some("p-q3"));
    push_done(&mut s, "约投放团队对齐口径", 2, Some("p-q3"));
    push_overdue(&mut s, "补充竞品价格带分析", 0, Some("p-q3"), anchor, 3, created_at);
    push_future(&mut s, "制定 11 月投放预算", 2, Some("p-q3"), anchor + Duration::days(8), created_at);

    // 首页改版：3 完成 + 1 未来
    push_done(&mut s, "首页信息架构梳理", 1, Some("p-home"));
    push_done(&mut s, "首屏视觉稿 v2", 2, Some("p-home"));
    push_done(&mut s, "组件库命名统一", 2, Some("p-home"));
    push_future(&mut s, "埋点方案评审", 1, Some("p-home"), anchor + Duration::days(7), created_at);

    // 个人博客迁移：2 完成 + 1 未来
    push_done(&mut s, "域名解析切换", 1, Some("p-blog"));
    push_done(&mut s, "迁移 8 篇旧文", 0, Some("p-blog"));
    push_future(&mut s, "配置 RSS 与站点地图", 2, Some("p-blog"), anchor + Duration::days(9), created_at);

    // 未挂项目：3 本周完成 + 1 上周完成 + 1 逾期 + 1 无日期
    push_done(&mut s, "整理本周会议纪要", 1, None);
    push_done(&mut s, "清理收件箱", 2, None);
    push_done(&mut s, "预约体检", 0, None);
    let last_week = week_start - Duration::days(3);
    s.todos.push(Todo {
        id: format!("t-{}", s.todos.len() + 1),
        title: "归档 9 月报销单".into(),
        priority: 2,
        date: Some(last_week),
        original_date: Some(last_week),
        project_id: None,
        status: TodoStatus::Done,
        done_at: Some(last_week.and_hms_opt(16, 0, 0).expect("合法时间")),
        note: String::new(),
        postpone_count: 0,
        postpone_log: Vec::new(),
        created_at,
    });
    push_overdue(&mut s, "回复甲方合同修改意见", 1, None, anchor, 4, created_at);
    s.todos.push(Todo {
        id: format!("t-{}", s.todos.len() + 1),
        title: "调研笔记工具".into(),
        priority: 3,
        date: None,
        original_date: None,
        project_id: None,
        status: TodoStatus::Todo,
        done_at: None,
        note: String::new(),
        postpone_count: 0,
        postpone_log: Vec::new(),
        created_at,
    });

    /* ---------------- 灵感 ---------------- */
    let note = |id: &str, content: &str, tags: &[&str], pinned: bool, offset: i64| Note {
        id: id.into(),
        content: content.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        pinned,
        created_at: now - Duration::days(offset),
        updated_at: now - Duration::days(offset),
    };
    s.notes.push(note(
        "n-1",
        "把周报从清单改成三段式：进展 / 风险 / 下一步",
        &["写作", "周报"],
        true,
        1,
    ));
    s.notes.push(note(
        "n-2",
        "项目卡点时先问「谁能解」，而不是「为什么卡」",
        &["管理"],
        false,
        2,
    ));
    s.notes.push(note(
        "n-3",
        "每周留 2 小时空白时间处理突发",
        &["节奏"],
        false,
        3,
    ));
    s.notes.push(note(
        "n-4",
        "首页改版的埋点命名统一用 snake_case",
        &["产品", "首页改版"],
        true,
        1,
    ));
    s.notes.push(note(
        "n-5",
        "博客迁移后写一篇复盘：迁移清单与踩坑",
        &["写作", "博客"],
        false,
        4,
    ));
    s.notes.push(note(
        "n-6",
        "Q3 复盘用漏斗图而不是折线图",
        &["数据", "Q3"],
        false,
        5,
    ));

    s.last_run_date = Some(anchor);
    s.seeded_at = Some(now);
    s
}

fn push_overdue(
    s: &mut DataSnapshot,
    title: &str,
    priority: u8,
    project: Option<&str>,
    anchor: NaiveDate,
    days: i64,
    created_at: NaiveDateTime,
) {
    let original = anchor - Duration::days(days);
    let first_to = anchor - Duration::days(1);
    s.todos.push(Todo {
        id: format!("t-{}", s.todos.len() + 1),
        title: title.into(),
        priority,
        date: Some(anchor),
        original_date: Some(original),
        project_id: project.map(str::to_string),
        status: TodoStatus::Todo,
        done_at: None,
        note: String::new(),
        postpone_count: 2,
        postpone_log: vec![
            PostponeEvent {
                from: Some(original),
                to: first_to,
                at: first_to.and_hms_opt(9, 0, 0).expect("合法时间"),
                auto: true,
            },
            PostponeEvent {
                from: Some(first_to),
                to: anchor,
                at: anchor.and_hms_opt(8, 30, 0).expect("合法时间"),
                auto: true,
            },
        ],
        created_at,
    });
}

fn push_future(
    s: &mut DataSnapshot,
    title: &str,
    priority: u8,
    project: Option<&str>,
    date: NaiveDate,
    created_at: NaiveDateTime,
) {
    s.todos.push(Todo {
        id: format!("t-{}", s.todos.len() + 1),
        title: title.into(),
        priority,
        date: Some(date),
        original_date: Some(date),
        project_id: project.map(str::to_string),
        status: TodoStatus::Todo,
        done_at: None,
        note: String::new(),
        postpone_count: 0,
        postpone_log: Vec::new(),
        created_at,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use office_desk_core::domain::project::{progress, sync_status};
    use office_desk_core::domain::stats;
    use office_desk_core::domain::todo::{bucket, Bucket};

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("合法日期")
    }

    #[test]
    fn seed_matches_regression_baseline() {
        // 固定锚点 2026-10-18（周日），与产品文档示例周 10/12–10/18 一致
        let anchor = day(2026, 10, 18);
        let s = seed(anchor, anchor.and_hms_opt(20, 0, 0).expect("合法时间"));

        assert_eq!(s.projects.len(), 3);
        assert_eq!(s.todos.len(), 18);
        assert_eq!(s.notes.len(), 6);

        let w = stats::week(&s, start_of_week(anchor));
        assert_eq!(w.done_count, 11);
        assert_eq!(w.denominator, 13);
        assert!((w.done_rate.expect("有完成率") - 11.0 / 13.0).abs() < 1e-9);
        assert_eq!(w.priority_count, [4, 3, 4, 0]);
        assert_eq!(w.blocked_projects, 1);
        // 文档示例写作 0·2·2·1·2·1·2，实际合计 10；种子按 11 件补正为周六 2 件
        assert_eq!(w.daily, [0, 2, 2, 1, 2, 2, 2]);
        assert_eq!(w.daily.iter().sum::<u32>(), 11);

        let overdue = s
            .todos
            .iter()
            .filter(|t| bucket(t, anchor) == Bucket::Overdue)
            .count();
        assert_eq!(overdue, 2);

        let names: Vec<&str> = s.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["Q3 增长方案", "首页改版", "个人博客迁移"]);
        assert_eq!(sync_status(&s.projects[0]), ProjectStatus::Blocked);
        assert_eq!(sync_status(&s.projects[1]), ProjectStatus::Active);

        let q3 = progress(&s.projects[0], &s.todos, anchor);
        assert_eq!((q3.total, q3.done, q3.overdue), (5, 3, 1));
        let home = progress(&s.projects[1], &s.todos, anchor);
        assert_eq!((home.total, home.done, home.overdue), (4, 3, 0));
        let blog = progress(&s.projects[2], &s.todos, anchor);
        assert_eq!((blog.total, blog.done, blog.overdue), (3, 2, 0));

        // 精力分布：首页改版 3 · Q3 3 · 其他 3 · 博客 2
        let count_of = |id: Option<&str>| {
            w.energy
                .iter()
                .find(|(k, _)| k.as_deref() == id)
                .map(|(_, v)| *v)
                .unwrap_or(0)
        };
        assert_eq!(count_of(Some("p-home")), 3);
        assert_eq!(count_of(Some("p-q3")), 3);
        assert_eq!(count_of(None), 3);
        assert_eq!(count_of(Some("p-blog")), 2);
    }
}
