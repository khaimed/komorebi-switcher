use windows::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::_egui_glue::EguiWindow;
use crate::main_window::MainWindow;

pub struct App {
    pub wgpu_instance: wgpu::Instance,
    pub host: HWND,
    pub main_window: Option<EguiWindow<MainWindow>>,
}

impl App {
    pub fn new(host: HWND) -> Self {
        let wgpu_instance = egui_wgpu::wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        Self {
            wgpu_instance,
            host,
            main_window: None,
        }
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
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            self.main_window.take();
            event_loop.exit();
        }

        let Some(main_window) = self.main_window.as_mut() else {
            return;
        };

        let resposne = main_window.handle_input(&event);

        if resposne.repaint {
            main_window.handle_redraw();
        }

        match event {
            WindowEvent::Resized(size) => {
                main_window.handle_resized(size);
            }

            WindowEvent::CursorLeft { .. } => {
                main_window.request_redraw();
            }
            _ => {}
        }
    }
}
