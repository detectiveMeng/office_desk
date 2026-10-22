//! 项目跟进视图：进度、当前 / 下一步 / 卡点行内编辑，卡点驱动状态。

use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use office_desk_core::domain::project::{progress, sort_projects, sync_status};
use office_desk_core::domain::types::{Project, ProjectStatus, TodoStatus};

use super::{bar, card, ghost_button, hint, pill, primary_button};
use crate::state::{Workspace, PROJECT_FILTERS};
use crate::theme::Palette;

impl Workspace {
    pub(crate) fn render_projects(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let mut projects = self.data.projects.clone();
        sort_projects(&mut projects, &self.data.todos, self.today);
        projects.retain(|project| self.project_filter.matches(sync_status(project)));
        let mut cards: Vec<AnyElement> = Vec::new();
        for project in &projects {
            cards.push(self.project_card(project, cx).into_any_element());
        }

        let mut filters: Vec<AnyElement> = Vec::new();
        for filter in PROJECT_FILTERS {
            let active = self.project_filter == filter;
            filters.push(
                div()
                    .cursor_pointer()
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(999.))
                    .text_size(px(11.5))
                    .when(active, |el| {
                        el.bg(rgb(p.teal_soft))
                            .text_color(rgb(p.teal))
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!active, |el| el.text_color(rgb(p.ink3)))
                    .child(filter.label())
                    .id(SharedString::from(format!("pf-{}", filter.label())))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.project_filter = filter;
                        this.confirm_delete = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        div()
            .id("projects-scroll")
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
                                Input::new(&self.project_input)
                                    .appearance(false)
                                    .text_size(px(13.))
                                    .w_full(),
                            ),
                    )
                    .child(
                        primary_button("＋ 加项目", &p)
                            .id("project-add")
                            .on_click(cx.listener(|this, _, window, cx| {
                                let input = this.project_input.clone();
                                this.submit_project(&input, window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.ink3))
                            .child("卡点驱动状态（不手动选「卡住」），进度来自关联待办"),
                    )
                    .child(div().flex().items_center().gap(px(4.)).children(filters)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(14.))
                    .children(cards)
                    .when(projects.is_empty(), |el| {
                        el.child(hint(
                            if self.data.projects.is_empty() {
                                "还没有项目。"
                            } else {
                                "该筛选下没有项目。"
                            },
                            &p,
                        ))
                    }),
            )
    }

    fn project_card(&self, project: &Project, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let status = sync_status(project);
        let (status_label, status_fg, status_bg) = match status {
            ProjectStatus::Active => ("进行中", p.teal, p.teal_soft),
            ProjectStatus::Blocked => ("卡住了", p.red, p.red_soft),
            ProjectStatus::Done => ("已完成", p.ink3, p.line2),
            ProjectStatus::Paused => ("暂停", p.amber, p.amber_soft),
        };
        let prog = progress(project, &self.data.todos, self.today);
        let rate = if prog.total > 0 {
            prog.done as f32 / prog.total as f32
        } else {
            0.0
        };
        let percent = if prog.total > 0 {
            format!("{:.0}%", rate * 100.0)
        } else {
            "暂无关联待办".to_string()
        };

        let linked: Vec<&office_desk_core::domain::types::Todo> = self
            .data
            .todos
            .iter()
            .filter(|t| t.project_id.as_deref() == Some(project.id.as_str()))
            .collect();

        let inputs = self.project_inputs.get(&project.id);
        let mut fields: Vec<Div> = Vec::new();
        if let Some(inputs) = inputs {
            fields.push(field_row("当前", &inputs.current, &p));
            fields.push(field_row("下一步", &inputs.next, &p));
            fields.push(field_row("卡点", &inputs.blocker, &p));
        }

        let mut linked_rows: Vec<Div> = Vec::new();
        for todo in linked.iter().take(4) {
            let done = todo.status == TodoStatus::Done;
            linked_rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.5))
                    .text_color(rgb(if done { p.ink3 } else { p.ink2 }))
                    .child(if done { "✓" } else { "·" })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_ellipsis()
                            .child(SharedString::from(todo.title.clone())),
                    ),
            );
        }
        if linked.len() > 4 {
            linked_rows.push(hint(format!("还有 {} 条…", linked.len() - 4), &p));
        }
        if linked.is_empty() {
            linked_rows.push(hint("暂无关联待办", &p));
        }

        let id_done = project.id.clone();
        let id_pause = project.id.clone();
        let id_delete = project.id.clone();
        let confirming_delete = self.confirm_delete.as_deref() == Some(project.id.as_str());

        card(&p)
            .w(px(400.))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .when(status == ProjectStatus::Done, |el| el.opacity(0.55))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(SharedString::from(project.name.clone())),
                    )
                    .child(pill(status_label, status_fg, status_bg)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.teal))
                            .child(SharedString::from(format!(
                                "{}/{} · {}",
                                prog.done, prog.total, percent
                            ))),
                    )
                    .when(prog.overdue > 0, |el| {
                        el.child(pill(
                            format!("逾期 {}", prog.overdue),
                            p.red,
                            p.red_soft,
                        ))
                    }),
            )
            .child(bar(rate, p.teal, p.line2))
            .children(fields)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .mt(px(2.))
                    .children(linked_rows),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .mt(px(2.))
                    .child(
                        ghost_button(
                            if status == ProjectStatus::Done {
                                "恢复项目"
                            } else {
                                "标记完成"
                            },
                            p.teal,
                            &p,
                        )
                        .id(SharedString::from(format!("pdone-{id_done}")))
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.toggle_project_done(&id_done, cx)
                            }),
                        ),
                    )
                    .child(
                        ghost_button(
                            if status == ProjectStatus::Paused {
                                "恢复推进"
                            } else {
                                "暂停"
                            },
                            p.amber,
                            &p,
                        )
                        .id(SharedString::from(format!("ppause-{id_pause}")))
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.toggle_project_pause(&id_pause, cx)
                            }),
                        ),
                    )
                    .child(div().flex_1())
                    .child(
                        ghost_button(
                            if confirming_delete {
                                "确认删除"
                            } else {
                                "删除"
                            },
                            p.red,
                            &p,
                        )
                        .id(SharedString::from(format!("pdel-{id_delete}")))
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.delete_project(&id_delete, cx)
                            }),
                        ),
                    ),
            )
    }
}

fn field_row(label: &'static str, input: &Entity<gpui_kit::component::input::InputState>, p: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .flex_none()
                .w(px(48.))
                .text_size(px(11.))
                .text_color(rgb(p.ink3))
                .child(label),
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
                    Input::new(input)
                        .appearance(false)
                        .text_size(px(12.))
                        .w_full(),
                ),
        )
}
