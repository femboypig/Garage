use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    OpenFile(PathBuf),
    SaveFile,
    Undo,
    Redo,
    ToggleSidebar,
    Exit,
    ShowSettings,
    ShowAbout,
    ShowCommandPalette,
    CloseModal,
    ChangeBufferFontSize(f32),
    ChangeUiFontSize(f32),
    ChangeBackend(wgpu::Backend),
    ChangeSidebarWidth(f32),
    ChangeTheme(String),
    ChangeGitBlame(bool),
    ChangeGitBranch(bool),
    SelectTab(usize),
    CloseTab(usize),
    ForceCloseTab(usize),
    SaveAndCloseTab(usize),
    MinimizeWindow,
    MaximizeWindow,
    NewTerminal,
    CloseTerminal(usize),
    SelectTerminal(usize),
    ToggleDock,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuType {
    Garage,
    File,
    Edit,
    Selection,
    View,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModalType {
    Settings,
    About,
    CommandPalette,
    UnsavedChanges,
}

pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}
