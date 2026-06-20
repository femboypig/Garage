use std::collections::HashSet;
use std::path::PathBuf;

use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;

use super::types::{FileNode, GitDiffHunk, MenuType, ModalType, SearchRenderItem};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommandPaletteMode {
    Commands,
    Languages,
    Encodings,
    LineEndings,
}

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

    pub git_file_blames:
        std::collections::HashMap<String, std::collections::HashMap<usize, String>>,
    pub git_blame_file_rx:
        Option<std::sync::mpsc::Receiver<(String, std::collections::HashMap<usize, String>)>>,
    pub git_blame_file_tx:
        std::sync::mpsc::Sender<(String, std::collections::HashMap<usize, String>)>,

    pub lsp_diagnostics: std::collections::HashMap<String, (usize, usize)>,
    pub lsp_diagnostics_details:
        std::collections::HashMap<String, Vec<crate::editor::DiagnosticDetail>>,

    pub git_statuses: std::collections::HashMap<PathBuf, String>,
    pub git_status_rx:
        Option<std::sync::mpsc::Receiver<std::collections::HashMap<PathBuf, String>>>,
    pub git_status_tx: std::sync::mpsc::Sender<std::collections::HashMap<PathBuf, String>>,

    pub git_diffs: std::collections::HashMap<String, Vec<GitDiffHunk>>,
    pub git_diff_rx: Option<std::sync::mpsc::Receiver<(String, Vec<GitDiffHunk>)>>,
    pub git_diff_tx: std::sync::mpsc::Sender<(String, Vec<GitDiffHunk>)>,

    pub languages: std::collections::HashMap<String, String>,

    pub command_palette_mode: CommandPaletteMode,
    pub forced_languages: std::collections::HashMap<String, String>,
    pub forced_encodings: std::collections::HashMap<String, String>,
    pub forced_line_endings: std::collections::HashMap<String, String>,
    pub external_change_warnings: std::collections::HashSet<String>,

    pub command_palette_query: String,
    pub global_search_query: String,
    pub global_search_results: Vec<(std::path::PathBuf, usize, String)>,
    pub global_search_selected: usize,
    pub last_global_search_selected: Option<usize>,
    pub global_search_scroll: usize,
    pub global_search_rx: Option<
        std::sync::mpsc::Receiver<(
            Vec<(std::path::PathBuf, usize, String)>,
            std::collections::HashMap<std::path::PathBuf, Vec<String>>,
        )>,
    >,
    pub global_search_tx: std::sync::mpsc::Sender<(
        Vec<(std::path::PathBuf, usize, String)>,
        std::collections::HashMap<std::path::PathBuf, Vec<String>>,
    )>,
    pub is_searching_globally: bool,
    pub command_palette_selected: usize,
    pub command_palette_scroll: usize,
    pub sidebar_scroll: usize,

    // Terminal Dock State
    pub show_dock: bool,
    pub dock_height: f32,
    pub active_dock_tab: usize,
    pub hovered_dock_tab_close: Option<usize>,
    pub event_loop_proxy: winit::event_loop::EventLoopProxy<()>,

    pub tab_scroll_x: f32,
    pub tab_scroll_is_dragging: bool,

    pub tree_rx: Option<std::sync::mpsc::Receiver<Vec<FileNode>>>,
    pub tree_tx: std::sync::mpsc::Sender<Vec<FileNode>>,
    pub diagnostics_file_rx: Option<std::sync::mpsc::Receiver<(String, Vec<String>)>>,
    pub diagnostics_file_tx: std::sync::mpsc::Sender<(String, Vec<String>)>,
    pub hovered_diagnostic: Option<crate::editor::DiagnosticDetail>,
    pub hover_start: Option<std::time::Instant>,
    pub hover_pos: Option<(usize, usize)>,
    pub hovered_copy_button: bool,
    pub mouse_in_popup: bool,
    pub diagnostics_click_targets: Vec<(f32, f32, f32, f32, String, usize, usize, String)>,
    pub diagnostics_file_cache: std::collections::HashMap<String, Vec<String>>,
    pub collapsed_diagnostics: std::collections::HashSet<String>,
    pub diagnostics_changed: bool,
    pub synced_revisions: std::collections::HashMap<String, usize>,
    pub keymap: crate::editor::keymap::Keymap,
    pub sidebar_input_type: String,
    pub sidebar_input_target: std::path::PathBuf,
    pub sidebar_input_value: String,
    pub sidebar_context_menu: Option<(f32, f32, std::path::PathBuf, bool)>,
    pub show_search_panel: bool,
    pub show_replace: bool,
    pub global_show_replace: bool,
    pub search_query: String,
    pub replace_query: String,
    pub search_focus_replace: bool,
    pub search_matches: Vec<(usize, usize)>,
    pub active_search_match_idx: usize,
    pub search_case_sensitive: bool,
    pub search_whole_word: bool,
    pub search_regex: bool,
    pub global_search_case_sensitive: bool,
    pub global_search_whole_word: bool,
    pub global_search_regex: bool,
    pub global_search_focus_replace: bool,
    pub global_replace_query: String,
    pub project_search_file_cache: std::collections::HashMap<std::path::PathBuf, Vec<String>>,
    pub project_search_render_items: Option<Vec<SearchRenderItem>>,
    pub collapsed_search_files: std::collections::HashSet<std::path::PathBuf>,
    pub last_searched_global_query: String,
    pub global_search_expanded_margins:
        std::collections::HashMap<(std::path::PathBuf, usize), (usize, usize)>,
    pub search_focused: bool,
    pub global_search_focused: bool,
    pub global_search_col: usize,
    pub last_frame_time: Option<std::time::Instant>,
    pub current_fps: f32,
    pub experimental: bool,
}

