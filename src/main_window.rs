use std::num::NonZero;
use std::sync::Arc;

use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, SetWindowPos, SWP_NOMOVE};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowAttributes};

use crate::_egui_glue::{EguiView, EguiWindow};
use crate::app::App;

pub struct MainWindowView {
    window: Arc<Window>,
    host: HWND,

    curr_width: i32,
}

impl MainWindowView {
    fn new(window: Arc<Window>, host: HWND) -> Self {
        Self {
            window,
            host,
            curr_width: 0,
        }
    }
}

impl App {
    pub fn create_main_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let mut attrs = WindowAttributes::default();

        let (w, h) = self.host_size()?;
        attrs = attrs.with_inner_size(PhysicalSize::new(w, h));

        let parent = unsafe { NonZero::new_unchecked(self.taskbar_host.0 as _) };
        let parent = Win32WindowHandle::new(parent);
        let parent = RawWindowHandle::Win32(parent);

        attrs = unsafe { attrs.with_parent_window(Some(parent)) };
        attrs = attrs
            .with_decorations(false)
            .with_transparent(true)
            .with_class_name("komorebi-switcher::window")
            .with_undecorated_shadow(false)
            .with_clip_children(false);

        let window = event_loop.create_window(attrs)?;
        let window = Arc::new(window);

        let state = MainWindowView::new(window.clone(), self.taskbar_host);

        let window = EguiWindow::new(window, &self.wgpu_instance, state);

        self.windows.insert(window.id(), window);

        Ok(())
    }
}

impl MainWindowView {
    fn resize_host_to_rect(&mut self, rect: egui::Rect) {
        let width = rect.width() as f64 + 16.0 /* default margin 8 on each side */;
        let width = self.window.scale_factor() * width;
        let width = width as i32;

        if width != self.curr_width {
            self.curr_width = width;

            let mut rect = RECT::default();
            if unsafe { GetClientRect(self.host, &mut rect) }.is_ok() {
                let _ = unsafe {
                    SetWindowPos(
                        self.host,
                        None,
                        0,
                        0,
                        width,
                        rect.bottom - rect.top,
                        SWP_NOMOVE,
                    )
                };
            }
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) -> egui::InnerResponse<()> {
        ui.horizontal(|ui| {
            for i in 1..20 {
                if ui.button(i.to_string()).clicked() {
                    dbg!(i);
                }
            }
        })
    }
}

impl EguiView for MainWindowView {
    fn update(&mut self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::default();
        visuals.panel_fill = egui::Color32::TRANSPARENT;
        ctx.set_visuals(visuals);

        egui::CentralPanel::default().show(ctx, |ui| {
            let response = self.draw(ui);
            self.resize_host_to_rect(response.response.rect);
        });
    }
}
