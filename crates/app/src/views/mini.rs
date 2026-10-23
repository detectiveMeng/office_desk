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

/// 小窗宽度（logical px）
const MINI_WIDTH: f32 = 320.;
/// 最多完整展示的待办条数，超出的在窗口内滚动
const MINI_VISIBLE_ROWS: usize = 3;
/// 标题栏高度（含底部分隔线）
const HEADER_HEIGHT: f32 = 32.;
/// 底部按钮区高度（含顶部分隔线）
const FOOTER_HEIGHT: f32 = 42.;
/// 单条待办行高
const ROW_HEIGHT: f32 = 32.;
/// 行间距
const ROW_GAP: f32 = 6.;
/// 列表上下内边距合计
const LIST_PADDING: f32 = 16.;
/// 空态内容高度
const EMPTY_HEIGHT: f32 = 46.;

/// 小窗高度：只按前三行算，窗口不会撑成大块遮挡桌面。
pub(crate) fn mini_height(item_count: usize) -> f32 {
    let rows = item_count.min(MINI_VISIBLE_ROWS);
    let body = if rows == 0 {
        EMPTY_HEIGHT
    } else {
        rows as f32 * ROW_HEIGHT + (rows - 1) as f32 * ROW_GAP
    };
    HEADER_HEIGHT + LIST_PADDING + body + FOOTER_HEIGHT
}

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

    // 高度随条数（最多三行）变化，复用已有窗口时也要跟着收紧
    let count = {
        let ws = workspace.read(cx);
        mini_items(&ws.data, ws.today).len()
    };
    let width = px(MINI_WIDTH);
    let height = px(mini_height(count));

    if let Some(handle) = state.handle.borrow().clone() {
        if handle
            .update(cx, |_, window, _| {
                window.resize(size(width, height));
                win32::show_window(window);
            })
            .is_ok()
        {
            return;
        }
        *state.handle.borrow_mut() = None;
    }

    let (origin, window_size) = {
        let bounds = window
            .display(cx)
            .map(|display| display.visible_bounds())
            .unwrap_or_else(|| Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1280.), px(720.)),
            });
        let origin = point(
            bounds.origin.x + bounds.size.width - width - px(16.),
            bounds.origin.y + bounds.size.height - height - px(16.),
        );
        (origin, size(width, height))
    };

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin,
            size: window_size,
        })),
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
            // 画布用最深的 bg，条目用 paper 卡片，形成明显色差
            .bg(rgb(p.bg))
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
        .gap(px(6.))
        .px(px(10.))
        .h(px(HEADER_HEIGHT))
        .bg(rgb(p.bg))
        .border_b_1()
        .border_color(rgb(p.line))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .w(px(16.))
                .h(px(16.))
                .rounded(px(4.))
                .bg(rgb(p.teal))
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(p.bg))
                .child("✓"),
        )
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(p.ink))
                .child("今日待办"),
        )
        .child(super::pill(format!("{count} 件"), p.teal_ink, p.teal_soft))
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
                .text_size(px(13.))
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
        .gap(px(ROW_GAP))
        .px(px(10.))
        .py(px(LIST_PADDING / 2.))
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
                .gap(px(7.))
                .px(px(8.))
                .h(px(ROW_HEIGHT))
                .rounded(px(7.))
                .bg(if overdue { rgb(p.red_soft) } else { rgb(p.paper) })
                .border_1()
                .border_color(if overdue { tint(p.red, 0x38) } else { rgb(p.line) })
                .child(
                    div()
                        .id(toggle_id)
                        .flex_none()
                        .w(px(12.))
                        .h(px(12.))
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
                        .text_size(px(11.5))
                        .text_color(rgb(p.ink2))
                        .text_ellipsis()
                        .child(SharedString::from(todo.title.clone())),
                )
                .when(overdue, |el| {
                    el.child(
                        div()
                            .flex_none()
                            .text_size(px(10.))
                            .text_color(rgb(p.red))
                            .child(format!("逾期 {} 天", overdue_days(&todo, today))),
                    )
                })
        }))
}

fn empty(p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_size(px(11.))
        .text_color(rgb(p.ink3))
        .child("今天没有待办了")
}

fn footer(p: &Palette, cx: &mut Context<MiniView>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .px(px(10.))
        .h(px(FOOTER_HEIGHT))
        .bg(rgb(p.bg))
        .border_t_1()
        .border_color(rgb(p.line))
        .child(
            div()
                .id("mini-open")
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .py(px(5.))
                .rounded(px(7.))
                .bg(rgb(p.teal))
                .text_color(rgb(p.bg))
                .text_size(px(11.5))
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
    use super::{mini_height, mini_items, MINI_VISIBLE_ROWS};
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

    #[test]
    fn mini_height_grows_until_three_rows_then_stops() {
        let empty = mini_height(0);
        let one = mini_height(1);
        let two = mini_height(2);
        let three = mini_height(MINI_VISIBLE_ROWS);

        assert!(one < two && two < three);
        assert!(empty <= three);
        // 超过三行不再增高，改为窗口内滚动
        assert_eq!(three, mini_height(4));
        assert_eq!(three, mini_height(20));
        // 三行高度应该保持克制，不遮挡桌面
        assert!(three <= 200.0, "mini window too tall: {three}");
    }
}
