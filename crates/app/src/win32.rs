//! GPUI 没有暴露窗口显隐与原生句柄，这里直接操作 Win32 HWND。

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow};

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
