use std::ops::Deref;
use std::sync::Arc;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM,
};
use windows::Win32::Graphics::Dxgi::DXGI_PRESENT;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use super::Dx12Surface;
use crate::app::AppMessage;

pub struct DCompWindow {
    pub window: Arc<Window>,
    pub dx12_surface: Dx12Surface,
    pub view: Box<dyn DCompView>,
}

impl Deref for DCompWindow {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl DCompWindow {
    pub fn new(window: Arc<Window>, view: impl DCompView + 'static) -> anyhow::Result<Self> {
        let surface = Dx12Surface::new(&window)?;

        Ok(Self {
            window,
            dx12_surface: surface,
            view: Box::new(view),
        })
    }

    pub(crate) fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> anyhow::Result<()> {
        if let WindowEvent::Resized(size) = event {
            self.dx12_surface.configure(size.width, size.height)?;
        }

        let draw = matches!(
            event,
            WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::Touch(_)
                | WindowEvent::Ime(_)
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::Focused(_)
                | WindowEvent::ThemeChanged(_)
                | WindowEvent::HoveredFile(_)
                | WindowEvent::HoveredFileCancelled
                | WindowEvent::DroppedFile(_)
                | WindowEvent::ModifiersChanged(_)
                | WindowEvent::RedrawRequested
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::Destroyed
                | WindowEvent::Occluded(_)
                | WindowEvent::Resized(_)
                | WindowEvent::Moved(_)
                | WindowEvent::TouchpadPressure { .. }
                | WindowEvent::CloseRequested
        );

        if draw {
            let dc = &self.dx12_surface.dc;
            unsafe { dc.BeginDraw() };
            let color = D2D1_COLOR_F {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 0.5,
            };
            let rect = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: 28.0,
                bottom: 28.0,
            };
            let brush = unsafe { dc.CreateSolidColorBrush(&color, None) }?;
            unsafe { dc.Clear(None) };
            unsafe { dc.FillRectangle(&rect, &brush) };
            unsafe { dc.EndDraw(None, None) }?;
            unsafe { self.dx12_surface.swapchain.Present(1, DXGI_PRESENT(0)) }.ok()?;
        }

        self.view.handle_window_event(event_loop, event)
    }

    pub(crate) fn handle_app_message(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &AppMessage,
    ) -> anyhow::Result<()> {
        self.view.handle_app_message(event_loop, event)
    }
}

pub trait DCompView {
    fn handle_app_message(
        &mut self,
        event_loop: &ActiveEventLoop,
        message: &AppMessage,
    ) -> anyhow::Result<()> {
        let _ = event_loop;
        let _ = message;
        Ok(())
    }

    fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> anyhow::Result<()> {
        let _ = event_loop;
        let _ = event;
        Ok(())
    }

    fn draw(&mut self, dc: ID2D1DeviceContext) -> anyhow::Result<()>;
}
