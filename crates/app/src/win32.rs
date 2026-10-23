//! GPUI 没有暴露窗口显隐与原生句柄，这里直接操作 Win32 HWND。

use gpui_kit::{Point, Size, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
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

/// 外框相对客户区的左上偏移（物理像素）：DWM 的不可见边框左右、上下各占一半。
fn border_offset(window: &Window, client: Size<gpui_kit::Pixels>) -> (i32, i32) {
    let Some((left, top, right, bottom)) = window_rect(window) else {
        return (0, 0);
    };
    let scale = window.scale_factor();
    let client_w = (client.width.as_f32() * scale).round() as i32;
    let client_h = (client.height.as_f32() * scale).round() as i32;
    (
        ((right - left) - client_w) / 2,
        ((bottom - top) - client_h) / 2,
    )
}

/// 按客户区坐标摆窗口：`Window::resize` 用的是客户区尺寸，而 SetWindowPos 定位的是
/// 含不可见边框的外框，这里把目标客户区左上角换算成外框左上角，避免每次缩放都偏移。
pub(crate) fn set_window_bounds(
    window: &mut Window,
    origin: Point<gpui_kit::Pixels>,
    size: Size<gpui_kit::Pixels>,
) {
    // 必须在 resize 之前取偏移：resize 是异步落到系统窗口上的，此时外框与
    // `window.bounds()` 还是同一时刻的旧值，两者之差恰好就是不可见边框。
    let (offset_x, offset_y) = border_offset(window, window.bounds().size);
    window.resize(size);
    let Some(hwnd) = current_hwnd(window) else {
        return;
    };
    let scale = window.scale_factor();
    let x = (origin.x.as_f32() * scale).round() as i32 - offset_x;
    let y = (origin.y.as_f32() * scale).round() as i32 - offset_y;
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

/// 去掉 DWM 给窗口画的原生描边（Windows 11 会给活动窗口加一圈强调色边框）。
///
/// 小精灵窗口整窗透明，这圈边框会显得像多出来一个外框，所以显式关掉。
pub(crate) fn hide_native_border(window: &Window) {
    let Some(hwnd) = current_hwnd(window) else {
        return;
    };
    let color: u32 = DWMWA_COLOR_NONE;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &color as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// 当前系统光标位置（物理像素）。
pub(crate) fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    unsafe { (GetCursorPos(&mut point) != 0).then_some((point.x, point.y)) }
}

/// 左键是否处于按下状态。
pub(crate) fn left_button_down() -> bool {
    unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 }
}

/// 窗口矩形（物理像素）：左上、右下。
pub(crate) fn window_rect(window: &Window) -> Option<(i32, i32, i32, i32)> {
    let hwnd = current_hwnd(window)?;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        (GetWindowRect(hwnd, &mut rect) != 0).then_some((rect.left, rect.top, rect.right, rect.bottom))
    }
}

/// 只移动窗口位置（不改尺寸），并保持置顶，用于拖动小精灵。
pub(crate) fn move_window(window: &Window, x: i32, y: i32) {
    let Some(hwnd) = current_hwnd(window) else {
        return;
    };
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

/// 外框左上角在 (x, y) 时，客户区右下角的逻辑坐标；用于记住拖动落点。
pub(crate) fn anchor_for(window: &Window, x: i32, y: i32) -> Option<(f32, f32)> {
    let client = window.bounds().size;
    let (offset_x, offset_y) = border_offset(window, client);
    let scale = window.scale_factor();
    Some((
        (x + offset_x) as f32 / scale + client.width.as_f32(),
        (y + offset_y) as f32 / scale + client.height.as_f32(),
    ))
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
