use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::terminal::TerminalInstance;
use winit::keyboard::ModifiersState;
use std::time::Instant;

pub struct Tab {
    pub path: Option<String>,
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub scroll_x: usize,
    pub scroll_y: usize,
}

pub struct AppState {
    pub tabs: Vec<Tab>,
    pub active_tab_idx: usize,
    pub modifiers: ModifiersState,
    pub is_dragging: bool,
    pub is_dragging_scroll: bool,
    pub is_dragging_horizontal_scroll: bool,
    pub is_dragging_minimap: bool,
    pub scroll_drag_offset_y: f32,
    pub scroll_drag_offset_x: f32,
    pub is_dragging_sidebar: bool,
    pub is_dragging_dock_border: bool,
    pub internal_clipboard: String,
    pub dock_terminals: Vec<TerminalInstance>,
    pub active_terminal_idx: usize,
    pub terminal_focus: bool,
    pub last_click_time: Option<Instant>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub lsp_client: Option<crate::editor::lsp::LspClient>,
}

impl AppState {
    pub fn new(initial_tab: Tab, lsp_client: Option<crate::editor::lsp::LspClient>) -> Self {
        Self {
            tabs: vec![initial_tab],
            active_tab_idx: 0,
            modifiers: ModifiersState::default(),
            is_dragging: false,
            is_dragging_scroll: false,
            is_dragging_horizontal_scroll: false,
            is_dragging_minimap: false,
            scroll_drag_offset_y: 0.0,
            scroll_drag_offset_x: 0.0,
            is_dragging_sidebar: false,
            is_dragging_dock_border: false,
            internal_clipboard: String::new(),
            dock_terminals: Vec::new(),
            active_terminal_idx: 0,
            terminal_focus: false,
            last_click_time: None,
            mouse_x: 0.0,
            mouse_y: 0.0,
            lsp_client,
        }
    }
}
