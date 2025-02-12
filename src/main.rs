#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_registry::CURRENT_USER;
use winit::event_loop::EventLoop;

use crate::app::{App, AppMessage};

mod _egui_glue;
mod app;
mod komorebi;
mod main_window;

const APP_REG_KEY: &str = "SOFTWARE\\amrbashir\\komorebi-switcher";
const WINDOW_POS_X_KEY: &str = "window-pos-x";
const WINDOW_POS_Y_KEY: &str = "window-pos-y";

unsafe extern "system" fn enum_child_resize(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let rect = lparam.0 as *const RECT;
    let rect = *rect;

    let _ = SetWindowPos(
        hwnd,
        None,
        0,
        0,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOMOVE | SWP_FRAMECHANGED,
    );

    true.into()
}

unsafe extern "system" fn enum_child_close(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let _ = SendMessageW(hwnd, WM_CLOSE, None, None);
    true.into()
}

unsafe extern "system" fn wndproc_host(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // Disable position changes in y direction
        WM_WINDOWPOSCHANGING => {
            let window_pos = &mut *(lparam.0 as *mut WINDOWPOS);
            window_pos.y = 0;
        }

        // Save host position to be loaded on startup
        WM_WINDOWPOSCHANGED => {
            let window_pos = &*(lparam.0 as *const WINDOWPOS);

            let key = CURRENT_USER.create(APP_REG_KEY);
            if let Ok(key) = key {
                let _ = key.set_string(WINDOW_POS_X_KEY, &window_pos.x.to_string());
                let _ = key.set_string(WINDOW_POS_Y_KEY, &window_pos.y.to_string());
            }
        }

        // Resize children when this host is resized
        WM_SIZE => {
            let mut rect = RECT::default();
            if GetClientRect(hwnd, &mut rect).is_ok() {
                let _ = EnumChildWindows(
                    Some(hwnd),
                    Some(enum_child_resize),
                    LPARAM(&rect as *const _ as _),
                );
            }
        }

        // Close children when this host is closed
        WM_CLOSE => {
            let _ = EnumChildWindows(Some(hwnd), Some(enum_child_close), LPARAM::default());
        }

        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn create_host(hinstance: HMODULE, taskbar_hwnd: HWND) -> anyhow::Result<HWND> {
    let mut rect = RECT::default();
    GetClientRect(taskbar_hwnd, &mut rect)?;

    let window_class = w!("komorebi-switcher::host");

    let wc = WNDCLASSW {
        hInstance: hinstance.into(),
        lpszClassName: window_class,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc_host),
        ..Default::default()
    };

    let atom = RegisterClassW(&wc);
    debug_assert!(atom != 0);

    let key = CURRENT_USER.create(APP_REG_KEY)?;
    let window_pos_x = key.get_string(WINDOW_POS_X_KEY).ok();
    let window_pos_y = key.get_string(WINDOW_POS_Y_KEY).ok();
    let window_pos_x = window_pos_x.and_then(|s| s.parse().ok());
    let window_pos_y = window_pos_y.and_then(|s| s.parse().ok());

    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_NOACTIVATE,
        window_class,
        PCWSTR::null(),
        WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS,
        window_pos_x.unwrap_or(100),
        window_pos_y.unwrap_or(0),
        200,
        rect.bottom - rect.top,
        None,
        None,
        None,
        None,
    )?;

    SetParent(hwnd, Some(taskbar_hwnd))?;

    SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_COLORKEY)?;

    SetWindowPos(
        hwnd,
        Some(HWND_TOP),
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    )?;

    Ok(hwnd)
}

fn main() -> anyhow::Result<()> {
    let evl = EventLoop::<AppMessage>::with_user_event().build()?;

    let taskbar_hwnd = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }?;

    let hinstance = unsafe { GetModuleHandleW(None) }?;
    let host = unsafe { create_host(hinstance, taskbar_hwnd) }?;

    let proxy = evl.create_proxy();
    muda::MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(AppMessage::MenuEvent(e));
    }));

    let mut app = App::new(taskbar_hwnd, host, evl.create_proxy());
    evl.run_app(&mut app)?;

    Ok(())
}
