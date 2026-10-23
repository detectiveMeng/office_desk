//! GPUI 没有暴露窗口显隐与原生句柄，这里直接操作 Win32 HWND。

use gpui_kit::{Point, Size, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowRect, HWND_TOPMOST, IsWindowVisible, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos, ShowWindow,
};

/// 取窗口原生句柄（仅 Windows 平台有效）。
pub(crate) fn current_hwnd(window: &Window) -> Option<HWND> {
    // Window 自带的 window_handle() 返回的是 GPUI 句柄，这里显式走 raw-window-handle。
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

/// 收起窗口（不销毁，保留应用进程与状态）。
pub(crate) fn hide_window(window: &Window) {
    if let Some(hwnd) = current_hwnd(window) {
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
}

/// 重新显示窗口，但不抢占当前焦点。
pub(crate) fn show_window(window: &Window) {
    if let Some(hwnd) = current_hwnd(window) {
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    }
}

/// 移动 + 调整窗口尺寸（逻辑像素），并保持置顶，用于小精灵在「图标 / 卡片」两态间切换。
///
/// 尺寸必须走 `Window::resize` 才能让 GPUI 重新布局；只移动位置时才用 Win32，
/// 这样缩放的同时还能保住右下角锚点。
pub(crate) fn set_window_bounds(
    window: &mut Window,
    origin: Point<gpui_kit::Pixels>,
    size: Size<gpui_kit::Pixels>,
) {
    window.resize(size);
    let Some(hwnd) = current_hwnd(window) else {
        return;
    };
    let scale = window.scale_factor();
    let x = (origin.x.as_f32() * scale).round() as i32;
    let y = (origin.y.as_f32() * scale).round() as i32;
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// 鼠标是否落在窗口矩形内（窗口隐藏时一律返回 false）。
///
/// 用系统光标位置判断，而不是依赖 GPUI 的 hover 事件：弹出式无焦点窗口在
/// 刚显示时 hover 状态并不可靠，这里以系统光标为准。
pub(crate) fn cursor_inside(window: &Window) -> bool {
    let Some(hwnd) = current_hwnd(window) else {
        return false;
    };
    unsafe {
        if IsWindowVisible(hwnd) == 0 {
            return false;
        }
        let mut point = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut point) == 0 {
            return false;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return false;
        }
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    }
}
