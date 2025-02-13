use std::ops::Deref;
use std::sync::Arc;

use anyhow::Context;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::core::*;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::D2D1CreateDevice;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::IDXGIDevice3;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::app::AppMessage;

pub trait EguiView {
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

    fn update(&mut self, ctx: &egui::Context);
}

pub struct EguiWindow {
    pub window: Arc<Window>,
    pub surface: SurfaceState,
    pub view: Box<dyn EguiView>,
}

impl Deref for EguiWindow {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl EguiWindow {
    pub fn new(
        window: Arc<Window>,
        instance: &wgpu::Instance,
        view: impl EguiView + 'static,
    ) -> anyhow::Result<Self> {
        let surface = pollster::block_on(SurfaceState::new(&window, instance))?;
        Ok(Self {
            window,
            surface,
            view: Box::new(view),
        })
    }

    pub fn handle_input(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.surface.handle_input(&self.window, event)
    }

    pub fn handle_resized(&mut self, size: PhysicalSize<u32>) {
        self.surface.handle_resized(size.width, size.height);
    }

    pub fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> anyhow::Result<()> {
        let resposne = self.handle_input(&event);

        if resposne.repaint {
            self.handle_redraw()?;
        }

        match event {
            WindowEvent::Resized(size) => {
                self.handle_resized(size);
            }

            WindowEvent::CursorLeft { .. } => {
                self.request_redraw();
            }
            _ => {}
        }

        self.view.handle_window_event(event_loop, event)
    }
}

impl EguiWindow {
    pub fn handle_redraw(&mut self) -> anyhow::Result<()> {
        self.surface.handle_redraw(&self.window, self.view.as_mut())
    }
}

pub struct SurfaceState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    scale_factor: f32,
    egui_renderer: EguiRenderer,
    #[allow(unused)]
    dx12_surface: Dx12Surface,
}

impl SurfaceState {
    pub async fn new(window: &Arc<Window>, instance: &wgpu::Instance) -> anyhow::Result<Self> {
        let dx12_surface = Dx12Surface::new(window)?;

        let visual = dx12_surface.wgpu_visual.as_raw();
        let visual = wgpu::SurfaceTargetUnsafe::CompositionVisual(visual);
        let surface = unsafe { instance.create_surface_unsafe(visual)? };

        let power_pref = wgpu::PowerPreference::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: power_pref,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("Failed to find an appropriate adapter")?;

        let (width, height) = window.inner_size().into();

        let features = wgpu::Features::empty();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: features,
                    required_limits: Default::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await?;

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let selected_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|d| **d == selected_format)
            .context("failed to select proper surface texture format!")?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 0,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        unsafe { dx12_surface.desktop.Commit()? };

        let egui_renderer = EguiRenderer::new(&device, surface_config.format, None, 1, window);

        let scale_factor = 1.0;

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            egui_renderer,
            scale_factor,
            dx12_surface,
        })
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn handle_input(
        &mut self,
        window: &Window,
        event: &WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_renderer.state.on_window_event(window, event)
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
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: window.scale_factor() as f32 * self.scale_factor,
        };

        let surface_texture = self.surface.get_current_texture()?;

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            self.egui_renderer.begin_frame(window);

            let ctx = self.egui_renderer.egui_ctx();

            egui_view.update(ctx);

            self.egui_renderer.end_frame_and_draw(
                &self.device,
                &self.queue,
                &mut encoder,
                window,
                &surface_view,
                screen_descriptor,
            );
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }
}

pub struct EguiRenderer {
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    frame_started: bool,
}

impl EguiRenderer {
    pub fn new(
        device: &wgpu::Device,
        output_color_format: wgpu::TextureFormat,
        output_depth_format: Option<wgpu::TextureFormat>,
        msaa_samples: u32,
        window: &Window,
    ) -> Self {
        let egui_context = egui::Context::default();

        let egui_state = egui_winit::State::new(
            egui_context,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(2 * 1024), // default dimension is 2048
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
            true,
        );

        Self {
            state: egui_state,
            renderer: egui_renderer,
            frame_started: false,
        }
    }

    pub fn egui_ctx(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    pub fn set_pixels_per_point(&mut self, v: f32) {
        self.egui_ctx().set_pixels_per_point(v);
    }

    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
    ) {
        if !self.frame_started {
            panic!("begin_frame must be called before end_frame_and_draw can be called!");
        }

        self.set_pixels_per_point(screen_descriptor.pixels_per_point);

        let full_output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .egui_ctx()
            .tessellate(full_output.shapes, self.egui_ctx().pixels_per_point());

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
        });

        self.renderer
            .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);

        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }

        self.frame_started = false;
    }
}

struct Dx12Surface {
    #[allow(unused)]
    device: ID3D11Device,
    desktop: IDCompositionDesktopDevice,
    #[allow(unused)]
    target: IDCompositionTarget,
    wgpu_visual: IDCompositionVisual2,
}

impl Dx12Surface {
    fn new(window: &Window) -> anyhow::Result<Self> {
        let device = unsafe {
            let mut device = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
            // SAFETY: D3D11CreateDevice succeded, device is Some
            .map(|()| device.unwrap())?
        };

        let dxgi3: IDXGIDevice3 = device.cast()?;
        let device_2d = unsafe { D2D1CreateDevice(&dxgi3, None) }?;

        let desktop: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice2(&device_2d)? };

        let hwnd = window.window_handle()?;
        let RawWindowHandle::Win32(hwnd) = hwnd.as_raw() else {
            unreachable!()
        };
        let hwnd = HWND(hwnd.hwnd.get() as _);

        let target = unsafe { desktop.CreateTargetForHwnd(hwnd, true) }?;

        let root_visual = unsafe { desktop.CreateVisual() }?;
        unsafe { target.SetRoot(&root_visual) }?;

        let wgpu_visual = unsafe { desktop.CreateVisual() }?;
        unsafe { root_visual.AddVisual(&wgpu_visual, false, None) }?;

        Ok(Self {
            desktop,
            device,
            target,
            wgpu_visual,
        })
    }
}
