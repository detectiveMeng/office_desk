//! 工作台状态与动作层：内存快照 + SQLite 落库 + 四视图切换。
//!
//! 所有动作统一走「改快照 → storage.save → cx.notify」，保证界面与磁盘一致。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration, Local, NaiveDate, NaiveDateTime};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::*;
use office_desk_core::domain::date::start_of_week;
use office_desk_core::domain::project::sync_status;
use office_desk_core::domain::todo::{bucket, Bucket};
use office_desk_core::domain::types::*;

use crate::config::Config;
use crate::storage::Storage;
use crate::theme::{Palette, ThemeMode};

/// 左侧导航的四个模块
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Page {
    Today,
    Notes,
    Projects,
    Review,
}

impl Page {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Page::Today => "今日待办",
            Page::Notes => "灵感速记",
            Page::Projects => "项目跟进",
            Page::Review => "每周复盘",
        }
    }
}

/// 项目卡上的三个可编辑字段
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Current,
    Next,
    Blocker,
}

pub(crate) struct ProjectInputs {
    pub(crate) current: Entity<InputState>,
    pub(crate) next: Entity<InputState>,
    pub(crate) blocker: Entity<InputState>,
}

pub struct Workspace {
    pub(crate) storage: Storage,
    pub(crate) data: DataSnapshot,
    pub(crate) page: Page,
    pub(crate) mode: ThemeMode,
    pub(crate) palette: Palette,
    pub(crate) today: NaiveDate,
    pub(crate) quick_input: Entity<InputState>,
    pub(crate) quick_priority: u8,
    pub(crate) note_input: Entity<InputState>,
    pub(crate) tag_filter: Option<String>,
    pub(crate) show_done: bool,
    pub(crate) week_offset: i32,
    pub(crate) review_inputs: [Entity<InputState>; 3],
    pub(crate) project_inputs: HashMap<String, ProjectInputs>,
    pub(crate) notice: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        storage: Storage,
        data: DataSnapshot,
        config: Config,
    ) -> Self {
        let today = Local::now().date_naive();
        let week_start = start_of_week(today);

        let quick_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("记一件事，回车添加（默认 重要不紧急 · 今天）")
        });
        let note_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("随手记一条灵感，可用 #标签 标记")
        });

        let review_inputs = [
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("这周做得好的")
                    .default_value(review_value(&data, week_start, 0))
            }),
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("值得留意 / 想改的")
                    .default_value(review_value(&data, week_start, 1))
            }),
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("下周聚焦什么")
                    .default_value(review_value(&data, week_start, 2))
            }),
        ];

        let mut subscriptions: Vec<Subscription> = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &quick_input,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit_quick(input, window, cx);
                }
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &note_input,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit_note(input, window, cx);
                }
            },
        ));
        for (index, input) in review_inputs.iter().enumerate() {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        this.save_review_field(index, window, cx);
                    }
                },
            ));
        }

        let mut project_inputs: HashMap<String, ProjectInputs> = HashMap::new();
        for project in &data.projects {
            let current = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("当前在推进什么")
                    .default_value(project.current.clone())
            });
            let next = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("下一步是什么")
                    .default_value(project.next.clone())
            });
            let blocker = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("卡在哪里（留空 = 进行中）")
                    .default_value(project.blocker.clone())
            });

            let pid = project.id.clone();
            for (field, entity) in [
                (Field::Current, &current),
                (Field::Next, &next),
                (Field::Blocker, &blocker),
            ] {
                let pid = pid.clone();
                subscriptions.push(cx.subscribe_in(
                    entity,
                    window,
                    move |this, _input, event: &InputEvent, _window, cx| {
                        if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                            this.save_project_field(&pid, field, cx);
                        }
                    },
                ));
            }

            project_inputs.insert(
                project.id.clone(),
                ProjectInputs {
                    current,
                    next,
                    blocker,
                },
            );
        }

        Self {
            storage,
            data,
            page: Page::Today,
            mode: config.theme,
            palette: Palette::of(config.theme),
            today,
            quick_input,
            quick_priority: 1,
            note_input,
            tag_filter: None,
            show_done: false,
            week_offset: 0,
            review_inputs,
            project_inputs,
            notice: None,
            _subscriptions: subscriptions,
        }
    }

    /* ---------------- 通用 ---------------- */

    fn now(&self) -> NaiveDateTime {
        Local::now().naive_local()
    }

    pub(crate) fn shown_week_start(&self) -> NaiveDate {
        start_of_week(self.today) + Duration::days(7 * self.week_offset as i64)
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.storage.save(&self.data) {
            self.notice = Some(format!("保存失败：{err}"));
        }
        cx.notify();
    }

    fn flash(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.notice = Some(message.into());
        self.commit(cx);
    }

    pub(crate) fn project_name(&self, id: &str) -> Option<&str> {
        self.data
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
    }

    /* ---------------- 待办动作 ---------------- */

    pub(crate) fn submit_quick(
        &mut self,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = input.read(cx).value().trim().to_string();
        if title.is_empty() {
            return;
        }
        let now = self.now();
        self.data.todos.push(Todo {
            id: next_id("t"),
            title,
            priority: self.quick_priority,
            date: Some(self.today),
            original_date: Some(self.today),
            project_id: None,
            status: TodoStatus::Todo,
            done_at: None,
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at: now,
        });
        input.update(cx, |state, cx| state.set_value("", window, cx));
        self.flash("已加入今天", cx);
    }

    pub(crate) fn toggle_todo(&mut self, id: &str, cx: &mut Context<Self>) {
        let now = self.now();
        if let Some(todo) = self.data.todos.iter_mut().find(|t| t.id == id) {
            if todo.status == TodoStatus::Done {
                todo.status = TodoStatus::Todo;
                todo.done_at = None;
            } else {
                todo.status = TodoStatus::Done;
                todo.done_at = Some(now);
            }
        }
        self.commit(cx);
    }

    pub(crate) fn postpone_todo(&mut self, id: &str, cx: &mut Context<Self>) {
        let now = self.now();
        let tomorrow = self.today + Duration::days(1);
        let mut message = None;
        if let Some(todo) = self.data.todos.iter_mut().find(|t| t.id == id) {
            if todo.status == TodoStatus::Todo {
                let from = todo.date;
                todo.postpone_log.push(PostponeEvent {
                    from,
                    to: tomorrow,
                    at: now,
                    auto: false,
                });
                todo.postpone_count += 1;
                todo.date = Some(tomorrow);
                message = Some(format!("已顺延到明天，第 {} 次", todo.postpone_count));
            }
        }
        match message {
            Some(msg) => self.flash(msg, cx),
            None => self.commit(cx),
        }
    }

    pub(crate) fn postpone_all_overdue(&mut self, cx: &mut Context<Self>) {
        let now = self.now();
        let today = self.today;
        let tomorrow = today + Duration::days(1);
        let mut count = 0usize;
        for todo in self.data.todos.iter_mut() {
            if todo.status == TodoStatus::Todo && bucket(todo, today) == Bucket::Overdue {
                let from = todo.date;
                todo.postpone_log.push(PostponeEvent {
                    from,
                    to: tomorrow,
                    at: now,
                    auto: false,
                });
                todo.postpone_count += 1;
                todo.date = Some(tomorrow);
                count += 1;
            }
        }
        if count > 0 {
            self.flash(format!("已把 {count} 件逾期任务顺延到明天"), cx);
        } else {
            self.commit(cx);
        }
    }

    pub(crate) fn delete_todo(&mut self, id: &str, cx: &mut Context<Self>) {
        self.data.todos.retain(|t| t.id != id);
        self.flash("已删除任务", cx);
    }

    pub(crate) fn cycle_priority(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(todo) = self.data.todos.iter_mut().find(|t| t.id == id) {
            todo.priority = (todo.priority + 1) % 4;
        }
        self.commit(cx);
    }

    /* ---------------- 灵感动作 ---------------- */

    pub(crate) fn submit_note(
        &mut self,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = input.read(cx).value().trim().to_string();
        if content.is_empty() {
            return;
        }
        let now = self.now();
        let mut tags: Vec<String> = Vec::new();
        for word in content.split_whitespace() {
            if let Some(tag) = word.strip_prefix('#') {
                let tag = tag.trim_matches(|c: char| c == ',' || c == '，' || c == '。' || c == '#');
                if !tag.is_empty() && !tags.iter().any(|t| t == tag) {
                    tags.push(tag.to_string());
                }
            }
        }
        self.data.notes.push(Note {
            id: next_id("n"),
            content,
            tags,
            pinned: false,
            created_at: now,
            updated_at: now,
        });
        input.update(cx, |state, cx| state.set_value("", window, cx));
        self.flash("已记录灵感", cx);
    }

    pub(crate) fn toggle_pin(&mut self, id: &str, cx: &mut Context<Self>) {
        let now = self.now();
        if let Some(note) = self.data.notes.iter_mut().find(|n| n.id == id) {
            note.pinned = !note.pinned;
            note.updated_at = now;
        }
        self.commit(cx);
    }

    pub(crate) fn delete_note(&mut self, id: &str, cx: &mut Context<Self>) {
        self.data.notes.retain(|n| n.id != id);
        self.flash("已删除灵感", cx);
    }

    pub(crate) fn note_to_todo(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(note) = self.data.notes.iter().find(|n| n.id == id) else {
            return;
        };
        let title = note.content.clone();
        let now = self.now();
        self.data.todos.push(Todo {
            id: next_id("t"),
            title,
            priority: 1,
            date: Some(self.today),
            original_date: Some(self.today),
            project_id: None,
            status: TodoStatus::Todo,
            done_at: None,
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at: now,
        });
        self.flash("已转为今天的任务", cx);
    }

    /* ---------------- 项目动作 ---------------- */

    fn save_project_field(
        &mut self,
        pid: &str,
        _field: Field,
        cx: &mut Context<Self>,
    ) {
        let Some(inputs) = self.project_inputs.get(pid) else {
            return;
        };
        let current = inputs.current.read(cx).value().to_string();
        let next = inputs.next.read(cx).value().to_string();
        let blocker = inputs.blocker.read(cx).value().to_string();
        let now = self.now();
        if let Some(project) = self.data.projects.iter_mut().find(|p| p.id == pid) {
            project.current = current;
            project.next = next;
            project.blocker = blocker;
            project.updated_at = now;
            project.status = sync_status(project);
        }
        self.commit(cx);
    }

    pub(crate) fn toggle_project_done(&mut self, id: &str, cx: &mut Context<Self>) {
        let now = self.now();
        if let Some(project) = self.data.projects.iter_mut().find(|p| p.id == id) {
            project.status = if project.status == ProjectStatus::Done {
                if project.blocker.trim().is_empty() {
                    ProjectStatus::Active
                } else {
                    ProjectStatus::Blocked
                }
            } else {
                ProjectStatus::Done
            };
            project.updated_at = now;
        }
        self.commit(cx);
    }

    pub(crate) fn toggle_project_pause(&mut self, id: &str, cx: &mut Context<Self>) {
        let now = self.now();
        if let Some(project) = self.data.projects.iter_mut().find(|p| p.id == id) {
            project.status = if project.status == ProjectStatus::Paused {
                if project.blocker.trim().is_empty() {
                    ProjectStatus::Active
                } else {
                    ProjectStatus::Blocked
                }
            } else {
                ProjectStatus::Paused
            };
            project.updated_at = now;
        }
        self.commit(cx);
    }

    /* ---------------- 复盘动作 ---------------- */

    fn save_review_field(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        let value = self.review_inputs[index].read(cx).value().to_string();
        let week_start = self.shown_week_start();
        let now = self.now();
        if let Some(review) = self
            .data
            .reviews
            .iter_mut()
            .find(|r| r.week_start == week_start)
        {
            match index {
                0 => review.good = value,
                1 => review.watch = value,
                _ => review.focus = value,
            }
            review.updated_at = Some(now);
        } else {
            let mut review = WeeklyReview {
                week_start,
                good: String::new(),
                watch: String::new(),
                focus: String::new(),
                updated_at: Some(now),
            };
            match index {
                0 => review.good = value,
                1 => review.watch = value,
                _ => review.focus = value,
            }
            self.data.reviews.push(review);
        }
        self.commit(cx);
    }

    pub(crate) fn shift_week(&mut self, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        self.week_offset += delta;
        let week_start = self.shown_week_start();
        for index in 0..3 {
            let value = review_value(&self.data, week_start, index);
            self.review_inputs[index].update(cx, |state, cx| {
                state.set_value(value, window, cx);
            });
        }
        self.notice = None;
        cx.notify();
    }

    pub(crate) fn reset_week(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.week_offset = 0;
        let week_start = self.shown_week_start();
        for index in 0..3 {
            let value = review_value(&self.data, week_start, index);
            self.review_inputs[index].update(cx, |state, cx| {
                state.set_value(value, window, cx);
            });
        }
        self.notice = None;
        cx.notify();
    }

    /* ---------------- 主题 ---------------- */

    pub(crate) fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.mode = self.mode.toggled();
        self.palette = Palette::of(self.mode);
        let config = Config { theme: self.mode };
        if let Err(err) = config.save() {
            self.notice = Some(format!("主题已切换，但保存配置失败：{err}"));
        } else {
            self.notice = Some(format!("已切换到{}主题", self.mode.label()));
        }
        cx.notify();
    }
}