impl UiState {
    pub fn new(
        atlas: &mut FontAtlas,
        _queue: &wgpu::Queue,
        config: crate::editor::config::AppConfig,
        event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
        experimental: bool,
    ) -> Self {
        let ui_font_size = config.ui_font_size;
        let buffer_font_size = config.buffer_font_size;

        // UI Metrics
        let ui_metrics = atlas.font.metrics('m', ui_font_size);
        let ui_char_width = ui_metrics.advance_width.round().max(8.0);
        let ui_font_metrics =
            atlas
                .font
                .horizontal_line_metrics(ui_font_size)
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
        let buf_font_metrics = atlas
            .font
            .horizontal_line_metrics(buffer_font_size)
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
        let (status_tx, status_rx) = std::sync::mpsc::channel();
        let (diff_tx, diff_rx) = std::sync::mpsc::channel();
        let (blame_file_tx, blame_file_rx) = std::sync::mpsc::channel();
        let (tree_tx, tree_rx) = std::sync::mpsc::channel();
        let (diagnostics_file_tx, diagnostics_file_rx) = std::sync::mpsc::channel();
        let (global_search_tx, global_search_rx) = std::sync::mpsc::channel();

        let mut languages = std::collections::HashMap::new();
        if let Ok(content) = std::fs::read_to_string("assets/languages.json")
            && let Ok(map) =
                serde_json::from_str::<std::collections::HashMap<String, String>>(&content)
        {
            languages = map;
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
            tab_scroll_x: 0.0,
            tab_scroll_is_dragging: false,
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
            git_file_blames: std::collections::HashMap::new(),
            git_blame_file_rx: Some(blame_file_rx),
            git_blame_file_tx: blame_file_tx,
            git_statuses: std::collections::HashMap::new(),
            git_status_rx: Some(status_rx),
            git_status_tx: status_tx,
            git_diffs: std::collections::HashMap::new(),
            git_diff_rx: Some(diff_rx),
            git_diff_tx: diff_tx,
            tree_rx: Some(tree_rx),
            tree_tx,
            diagnostics_file_rx: Some(diagnostics_file_rx),
            diagnostics_file_tx,
            languages,
            lsp_diagnostics: std::collections::HashMap::new(),
            lsp_diagnostics_details: std::collections::HashMap::new(),
            command_palette_mode: CommandPaletteMode::Commands,
            forced_languages: std::collections::HashMap::new(),
            forced_encodings: std::collections::HashMap::new(),
            forced_line_endings: std::collections::HashMap::new(),
            external_change_warnings: std::collections::HashSet::new(),
            command_palette_query: String::new(),
            global_search_query: String::new(),
            global_search_results: Vec::new(),
            global_search_selected: 0,
            last_global_search_selected: None,
            global_search_scroll: 0,
            global_search_rx: Some(global_search_rx),
            global_search_tx,
            is_searching_globally: false,
            command_palette_selected: 0,
            command_palette_scroll: 0,
            sidebar_scroll: 0,
            show_dock: false,
            dock_height: 250.0,
            active_dock_tab: 0,
            hovered_dock_tab_close: None,
            event_loop_proxy,
            hovered_diagnostic: None,
            hover_start: None,
            hover_pos: None,
            hovered_copy_button: false,
            mouse_in_popup: false,
            diagnostics_click_targets: Vec::new(),
            diagnostics_file_cache: std::collections::HashMap::new(),
            collapsed_diagnostics: std::collections::HashSet::new(),
            project_search_file_cache: std::collections::HashMap::new(),
            project_search_render_items: None,
            collapsed_search_files: std::collections::HashSet::new(),
            diagnostics_changed: true,
            synced_revisions: std::collections::HashMap::new(),
            keymap: crate::editor::keymap::Keymap::load(),
            sidebar_input_type: String::new(),
            sidebar_input_target: std::path::PathBuf::new(),
            sidebar_input_value: String::new(),
            sidebar_context_menu: None,
            show_search_panel: false,
            show_replace: false,
            global_show_replace: false,
            search_query: String::new(),
            replace_query: String::new(),
            search_focus_replace: false,
            search_matches: Vec::new(),
            active_search_match_idx: 0,
            search_case_sensitive: false,
            search_whole_word: false,
            search_regex: false,
            global_search_case_sensitive: false,
            global_search_whole_word: false,
            global_search_regex: false,
            global_search_focus_replace: false,
            global_replace_query: String::new(),
            last_searched_global_query: String::new(),
            global_search_expanded_margins: std::collections::HashMap::new(),
            search_focused: false,
            global_search_focused: false,
            global_search_col: 0,
            last_frame_time: None,
            current_fps: 0.0,
            experimental,
        };

        state.rebuild_tree();
        state
    }

