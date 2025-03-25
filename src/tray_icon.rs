use tray_icon::menu::{Menu, MenuItem};
use tray_icon::TrayIconBuilder;
use winit::event_loop::ActiveEventLoop;

use crate::app::AppMessage;

pub struct TrayIcon {
    #[allow(unused)]
    icon: tray_icon::TrayIcon,
}

impl TrayIcon {
    /// The ID of the "Quit" menu item.
    const M_QUIT_ID: &str = "quit";

    pub fn new() -> anyhow::Result<Self> {
        let icon = tray_icon::Icon::from_resource(1, Some((32, 32)))?;
        let menu = Menu::with_items(&[&MenuItem::with_id(Self::M_QUIT_ID, "Quit", true, None)])?;

        TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(std::env!("CARGO_PKG_NAME"))
            .with_menu(Box::new(menu.clone()))
            .build()
            .map_err(Into::into)
            .map(|icon| Self { icon })
    }

    pub fn handle_app_message(&self, event_loop: &ActiveEventLoop, event: &AppMessage) {
        match event {
            AppMessage::MenuEvent(event) if event.id() == Self::M_QUIT_ID => event_loop.exit(),
            _ => {}
        }
    }
}
