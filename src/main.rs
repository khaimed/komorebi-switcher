use app::{App, AppMessage};
use utils::RGB;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use winit::event_loop::EventLoop;

mod app;
mod utils;

unsafe extern "system" fn wndproc_host(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Resize children when this host is resized
    if msg == WM_SIZE {
        unsafe extern "system" fn enumerate_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
            dbg!(1);
            let _ = PostMessageW(Some(hwnd), WM_SIZE, WPARAM::default(), LPARAM::default());
            true.into()
        }
        let _ = EnumChildWindows(Some(hwnd), Some(enumerate_callback), LPARAM::default());
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

    SetLayeredWindowAttributes(hwnd, RGB(0, 255, 255), 255, LWA_ALPHA)?;

    Ok(hwnd)
}

fn main() -> anyhow::Result<()> {
    let evl = EventLoop::<AppMessage>::with_user_event().build()?;

    let hinstance = unsafe { GetModuleHandleW(None) }?;
    let host = unsafe { create_host(hinstance) }?;

    let mut app = App::new(evl.create_proxy(), host);
    evl.run_app(&mut app)?;

    Ok(())
}
