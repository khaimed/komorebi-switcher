use std::num::NonZero;
use std::sync::Arc;

use muda::ContextMenu;
use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::UI::ViewManagement::{UIColorType, UISettings};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowAttributes};

use crate::app::{App, AppMessage};
use crate::egui_glue::{EguiView, EguiWindow};
use crate::komorebi::listen_for_workspaces;

impl App {
    pub fn create_main_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        log::info!("Creating main window");

        let mut attrs = WindowAttributes::default();

        // get host width/height
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.host, &mut rect) }?;
        let width = rect.right - rect.left;
        let heigth = rect.bottom - rect.top;

        attrs = attrs.with_inner_size(PhysicalSize::new(width, heigth));

        let parent = unsafe { NonZero::new_unchecked(self.host.0 as _) };
        let parent = Win32WindowHandle::new(parent);
        let parent = RawWindowHandle::Win32(parent);

        attrs = unsafe { attrs.with_parent_window(Some(parent)) };
        attrs = attrs
            .with_decorations(false)
            .with_transparent(true)
            .with_active(false)
            .with_class_name("komorebi-switcher::window")
            .with_undecorated_shadow(false)
            .with_no_redirection_bitmap(true)
            .with_clip_children(false);

        let window = event_loop.create_window(attrs)?;
        let window = Arc::new(window);

        let state = MainWindowView::new(window.clone(), self.host)?;

        let proxy = self.proxy.clone();

        std::thread::spawn(move || listen_for_workspaces(proxy));

        let window = EguiWindow::new(window, &self.wgpu_instance, state)?;

        self.windows.insert(window.id(), window);

        Ok(())
    }
}

pub struct MainWindowView {
    window: Arc<Window>,
    host: HWND,
    curr_width: i32,
    workspaces: Vec<crate::komorebi::Workspace>,
    context_menu: muda::Menu,
    is_dragging: bool,
    accent_light2_color: Option<egui::Color32>,
    accent_color: Option<egui::Color32>,
    forgreound_color: Option<egui::Color32>,
}

impl MainWindowView {
    fn new(window: Arc<Window>, host: HWND) -> anyhow::Result<Self> {
        let workspaces = crate::komorebi::read_workspaces().unwrap_or_default();

        let context_menu = muda::Menu::with_items(&[
            &muda::MenuItem::with_id("move", "Move", true, None),
            &muda::MenuItem::with_id("quit", "Quit", true, None),
        ])?;

        let mut view = Self {
            window,
            host,
            curr_width: 0,
            workspaces,
            context_menu,
            is_dragging: false,
            accent_color: None,
            accent_light2_color: None,
            forgreound_color: None,
        };

        if let Err(e) = view.update_system_colors() {
            log::error!("Failed to get system colors: {e}");
        }

        Ok(view)
    }

    fn update_system_colors(&mut self) -> anyhow::Result<()> {
        let settings = UISettings::new()?;

        let color = settings.GetColorValue(UIColorType::Accent)?;
        let color = egui::Color32::from_rgb(color.R, color.G, color.B);
        self.accent_color.replace(color);

        let color = settings.GetColorValue(UIColorType::AccentLight2)?;
        let color = egui::Color32::from_rgb(color.R, color.G, color.B);
        self.accent_light2_color.replace(color);

        let color = settings.GetColorValue(UIColorType::Foreground)?;
        let color = egui::Color32::from_rgb(color.R, color.G, color.B);
        self.forgreound_color.replace(color);

        Ok(())
    }

