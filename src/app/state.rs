use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::terminal::TerminalInstance;
use winit::keyboard::ModifiersState;
use std::time::Instant;

#[derive(Clone)]
pub struct Tab {
    pub path: Option<String>,
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub scroll_x: usize,
    pub scroll_y: usize,
}

#[derive(Clone)]
pub struct Pane {
    pub tabs: Vec<Tab>,
    pub active_tab_idx: usize,
    pub tab_scroll_x: f32,
}

pub struct AppState {
    pub tabs: Vec<Tab>,
    pub active_tab_idx: usize,
    pub tab_scroll_x: f32,
    pub inactive_panes: Vec<Pane>,
    pub active_pane_idx: usize,
    pub is_split_horizontal: bool,
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
    pub dragged_tab_idx: Option<usize>,
    pub drag_start_pos: Option<(f32, f32)>,
    pub pending_open_files: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl AppState {
    pub fn new(initial_tab: Tab) -> Self {
        Self {
            tabs: vec![initial_tab],
            active_tab_idx: 0,
            tab_scroll_x: 0.0,
            inactive_panes: Vec::new(),
            active_pane_idx: 0,
            is_split_horizontal: false,
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
            dragged_tab_idx: None,
            drag_start_pos: None,
            pending_open_files: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn switch_pane(&mut self, target_pane_idx: usize) {
        if target_pane_idx == self.active_pane_idx {
            return;
        }
        if self.inactive_panes.is_empty() {
            return;
        }
        
        let current_active_pane = Pane {
            tabs: std::mem::take(&mut self.tabs),
            active_tab_idx: self.active_tab_idx,
            tab_scroll_x: self.tab_scroll_x,
        };
        
        let target_pane = std::mem::replace(&mut self.inactive_panes[0], current_active_pane);
        
        self.tabs = target_pane.tabs;
        self.active_tab_idx = target_pane.active_tab_idx;
        self.tab_scroll_x = target_pane.tab_scroll_x;
        self.active_pane_idx = target_pane_idx;
    }

    pub fn copy_to_clipboard(&mut self, text: String) {
        self.internal_clipboard = text.clone();
        
        // Linux system clipboard commands
        // Try wl-copy first (Wayland)
        if let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return;
                }
            }
        }
        // Try xclip (X11)
        if let Ok(mut child) = std::process::Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return;
                }
            }
        }
        // Try xsel (X11)
        if let Ok(mut child) = std::process::Command::new("xsel")
            .arg("--clipboard")
            .arg("--input")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    pub fn paste_from_clipboard(&self) -> String {
        // Try wl-paste (Wayland)
        if let Ok(output) = std::process::Command::new("wl-paste")
            .arg("--no-newline")
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
        // Try xclip (X11)
        if let Ok(output) = std::process::Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .arg("-o")
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
        // Try xsel (X11)
        if let Ok(output) = std::process::Command::new("xsel")
            .arg("--clipboard")
            .arg("--output")
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
        self.internal_clipboard.clone()
    }

    pub fn is_actually_dragging_tab(&self) -> bool {
        if self.dragged_tab_idx.is_some() {
            if let Some((sx, sy)) = self.drag_start_pos {
                let dx = self.mouse_x - sx;
                let dy = self.mouse_y - sy;
                return (dx * dx + dy * dy).sqrt() >= 8.0;
            }
        }
        false
    }

    pub fn get_pane_scroll_x(&self, pane_idx: usize) -> f32 {
        if pane_idx == self.active_pane_idx {
            self.tab_scroll_x
        } else if !self.inactive_panes.is_empty() {
            self.inactive_panes[0].tab_scroll_x
        } else {
            0.0
        }
    }

    pub fn set_pane_scroll_x(&mut self, pane_idx: usize, val: f32) {
        if pane_idx == self.active_pane_idx {
            self.tab_scroll_x = val;
        } else if !self.inactive_panes.is_empty() {
            self.inactive_panes[0].tab_scroll_x = val;
        }
    }
}

