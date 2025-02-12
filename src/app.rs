use std::collections::HashMap;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::_egui_glue::EguiWindow;

pub enum AppMessage {
    UpdateWorkspaces(Vec<crate::komorebi::Workspace>),
    MenuEvent(muda::MenuEvent),
}

pub struct App {
    pub wgpu_instance: wgpu::Instance,
    pub proxy: EventLoopProxy<AppMessage>,
    pub taskbar_hwnd: HWND,
    pub host: HWND,
    pub windows: HashMap<WindowId, EguiWindow>,
}

impl App {
    pub fn new(taskbar_hwnd: HWND, host: HWND, proxy: EventLoopProxy<AppMessage>) -> Self {
        let wgpu_instance = egui_wgpu::wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        Self {
            wgpu_instance,
            taskbar_hwnd,
            host,
            windows: Default::default(),
            proxy,
        }
    }

    pub fn host_size(&self) -> anyhow::Result<(u32, u32)> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.host, &mut rect) }?;
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        Ok((w as u32, h as u32))
    }
}

impl ApplicationHandler<AppMessage> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            self.create_main_window(event_loop)
                .expect("Failed to create main window");
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppMessage) {
        for window in self.windows.values_mut() {
            window.view.handle_app_message(event_loop, &event);
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            self.windows.remove(&window_id);
            event_loop.exit();
        }

        let Some(window) = self.windows.get_mut(&window_id) else {
            return;
        };

        window.handle_window_event(event_loop, event);
    }
}