    fn host_client_rect(&self) -> anyhow::Result<RECT> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.host, &mut rect) }?;
        Ok(rect)
    }

    fn host_window_rect(&self) -> anyhow::Result<RECT> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(self.host, &mut rect) }?;
        Ok(rect)
    }

    fn resize_host_to_rect(&mut self, rect: egui::Rect) -> anyhow::Result<()> {
        let width = rect.width() as f64 + 2.0 /* default margin 1 on each side */;
        let width = self.window.scale_factor() * width;
        let width = width as i32;

        if width != self.curr_width {
            self.curr_width = width;

            if let Ok(rect) = self.host_client_rect() {
                log::debug!("Resizing host to match content rect");

                unsafe {
                    SetWindowPos(
                        self.host,
                        None,
                        0,
                        0,
                        width,
                        rect.bottom - rect.top,
                        SWP_NOMOVE,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn start_host_dragging(&mut self) -> anyhow::Result<()> {
        self.is_dragging = true;

        let rect = self.host_client_rect()?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        let x = rect.left + width / 2;
        let y = rect.top + height / 2;

        let points = &mut [POINT { x, y }];
        unsafe { MapWindowPoints(Some(self.host), None, points) };

        unsafe { SetCursorPos(points[0].x, points[0].y)? };

        Ok(())
    }

    fn drag_host_window(&mut self) -> anyhow::Result<()> {
        let mut pos = POINT::default();
        unsafe { GetCursorPos(&mut pos) }?;

        let points = POINTS {
            x: pos.x as i16,
            y: pos.y as i16,
        };

        unsafe {
            ReleaseCapture()?;

            PostMessageW(
                Some(self.host),
                WM_NCLBUTTONDOWN,
                WPARAM(HTCAPTION as _),
                LPARAM(&points as *const _ as _),
            )?;

            self.is_dragging = false;
        }

        Ok(())
    }

    fn close_host(&self) -> anyhow::Result<()> {
        log::info!("Closing host window");

        unsafe {
            PostMessageW(
                Some(self.host),
                WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            )
            .map_err(Into::into)
        }
    }

    fn show_context_menu(&self) {
        log::debug!("Showing context menu");

        let hwnd = self.host.0 as _;
        unsafe { self.context_menu.show_context_menu_for_hwnd(hwnd, None) };
    }

    fn is_dark_mode(&self, ui: &egui::Ui) -> bool {
        self.forgreound_color
            .map(|c| c == egui::Color32::WHITE)
            .unwrap_or_else(|| ui.visuals().dark_mode)
    }

    fn is_taskbar_on_top(&self) -> bool {
        self.host_window_rect()
            .map(|r| {
                let current_monitor = self.window.current_monitor();
                let y = current_monitor.map(|m| m.position().y).unwrap_or(0);
                r.top == y
            })
            .unwrap_or(false)
    }

    fn workspace_button(
        &self,
        workspace: &crate::komorebi::Workspace,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        const RADIUS: f32 = 4.0;
        const MIN_SIZE: egui::Vec2 = egui::vec2(28.0, 28.0);
        const LINE_FOCUSED_WIDTH: f32 = 15.0;
        const LINE_NOTEMPTY_WIDTH: f32 = 6.0;
        const LINE_HEIGHT: f32 = 3.5;
        const TEXT_PADDING: egui::Vec2 = egui::vec2(16.0, 8.0);

        let dark_mode = self.is_dark_mode(ui);

        let font_id = egui::FontId::default();
        let text_color = self.forgreound_color.unwrap_or(if dark_mode {
            egui::Color32::WHITE
        } else {
            egui::Color32::BLACK
        });

        let text = workspace.name.clone();
        let text_galley = ui
            .painter()
            .layout_no_wrap(text, font_id.clone(), text_color);

        let size = MIN_SIZE.max(text_galley.rect.size() + TEXT_PADDING);

        let (rect, response) = ui.allocate_at_least(size, egui::Sense::CLICK | egui::Sense::HOVER);

        let painter = ui.painter();

        if response.hovered() || workspace.focused {
            let color = if dark_mode {
                egui::Color32::from_rgba_premultiplied(15, 15, 15, 3)
            } else {
                egui::Color32::from_rgba_premultiplied(30, 30, 30, 3)
            };

            painter.rect_filled(rect, RADIUS, color);
        }

        let line_width = if workspace.focused {
            LINE_FOCUSED_WIDTH
        } else {
            LINE_NOTEMPTY_WIDTH
        };

        let x = rect.min.x + rect.width() / 2.0 - line_width / 2.0;

        let mut line_rect = rect.with_min_x(x).with_max_x(x + line_width);

        if self.is_taskbar_on_top() {
            line_rect = line_rect.with_max_y(rect.min.y + LINE_HEIGHT);
        } else {
            line_rect = line_rect.with_min_y(rect.max.y - LINE_HEIGHT);
        };

        if workspace.focused {
            let color = if dark_mode {
                self.accent_light2_color
            } else {
                self.accent_color
            };

            let color = color.unwrap_or(egui::Color32::CYAN);

            painter.rect_filled(line_rect, RADIUS, color);
        } else if !workspace.is_empty {
            let color = if dark_mode {
                egui::Color32::from_rgba_unmultiplied(180, 173, 170, 125)
            } else {
                egui::Color32::from_rgba_unmultiplied(31, 31, 31, 150)
            };

            painter.rect_filled(line_rect, RADIUS, color);
        }

        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &workspace.name,
            font_id,
            text_color,
        );

        response
    }

    fn workspaces_row(&mut self, ui: &mut egui::Ui) -> egui::InnerResponse<()> {
        if self.is_dragging {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);

            if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                if let Err(e) = self.drag_host_window() {
                    log::error!("Failed to start host darggign: {e}");
                }
            }
        }

        if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) {
            self.show_context_menu();
        }

        ui.horizontal_centered(|ui| {
            let spacing = ui.style().spacing.item_spacing;
            ui.style_mut().spacing.item_spacing = egui::vec2(4., 4.);
            for workspace in self.workspaces.iter() {
                if self.workspace_button(workspace, ui).clicked() && !self.is_dragging {
                    crate::komorebi::change_workspace(workspace.idx);
                }
            }
            ui.style_mut().spacing.item_spacing = spacing;
        })
    }

    fn transparent_panel(ctx: &egui::Context) -> egui::CentralPanel {
        let visuals = egui::Visuals {
            panel_fill: egui::Color32::TRANSPARENT,
            ..Default::default()
        };
        ctx.set_visuals(visuals);

        let margin = egui::Margin::symmetric(1, 0);
        let frame = egui::Frame::central_panel(&ctx.style()).inner_margin(margin);

        egui::CentralPanel::default().frame(frame)
    }
}

impl EguiView for MainWindowView {
    fn handle_app_message(
        &mut self,
        _event_loop: &ActiveEventLoop,
        message: &AppMessage,
    ) -> anyhow::Result<()> {
        match message {
            AppMessage::UpdateWorkspaces(workspaces) => self.workspaces = workspaces.clone(),
            AppMessage::MenuEvent(e) if e.id() == "move" => self.start_host_dragging()?,
            AppMessage::MenuEvent(e) if e.id() == "quit" => self.close_host()?,
            AppMessage::SystemSettingsChanged => self.update_system_colors()?,
            _ => {}
        }

        Ok(())
    }

    fn update(&mut self, ctx: &egui::Context) {
        Self::transparent_panel(ctx).show(ctx, |ui| {
            let response = self.workspaces_row(ui);

            if let Err(e) = self.resize_host_to_rect(response.response.rect) {
                log::error!("Failed to resize host to rect: {e}");
            }
        });
    }
}
