use app::App;
use utils::RGB;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use winit::event_loop::EventLoop;

mod _egui_glue;
mod app;
mod main_window;
mod utils;

unsafe extern "system" fn enum_child_resize(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let rect = lparam.0 as *const RECT;
    let rect = *rect;

    let _ = MoveWindow(
        hwnd,
        0,
        0,
        rect.right - rect.left,
        rect.bottom - rect.top,
        true,
    );

    true.into()
}

unsafe extern "system" fn wndproc_host(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Resize children when this host is resized
    if msg == WM_SIZE {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_ok() {
            let _ = EnumChildWindows(
                Some(hwnd),
                Some(enum_child_resize),
                LPARAM(&rect as *const _ as _),
            );
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn create_host(hinstance: HMODULE) -> anyhow::Result<HWND> {
    let taskbar_hwnd = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null())?;

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

    let hwnd = CreateWindowExW(
        WS_EX_LAYERED,
        window_class,
        PCWSTR::null(),
        WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS,
        100,
        0,
        200,
        rect.bottom - rect.top,
        None,
        None,
        None,
        None,
    )?;

    SetParent(hwnd, Some(taskbar_hwnd))?;

    SetLayeredWindowAttributes(hwnd, RGB(0, 0, 0), 0, LWA_COLORKEY)?;

    Ok(hwnd)
}

fn main() -> anyhow::Result<()> {
    let evl = EventLoop::new()?;

    let hinstance = unsafe { GetModuleHandleW(None) }?;
    let host = unsafe { create_host(hinstance) }?;

    let mut app = App::new(host);
    evl.run_app(&mut app)?;

    Ok(())
}