fn review_value(data: &DataSnapshot, week_start: NaiveDate, index: usize) -> String {
    data.reviews
        .iter()
        .find(|r| r.week_start == week_start)
        .map(|r| match index {
            0 => r.good.clone(),
            1 => r.watch.clone(),
            _ => r.focus.clone(),
        })
        .unwrap_or_default()
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_id(prefix: &str) -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let n = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{micros:x}-{n:x}")
}

/* ---------------- 根视图 ---------------- */

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let titlebar = self.render_titlebar(window);
        let sidebar = self.render_sidebar(cx);
        let body = match self.page {
            Page::Today => self.render_today(cx).into_any_element(),
            Page::Notes => self.render_notes(cx).into_any_element(),
            Page::Projects => self.render_projects(cx).into_any_element(),
            Page::Review => self.render_review(cx).into_any_element(),
        };
        let notice = self.notice.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.ink))
            .child(titlebar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(body)
                            .when_some(notice, |el, text| {
                                el.child(
                                    div()
                                        .absolute()
                                        .bottom(px(18.))
                                        .right(px(18.))
                                        .px(px(14.))
                                        .py(px(8.))
                                        .rounded(px(8.))
                                        .bg(rgb(palette.teal_ink))
                                        .text_color(rgb(palette.paper))
                                        .text_size(px(12.))
                                        .child(SharedString::from(text)),
                                )
                            }),
                    ),
            )
    }
}

