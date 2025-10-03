use windows::Win32::Foundation::RECT;

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    pub name: String,
    pub index: usize,
    pub focused: bool,
    pub is_empty: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Monitor {
    pub name: String,
    pub index: usize,
    pub id: String,
    pub workspaces: Vec<Workspace>,
    pub rect: RECT,
}

#[derive(Debug, Clone, Default)]
pub struct State {
    pub monitors: Vec<Monitor>,
}