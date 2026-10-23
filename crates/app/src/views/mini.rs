//! 收起后的右下角小窗：只展示「今日待办」，点标题可拖拽，点「打开工作台」恢复主窗口。

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::*;
use office_desk_core::domain::todo::{bucket, overdue_days, sort_for_today, Bucket};
use office_desk_core::domain::types::{DataSnapshot, Todo, TodoStatus};

use super::tint;
use crate::state::Workspace;
use crate::theme::Palette;
use crate::tray;
use crate::win32;

/// 小窗宽高（logical px）
const MINI_WIDTH: f32 = 320.;
const MINI_HEIGHT: f32 = 420.;

/// 记录小窗句柄，供主窗口再次收起时复用、以及托盘恢复时隐藏。
#[derive(Default)]
pub(crate) struct MiniWindowState {
    pub(crate) handle: RefCell<Option<AnyWindowHandle>>,
}

/// 小窗展示的任务：今天到期 + 逾期，按今日清单口径排序。
pub(crate) fn mini_items(data: &DataSnapshot, today: chrono::NaiveDate) -> Vec<Todo> {
    let mut items: Vec<Todo> = data
        .todos
        .iter()
        .filter(|t| t.status == TodoStatus::Todo)
        .filter(|t| matches!(bucket(t, today), Bucket::Overdue | Bucket::Today))
        .cloned()
        .collect();
    sort_for_today(&mut items, today);
    items
}

/// 显示（必要时创建）右下角小窗。
pub(crate) fn show_mini(
    window: &mut Window,
    cx: &mut App,
    workspace: Entity<Workspace>,
    state: &Rc<MiniWindowState>,
) {
    tray::ensure_started();

    if let Some(handle) = state.handle.borrow().clone() {
        if handle
            .update(cx, |_, window, _| win32::show_window(window))
            .is_ok()
        {
            return;
        }
        *state.handle.borrow_mut() = None;
    }

    let (origin, size) = {
        let bounds = window
            .display(cx)
            .map(|display| display.visible_bounds())
            .unwrap_or_else(|| Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1280.), px(720.)),
            });
        let width = px(MINI_WIDTH);
        let height = px(MINI_HEIGHT);
        let origin = point(
            bounds.origin.x + bounds.size.width - width - px(16.),
            bounds.origin.y + bounds.size.height - height - px(16.),
        );
        (origin, size(width, height))
    };

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size })),
        titlebar: Some(TitlebarOptions {
            title: Some("今日待办".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        kind: WindowKind::PopUp,
        is_resizable: false,
        is_minimizable: false,
        focus: false,
        show: true,
        ..Default::default()
    };

    let workspace_for_view = workspace.clone();
    let state_for_view = state.clone();
    let opened = cx.open_window(options, move |window, cx| {
        if let Some(hwnd) = win32::current_hwnd(window) {
            tray::set_mini_hwnd(hwnd as isize);
        }
        let view = cx.new(|cx| MiniView::new(workspace_for_view.clone(), state_for_view.clone(), cx));
        cx.new(|cx| base::Root::new(view, window, cx))
    });

    if let Ok(handle) = opened {
        *state.handle.borrow_mut() = Some(handle.into());
    }
}

/// 右下角小窗视图。
struct MiniView {
    workspace: Entity<Workspace>,
    state: Rc<MiniWindowState>,
    _observe: Subscription,
}

impl MiniView {
    fn new(workspace: Entity<Workspace>, state: Rc<MiniWindowState>, cx: &mut Context<Self>) -> Self {
        let _observe = cx.observe(&workspace, |_, _, cx| cx.notify());
        Self {
            workspace,
            state,
            _observe,
        }
    }
}

impl Render for MiniView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (p, items) = {
            let ws = self.workspace.read(cx);
            (ws.palette, mini_items(&ws.data, ws.today))
        };
        let today = self.workspace.read(cx).today;
        let count = items.len();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(p.paper))
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(p.line))
            .overflow_hidden()
            .child(header(count, &p, &self.state, cx))
            .child(if items.is_empty() {
                empty(&p).into_any_element()
            } else {
                list(items, today, &p, &self.workspace, cx).into_any_element()
            })
            .child(footer(&p, cx))
    }
}