impl Workspace {
    /// 自绘标题栏：左侧与侧栏同色延续，右侧与内容区同色，窗口控件贴右上角。
    ///
    /// Windows 下 `window_control_area` 让系统负责拖拽 / 双击最大化 / 最小化 / 关闭，
    /// 因此这里不需要绑定点击事件。
    fn render_titlebar(&self, window: &Window) -> Div {
        let palette = self.palette;
        let maximize = if window.is_maximized() {
            RESTORE_SVG
        } else {
            MAXIMIZE_SVG
        };

        div()
            .flex()
            .flex_row()
            .h(px(34.))
            .w_full()
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(palette.line))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(7.))
                    .pl(px(16.))
                    .w(px(232.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .bg(rgb(palette.paper))
                    .border_r_1()
                    .border_color(rgb(palette.line))
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("办公工作台"),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(palette.ink3))
                            .child("Workbench"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .h_full()
                    .items_center()
                    .justify_end()
                    .bg(rgb(palette.bg))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h_full()
                            .pl(px(24.))
                            .flex_shrink_0()
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.page.title()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .window_control_area(WindowControlArea::Drag),
                    )
                    .child(self.titlebar_button(WindowControlArea::Min, MINIMIZE_SVG, false))
                    .child(self.titlebar_button(WindowControlArea::Max, maximize, false))
                    .child(self.titlebar_button(WindowControlArea::Close, CLOSE_SVG, true)),
            )
    }

    fn titlebar_button(
        &self,
        area: WindowControlArea,
        icon: &'static [u8],
        is_close: bool,
    ) -> AnyElement {
        let palette = self.palette;
        let (hover_bg, hover_fg) = if is_close {
            (palette.red, 0xffffff)
        } else {
            (palette.hover, palette.ink)
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(46.))
            .h_full()
            .flex_shrink_0()
            .text_color(rgb(palette.ink2))
            .hover(move |style| style.bg(rgb(hover_bg)).text_color(rgb(hover_fg)))
            .window_control_area(area)
            .child(Icon::default().data(icon).small())
            .into_any_element()
    }
}

