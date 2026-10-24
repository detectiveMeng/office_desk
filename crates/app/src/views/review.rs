//! 每周复盘：四类图表 + 三段手写总结。

use chrono::{Duration, NaiveDate};
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use niu_ma_workstation_core::domain::stats;

use super::{
    bar, card, ghost_button, hint, pill, priority_colors, priority_label, weekday_cn,
};
use crate::state::Workspace;

impl Workspace {
    pub(crate) fn render_review(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let week_start = self.shown_week_start();
        let week_end = week_start + Duration::days(6);
        let st = stats::week(&self.data, week_start);
        let is_current = self.week_offset == 0;

        let saved_at = self
            .data
            .reviews
            .iter()
            .find(|r| r.week_start == week_start)
            .and_then(|r| r.updated_at)
            .map(|t| t.format("%m-%d %H:%M").to_string());

        /* KPI */
        let done_rate = st
            .done_rate
            .map(|v| format!("{:.1}%", v * 100.0))
            .unwrap_or_else(|| "—".to_string());
        let kpis = div()
            .flex()
            .gap(px(12.))
            .child(kpi("本周完成", st.done_count.to_string(), p.teal, &p))
            .child(kpi("完成率", done_rate, p.teal, &p))
            .child(kpi("顺延事件", st.postpone_count.to_string(), p.amber, &p))
            .child(kpi(
                "卡住项目",
                st.blocked_projects.to_string(),
                p.red,
                &p,
            ));

        /* 优先级分布 */
        let priority_max = st.priority_count.iter().copied().max().unwrap_or(1).max(1);
        let mut priority_rows: Vec<Div> = Vec::new();
        for index in 0..4usize {
            let count = st.priority_count[index];
            let (fg, _) = priority_colors(index as u8, &p);
            priority_rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(92.))
                            .text_size(px(11.))
                            .text_color(rgb(p.ink2))
                            .child(SharedString::from(format!(
                                "P{index} {}",
                                priority_label(index as u8)
                            ))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .child(bar(count as f32 / priority_max as f32, fg, p.line2)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(24.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.ink2))
                            .child(SharedString::from(count.to_string())),
                    ),
            );
        }

        /* 精力分布 */
        let energy_max = st
            .energy
            .iter()
            .map(|(_, v)| *v)
            .max()
            .unwrap_or(1)
            .max(1);
        let mut energy_rows: Vec<Div> = Vec::new();
        for (project_id, count) in &st.energy {
            let label = project_id
                .as_deref()
                .and_then(|id| self.project_name(id))
                .unwrap_or("其他");
            energy_rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(92.))
                            .text_size(px(11.))
                            .text_color(rgb(p.ink2))
                            .text_ellipsis()
                            .child(SharedString::from(label.to_string())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .child(bar(*count as f32 / energy_max as f32, p.blue, p.line2)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(24.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.ink2))
                            .child(SharedString::from(count.to_string())),
                    ),
            );
        }
        if energy_rows.is_empty() {
            energy_rows.push(hint("这一周还没有完成的任务。", &p));
        }

        /* 一周节奏 */
        let daily_max = st.daily.iter().copied().max().unwrap_or(1).max(1);
        let mut days: Vec<Div> = Vec::new();
        for offset in 0..7i64 {
            let count = st.daily[offset as usize];
            let height = 6.0 + (count as f32 / daily_max as f32) * 78.0;
            let day = week_start + Duration::days(offset);
            days.push(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .flex_1()
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .h(px(84.))
                            .w_full()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(height))
                                    .rounded(px(3.))
                                    .bg(rgb(if count > 0 { p.teal } else { p.line2 })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(p.ink3))
                            .child(weekday_cn(day)),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(if count > 0 { p.teal } else { p.ink3 }))
                            .child(SharedString::from(count.to_string())),
                    ),
            );
        }

        /* 卡点汇总：本周所有卡住过的项目，含已解除 */
        let mut blocker_items: Vec<(String, String, NaiveDate, Option<NaiveDate>)> = Vec::new();
        for project in &self.data.projects {
            let mut has_open = false;
            for event in &project.blocker_log {
                if event.start_at >= week_start && event.start_at <= week_end {
                    blocker_items.push((
                        project.name.clone(),
                        event.text.clone(),
                        event.start_at,
                        event.end_at,
                    ));
                    if event.end_at.is_none() {
                        has_open = true;
                    }
                }
            }
            // 兼容早期没有卡点流水的数据
            if !project.blocker.is_empty() && !has_open {
                blocker_items.push((
                    project.name.clone(),
                    project.blocker.clone(),
                    self.today,
                    None,
                ));
            }
        }
        blocker_items.sort_by(|a, b| {
            a.3.is_some()
                .cmp(&b.3.is_some())
                .then_with(|| b.2.cmp(&a.2))
        });

        let mut blocker_rows: Vec<Div> = Vec::new();
        for (name, text, start, end) in &blocker_items {
            let days = end
                .unwrap_or(self.today)
                .signed_duration_since(*start)
                .num_days()
                .max(0)
                + 1;
            blocker_rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(96.))
                            .text_size(px(11.))
                            .text_color(rgb(p.ink2))
                            .text_ellipsis()
                            .child(SharedString::from(name.clone())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(11.))
                            .text_color(rgb(p.ink3))
                            .text_ellipsis()
                            .child(SharedString::from(text.clone())),
                    )
                    .child(pill(
                        if end.is_some() {
                            format!("已解除 · {days} 天")
                        } else {
                            format!("卡住中 · {days} 天")
                        },
                        if end.is_some() { p.ink3 } else { p.amber },
                        if end.is_some() { p.line2 } else { p.amber_soft },
                    )),
            );
        }

        /* 三段总结 */
        let week_label = format!(
            "{} – {}",
            week_start.format("%m/%d"),
            week_end.format("%m/%d")
        );
        let saved_hint = saved_at
            .map(|t| format!("上次保存 {t}"))
            .unwrap_or_else(|| "还没有保存过".to_string());

        let mut summary: Vec<Div> = Vec::new();
        for (index, label) in ["做得好", "值得留意", "下周聚焦"].iter().enumerate() {
            summary.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(60.))
                            .text_size(px(11.))
                            .text_color(rgb(p.ink3))
                            .child(*label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .px(px(8.))
                            .py(px(3.))
                            .rounded(px(6.))
                            .bg(rgb(p.line2))
                            .child(
                                Input::new(&self.review_inputs[index])
                                    .appearance(false)
                                    .text_size(px(12.))
                                    .w_full(),
                            ),
                    ),
            );
        }

        div()
            .id("review-scroll")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .px(px(24.))
            .pt(px(18.))
            .pb(px(28.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.ink3))
                            .child(SharedString::from(format!(
                                "{week_label} · 分母 = 本周到期 ∪ 本周完成"
                            ))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                ghost_button("‹ 上一周", p.ink2, &p)
                                    .id("week-prev")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.shift_week(-1, window, cx)
                                    })),
                            )
                            .when(!is_current, |el| {
                                el.child(
                                    ghost_button("本周", p.teal, &p)
                                        .id("week-now")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reset_week(window, cx)
                                        })),
                                )
                            })
                            .child(
                                ghost_button("下一周 ›", p.ink2, &p)
                                    .id("week-next")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.shift_week(1, window, cx)
                                    })),
                            ),
                    ),
            )
            .child(kpis)
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .child(
                        card(&p)
                            .flex_1()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(section("优先级分布", &p))
                            .children(priority_rows),
                    )
                    .child(
                        card(&p)
                            .flex_1()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(section("精力分布", &p))
                            .children(energy_rows),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .child(
                        card(&p)
                            .flex_1()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(section("一周节奏", &p))
                            .child(div().flex().items_end().gap(px(10.)).children(days)),
                    )
                    .child(
                        card(&p)
                            .flex_1()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(section("卡点汇总", &p))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .children(blocker_rows)
                                    .when(blocker_items.is_empty(), |el| {
                                        el.child(hint("本周没有卡点记录。", &p))
                                    }),
                            ),
                    ),
            )
            .child(
                card(&p)
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(section("本周复盘", &p))
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(rgb(p.ink3))
                                    .child(SharedString::from(saved_hint)),
                            ),
                    )
                    .children(summary)
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(rgb(p.ink3))
                            .child("回车或移开焦点即保存；切换周会自动载入该周的记录"),
                    ),
            )
    }
}

fn kpi(label: &'static str, value: String, color: u32, p: &crate::theme::Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .flex_1()
        .bg(rgb(p.paper))
        .rounded(px(12.))
        .border_1()
        .border_color(rgb(p.line))
        .px(px(16.))
        .py(px(14.))
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(p.ink3))
                .child(label),
        )
        .child(
            div()
                .text_size(px(22.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(color))
                .child(SharedString::from(value)),
        )
}

fn section(title: &'static str, p: &crate::theme::Palette) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(p.ink))
        .child(title)
}
