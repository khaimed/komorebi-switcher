#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::core::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use winit::event_loop::EventLoop;

use crate::app::{App, AppMessage};

mod app;
mod egui_glue;
mod host;
mod komorebi;
mod main_window;
mod widgets;

fn run() -> anyhow::Result<()> {
    let evl = EventLoop::<AppMessage>::with_user_event().build()?;

    let proxy = evl.create_proxy();
    muda::MenuEvent::set_event_handler(Some(move |e| {
        if let Err(e) = proxy.send_event(AppMessage::MenuEvent(e)) {
            log::error!("Failed to send `AppMessage::MenuEvent`: {e}")
        }
    }));

    let taskbar_hwnd = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }?;

    let mut app = App::new(taskbar_hwnd, evl.create_proxy());
    evl.run_app(&mut app)?;

    Ok(())
}

fn main() {
    let env = env_logger::Env::default().default_filter_or("komorebi_switcher=info");
    let _ = env_logger::Builder::from_env(env).try_init();

    if let Err(e) = run() {
        log::error!("{e}");
        std::process::exit(1);
    }
}
