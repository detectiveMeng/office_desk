//! 今日待办视图：置顶区 → 快速添加 → 逾期 / 今天 / 未来 / 已完成。

use chrono::{Duration, NaiveDate};
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use office_desk_core::domain::todo::{bucket, group_future, overdue_days, sort_for_today, Bucket};
use office_desk_core::domain::types::{Project, Todo, TodoStatus};

use super::{card, ghost_button, hint, pill, primary_button, priority_pill, weekday_cn};
use crate::state::Workspace;
use crate::theme::Palette;

impl Workspace {
    pub(crate) fn render_today(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let today = self.today;

        let mut all = self.data.todos.clone();
        sort_for_today(&mut all, today);

        let overdue: Vec<Todo> = all
            .iter()
            .filter(|t| bucket(t, today) == Bucket::Overdue)
            .cloned()
            .collect();
        let today_list: Vec<Todo> = all
            .iter()
            .filter(|t| bucket(t, today) == Bucket::Today)
            .cloned()
            .collect();
        let mut done: Vec<Todo> = all
            .iter()
            .filter(|t| t.status == TodoStatus::Done)
            .cloned()
            .collect();
        done.sort_by(|a, b| b.done_at.cmp(&a.done_at));
        let future_groups: Vec<(Option<NaiveDate>, Vec<Todo>)> = group_future(&all, today)
            .into_iter()
            .map(|(day, list)| (day, list.into_iter().cloned().collect()))
            .collect();
        let future_count: usize = future_groups.iter().map(|(_, v)| v.len()).sum();

        let blocked: Vec<Project> = self
            .data
            .projects
            .iter()
            .filter(|proj| office_desk_core::domain::project::sync_status(proj)
                == office_desk_core::domain::types::ProjectStatus::Blocked)
            .cloned()
            .collect();

        let mut children: Vec<AnyElement> = Vec::new();

        /* 页头：元信息 + 主操作（页面名已在标题栏） */
        children.push(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(p.ink3))
                        .child(SharedString::from(format!(
                            "{} {} · {} 件待处理",
                            today.format("%Y-%m-%d"),
                            weekday_cn(today),
                            overdue.len() + today_list.len()
                        ))),
                )
                .child(
                    ghost_button("全部顺延到明天", p.red, &p)
                        .id("postpone-all")
                        .on_click(cx.listener(|this, _, _, cx| this.postpone_all_overdue(cx))),
                )
                .into_any_element(),
        );

        /* 置顶区：今天要处理的 */
        if !overdue.is_empty() || !blocked.is_empty() {
            let mut rows: Vec<Div> = Vec::new();
            for todo in &overdue {
                rows.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(8.))
                        .bg(rgb(p.red_soft))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(13.))
                                .text_color(rgb(p.red))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_ellipsis()
                                .child(SharedString::from(todo.title.clone())),
                        )
                        .child(pill(
                            format!("逾期 {} 天", overdue_days(todo, today)),
                            p.red,
                            p.paper,
                        ))
                        .child({
                            let id = todo.id.clone();
                            ghost_button("顺延", p.red, &p)
                                .id(SharedString::from(format!("pin-postpone-{id}")))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.postpone_todo(&id, cx)
                                    }),
                                )
                        }),
                );
            }
            for project in &blocked {
                rows.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(8.))
                        .bg(rgb(p.amber_soft))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(p.amber))
                                .child(SharedString::from(project.name.clone())),
                        )
                        .child(pill("卡住了", p.amber, p.paper))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(11.))
                                .text_color(rgb(p.ink2))
                                .text_ellipsis()
                                .child(SharedString::from(project.blocker.clone())),
                        ),
                );
            }

            children.push(
                card(&p)
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("今天要处理的"),
                            )
                            .child(pill(
                                format!("{} 项", overdue.len() + blocked.len()),
                                p.red,
                                p.red_soft,
                            )),
                    )
                    .children(rows)
                    .into_any_element(),
            );
        }

        /* 快速添加 */
        children.push(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(rgb(p.line))
                        .bg(rgb(p.paper))
                        .child(
                            Input::new(&self.quick_input)
                                .appearance(false)
                                .text_size(px(13.))
                                .w_full(),
                        ),
                )
                .child(
                    priority_pill(self.quick_priority, &p)
                        .id("quick-priority")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.quick_priority = (this.quick_priority + 1) % 4;
                            cx.notify();
                        })),
                )
                .child(
                    primary_button("＋ 加任务", &p)
                        .id("quick-add")
                        .on_click(cx.listener(|this, _, window, cx| {
                            let input = this.quick_input.clone();
                            this.submit_quick(&input, window, cx);
                        })),
                )
                .into_any_element(),
        );

        /* 逾期 */
        if !overdue.is_empty() {
            let mut rows: Vec<Div> = Vec::new();
            for todo in &overdue {
                rows.push(self.todo_row(todo, true, cx));
            }
            children.push(
                card(&p)
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(section_title("逾期", overdue.len(), p.red, &p))
                    .children(rows)
                    .into_any_element(),
            );
        }

        /* 今天 */
        let mut today_rows: Vec<Div> = Vec::new();
        for todo in &today_list {
            today_rows.push(self.todo_row(todo, false, cx));
        }
        children.push(
            card(&p)
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(section_title("今天", today_list.len(), p.teal, &p))
                .when(today_list.is_empty(), |el| {
                    el.child(hint("今天没有新任务，先清掉逾期或安排一件。", &p))
                })
                .children(today_rows)
                .into_any_element(),
        );

        /* 未来 */
        if !future_groups.is_empty() {
            let mut groups: Vec<Div> = Vec::new();
            for (day, list) in &future_groups {
                let mut rows: Vec<Div> = Vec::new();
                for todo in list {
                    rows.push(self.todo_row(todo, false, cx));
                }
                groups.push(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .mt(px(6.))
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(p.ink3))
                                .child(SharedString::from(future_label(*day, today))),
                        )
                        .children(rows),
                );
            }
            children.push(
                card(&p)
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(section_title("未来", future_count, p.ink2, &p))
                    .children(groups)
                    .into_any_element(),
            );
        }

        /* 已完成折叠 */
        let mut done_block = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(7.))
                    .text_size(px(12.))
                    .text_color(rgb(p.ink2))
                    .child(SharedString::from(format!(
                        "{}已完成 {} 件",
                        if self.show_done { "▾ " } else { "▸ " },
                        done.len()
                    )))
                    .id("done-toggle")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_done = !this.show_done;
                        cx.notify();
                    })),
            );
        if self.show_done {
            let mut rows: Vec<Div> = Vec::new();
            for todo in &done {
                rows.push(self.todo_row(todo, false, cx));
            }
            done_block = done_block.children(rows);
        }
        children.push(done_block.into_any_element());

        div()
            .id("today-scroll")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .px(px(24.))
            .pt(px(18.))
            .pb(px(28.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(children)
    }

    fn todo_row(&self, todo: &Todo, overdue_style: bool, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let today = self.today;
        let done = todo.status == TodoStatus::Done;
        let overdue = bucket(todo, today) == Bucket::Overdue;
        let days = overdue_days(todo, today);
        let project = todo
            .project_id
            .as_deref()
            .and_then(|id| self.project_name(id))
            .map(str::to_string);
        let meta = if overdue {
            let original = todo
                .original_date
                .map(|d| d.format("%m-%d").to_string())
                .unwrap_or_default();
            let postpone = if todo.postpone_count > 0 {
                format!(" · 顺延 {} 次", todo.postpone_count)
            } else {
                String::new()
            };
            format!("原定 {original} · 逾期 {days} 天{postpone}")
        } else {
            String::new()
        };

        let id_toggle = todo.id.clone();
        let id_postpone = todo.id.clone();
        let id_priority = todo.id.clone();
        let id_delete = todo.id.clone();

        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .when(overdue && overdue_style, |el| {
                el.bg(rgb(p.red_soft))
                    .border_l_3()
                    .border_color(rgb(p.red))
            })
            .when(!overdue && !done, |el| el.hover(|style| style.bg(rgb(p.hover))))
            .child(
                div()
                    .flex_none()
                    .w(px(16.))
                    .h(px(16.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(if done { p.teal } else { p.line }))
                    .when(done, |el| el.bg(rgb(p.teal)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(done, |el| {
                        el.child(div().text_size(px(10.)).text_color(rgb(p.paper)).child("✓"))
                    })
                    .id(SharedString::from(format!("check-{id_toggle}")))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_todo(&id_toggle, cx)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_ellipsis()
                            .when(done, |el| el.text_color(rgb(p.ink3)).line_through())
                            .child(SharedString::from(todo.title.clone())),
                    )
                    .when(!meta.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(p.red))
                                .child(SharedString::from(meta.clone())),
                        )
                    }),
            )
            .when_some(project, |el, name| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(rgb(p.ink3))
                        .child(SharedString::from(name)),
                )
            })
            .child(
                priority_pill(todo.priority, &p)
                    .id(SharedString::from(format!("prio-{id_priority}")))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.cycle_priority(&id_priority, cx)),
                    ),
            )
            .when(todo.postpone_count >= 2 && !done, |el| {
                el.child(pill(
                    format!("顺延 {} 次", todo.postpone_count),
                    p.amber,
                    p.amber_soft,
                ))
            })
            .when(!done, |el| {
                el.child(
                    ghost_button("顺延", p.teal, &p)
                        .id(SharedString::from(format!("post-{id_postpone}")))
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.postpone_todo(&id_postpone, cx)
                            }),
                        ),
                )
            })
            .child(
                div()
                    .flex_none()
                    .px(px(6.))
                    .text_size(px(14.))
                    .text_color(rgb(p.ink3))
                    .child("×")
                    .id(SharedString::from(format!("del-{id_delete}")))
                    .on_click(cx.listener(move |this, _, _, cx| this.delete_todo(&id_delete, cx))),
            )
    }
}

fn section_title(title: &'static str, count: usize, color: u32, p: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(pill(format!("{count}"), color, p.line2))
}

fn future_label(day: Option<NaiveDate>, today: NaiveDate) -> String {
    match day {
        None => "无日期".to_string(),
        Some(d) if d == today + Duration::days(1) => "明天".to_string(),
        Some(d) => format!("{} {}", weekday_cn(d), d.format("%m-%d")),
    }
}
