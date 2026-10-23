//! 右下角通知区托盘图标：关闭主窗口后应用仍可从这里恢复。
//!
//! GPUI 没有托盘能力，这里用 Win32 消息窗口 + Shell_NotifyIcon 单独起一个
//! 消息循环线程；左键单击恢复主窗口，右键菜单可恢复或真正退出。

#![cfg(windows)]

use std::sync::Once;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TRAY_UID: u32 = 1;
const WM_TRAY: u32 = WM_APP + 1;
const MENU_SHOW: usize = 1;
const MENU_QUIT: usize = 2;

static START: Once = Once::new();
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static MINI_HWND: AtomicIsize = AtomicIsize::new(0);

pub(crate) fn set_main_hwnd(hwnd: isize) {
    MAIN_HWND.store(hwnd, Ordering::SeqCst);
}

pub(crate) fn set_mini_hwnd(hwnd: isize) {
    MINI_HWND.store(hwnd, Ordering::SeqCst);
}

pub(crate) fn clear_mini_hwnd() {
    MINI_HWND.store(0, Ordering::SeqCst);
}

/// 首次收起时启动托盘线程（只启动一次）。
pub(crate) fn ensure_started() {
    START.call_once(|| {
        std::thread::spawn(|| unsafe { run_tray() });
    });
}

/// 显示主窗口、隐藏右下角小窗。
pub(crate) fn restore_main() {
    let main = MAIN_HWND.load(Ordering::SeqCst);
    if main != 0 {
        unsafe {
            // SW_RESTORE：无论是隐藏还是最小化都能恢复到正常大小
            ShowWindowAsync(main as HWND, SW_RESTORE);
            SetForegroundWindow(main as HWND);
        }
    }
    hide_mini();
}

pub(crate) fn hide_mini() {
    let mini = MINI_HWND.load(Ordering::SeqCst);
    if mini != 0 {
        unsafe { ShowWindowAsync(mini as HWND, SW_HIDE) };
    }
}

unsafe fn run_tray() {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = windows_sys::w!("OfficeDeskTrayWindow");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        // 用隐藏的普通顶层窗口接收托盘回调（消息窗口不会被 FindWindow 等工具发现）
        let hwnd = CreateWindowExW(
            0,
            class_name,
            windows_sys::w!("OfficeDeskTray"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }

        // 进程内嵌的图标资源 id 为 1（见 assets/icon.rc）
        let icon = LoadImageW(
            hinstance,
            1 as *const u16,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        ) as HICON;

        let mut data = NOTIFYICONDATAW::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = TRAY_UID;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = icon;
        let tip: Vec<u16> = "办公工作台 · 今日待办".encode_utf16().take(127).collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        Shell_NotifyIconW(NIM_ADD, &data);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TRAY => {
                match (lparam as u32) & 0xFFFF {
                    WM_LBUTTONUP => restore_main(),
                    WM_RBUTTONUP => show_menu(hwnd),
                    _ => {}
                }
                0
            }
            WM_COMMAND => {
                match wparam & 0xFFFF {
                    MENU_SHOW => restore_main(),
                    MENU_QUIT => std::process::exit(0),
                    _ => {}
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, MENU_SHOW, windows_sys::w!("显示工作台"));
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, MENU_QUIT, windows_sys::w!("退出"));

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        PostMessageW(hwnd, WM_NULL, 0, 0);

        match cmd as usize {
            MENU_SHOW => restore_main(),
            MENU_QUIT => std::process::exit(0),
            _ => {}
        }
    }
}
