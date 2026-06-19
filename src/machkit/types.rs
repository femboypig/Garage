use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    OpenFile(PathBuf),
    OpenFileAt(PathBuf, usize),
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
    SplitVertical,
    SplitHorizontal,
    Find,
    FindInProject,
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
    SidebarInput,
    GlobalSearch,
}

pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

pub use crate::git::GitDiffHunk;

#[derive(Clone, Debug, PartialEq)]
pub enum SearchRenderItem {
    FileHeader {
        path: PathBuf,
    },
    CodeLine {
        path: PathBuf,
        line_idx: usize,
        content: String,
        is_match: bool,
        result_idx: Option<usize>,
        is_first_in_range: bool,
        is_last_in_range: bool,
        start_line_of_range: usize,
        end_line_of_range: usize,
    },
    Separator {
        path: PathBuf,
    },
}

pub struct FrameInput<'a> {
    pub buffer: &'a crate::editor::buffer::Buffer,
    pub cursor: &'a crate::editor::cursor::Cursor,
    pub secondary_cursors: &'a [crate::editor::cursor::Cursor],
    pub width: f32,
    pub height: f32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub current_backend: wgpu::Backend,
    pub tab_paths: &'a [Option<String>],
    pub tab_modified: &'a [bool],
    pub active_tab_idx: usize,
    pub dragged_tab_idx: Option<usize>,
    pub inactive_panes: &'a [crate::app::state::Pane],
    pub active_pane_idx: usize,
    pub is_split_horizontal: bool,
    pub terminals: &'a [crate::terminal::TerminalInstance],
    pub active_terminal_idx: usize,
    pub terminal_focus: bool,
    pub is_window_maximized: bool,
    pub tab_scroll_x: f32,
}
