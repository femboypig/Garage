use std::collections::HashSet;
use std::path::PathBuf;

use crate::renderer::atlas::FontAtlas;
use crate::editor::cursor::Cursor;

pub mod components;
pub mod types;
pub mod tree;
pub mod click;
pub mod frame;

pub use types::{UiAction, MenuType, ModalType, FileNode};
pub use crate::renderer::wgpu::Vertex;

pub struct UiState {
    pub ui_font_size: f32,
    pub buffer_font_size: f32,

    pub ui_char_width: f32,
    pub ui_line_height: f32,
    pub ui_font_ascent: f32,

    pub buffer_char_width: f32,
    pub buffer_line_height: f32,
    pub buffer_font_ascent: f32,

    pub scroll_y: usize,
    pub scroll_x: usize,
    
    // Layout Sizes
    pub titlebar_height: f32,
    pub status_height: f32,
    pub sidebar_width: f32,
    pub target_sidebar_width: f32,
    pub tabbar_height: f32,
    pub breadcrumb_height: f32,
    
    // Project Tree State
    pub expanded_dirs: HashSet<PathBuf>,
    pub visible_nodes: Vec<FileNode>,
    pub selected_file: Option<PathBuf>,
    
    // Menu & Modal State
    pub active_menu: Option<MenuType>,
    pub active_modal: Option<ModalType>,
    pub tab_to_close: Option<usize>,
    pub theme_dropdown_open: bool,

    pub config: crate::editor::config::AppConfig,
    pub active_device_name: String,

    pub last_blame_file: Option<String>,
    pub last_blame_line: Option<usize>,
    pub last_blame_result: Option<String>,
    pub git_branch: Option<String>,
    pub last_branch_check: Option<std::time::Instant>,

    pub git_branch_rx: Option<std::sync::mpsc::Receiver<String>>,
    pub git_branch_tx: std::sync::mpsc::Sender<String>,
    pub git_blame_rx: Option<std::sync::mpsc::Receiver<(String, usize, Option<String>)>>,
    pub git_blame_tx: std::sync::mpsc::Sender<(String, usize, Option<String>)>,

    pub languages: std::collections::HashMap<String, String>,

    pub command_palette_query: String,
    pub command_palette_selected: usize,
    pub command_palette_scroll: usize,
    pub sidebar_scroll: usize,

    // Terminal Dock State
    pub show_dock: bool,
    pub dock_height: f32,
    pub active_dock_tab: usize,
    pub hovered_dock_tab_close: Option<usize>,
}

impl UiState {
    pub fn new(atlas: &mut FontAtlas, _queue: &wgpu::Queue, config: crate::editor::config::AppConfig) -> Self {
        let ui_font_size = config.ui_font_size;
        let buffer_font_size = config.buffer_font_size;

        // UI Metrics
        let ui_metrics = atlas.font.metrics('m', ui_font_size);
        let ui_char_width = ui_metrics.advance_width.round().max(8.0);
        let ui_font_metrics = atlas.font.horizontal_line_metrics(ui_font_size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: ui_font_size * 0.8,
                descent: -ui_font_size * 0.2,
                line_gap: ui_font_size * 0.2,
                new_line_size: ui_font_size * 1.2,
            });
        let ui_line_height = ui_font_metrics.new_line_size.round();
        let ui_font_ascent = ui_font_metrics.ascent.round();

