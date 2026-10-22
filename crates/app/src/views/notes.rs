//! 灵感速记：新增、#标签筛选、置顶、转任务、删除。

use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use office_desk_core::domain::types::Note;

use super::{card, ghost_button, hint, pill, primary_button};
use crate::state::Workspace;

impl Workspace {
    pub(crate) fn render_notes(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;

        let mut notes: Vec<Note> = self
            .data
            .notes
            .iter()
            .filter(|n| match &self.tag_filter {
                Some(tag) => n.tags.iter().any(|t| t == tag),
                None => true,
            })
            .cloned()
            .collect();
        notes.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });

        let mut tags: Vec<String> = Vec::new();
        for note in &self.data.notes {
            for tag in &note.tags {
                if !tags.iter().any(|t| t == tag) {
                    tags.push(tag.clone());
                }
            }
        }
        tags.sort();

        let mut chips: Vec<AnyElement> = Vec::new();
        chips.push(
            div()
                .cursor_pointer()
                .px(px(10.))
                .py(px(4.))
                .rounded(px(6.))
                .border_1()
                .text_size(px(11.))
                .when(self.tag_filter.is_none(), |el| {
                    el.bg(rgb(p.teal_soft))
                        .border_color(rgb(p.teal))
                        .text_color(rgb(p.teal_ink))
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(self.tag_filter.is_some(), |el| {
                    el.bg(rgb(p.paper))
                        .border_color(rgb(p.line))
                        .text_color(rgb(p.ink2))
                })
                .child("全部")
                .id("chip-all")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tag_filter = None;
                    cx.notify();
                }))
                .into_any_element(),
        );
        for tag in &tags {
            let active = self.tag_filter.as_deref() == Some(tag.as_str());
            let tag_owned = tag.clone();
            chips.push(
                div()
                    .cursor_pointer()
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .text_size(px(11.))
                    .when(active, |el| {
                        el.bg(rgb(p.teal_soft))
                            .border_color(rgb(p.teal))
                            .text_color(rgb(p.teal_ink))
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!active, |el| {
                        el.bg(rgb(p.paper))
                            .border_color(rgb(p.line))
                            .text_color(rgb(p.ink2))
                    })
                    .child(SharedString::from(format!("#{tag}")))
                    .id(SharedString::from(format!("chip-{tag}")))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tag_filter =
                            if this.tag_filter.as_deref() == Some(tag_owned.as_str()) {
                                None
                            } else {
                                Some(tag_owned.clone())
                            };
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        let mut note_cards: Vec<Div> = Vec::new();
        for note in &notes {
            note_cards.push(self.note_card(note, cx));
        }

        div()
            .id("notes-scroll")
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
                    .text_size(px(12.))
                    .text_color(rgb(p.ink3))
                    .child("先记下来，再决定做不做；一条灵感可以一键转成今天的任务"),
            )
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
                                Input::new(&self.note_input)
                                    .appearance(false)
                                    .text_size(px(13.))
                                    .w_full(),
                            ),
                    )
                    .child(
                        primary_button("记下来", &p)
                            .id("note-add")
                            .on_click(cx.listener(|this, _, window, cx| {
                                let input = this.note_input.clone();
                                this.submit_note(&input, window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(p.ink3))
                            .child("转任务时归入"),
                    )
                    .child({
                        let label = self
                            .default_project
                            .as_deref()
                            .and_then(|id| self.project_name(id))
                            .map(str::to_string)
                            .unwrap_or_else(|| "不归项目".to_string());
                        ghost_button(label, p.ink2, &p)
                            .max_w(px(180.))
                            .text_ellipsis()
                            .id("notes-project")
                            .on_click(cx.listener(|this, _, _, cx| this.cycle_default_project(cx)))
                    }),
            )
            .child(div().flex().flex_wrap().gap(px(6.)).children(chips))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .children(note_cards)
                    .when(notes.is_empty(), |el| {
                        el.child(hint("这个标签下还没有灵感。", &p))
                    }),
            )
    }

    fn note_card(&self, note: &Note, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let id_pin = note.id.clone();
        let id_todo = note.id.clone();
        let id_delete = note.id.clone();
        let id_edit = note.id.clone();
        let editing = self.editing_note.as_deref() == Some(note.id.as_str());
        let confirming_delete = self.confirm_delete.as_deref() == Some(note.id.as_str());

        let mut tag_pills: Vec<Div> = Vec::new();
        for tag in &note.tags {
            tag_pills.push(pill(
                format!("#{tag}"),
                p.blue,
                p.blue_soft,
            ));
        }

        card(&p)
            .p(px(14.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .when(note.pinned, |el| el.border_color(rgb(p.teal)))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .child(
                        if editing {
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(6.))
                                .border_1()
                                .border_color(rgb(p.teal))
                                .bg(rgb(p.bg))
                                .id("note-edit")
                                .child(
                                    Input::new(&self.note_edit_input)
                                        .appearance(false)
                                        .text_size(px(13.))
                                        .w_full(),
                                )
                        } else {
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(rgb(p.ink))
                                .child(SharedString::from(note.content.clone()))
                                .id(SharedString::from(format!("ne-{id_edit}")))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_edit_note(&id_edit, window, cx)
                                }))
                        },
                    )
                    .when(note.pinned, |el| {
                        el.child(pill("置顶", p.teal, p.teal_soft))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(5.))
                            .flex_1()
                            .children(tag_pills),
                    )
                    .child(
                        ghost_button(
                            if note.pinned { "取消置顶" } else { "置顶" },
                            p.teal,
                            &p,
                        )
                        .id(SharedString::from(format!("pin-{id_pin}")))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_pin(&id_pin, cx)
                        })),
                    )
                    .child(
                        ghost_button("转任务", p.blue, &p)
                            .id(SharedString::from(format!("nt-{id_todo}")))
                            .on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.note_to_todo(&id_todo, cx)
                                }),
                            ),
                    )
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
                        .id(SharedString::from(format!("nd-{id_delete}")))
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.delete_note(&id_delete, cx)
                            }),
                        ),
                    ),
            )
    }
}