    pub fn update_buffer_font_size(&mut self, font: &fontdue::Font, new_size: f32) {
        self.buffer_font_size = new_size;
        let buf_metrics = font.metrics('m', new_size);
        self.buffer_char_width = buf_metrics.advance_width.round().max(8.0);
        let buf_font_metrics =
            font.horizontal_line_metrics(new_size)
                .unwrap_or(fontdue::LineMetrics {
                    ascent: new_size * 0.8,
                    descent: -new_size * 0.2,
                    line_gap: new_size * 0.2,
                    new_line_size: new_size * 1.2,
                });
        self.buffer_line_height = buf_font_metrics.new_line_size.round();
        self.buffer_font_ascent = buf_font_metrics.ascent.round();
    }
    pub fn perform_search(&mut self, state: &crate::app::state::AppState) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            self.active_search_match_idx = 0;
            return;
        }

        if state.active_tab_idx >= state.tabs.len() {
            self.active_search_match_idx = 0;
            return;
        }

        let buffer = &state.tabs[state.active_tab_idx].buffer;
        let query = &self.search_query;

        let pattern = if self.search_regex {
            query.clone()
        } else {
            regex::escape(query)
        };

        let pattern = if self.search_whole_word {
            format!(r"\b{}\b", pattern)
        } else {
            pattern
        };

        let mut builder = regex::RegexBuilder::new(&pattern);
        builder.case_insensitive(!self.search_case_sensitive);

        if let Ok(re) = builder.build() {
            for (line_idx, line) in buffer.lines().iter().enumerate() {
                for m in re.find_iter(line) {
                    let char_idx = line[..m.start()].chars().count();
                    self.search_matches.push((line_idx, char_idx));
                }
            }
        }

        if !self.search_matches.is_empty() {
            self.active_search_match_idx = self
                .active_search_match_idx
                .min(self.search_matches.len() - 1);
        } else {
            self.active_search_match_idx = 0;
        }
    }

    pub fn update_ui_font_size(&mut self, font: &fontdue::Font, new_size: f32) {
        self.ui_font_size = new_size;
        let ui_metrics = font.metrics('m', new_size);
        self.ui_char_width = ui_metrics.advance_width.round().max(8.0);
        let ui_font_metrics =
            font.horizontal_line_metrics(new_size)
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
        if let Some(overridden) = self.config.override_tiling_wm {
            return overridden;
        }
        std::env::var("I3SOCK").is_ok()
            || std::env::var("SWAYSOCK").is_ok()
            || std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .map(|s| {
                    let s_lower = s.to_lowercase();
                    s_lower.contains("i3")
                        || s_lower.contains("sway")
                        || s_lower.contains("hyprland")
                        || s_lower.contains("river")
                        || s_lower.contains("qtile")
                })
                .unwrap_or(false)
    }
    pub fn get_tab_name(&self, path_opt: Option<&str>) -> String {
        if path_opt == Some("diagnostics://project") {
            let mut err_count = 0;
            let mut warn_count = 0;
            for (e, w) in self.lsp_diagnostics.values() {
                err_count += *e;
                warn_count += *w;
            }
            if err_count > 0 {
                format!("  ⊗ {}", err_count)
            } else if warn_count > 0 {
                format!("  ⚠ {}", warn_count)
            } else {
                "  ⊗ 0".to_string()
            }
        } else if path_opt == Some("search://project") {
            "Project Search".to_string()
        } else {
            path_opt
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string())
        }
    }

    pub fn scroll_to_tab(
        &self,
        active_idx: usize,
        tab_paths: &[Option<String>],
        visible_width: f32,
        current_scroll_x: f32,
    ) -> f32 {
        if tab_paths.is_empty() || active_idx >= tab_paths.len() {
            return current_scroll_x;
        }

        let tab_close_icon_sz = (self.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let dot_reserved = 18.0f32;

        let mut total_tabs_width = 0.0f32;
        let mut target_tab_x = 0.0f32;
        let mut target_tab_w = 0.0f32;

        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = self.get_tab_name(path_opt.as_deref());
            let name_w = file_name.chars().count() as f32 * self.ui_char_width;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);

            if idx == active_idx {
                target_tab_x = total_tabs_width;
                target_tab_w = tab_w;
            }
            total_tabs_width += tab_w;
        }

        let max_scroll_x = (total_tabs_width - visible_width).max(0.0);
        let plus_margin = 40.0f32;

        let mut new_scroll_x = current_scroll_x;
        if target_tab_x < current_scroll_x {
            new_scroll_x = target_tab_x;
        } else if target_tab_x + target_tab_w + plus_margin > current_scroll_x + visible_width {
            new_scroll_x = target_tab_x + target_tab_w + plus_margin - visible_width;
        }

        new_scroll_x.clamp(0.0, max_scroll_x)
    }

    pub fn scroll_to_cursor(
        &mut self,
        cursor: &Cursor,
        buffer_len: usize,
        width: f32,
        height: f32,
    ) {
        let editor_height = height
            - self.titlebar_height
            - self.status_height
            - self.tabbar_height
            - self.breadcrumb_height
            - 14.0;
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

    pub fn invalidate_search_render_items(&mut self) {
        self.project_search_render_items = None;
    }
}
