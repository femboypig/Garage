use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::terminal::TerminalInstance;

pub mod components;


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

    /// Re-scan the directory to populate the project tree
    pub fn rebuild_tree(&mut self) {
        self.visible_nodes.clear();
        self.scan_dir_recursive(Path::new("."), 0);
    }

    pub fn scrollbar_width(&self) -> f32 {
        14.0
    }

    pub fn minimap_width(&self) -> f32 {
        (self.buffer_font_size * 7.5).round().max(60.0)
    }

    pub fn get_max_line_len(&mut self, buffer: &Buffer, active_file_path: Option<&str>, cursor_line: usize) -> usize {
        let mut max_len = 0;
        for (line_idx, line) in buffer.lines().iter().enumerate() {
            let mut len = line.chars().count();
            if self.config.show_git_blame && line_idx == cursor_line {
                if let Some(blame_str) = self.get_or_update_blame(active_file_path, line_idx) {
                    len += 4 + blame_str.chars().count();
                }
            }
            if len > max_len {
                max_len = len;
            }
        }
        max_len
    }

    fn scan_dir_recursive(&mut self, dir: &Path, depth: usize) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut entries_vec = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    
                    // Skip large/ignored folders to optimize directory scanning performance
                    if name == ".git" || name == "target" || name == ".gemini" {
                        continue;
                    }
                    
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    entries_vec.push((path, name, is_dir));
                }
            }

            // Sort: directories first, then files alphabetically
            entries_vec.sort_by(|a, b| {
                if a.2 != b.2 {
                    b.2.cmp(&a.2)
                } else {
                    a.1.cmp(&b.1)
                }
            });

            for (path, name, is_dir) in entries_vec {
                let is_expanded = self.expanded_dirs.contains(&path);
                self.visible_nodes.push(FileNode {
                    path: path.clone(),
                    name,
                    is_dir,
                    depth,
                });
                if is_dir && is_expanded {
                    self.scan_dir_recursive(&path, depth + 1);
                }
            }
        }
    }

    /// Handle click coordinates to determine if a menu, tree, or scroll item was clicked
    pub fn handle_click(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        tab_paths: &[Option<String>],
        tab_modified: &[bool],
        _dock_terminals_len: usize,
    ) -> UiAction {
        // If a modal is open, check click boundaries and buttons
        if let Some(modal) = self.active_modal {
            let modal_w = match modal {
                ModalType::Settings => (45.0 * self.ui_char_width).max(500.0).round(),
                ModalType::About => 520.0,
                ModalType::CommandPalette => (50.0 * self.ui_char_width).max(500.0).round(),
                ModalType::UnsavedChanges => 520.0,
            };
            let modal_h = match modal {
                ModalType::Settings => {
                    let row_height = (self.ui_line_height * 2.2).round();
                    (row_height * 8.2).max(430.0).round()
                }
                ModalType::About => 190.0,
                ModalType::CommandPalette => {
                    let item_height = (self.ui_line_height * 1.6).round().max(26.0);
                    let filtered_len = self.get_filtered_commands().len();
                    let visible_items = filtered_len.min(10);
                    let header_h = 15.0 + self.ui_line_height + 15.0 + 1.0;
                    (header_h + visible_items as f32 * item_height + 15.0).round()
                }
                ModalType::UnsavedChanges => 200.0,
            };
            let modal_x = ((width - modal_w) / 2.0).round();
            let modal_y = ((height - modal_h) / 2.0).round();

            let clicked_outside = mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h;

            if modal == ModalType::Settings {
                let row_height = (self.ui_line_height * 2.2).round();
                let control_x = modal_x + 24.0 * self.ui_char_width;
                let btn_h = (self.ui_line_height * 1.3).round().max(24.0);
                let btn_w = (self.ui_char_width * 3.0).round().max(24.0);
                let backend_btn_w = (self.ui_char_width * 10.0).round().max(80.0);
                let theme_btn_w = (self.ui_char_width * 16.0).round().max(140.0);

                let row1_y = modal_y + row_height * 1.0;
                let btn1_y = row1_y + ((self.ui_line_height - btn_h) / 2.0).round();
                let row2_y = modal_y + row_height * 2.0;
                let btn2_y = row2_y + ((self.ui_line_height - btn_h) / 2.0).round();
                let row3_y = modal_y + row_height * 3.0;
                let btn3_y = row3_y + ((self.ui_line_height - btn_h) / 2.0).round();
                let row4_y = modal_y + row_height * 4.0;
                let btn4_y = row4_y + ((self.ui_line_height - btn_h) / 2.0).round();
                let row5_y = modal_y + row_height * 5.0;
                let btn5_y = row5_y + ((self.ui_line_height - btn_h) / 2.0).round();
                let row6_y = modal_y + row_height * 6.0;
                let btn6_y = row6_y + ((self.ui_line_height - btn_h) / 2.0).round();

                // Handle dropdown clicks if open
                if self.theme_dropdown_open {
                    let dropdown_y = btn4_y + btn_h;
                    let item_height = (self.ui_line_height * 1.5).round().max(24.0);
                    let dropdown_h = 2.0 * item_height;

                    if mx >= control_x && mx <= control_x + theme_btn_w && my >= dropdown_y && my <= dropdown_y + dropdown_h {
                        let idx = ((my - dropdown_y) / item_height).floor() as usize;
                        let themes = ["Light Theme", "Dark Theme"];
                        if idx < 2 {
                            self.theme_dropdown_open = false;
                            return UiAction::ChangeTheme(themes[idx].to_string());
                        }
                    }

                    // Check if clicked the theme button itself to close it
                    if mx >= control_x && mx <= control_x + theme_btn_w && my >= btn4_y && my <= btn4_y + btn_h {
                        self.theme_dropdown_open = false;
                        return UiAction::None;
                    }

                    // Otherwise, close the dropdown and let the click continue to other controls
                    self.theme_dropdown_open = false;
                }

                // Check other buttons
                // Row 1: Editor Font Size [-] and [+]
                // Decrease [-]
                if mx >= control_x && mx <= control_x + btn_w && my >= btn1_y && my <= btn1_y + btn_h {
                    return UiAction::ChangeBufferFontSize(-1.0);
                }
                // Increase [+]
                let inc_btn_x = control_x + btn_w + self.ui_char_width;
                if mx >= inc_btn_x && mx <= inc_btn_x + btn_w && my >= btn1_y && my <= btn1_y + btn_h {
                    return UiAction::ChangeBufferFontSize(1.0);
                }

                // Row 2: UI Font Size [-] and [+]
                // Decrease [-]
                if mx >= control_x && mx <= control_x + btn_w && my >= btn2_y && my <= btn2_y + btn_h {
                    return UiAction::ChangeUiFontSize(-1.0);
                }
                // Increase [+]
                if mx >= inc_btn_x && mx <= inc_btn_x + btn_w && my >= btn2_y && my <= btn2_y + btn_h {
                    return UiAction::ChangeUiFontSize(1.0);
                }

                // Row 3: Backend Selection
                if mx >= control_x && mx <= control_x + backend_btn_w && my >= btn3_y && my <= btn3_y + btn_h {
                    return UiAction::ChangeBackend(wgpu::Backend::Vulkan);
                }
                let opengl_btn_x = control_x + backend_btn_w + self.ui_char_width;
                if mx >= opengl_btn_x && mx <= opengl_btn_x + backend_btn_w && my >= btn3_y && my <= btn3_y + btn_h {
                    return UiAction::ChangeBackend(wgpu::Backend::Gl);
                }

                // Row 4: Theme Selector Button Click
                if mx >= control_x && mx <= control_x + theme_btn_w && my >= btn4_y && my <= btn4_y + btn_h {
                    self.theme_dropdown_open = true;
                    return UiAction::None;
                }

                // Row 5: Git Blame Selection
                if mx >= control_x && mx <= control_x + backend_btn_w && my >= btn5_y && my <= btn5_y + btn_h {
                    return UiAction::ChangeGitBlame(true);
                }
                let disabled5_btn_x = control_x + backend_btn_w + self.ui_char_width;
                if mx >= disabled5_btn_x && mx <= disabled5_btn_x + backend_btn_w && my >= btn5_y && my <= btn5_y + btn_h {
                    return UiAction::ChangeGitBlame(false);
                }

                // Row 6: Git Branch Selection
                if mx >= control_x && mx <= control_x + backend_btn_w && my >= btn6_y && my <= btn6_y + btn_h {
                    return UiAction::ChangeGitBranch(true);
                }
                let disabled6_btn_x = control_x + backend_btn_w + self.ui_char_width;
                if mx >= disabled6_btn_x && mx <= disabled6_btn_x + backend_btn_w && my >= btn6_y && my <= btn6_y + btn_h {
                    return UiAction::ChangeGitBranch(false);
                }
            }

            if modal == ModalType::CommandPalette {
                let input_y = modal_y + 15.0;
                let sep_y = input_y + self.ui_line_height + 15.0;
                let list_y = sep_y + 1.0;
                let item_height = (self.ui_line_height * 1.6).round().max(26.0);
                let filtered = self.get_filtered_commands();
                let max_visible_items = ((modal_y + modal_h - 15.0 - list_y) / item_height).floor() as usize;
                
                // Scrollbar click detection
                if filtered.len() > max_visible_items {
                    let track_x = modal_x + modal_w - 12.0;
                    if mx >= track_x && mx <= modal_x + modal_w && my >= list_y && my <= modal_y + modal_h - 15.0 {
                        let track_h = max_visible_items as f32 * item_height;
                        let relative_y = (my - list_y).clamp(0.0, track_h);
                        let scroll_ratio = relative_y / track_h;
                        let max_scroll = filtered.len().saturating_sub(max_visible_items);
                        self.command_palette_scroll = (scroll_ratio * max_scroll as f32).round() as usize;
                        return UiAction::None;
                    }
                }

                let list_w = if filtered.len() > max_visible_items { modal_w - 12.0 } else { modal_w };
                if mx >= modal_x && mx <= modal_x + list_w && my >= list_y && my <= modal_y + modal_h - 15.0 {
                    let idx = ((my - list_y) / item_height).floor() as usize + self.command_palette_scroll;
                    if idx < filtered.len() {
                        let cmd = filtered[idx];
                        self.active_modal = None;
                        return self.execute_command(cmd, buffer, cursor);
                    }
                }
            }

            if modal == ModalType::UnsavedChanges {
                let btn_w = 130.0f32;
                let btn_h = 34.0f32;
                let spacing = 15.0f32;
                let total_btn_block_w = 3.0 * btn_w + 2.0 * spacing;
                let start_btn_x = modal_x + ((modal_w - total_btn_block_w) / 2.0).round();
                let btn_y = modal_y + modal_h - btn_h - 20.0;

                if let Some(tab_idx) = self.tab_to_close {
                    // Check Save button
                    let save_x = start_btn_x;
                    if mx >= save_x && mx <= save_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
                        self.active_modal = None;
                        self.tab_to_close = None;
                        return UiAction::SaveAndCloseTab(tab_idx);
                    }

                    // Check Don't Save button
                    let dont_save_x = start_btn_x + btn_w + spacing;
                    if mx >= dont_save_x && mx <= dont_save_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
                        self.active_modal = None;
                        self.tab_to_close = None;
                        return UiAction::ForceCloseTab(tab_idx);
                    }

                    // Check Cancel button
                    let cancel_x = start_btn_x + 2.0 * (btn_w + spacing);
                    if mx >= cancel_x && mx <= cancel_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
                        self.active_modal = None;
                        self.tab_to_close = None;
                        return UiAction::CloseModal;
                    }
                }

                if clicked_outside {
                    self.active_modal = None;
                    self.tab_to_close = None;
                    return UiAction::CloseModal;
                }
                return UiAction::None;
            }

            // Check if clicked close button (centered horizontally)
            let btn_w = (12.0 * self.ui_char_width).max(100.0).round();
            let btn_h = (self.ui_line_height * 1.6).max(30.0).round();
            let btn_x = modal_x + ((modal_w - btn_w) / 2.0).round();
            let btn_y = modal_y + modal_h - btn_h - (self.ui_line_height * 1.0).round();

            let inside_close_btn = modal != ModalType::CommandPalette && modal != ModalType::UnsavedChanges && mx >= btn_x && mx <= btn_x + btn_w && my >= btn_y && my <= btn_y + btn_h;

            if (inside_close_btn || clicked_outside) && modal != ModalType::UnsavedChanges {
                self.active_modal = None;
                return UiAction::CloseModal;
            }

            return UiAction::None;
        }

        // 1. Check Titlebar Menu Clicks (Contiguous adjacent layout)
        if my < self.titlebar_height {
            if !self.is_tiling_wm() {
                let btn_w = 45.0f32;
                if mx >= width - btn_w {
                    return UiAction::Exit;
                } else if mx >= width - btn_w * 2.0 && mx < width - btn_w {
                    return UiAction::MaximizeWindow;
                } else if mx >= width - btn_w * 3.0 && mx < width - btn_w * 2.0 {
                    return UiAction::MinimizeWindow;
                }
            }

            let menu_items_raw = [
                ("Garage", MenuType::Garage),
                ("File", MenuType::File),
                ("Edit", MenuType::Edit),
                ("Selection", MenuType::Selection),
                ("View", MenuType::View),
            ];
            let mut current_x = 0.0;
            for (i, (label, menu_type)) in menu_items_raw.iter().enumerate() {
                let label_len = label.chars().count() as f32;
                let text_w = label_len * self.ui_char_width;
                let (left_pad, right_pad) = if i == 0 {
                    (14.0, 10.0)
                } else {
                    (10.0, 10.0)
                };
                let item_w = text_w + left_pad + right_pad;
                let x_min = current_x;
                let x_max = current_x + item_w;
                if mx >= x_min && mx < x_max {
                    self.active_menu = if self.active_menu == Some(*menu_type) { None } else { Some(*menu_type) };
                    return UiAction::None;
                }
                current_x = x_max;
            }
            self.active_menu = None;
            return UiAction::None;
        }

        // 2. Check Dropdown Clicks (if active)
        if let Some(menu) = self.active_menu {
            let items = match menu {
                MenuType::Garage => vec!["Settings", "About", "Exit"],
                MenuType::File => vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
                MenuType::Edit => vec!["Undo (Ctrl+Z)", "Redo (Ctrl+Y)"],
                MenuType::Selection => vec!["Select All", "Clear Selection"],
                MenuType::View => vec!["Toggle Sidebar", "Command Palette (Ctrl+Shift+P)"],
            };
            
            // Calculate dynamic menu_x matching the contiguous header position
            let menu_items_raw = [
                ("Garage", MenuType::Garage),
                ("File", MenuType::File),
                ("Edit", MenuType::Edit),
                ("Selection", MenuType::Selection),
                ("View", MenuType::View),
            ];
            let mut menu_x = 0.0;
            let mut current_x = 0.0;
            for (i, (label, m_type)) in menu_items_raw.iter().enumerate() {
                let label_len = label.chars().count() as f32;
                let text_w = label_len * self.ui_char_width;
                let (left_pad, right_pad) = if i == 0 {
                    (14.0, 10.0)
                } else {
                    (10.0, 10.0)
                };
                let item_w = text_w + left_pad + right_pad;
                if m_type == &menu {
                    menu_x = current_x;
                    break;
                }
                current_x = current_x + item_w;
            }

            let item_height = (self.ui_line_height * 1.6).round().max(26.0);
            let dropdown_h = items.len() as f32 * item_height;
            let max_chars = items.iter().map(|s| s.chars().count()).max().unwrap_or(10) as f32;
            let dropdown_w = (max_chars * self.ui_char_width + 30.0).round();

            let menu_action = if mx >= menu_x && mx < menu_x + dropdown_w && my >= self.titlebar_height && my < self.titlebar_height + dropdown_h {
                let idx = ((my - self.titlebar_height) / item_height).floor() as usize;
                match menu {
                    MenuType::Garage => match idx {
                        0 => Some(UiAction::ShowSettings),
                        1 => Some(UiAction::ShowAbout),
                        2 => Some(UiAction::Exit),
                        _ => None,
                    },
                    MenuType::File => match idx {
                        0 => Some(UiAction::SaveFile),
                        1 => Some(UiAction::ToggleSidebar),
                        2 => Some(UiAction::Exit),
                        _ => None,
                    },
                    MenuType::Edit => match idx {
                        0 => Some(UiAction::Undo),
                        1 => Some(UiAction::Redo),
                        _ => None,
                    },
                    MenuType::Selection => match idx {
                        0 => {
                            cursor.selection_anchor = Some((0, 0));
                            cursor.line = buffer.len() - 1;
                            cursor.col = buffer.lines()[cursor.line].chars().count();
                            cursor.intended_col = cursor.col;
                            Some(UiAction::None)
                        }
                        1 => {
                            cursor.clear_selection();
                            Some(UiAction::None)
                        }
                        _ => None,
                    },
                    MenuType::View => match idx {
                        0 => Some(UiAction::ToggleSidebar),
                        1 => Some(UiAction::ShowCommandPalette),
                        _ => None,
                    },
                }
            } else {
                None
            };

            self.active_menu = None;
            if let Some(action) = menu_action {
                return action;
            }
            return UiAction::None;
        }

        // 3. Check Tabbar Clicks
        let main_y = self.titlebar_height;
        if my >= main_y && my < main_y + self.tabbar_height {
            // Check actual file tabs
            let tab_close_icon_sz = (self.ui_font_size * 0.8).round().max(10.0);
            let activity_bar_width = 0.0;
            let mut current_tab_x = activity_bar_width + self.sidebar_width;
            let close_reserved = 8.0f32 + tab_close_icon_sz;

            for idx in 0..tab_paths.len() {
                let path_opt = &tab_paths[idx];
                let _is_modified = tab_modified.get(idx).copied().unwrap_or(false);
                let dot_reserved = 18.0f32;
                let file_name = path_opt.as_ref()
                    .and_then(|p| Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "untitled.txt".to_string());

                let name_w = file_name.chars().count() as f32 * self.ui_char_width;
                let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);

                if mx >= current_tab_x && mx < current_tab_x + tab_w {
                    // Check if clicked the close button
                    let close_x = current_tab_x + tab_w - 10.0 - tab_close_icon_sz;
                    let close_y = (main_y + self.tabbar_height / 2.0 - tab_close_icon_sz / 2.0).round();
                    
                    // Allow some padding around the close icon for easier clicking
                    if mx >= close_x - 3.0 && mx < close_x + tab_close_icon_sz + 3.0 && my >= close_y - 3.0 && my <= close_y + tab_close_icon_sz + 3.0 {
                        self.active_menu = None;
                        return UiAction::CloseTab(idx);
                    } else {
                        self.active_menu = None;
                        return UiAction::SelectTab(idx);
                    }
                }
                current_tab_x += tab_w;
            }

            self.active_menu = None;
            return UiAction::None;
        }

        // 3.5. Check Activity Bar Clicks (reverted)
        let activity_bar_width = 0.0;

        // 4. Check Sidebar Clicks
        if self.sidebar_width > 0.0 && mx >= activity_bar_width && mx < activity_bar_width + self.sidebar_width && my > main_y && my < height - self.status_height {
            let tree_y = my - main_y;
            let row_idx = (tree_y / self.ui_line_height).floor() as usize;
            let r = row_idx + self.sidebar_scroll;
            if r >= 1 {
                let node_idx = r - 1;
                if node_idx < self.visible_nodes.len() {
                    let path = self.visible_nodes[node_idx].path.clone();
                    let is_dir = self.visible_nodes[node_idx].is_dir;
                    if is_dir {
                        if self.expanded_dirs.contains(&path) {
                            self.expanded_dirs.remove(&path);
                        } else {
                            self.expanded_dirs.insert(path);
                        }
                        self.rebuild_tree();
                    } else {
                        self.selected_file = Some(path.clone());
                        return UiAction::OpenFile(path);
                    }
                }
            }
            return UiAction::None;
        }

        // 5. Check Dock Tab Clicks
        let mut dock_start_y = height - self.status_height;
        if self.show_dock {
            dock_start_y = (height - self.status_height - self.dock_height).max(main_y + self.tabbar_height + self.breadcrumb_height + 50.0);
        }
        let dock_tabbar_h = 28.0f32;
        if self.show_dock && my >= dock_start_y && my < dock_start_y + dock_tabbar_h {
            let mut cur_x = self.sidebar_width + 10.0f32;
            let tab_y = dock_start_y + 1.0;
            let tab_h = dock_tabbar_h - 1.0;
            
            for idx in 0.._dock_terminals_len {
                let term_name = format!("terminal-{}", idx + 1);
                let term_name_w = term_name.chars().count() as f32 * self.ui_char_width * 0.9;
                let icon_sz = 12.0f32;
                let close_sz = 10.0f32;
                let tab_w = 12.0 + icon_sz + 6.0 + term_name_w + 8.0 + close_sz + 10.0;
                
                if mx >= cur_x && mx < cur_x + tab_w {
                    // Check if clicked close button of the dock tab
                    let close_x = cur_x + tab_w - 8.0 - close_sz;
                    let close_y = (tab_y + (tab_h - close_sz) / 2.0).round();
                    if mx >= close_x - 3.0 && mx < close_x + close_sz + 3.0 && my >= close_y - 3.0 && my <= close_y + close_sz + 3.0 {
                        return UiAction::CloseTerminal(idx);
                    } else {
                        return UiAction::SelectTerminal(idx);
                    }
                }
                cur_x += tab_w;
            }
            
            // Check '+' button to add new terminal
            let add_btn_w = 28.0f32;
            if mx >= cur_x && mx < cur_x + add_btn_w {
                return UiAction::NewTerminal;
            }
            
            // Check close dock button
            let close_dock_w = 28.0f32;
            let close_dock_x = width - 10.0 - close_dock_w;
            if mx >= close_dock_x && mx < close_dock_x + close_dock_w {
                return UiAction::ToggleDock;
            }
        }

        // 6. Check Statusbar Button Clicks
        let status_y = height - self.status_height;
        if my >= status_y {
            let sb_btn_w = 26.0f32;
            let term_btn_x = width - 10.0 - sb_btn_w;

            if mx >= term_btn_x && mx < term_btn_x + sb_btn_w {
                return UiAction::ToggleDock;
            }
        }

        self.active_menu = None;
        UiAction::None
    }

    /// Push a solid rectangle (quad) into the vertex and index vectors
    pub fn push_quad(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        white_uv: [f32; 2],
        color: [f32; 4],
    ) {
        // Round panel coordinates to integer pixels for crisp borders
        let rx = x.round();
        let ry = y.round();
        let rw = w.round();
        let rh = h.round();

        let start = vertices.len() as u16;
        vertices.push(Vertex {
            position: [rx, ry],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [rx + rw, ry],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [rx + rw, ry + rh],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [rx, ry + rh],
            tex_coords: white_uv,
            color,
        });
        indices.extend_from_slice(&[start, start + 1, start + 2, start + 2, start + 3, start]);
    }

    /// Push a single text character glyph using the font atlas
    pub fn push_char(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        c: char,
        pen_x: f32,
        baseline_y: f32,
        color: [f32; 4],
        font_size: f32,
        char_width: f32,
    ) -> f32 {
        if let Some(info) = atlas.get_or_rasterize(queue, c, font_size) {
            if info.width == 0.0 || info.height == 0.0 {
                return char_width;
            }

            // CRITICAL: Round coordinates to exact integer pixels to eliminate bilinear filtering blur!
            let x = (pen_x + info.bearing_x).round();
            let y = (baseline_y - info.bearing_y - info.height).round();
            let w = info.width.round();
            let h = info.height.round();

            let start = vertices.len() as u16;
            vertices.push(Vertex {
                position: [x, y],
                tex_coords: info.uv_min,
                color,
            });
            vertices.push(Vertex {
                position: [x + w, y],
                tex_coords: [info.uv_max[0], info.uv_min[1]],
                color,
            });
            vertices.push(Vertex {
                position: [x + w, y + h],
                tex_coords: info.uv_max,
                color,
            });
            vertices.push(Vertex {
                position: [x, y + h],
                tex_coords: [info.uv_min[0], info.uv_max[1]],
                color,
            });
            indices.extend_from_slice(&[start, start + 1, start + 2, start + 2, start + 3, start]);
        }
        char_width
    }

    pub fn push_icon(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        icon_path: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        size: f32,
    ) -> f32 {
        if let Some(info) = atlas.get_or_rasterize_icon(queue, icon_path, size) {
            let start = vertices.len() as u16;
            let rx = x.round();
            let ry = y.round();
            let rw = info.width.round();
            let rh = info.height.round();

            vertices.push(Vertex {
                position: [rx, ry],
                tex_coords: [info.uv_min[0], info.uv_min[1]],
                color,
            });
            vertices.push(Vertex {
                position: [rx + rw, ry],
                tex_coords: [info.uv_max[0], info.uv_min[1]],
                color,
            });
            vertices.push(Vertex {
                position: [rx + rw, ry + rh],
                tex_coords: [info.uv_max[0], info.uv_max[1]],
                color,
            });
            vertices.push(Vertex {
                position: [rx, ry + rh],
                tex_coords: [info.uv_min[0], info.uv_max[1]],
                color,
            });

            indices.extend_from_slice(&[start, start + 1, start + 2, start + 2, start + 3, start]);
            rw
        } else {
            0.0
        }
    }

    pub fn push_str(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        text: &str,
        mut x: f32,
        y: f32,
        color: [f32; 4],
        font_size: f32,
        char_width: f32,
    ) -> f32 {
        let start_x = x;
        for c in text.chars() {
            x += self.push_char(vertices, indices, atlas, queue, c, x, y, color, font_size, char_width);
        }
        x - start_x
    }

    /// Parse enclosing function/struct backwards from cursor line
    pub fn find_current_function(&self, buffer: &Buffer, cursor_line: usize) -> Option<String> {
        for i in (0..=cursor_line).rev() {
            if i >= buffer.len() {
                continue;
            }
            let line = &buffer.lines()[i];
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") || trimmed.starts_with("pub(crate) fn ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for (idx, &part) in parts.iter().enumerate() {
                    if part == "fn" && idx + 1 < parts.len() {
                        let fn_name_full = parts[idx + 1];
                        let fn_name = fn_name_full.split('(').next().unwrap_or(fn_name_full);
                        return Some(format!("fn {}", fn_name));
                    }
                }
            } else if trimmed.starts_with("impl ") || trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for (idx, &part) in parts.iter().enumerate() {
                    if (part == "struct" || part == "impl") && idx + 1 < parts.len() {
                        let name_full = parts[idx + 1];
                        let name = name_full.split('{').next().unwrap_or(name_full);
                        return Some(format!("{} {}", part, name));
                    }
                }
            }
        }
        None
    }

    /// Fast token coloring logic for Rust code lines
    pub fn get_line_char_colors(&self, line_text: &str) -> Vec<[f32; 4]> {
        let chars: Vec<char> = line_text.chars().collect();
        let default_color = self.config.theme.syntax_default;
        let mut colors = vec![default_color; chars.len()];

        let keywords = [
            "use", "fn", "pub", "struct", "bool", "true", "false", "let", "mut",
            "impl", "for", "in", "if", "else", "return", "match", "self", "as",
            "ref", "type", "enum", "mod", "crate", "const", "static", "where",
            "break", "continue", "loop", "while",
        ];

        let mut i = 0;
        while i < chars.len() {
            // 1. Comment check
            if i + 1 < chars.len() && chars[i] == '/' && chars[i+1] == '/' {
                for j in i..chars.len() {
                    colors[j] = self.config.theme.syntax_comment;
                }
                break;
            }

            // 2. String literal check
            if chars[i] == '"' {
                colors[i] = self.config.theme.syntax_string;
                i += 1;
                while i < chars.len() {
                    colors[i] = self.config.theme.syntax_string;
                    if chars[i] == '"' && (i == 0 || chars[i-1] != '\\') {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // 3. Char literal check
            if chars[i] == '\'' {
                colors[i] = self.config.theme.syntax_string;
                i += 1;
                while i < chars.len() {
                    colors[i] = self.config.theme.syntax_string;
                    if chars[i] == '\'' && (i == 0 || chars[i-1] != '\\') {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // 4. Attribute check
            if chars[i] == '#' {
                colors[i] = self.config.theme.syntax_attribute;
                i += 1;
                continue;
            }

            // 5. Identifier check
            if chars[i].is_alphabetic() || chars[i] == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let color = if keywords.contains(&word.as_str()) {
                    self.config.theme.syntax_keyword
                } else if word.chars().next().map_or(false, |c| c.is_uppercase()) {
                    self.config.theme.syntax_type
                } else {
                    default_color
                };
                for j in start..i {
                    colors[j] = color;
                }
                continue;
            }

            // 6. Number check
            if chars[i].is_ascii_digit() {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                for j in start..i {
                    colors[j] = self.config.theme.syntax_number;
                }
                continue;
            }

            i += 1;
        }

        colors
    }

    pub fn update_git_branch(&mut self) {
        let output = std::process::Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .output();
        
        self.git_branch = match output {
            Ok(out) if out.status.success() => {
                let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if branch.is_empty() {
                    None
                } else {
                    Some(branch)
                }
            }
            _ => None,
        };
    }

    pub fn get_or_update_blame(&mut self, file_path: Option<&str>, line_idx: usize) -> Option<String> {
        let file_path = file_path?;
        // Check if cached
        if self.last_blame_file.as_deref() == Some(file_path) && self.last_blame_line == Some(line_idx) {
            return self.last_blame_result.clone();
        }

        // Update cache
        self.last_blame_file = Some(file_path.to_string());
        self.last_blame_line = Some(line_idx);

        // Run git blame for a single line (1-based index)
        let git_line = line_idx + 1;
        let output = std::process::Command::new("git")
            .args(&["blame", "-L", &format!("{},{}", git_line, git_line), "--porcelain", file_path])
            .output();

        let blame_res = match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut author = None;
                let mut author_time = None;
                let mut summary = None;

                for line in stdout.lines() {
                    if line.starts_with("author ") {
                        author = Some(line["author ".len()..].trim().to_string());
                    } else if line.starts_with("author-time ") {
                        author_time = line["author-time ".len()..].trim().parse::<u64>().ok();
                    } else if line.starts_with("summary ") {
                        summary = Some(line["summary ".len()..].trim().to_string());
                    }
                }

                if let (Some(auth), Some(time), Some(sum)) = (author, author_time, summary) {
                    if auth == "Not Committed Yet" {
                        Some("Not Committed Yet".to_string())
                    } else {
                        // Calculate relative time
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let diff = now.saturating_sub(time);
                        let time_str = if diff < 60 {
                            "just now".to_string()
                        } else if diff < 3600 {
                            format!("{}m ago", diff / 60)
                        } else if diff < 86400 {
                            format!("{}h ago", diff / 3600)
                        } else if diff < 2592000 {
                            let days = diff / 86400;
                            if days == 1 { "yesterday".to_string() } else { format!("{} days ago", days) }
                        } else if diff < 31536000 {
                            let months = diff / 2592000;
                            if months == 1 { "1 month ago".to_string() } else { format!("{} months ago", months) }
                        } else {
                            let years = diff / 31536000;
                            if years == 1 { "1 year ago".to_string() } else { format!("{} years ago", years) }
                        };

                        Some(format!("{} • {} • {}", auth, time_str, sum))
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        self.last_blame_result = blame_res.clone();
        blame_res
    }

    pub fn get_all_commands(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Theme: Light Theme", "Switch to the Light Theme"),
            ("Theme: Dark Theme", "Switch to the default Dark Theme"),
            ("Sidebar: Toggle Visibility", "Show or hide the file tree sidebar"),
            ("Font Size: Increase Editor Font", "Increase the text size of the editor"),
            ("Font Size: Decrease Editor Font", "Decrease the text size of the editor"),
            ("Git Blame: Toggle Inline Annotations", "Enable/disable inline git blame"),
            ("Git Branch: Toggle Branch Statusbar", "Enable/disable git branch status"),
            ("Settings: Open settings modal", "Configure editor options"),
            ("About: Open about dialog", "View editor information"),
            ("Exit: Quit Garage", "Close the code editor"),
        ]
    }

    pub fn get_filtered_commands(&self) -> Vec<(&'static str, &'static str)> {
        let query = self.command_palette_query.to_lowercase();
        if query.is_empty() {
            return self.get_all_commands();
        }
        self.get_all_commands()
            .into_iter()
            .filter(|(name, desc)| {
                name.to_lowercase().contains(&query) || desc.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn execute_command(
        &mut self,
        cmd: (&'static str, &'static str),
        _buffer: &mut Buffer,
        _cursor: &mut Cursor,
    ) -> UiAction {
        match cmd.0 {
            "Theme: Light Theme" => UiAction::ChangeTheme("Light Theme".to_string()),
            "Theme: Dark Theme" => UiAction::ChangeTheme("Dark Theme".to_string()),
            "Sidebar: Toggle Visibility" => UiAction::ToggleSidebar,
            "Font Size: Increase Editor Font" => UiAction::ChangeBufferFontSize(1.0),
            "Font Size: Decrease Editor Font" => UiAction::ChangeBufferFontSize(-1.0),
            "Git Blame: Toggle Inline Annotations" => {
                let enabled = !self.config.show_git_blame;
                UiAction::ChangeGitBlame(enabled)
            }
            "Git Branch: Toggle Branch Statusbar" => {
                let enabled = !self.config.show_git_branch;
                UiAction::ChangeGitBranch(enabled)
            }
            "Settings: Open settings modal" => {
                self.active_modal = Some(ModalType::Settings);
                UiAction::None
            }
            "About: Open about dialog" => {
                self.active_modal = Some(ModalType::About);
                UiAction::None
            }
            "Exit: Quit Garage" => UiAction::Exit,
            _ => UiAction::None,
        }
    }

    /// Build entire UI frame (Titlebar, Sidebar, Scrollbar, Dropdowns, Modals)
    pub fn build_frame(
        &mut self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        buffer: &Buffer,
        cursor: &Cursor,
        width: f32,
        height: f32,
        mouse_x: f32,
        mouse_y: f32,
        current_backend: wgpu::Backend,
        tab_paths: &[Option<String>],
        tab_modified: &[bool],
        active_tab_idx: usize,
        terminals: &[TerminalInstance],
        active_terminal_idx: usize,
        terminal_focus: bool,
        _is_window_maximized: bool,
    ) {
        self.active_dock_tab = active_terminal_idx;

        // Throttled git branch check
        if self.config.show_git_branch {
            if self.last_branch_check.is_none() || self.last_branch_check.unwrap().elapsed() > std::time::Duration::from_secs(5) {
                self.update_git_branch();
                self.last_branch_check = Some(std::time::Instant::now());
            }
        } else {
            self.git_branch = None;
        }
        let main_y = self.titlebar_height;
        let main_height = height - self.titlebar_height - self.status_height;

        // Instant expand/collapse sidebar width (no animation delay)
        self.sidebar_width = self.target_sidebar_width;

        let mut dock_y = height - self.status_height;
        if self.show_dock {
            dock_y = (height - self.status_height - self.dock_height).max(main_y + self.tabbar_height + self.breadcrumb_height + 50.0);
        }
        let status_y = if self.show_dock { dock_y.round() } else { (height - self.status_height).round() };
        let dock_start_y = status_y;

        // --- 1. Draw Titlebar Menu Headers (Light Theme) ---
        self::components::titlebar::draw_titlebar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            mouse_x,
            mouse_y,
        );

        // --- 2. Draw Sidebar Panel (Light Theme) ---
        self::components::sidebar::draw_sidebar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            main_y,
            main_height,
            mouse_x,
            mouse_y,
        );

        // --- 3. Draw Editor Tabbar, Breadcrumbs, Text Area, Gutter, Scrollbars & Minimap ---
        self::components::editor_view::draw_editor_view(
            self,
            vertices,
            indices,
            atlas,
            queue,
            buffer,
            cursor,
            width,
            mouse_x,
            mouse_y,
            tab_paths,
            tab_modified,
            active_tab_idx,
            status_y,
        );

        // --- 4.5. Draw Bottom Dock ---
        self::components::dock::draw_dock(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            height,
            mouse_x,
            mouse_y,
            terminals,
            terminal_focus,
            dock_start_y,
        );

        // --- 5. Draw Statusbar ---
        self::components::statusbar::draw_statusbar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            height,
            buffer,
            cursor,
            mouse_x,
            mouse_y,
        );

        // --- 6. Draw Context Dropdown Menus & 7. Modal Dialogs ---
        self::components::modals::draw_modals(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            height,
            mouse_x,
            mouse_y,
            current_backend,
            tab_paths,
        );
    }
}
