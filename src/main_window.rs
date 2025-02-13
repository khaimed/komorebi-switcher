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
            accent_light2_color: None,
        };

        if let Err(e) = view.update_system_accent() {
            log::error!("Failed to get system accent: {e}");
        }

        Ok(view)
    }

    fn update_system_accent(&mut self) -> anyhow::Result<()> {
        let settings = UISettings::new()?;

        let color = settings.GetColorValue(UIColorType::AccentLight2)?;
        let color = egui::Color32::from_rgb(color.R, color.G, color.B);
        self.accent_light2_color.replace(color);

        Ok(())
    }

    fn host_rect(&self) -> anyhow::Result<RECT> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.host, &mut rect) }?;
        Ok(rect)
    }

    fn resize_host_to_rect(&mut self, rect: egui::Rect) -> anyhow::Result<()> {
        let width = rect.width() as f64 + 2.0 /* default margin 1 on each side */;
        let width = self.window.scale_factor() * width;
        let width = width as i32;

        if width != self.curr_width {
            self.curr_width = width;

            if let Ok(rect) = self.host_rect() {
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

        let rect = self.host_rect()?;
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

    fn workspace_button(
        &self,
        workspace: &crate::komorebi::Workspace,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        let original_style = egui::Style::clone(ui.style());
        let style = ui.style_mut();

        let fill_color = if workspace.focused {
            if let Some(accent_light2_color) = self.accent_light2_color {
                accent_light2_color
            } else {
                style.visuals.selection.bg_fill
            }
        } else if workspace.is_empty {
            egui::Color32::TRANSPARENT
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 5)
        };

        let hover_color = if let Some(accent_light2_color) = self.accent_light2_color {
            accent_light2_color
        } else {
            style.visuals.selection.bg_fill
        };

        let hover_stroke_color = if is_light_color(hover_color) {
            egui::Color32::BLACK
        } else {
            style.visuals.widgets.hovered.fg_stroke.color
        };

        let inactive_stroke_color = if workspace.focused {
            hover_stroke_color
        } else {
            style.visuals.widgets.inactive.fg_stroke.color
        };

        let active_border_color = egui::Color32::LIGHT_GRAY;

        let inactive_border_color = if workspace.focused {
            active_border_color
        } else {
            egui::Color32::GRAY
        };

        let stroke_width = 1.5;

        style.visuals.widgets.inactive = egui::style::WidgetVisuals {
            bg_fill: fill_color,
            weak_bg_fill: fill_color,
            bg_stroke: egui::Stroke {
                width: stroke_width,
                color: inactive_border_color,
            },
            fg_stroke: egui::Stroke {
                color: inactive_stroke_color,
                ..style.visuals.widgets.inactive.fg_stroke
            },
            ..style.visuals.widgets.hovered
        };

        style.visuals.widgets.hovered = egui::style::WidgetVisuals {
            bg_fill: hover_color,
            weak_bg_fill: hover_color,
            bg_stroke: egui::Stroke {
                width: stroke_width,
                color: active_border_color,
            },
            fg_stroke: egui::Stroke {
                color: hover_stroke_color,
                ..style.visuals.widgets.inactive.fg_stroke
            },
            ..style.visuals.widgets.hovered
        };

        let btn = egui::Button::new(&workspace.name)
            .min_size(egui::vec2(24., 24.))
            .corner_radius(2);

        let response = ui.add(btn);

        *ui.style_mut() = original_style;

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
            for workspace in self.workspaces.iter() {
                if self.workspace_button(workspace, ui).clicked() && !self.is_dragging {
                    crate::komorebi::change_workspace(workspace.idx);
                }
            }
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
            AppMessage::SystemSettingsChanged => self.update_system_accent()?,
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

fn is_light_color(color: egui::Color32) -> bool {
    let r = color.r() as f32;
    let g = color.g() as f32;
    let b = color.b() as f32;
    let hsp = 0.299 * (r * r) + 0.587 * (g * g) + 0.114 * (b * b);
    let hsp = hsp.sqrt();
    hsp > 127.5
}
