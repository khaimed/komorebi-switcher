use std::{
    num::{NonZero, NonZeroU32},
    rc::Rc,
};

use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::GetClientRect,
};
use winit::raw_window_handle;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    platform::windows::WindowAttributesExtWindows,
    window::{Window, WindowAttributes, WindowId},
};

pub enum AppMessage {}

pub struct App {
    main_window: Option<Rc<Window>>,
    #[allow(unused)]
    proxy: EventLoopProxy<AppMessage>,
    context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    host: HWND,
}

impl App {
    pub fn new(proxy: EventLoopProxy<AppMessage>, host: HWND) -> Self {
        Self {
            proxy,
            host,
            main_window: None,
            context: None,
            surface: None,
        }
    }

    fn create_main_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let mut attrs = WindowAttributes::default();

        let mut rect = RECT::default();
        unsafe { GetClientRect(self.host, &mut rect) }?;
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;

        attrs = attrs.with_inner_size(PhysicalSize::new(w, h));

        let parent = Win32WindowHandle::new(unsafe { NonZero::new_unchecked(self.host.0 as _) });
        let parent = RawWindowHandle::Win32(parent);

        attrs = unsafe { attrs.with_parent_window(Some(parent)) };
        attrs = attrs
            .with_decorations(false)
            .with_transparent(true)
            .with_class_name("komorebi-switcher::window")
            .with_undecorated_shadow(false)
            .with_clip_children(false);

        let window = event_loop.create_window(attrs)?;
        let window = Rc::new(window);

        let context =
            softbuffer::Context::new(window.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
        let surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        self.main_window.replace(window);

        self.context.replace(context);
        self.surface.replace(surface);

        Ok(())
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

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                let Some(surface) = &mut self.surface else {
                    return;
                };

                let size = (NonZeroU32::new(size.width), NonZeroU32::new(size.height));

                if let (Some(w), Some(h)) = size {
                    let _ = surface.resize(w, h);
                }
            }

            WindowEvent::RedrawRequested => {
                let Some(surface) = &mut self.surface else {
                    return;
                };

                let Some(main_window) = &self.main_window else {
                    return;
                };

                let size = main_window.inner_size();

                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    let Ok(mut buffer) = surface.buffer_mut() else {
                        return;
                    };

                    let width = width.get() as usize;
                    let height = height.get() as usize;

                    const DARK_GRAY: u32 = 0xff181818;
                    const LEMON: u32 = 0xffd1ffbd;

                    for y in 0..height {
                        for x in 0..width {
                            let color = if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                                LEMON
                            } else {
                                DARK_GRAY
                            };
                            buffer[y * width + x] = color;
                        }
                    }

                    let _ = buffer.present();
                }
            }

            WindowEvent::CloseRequested => {
                self.main_window.take();
                event_loop.exit();
            }

            WindowEvent::MouseInput { .. } => {
                dbg!(event);
            }

            _ => {}
        }
    }
}