/// 窗口控件图标：内联 SVG（组件库的图标资源未随 crate 发布，路径式图标无法加载）。
const MINIMIZE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="3" y="7.5" width="10" height="1" rx="0.5"/></svg>"#;
const MAXIMIZE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill-rule="evenodd" d="M3.5 3.5h9v9h-9zm1.2 1.2v6.6h6.6V4.7z"/></svg>"#;
const RESTORE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill-rule="evenodd" d="M5 2.5h8.5V11h-1.2V3.7H5zm-2.5 2.5h8.5v8.5H2.5zm1.2 1.2v6.1h6.1V6.2z"/></svg>"#;
const CLOSE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M4.1 3.1 8 7l3.9-3.9 1 1L9 8l3.9 3.9-1 1L8 9l-3.9 3.9-1-1L7 8 3.1 4.1z"/></svg>"#;

impl Workspace {
    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> Div {
        let palette = self.palette;
        let pages = [
            Page::Today,
            Page::Notes,
            Page::Projects,
            Page::Review,
        ];
        let mut nav: Vec<AnyElement> = Vec::new();
        for page in pages {
            let active = self.page == page;
            nav.push(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .text_size(px(13.))
                    .when(active, |el| {
                        el.bg(rgb(palette.teal_soft))
                            .text_color(rgb(palette.teal_ink))
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!active, |el| el.text_color(rgb(palette.ink2)))
                    .child(page.title())
                    .id(SharedString::from(format!("nav-{}", page.title())))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = page;
                        this.notice = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .w(px(232.))
            .h_full()
            .bg(rgb(palette.paper))
            .border_r_1()
            .border_color(rgb(palette.line))
            .px(px(16.))
            .pt(px(18.))
            .pb(px(14.))
            .gap(px(6.))
            .children(nav)
            .child(div().flex_1())
            .child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(palette.line))
                    .text_size(px(12.))
                    .text_color(rgb(palette.ink2))
                    .child(SharedString::from(format!(
                        "切换{}主题",
                        self.mode.toggled().label()
                    )))
                    .id("theme-toggle")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
            )
    }
}
