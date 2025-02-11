use std::collections::HashMap;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::_egui_glue::EguiWindow;

pub struct App {
    pub wgpu_instance: wgpu::Instance,
    pub taskbar_host: HWND,
    pub windows: HashMap<WindowId, EguiWindow>,
}

impl App {
    pub fn new(host: HWND) -> Self {
        let wgpu_instance = egui_wgpu::wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        Self {
            wgpu_instance,
            taskbar_host: host,
            windows: Default::default(),
        }
    }

    pub fn host_size(&self) -> anyhow::Result<(u32, u32)> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.taskbar_host, &mut rect) }?;
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        Ok((w as u32, h as u32))
    }
}

impl ApplicationHandler<()> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            self.create_main_window(event_loop)
                .expect("Failed to create main window");
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

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

        let resposne = window.handle_input(&event);

        if resposne.repaint {
            window.handle_redraw();
        }

        match event {
            WindowEvent::Resized(size) => {
                window.handle_resized(size);
            }

            WindowEvent::CursorLeft { .. } => {
                window.request_redraw();
            }
            _ => {}
        }
    }
}