fn header(
    count: usize,
    p: &Palette,
    state: &Rc<MiniWindowState>,
    cx: &mut Context<MiniView>,
) -> impl IntoElement {
    let state = state.clone();
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .px(px(12.))
        .py(px(8.))
        .bg(rgb(p.bg))
        .border_b_1()
        .border_color(rgb(p.line))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .w(px(18.))
                .h(px(18.))
                .rounded(px(5.))
                .bg(rgb(p.teal))
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(p.paper))
                .child("✓"),
        )
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(p.ink))
                .child("今日待办"),
        )
        .child(super::pill(format!("{count} 件"), p.teal, p.teal_soft))
        .child(
            div()
                .flex_1()
                .h(px(18.))
                .window_control_area(WindowControlArea::Drag),
        )
        .child(
            div()
                .id("mini-close")
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .w(px(18.))
                .h(px(18.))
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(14.))
                .text_color(rgb(p.ink3))
                .hover(|style| style.bg(rgb(p.hover)).text_color(rgb(p.ink)))
                .child("×")
                .on_click(cx.listener(move |_, _, window, _| {
                    *state.handle.borrow_mut() = None;
                    tray::clear_mini_hwnd();
                    window.remove_window();
                })),
        )
}

fn list(
    items: Vec<Todo>,
    today: chrono::NaiveDate,
    p: &Palette,
    workspace: &Entity<Workspace>,
    cx: &mut Context<MiniView>,
) -> impl IntoElement {
    div()
        .id("mini-list")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .gap(px(6.))
        .px(px(10.))
        .py(px(8.))
        .overflow_y_scroll()
        .children(items.into_iter().map(|todo| {
            let overdue = bucket(&todo, today) == Bucket::Overdue;
            let id = todo.id.clone();
            let toggle_id = SharedString::from(format!("mini-toggle-{}", todo.id));
            let ws = workspace.clone();
            let (fg, _) = super::priority_colors(todo.priority, p);
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .py(px(7.))
                .rounded(px(7.))
                .bg(rgb(p.bg))
                .border_1()
                .border_color(if overdue { tint(p.red, 0x38) } else { rgb(p.line2) })
                .child(
                    div()
                        .id(toggle_id)
                        .flex_none()
                        .w(px(13.))
                        .h(px(13.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(rgb(fg))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.hover)))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            ws.update(cx, |ws, cx| ws.toggle_todo(&id, cx));
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(rgb(p.ink))
                                .text_ellipsis()
                                .child(SharedString::from(todo.title.clone())),
                        )
                        .when(overdue, |el| {
                            el.child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(rgb(p.red))
                                    .child(format!("逾期 {} 天", overdue_days(&todo, today))),
                            )
                        }),
                )
        }))
}

fn empty(p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_size(px(12.))
        .text_color(rgb(p.ink3))
        .child("今天没有待办了")
}

fn footer(p: &Palette, cx: &mut Context<MiniView>) -> impl IntoElement {
    div()
        .px(px(12.))
        .py(px(9.))
        .border_t_1()
        .border_color(rgb(p.line))
        .child(
            div()
                .id("mini-open")
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .py(px(6.))
                .rounded(px(8.))
                .bg(rgb(p.teal))
                .text_color(rgb(p.paper))
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .hover(|style| style.opacity(0.9))
                .child("打开工作台")
                .on_click(cx.listener(|_, _, window, _| {
                    tray::restore_main();
                    win32::hide_window(window);
                })),
        )
}

#[cfg(test)]
mod tests {
    use super::mini_items;
    use chrono::NaiveDate;
    use office_desk_core::domain::types::{DataSnapshot, Todo, TodoStatus};

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn todo(id: &str, date: Option<NaiveDate>, original: Option<NaiveDate>, done: bool) -> Todo {
        let at = day(2026, 10, 1).and_hms_opt(9, 0, 0).unwrap();
        Todo {
            id: id.into(),
            title: id.into(),
            priority: 1,
            date,
            original_date: original,
            project_id: None,
            status: if done { TodoStatus::Done } else { TodoStatus::Todo },
            done_at: if done { Some(at) } else { None },
            note: String::new(),
            postpone_count: 0,
            postpone_log: Vec::new(),
            created_at: at,
        }
    }

    #[test]
    fn mini_items_keep_overdue_and_today_only() {
        let today = day(2026, 10, 20);
        let yesterday = day(2026, 10, 19);
        let tomorrow = day(2026, 10, 21);

        let mut data = DataSnapshot::new();
        data.todos.push(todo("done", Some(today), Some(today), true));
        data.todos.push(todo("today", Some(today), Some(today), false));
        data.todos.push(todo("overdue", Some(today), Some(yesterday), false));
        data.todos.push(todo("future", Some(tomorrow), Some(tomorrow), false));
        data.todos.push(todo("someday", None, None, false));

        let ids: Vec<String> = mini_items(&data, today).into_iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["overdue".to_string(), "today".to_string()]);
    }
}
