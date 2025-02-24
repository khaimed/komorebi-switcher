use std::sync::Arc;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::*;
use winit::event::WindowEvent;
use winit::window::Window;

use super::dcomp_surface::Dx12Surface;
use super::EguiView;

pub struct EguiDcompRenderer {
    pub state: egui_winit::State,
    dx_surface: Dx12Surface,
    frame_started: bool,
}

impl EguiDcompRenderer {
    pub fn new(window: &Arc<Window>) -> anyhow::Result<Self> {
        let egui_context = egui::Context::default();

        {
            let window = window.clone();
            egui_context.set_request_repaint_callback(move |_| {
                window.request_redraw();
            });
        }

        let egui_state = egui_winit::State::new(
            egui_context,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(2 * 1024), // default dimension is 2048
        );

        let dx_surface = Dx12Surface::new(window)?;

        Ok(Self {
            state: egui_state,
            frame_started: false,
            dx_surface,
        })
    }

    pub fn egui_ctx(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(&mut self, window: &Window) {
        if !self.frame_started {
            panic!("begin_frame must be called before end_frame_and_draw can be called!");
        }

        let full_output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let ppp = self.egui_ctx().pixels_per_point();

        let tris = self.egui_ctx().tessellate(full_output.shapes, ppp);

        self.render_tris(tris);

        self.frame_started = false;
    }

    fn render_tris(&self, tris: Vec<egui::epaint::ClippedPrimitive>) {
        let ppp = self.egui_ctx().pixels_per_point();

        let dx_surface = &self.dx_surface;
        let dc = &dx_surface.dc;

        unsafe { dc.BeginDraw() };
        unsafe { dc.Clear(None) };

        for tri in tris {
            let egui::epaint::Primitive::Mesh(mesh) = tri.primitive else {
                continue;
            };

            todo!("Render mesh");
        }

        let _ = unsafe { dc.EndDraw(None, None) };
        let _ = unsafe { dx_surface.swapchain.Present(1, DXGI_PRESENT(0)) };
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if let Err(e) = self.dx_surface.configure(width, height) {
            tracing::error!("Error while resizing dx12 surface: {e}");
        }
    }

    pub fn handle_input(
        &mut self,
        window: &Window,
        event: &WindowEvent,
    ) -> egui_winit::EventResponse {
        self.state.on_window_event(window, event)
    }

    pub fn handle_resized(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.resize_surface(width, height);
        }
    }

    pub fn handle_redraw(
        &mut self,
        window: &Arc<Window>,
        egui_view: &mut (impl EguiView + ?Sized),
    ) -> anyhow::Result<()> {
        self.begin_frame(window);

        let ctx = self.egui_ctx();

        egui_view.update(ctx);

        self.end_frame_and_draw(window);

        Ok(())
    }
}

fn create_solid_brush(
    dc: &ID2D1DeviceContext1,
    color: egui::Color32,
) -> windows::core::Result<ID2D1SolidColorBrush> {
    let color = D2D1_COLOR_F {
        r: color.r() as f32 / 255.0,
        g: color.g() as f32 / 255.0,
        b: color.b() as f32 / 255.0,
        a: color.a() as f32 / 255.0,
    };
    unsafe { dc.CreateSolidColorBrush(&color, None) }
}