        // Buffer Metrics
        let buf_metrics = atlas.font.metrics('m', buffer_font_size);
        let buffer_char_width = buf_metrics.advance_width.round().max(8.0);
        let buf_font_metrics = atlas.font.horizontal_line_metrics(buffer_font_size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: buffer_font_size * 0.8,
                descent: -buffer_font_size * 0.2,
                line_gap: buffer_font_size * 0.2,
                new_line_size: buffer_font_size * 1.2,
            });
        let buffer_line_height = buf_font_metrics.new_line_size.round();
        let buffer_font_ascent = buf_font_metrics.ascent.round();

        let mut expanded_dirs = HashSet::new();
        // Expand root by default
        expanded_dirs.insert(PathBuf::from("."));

        let titlebar_height = (ui_line_height * 1.45).round().max(25.0);
        let status_height = (ui_line_height * 1.5).round().max(24.0);
        let tabbar_height = (ui_line_height * 1.6).round().max(30.0);
        let breadcrumb_height = (ui_line_height * 1.3).round().max(22.0);

        let (branch_tx, branch_rx) = std::sync::mpsc::channel();
        let (blame_tx, blame_rx) = std::sync::mpsc::channel();

        let mut languages = std::collections::HashMap::new();
        if let Ok(content) = std::fs::read_to_string("assets/languages.json") {
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&content) {
                languages = map;
            }
        }
        if languages.is_empty() {
            languages.insert("rs".to_string(), "Rust".to_string());
            languages.insert("json".to_string(), "JSON".to_string());
            languages.insert("toml".to_string(), "TOML".to_string());
            languages.insert("md".to_string(), "Markdown".to_string());
        }

        let mut state = Self {
            ui_font_size,
            buffer_font_size,
            ui_char_width,
            ui_line_height,
            ui_font_ascent,
            buffer_char_width,
            buffer_line_height,
            buffer_font_ascent,
            scroll_y: 0,
            scroll_x: 0,
            titlebar_height,
            status_height,
            sidebar_width: config.sidebar_width,
            target_sidebar_width: config.sidebar_width,
            tabbar_height,
            breadcrumb_height,
            expanded_dirs,
            visible_nodes: Vec::new(),
            selected_file: None,
            active_menu: None,
            active_modal: None,
            tab_to_close: None,
            theme_dropdown_open: false,
            config,
            active_device_name: String::new(),
            last_blame_file: None,
            last_blame_line: None,
            last_blame_result: None,
            git_branch: None,
            last_branch_check: None,
            git_branch_rx: Some(branch_rx),
            git_branch_tx: branch_tx,
            git_blame_rx: Some(blame_rx),
            git_blame_tx: blame_tx,
            languages,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            command_palette_scroll: 0,
            sidebar_scroll: 0,
            show_dock: false,
            dock_height: 250.0,
            active_dock_tab: 0,
            hovered_dock_tab_close: None,
        };

        state.rebuild_tree();
        state
    }

    pub fn update_buffer_font_size(&mut self, font: &fontdue::Font, new_size: f32) {
        self.buffer_font_size = new_size;
        let buf_metrics = font.metrics('m', new_size);
        self.buffer_char_width = buf_metrics.advance_width.round().max(8.0);
        let buf_font_metrics = font.horizontal_line_metrics(new_size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: new_size * 0.8,
                descent: -new_size * 0.2,
                line_gap: new_size * 0.2,
                new_line_size: new_size * 1.2,
            });
        self.buffer_line_height = buf_font_metrics.new_line_size.round();
        self.buffer_font_ascent = buf_font_metrics.ascent.round();
    }

    pub fn update_ui_font_size(&mut self, font: &fontdue::Font, new_size: f32) {
        self.ui_font_size = new_size;
        let ui_metrics = font.metrics('m', new_size);
        self.ui_char_width = ui_metrics.advance_width.round().max(8.0);
        let ui_font_metrics = font.horizontal_line_metrics(new_size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: new_size * 0.8,
                descent: -new_size * 0.2,
                line_gap: new_size * 0.2,
                new_line_size: new_size * 1.2,
            });
        self.ui_line_height = ui_font_metrics.new_line_size.round();
        self.ui_font_ascent = ui_font_metrics.ascent.round();
        self.titlebar_height = (self.ui_line_height * 1.45).round().max(25.0);
        self.status_height = (self.ui_line_height * 1.5).round().max(24.0);
        self.tabbar_height = (self.ui_line_height * 1.6).round().max(30.0);
        self.breadcrumb_height = (self.ui_line_height * 1.3).round().max(22.0);
    }

    pub fn is_tiling_wm(&self) -> bool {
        std::env::var("I3SOCK").is_ok()
            || std::env::var("SWAYSOCK").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP").map(|s| s.to_lowercase().contains("i3") || s.to_lowercase().contains("sway")).unwrap_or(false)
    }

    pub fn scroll_to_cursor(&mut self, cursor: &Cursor, buffer_len: usize, width: f32, height: f32) {
        let editor_height = height - self.titlebar_height - self.status_height - self.tabbar_height - self.breadcrumb_height - 14.0;
        let visible_lines = (editor_height / self.buffer_line_height).floor() as usize;
        if visible_lines > 0 {
            if cursor.line < self.scroll_y {
                self.scroll_y = cursor.line;
            } else if cursor.line >= self.scroll_y + visible_lines {
                self.scroll_y = cursor.line - visible_lines + 1;
            }
            let max_scroll = (buffer_len as isize - visible_lines as isize).max(0) as usize;
            self.scroll_y = self.scroll_y.min(max_scroll);
        }

        // Horizontal scrolling layout math
        let max_line_digits = buffer_len.to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * self.buffer_char_width;
        let text_area_x = self.sidebar_width + gutter_width;
        let scrollbar_width = self.scrollbar_width();
        let minimap_width = self.minimap_width();
        let sb_x = width - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
        let text_viewport_w = (minimap_x - text_area_x).max(10.0);

        let visible_cols = (text_viewport_w / self.buffer_char_width).floor() as usize;
        if visible_cols > 0 {
            if cursor.col < self.scroll_x {
                self.scroll_x = cursor.col;
            } else if cursor.col >= self.scroll_x + visible_cols {
                self.scroll_x = cursor.col - visible_cols + 1;
            }
        }
    }

    pub fn scrollbar_width(&self) -> f32 {
        14.0
    }

    pub fn minimap_width(&self) -> f32 {
        (self.buffer_font_size * 7.5).round().max(60.0)
    }
}
