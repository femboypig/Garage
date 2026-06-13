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
}

impl AppState {
    pub fn new(initial_tab: Tab) -> Self {
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
        }
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
}
