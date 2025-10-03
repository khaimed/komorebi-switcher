use std::env;

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub hide_empty_workspaces: bool,
    pub hide_if_offline: bool,
    pub enable_scroll_switching: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            hide_empty_workspaces: true,
            hide_if_offline: false,
            enable_scroll_switching: true,
        }
    }
}

impl Options {
    pub fn from_env() -> Self {
        let mut opts = Self::default();

        if let Ok(val) = env::var("SWITCHER_HIDE_EMPTY_WORKSPACES") {
            opts.hide_empty_workspaces = matches!(val.as_str(), "1" | "true" | "yes");
        }
        if let Ok(val) = env::var("SWITCHER_HIDE_IF_OFFLINE") {
            opts.hide_if_offline = matches!(val.as_str(), "1" | "true" | "yes");
        }
        if let Ok(val) = env::var("SWITCHER_ENABLE_SCROLL_SWITCHING") {
            opts.enable_scroll_switching = matches!(val.as_str(), "1" | "true" | "yes");
        }

        opts
    }
}