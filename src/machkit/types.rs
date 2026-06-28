use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn centered_in(width: f32, height: f32, w: f32, h: f32) -> Self {
        Self {
            x: ((width - w) / 2.0).round(),
            y: ((height - h) / 2.0).round(),
            w,
            h,
        }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

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

impl ModalType {
    pub fn width(self, ui_char_width: f32) -> f32 {
        match self {
            Self::Settings => (45.0 * ui_char_width).max(500.0).round(),
            Self::About => 520.0,
            Self::CommandPalette => (50.0 * ui_char_width).max(500.0).round(),
            Self::UnsavedChanges => 520.0,
            Self::SidebarInput => 400.0,
            Self::GlobalSearch => 650.0,
        }
    }

    pub fn height(
        self,
        ui_line_height: f32,
        filtered_commands_len: usize,
        global_results_len: usize,
    ) -> f32 {
        match self {
            Self::Settings => {
                let row_height = (ui_line_height * 2.2).round();
                (row_height * 8.2).max(430.0).round()
            }
            Self::About => 190.0,
            Self::CommandPalette => {
                let item_height = (ui_line_height * 1.6).round().max(26.0);
                let visible_items = filtered_commands_len.min(10);
                let header_h = 15.0 + ui_line_height + 15.0 + 1.0;
                (header_h + visible_items as f32 * item_height).round()
            }
            Self::UnsavedChanges => 200.0,
            Self::SidebarInput => 150.0,
            Self::GlobalSearch => {
                let item_height = (ui_line_height * 1.6).round().max(26.0);
                let count = global_results_len.min(10).max(1);
                let header_h = 15.0 + ui_line_height + 15.0 + 1.0;
                (header_h + count as f32 * item_height).round()
            }
        }
    }

    pub fn rect(
        self,
        viewport_width: f32,
        viewport_height: f32,
        ui_char_width: f32,
        ui_line_height: f32,
        filtered_commands_len: usize,
        global_results_len: usize,
    ) -> Rect {
        let w = self.width(ui_char_width);
        let h = self.height(ui_line_height, filtered_commands_len, global_results_len);
        Rect::centered_in(viewport_width, viewport_height, w, h)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarInputMode {
    NewFile,
    NewFolder,
    Rename,
    Delete,
}

impl SidebarInputMode {
    pub fn title(self) -> &'static str {
        match self {
            Self::NewFile => "New File",
            Self::NewFolder => "New Folder",
            Self::Rename => "Rename",
            Self::Delete => "Confirm Delete",
        }
    }
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
    pub is_fullscreen: bool,
    pub tab_scroll_x: f32,
}
