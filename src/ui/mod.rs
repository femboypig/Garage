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

    pub config: crate::config::AppConfig,
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
    pub fn new(atlas: &mut FontAtlas, _queue: &wgpu::Queue, config: crate::config::AppConfig) -> Self {
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
        let active_file_path = tab_paths.get(active_tab_idx).and_then(|p| p.as_deref());
        let white_uv = atlas.white_pixel_uv();

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

        // Sidebar Navigator (Activity Bar) Width
        let activity_bar_width = 0.0;

        // Calculate dynamic layouts
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * self.buffer_char_width;
        let text_area_x = activity_bar_width + self.sidebar_width + gutter_width;
        
        let scrollbar_width = self.scrollbar_width();
        let minimap_width = self.minimap_width();
        let sb_x = width - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
        let text_viewport_w = minimap_x - text_area_x;

        let mut dock_y = height - self.status_height;
        if self.show_dock {
            dock_y = (height - self.status_height - self.dock_height).max(main_y + self.tabbar_height + self.breadcrumb_height + 50.0);
        }
        let status_y = if self.show_dock { dock_y.round() } else { (height - self.status_height).round() };
        let dock_start_y = status_y;
        let editor_y = main_y + self.tabbar_height + self.breadcrumb_height;
        let total_editor_height = status_y - editor_y;
        let editor_height = total_editor_height - 14.0;
        let visible_lines = (editor_height / self.buffer_line_height).floor() as usize;
        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as usize;
        self.scroll_y = self.scroll_y.min(max_scroll);

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

        // --- Tab Bar & Control Buttons (New) ---
        // Tab Bar background (gray)
        self.push_quad(
            vertices,
            indices,
            activity_bar_width + self.sidebar_width,
            main_y,
            width - (activity_bar_width + self.sidebar_width),
            self.tabbar_height,
            white_uv,
            self.config.theme.tabbar_bg,
        );
        // Pre-calculate active tab X and width to omit the border underneath it
        let mut active_tab_x = 0.0f32;
        let mut active_tab_w = 0.0f32;
        let mut has_active_tab = false;
        let tab_close_icon_sz = (self.ui_font_size * 0.8).round().max(10.0);

        let mut temp_x = activity_bar_width + self.sidebar_width;
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = path_opt.as_ref()
                .and_then(|p| Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string());
            let name_w = file_name.chars().count() as f32 * self.ui_char_width;
            let dot_reserved = 18.0f32;
            let close_reserved = 8.0f32 + tab_close_icon_sz;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
            if idx == active_tab_idx {
                active_tab_x = temp_x;
                active_tab_w = tab_w;
                has_active_tab = true;
            }
            temp_x += tab_w;
        }

        // Tab bar bottom border
        let tabbar_start_x = activity_bar_width + self.sidebar_width;
        if has_active_tab {
            if active_tab_x > tabbar_start_x {
                self.push_quad(
                    vertices,
                    indices,
                    tabbar_start_x,
                    main_y + self.tabbar_height - 1.0,
                    active_tab_x - tabbar_start_x,
                    1.0,
                    white_uv,
                    self.config.theme.tabbar_border,
                );
            }
            let right_start_x = active_tab_x + active_tab_w;
            if right_start_x < width {
                self.push_quad(
                    vertices,
                    indices,
                    right_start_x,
                    main_y + self.tabbar_height - 1.0,
                    width - right_start_x,
                    1.0,
                    white_uv,
                    self.config.theme.tabbar_border,
                );
            }
        } else {
            self.push_quad(
                vertices,
                indices,
                tabbar_start_x,
                main_y + self.tabbar_height - 1.0,
                width - tabbar_start_x,
                1.0,
                white_uv,
                self.config.theme.tabbar_border,
            );
        }

        // Draw active/inactive file tabs
        let mut current_tab_x = activity_bar_width + self.sidebar_width;
        let tab_baseline = (main_y + self.tabbar_height / 2.0 + self.ui_font_ascent / 2.0 - 3.5).round();

        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let is_active = idx == active_tab_idx;
            let is_modified = tab_modified.get(idx).copied().unwrap_or(false);

            let file_name = path_opt.as_ref()
                .and_then(|p| Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string());

            // Compute tab width
            let name_w = file_name.chars().count() as f32 * self.ui_char_width;
            let dot_reserved = 18.0f32;
            let close_reserved = 8.0f32 + tab_close_icon_sz;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);

            // Draw tab background
            let bg_color = if is_active {
                self.config.theme.tab_active_bg
            } else {
                self.config.theme.tabbar_bg
            };
            let tab_h = if is_active {
                self.tabbar_height
            } else {
                self.tabbar_height - 1.0
            };
            self.push_quad(
                vertices,
                indices,
                current_tab_x,
                main_y,
                tab_w,
                tab_h,
                white_uv,
                bg_color,
            );

            // Draw separators/borders
            if idx > 0 {
                self.push_quad(
                    vertices,
                    indices,
                    current_tab_x,
                    main_y,
                    1.0,
                    self.tabbar_height,
                    white_uv,
                    self.config.theme.tabbar_border,
                );
            }
            self.push_quad(
                vertices,
                indices,
                current_tab_x + tab_w - 1.0,
                main_y,
                1.0,
                self.tabbar_height,
                white_uv,
                self.config.theme.tabbar_border,
            );

            // Draw unsaved circle icon if modified
            if is_modified {
                let dot_size = (self.ui_font_size * 0.55).round().max(7.0);
                let dot_x = (current_tab_x + 10.0).round();
                let dot_y = (main_y + self.tabbar_height / 2.0 - dot_size / 2.0).round();
                self.push_icon(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "circle",
                    dot_x,
                    dot_y,
                    self.config.theme.tab_text,
                    dot_size,
                );
            }

            // Draw tab label
            let label_x = current_tab_x + 12.0 + dot_reserved;
            let label_color = if is_active {
                self.config.theme.tab_text
            } else {
                let mut c = self.config.theme.tab_text;
                c[3] *= 0.6;
                c
            };
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &file_name,
                label_x,
                tab_baseline,
                label_color,
                self.ui_font_size,
                self.ui_char_width,
            );

            let is_tab_hovered = self.active_modal.is_none() && mouse_x >= current_tab_x && mouse_x < current_tab_x + tab_w && mouse_y >= main_y && mouse_y < main_y + self.tabbar_height;
            if is_tab_hovered {
                // Draw close button SVG icon
                let close_x = current_tab_x + tab_w - 10.0 - tab_close_icon_sz;
                let close_y = (main_y + self.tabbar_height / 2.0 - tab_close_icon_sz / 2.0).round();

                let is_close_hovered = self.active_modal.is_none() && mouse_x >= close_x - 3.0 && mouse_x < close_x + tab_close_icon_sz + 3.0 && mouse_y >= close_y - 3.0 && mouse_y < close_y + tab_close_icon_sz + 3.0;
                let close_color = if is_close_hovered {
                    [1.0, 0.3, 0.3, 1.0]
                } else {
                    let mut c = self.config.theme.tab_text;
                    c[3] *= 0.4;
                    c
                };

                self.push_icon(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "close",
                    close_x,
                    close_y,
                    close_color,
                    tab_close_icon_sz,
                );
            }

            current_tab_x += tab_w;
        }

        // --- Breadcrumb Bar (New) ---
        // Breadcrumb bar background (white)
        self.push_quad(
            vertices,
            indices,
            activity_bar_width + self.sidebar_width,
            main_y + self.tabbar_height,
            width - (activity_bar_width + self.sidebar_width),
            self.breadcrumb_height,
            white_uv,
            self.config.theme.breadcrumb_bg,
        );
        // Breadcrumb bottom border
        self.push_quad(
            vertices,
            indices,
            activity_bar_width + self.sidebar_width,
            main_y + self.tabbar_height + self.breadcrumb_height - 1.0,
            width - (activity_bar_width + self.sidebar_width),
            1.0,
            white_uv,
            self.config.theme.breadcrumb_border,
        );
        
        // Construct breadcrumb text: relative_path > current_function
        let relative_path = self.selected_file.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        
        let current_fn = self.find_current_function(buffer, cursor.line);
        let breadcrumb_text = if let Some(ref func) = current_fn {
            format!("{} > {}", relative_path, func)
        } else {
            relative_path
        };
        self.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &breadcrumb_text,
            activity_bar_width + self.sidebar_width + 15.0,
            (main_y + self.tabbar_height + self.breadcrumb_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
            self.config.theme.breadcrumb_text,
            self.ui_font_size,
            self.ui_char_width,
        );

        // --- 3. Draw Editor Text Area & Gutter (Light Theme) ---
        self.push_quad(
            vertices,
            indices,
            activity_bar_width + self.sidebar_width,
            editor_y,
            gutter_width,
            total_editor_height,
            white_uv,
            self.config.theme.gutter_bg,
        );
        self.push_quad(
            vertices,
            indices,
            text_area_x - 1.0,
            editor_y,
            1.0,
            total_editor_height,
            white_uv,
            self.config.theme.gutter_border,
        );

        // Draw main editor background area
        self.push_quad(
            vertices,
            indices,
            text_area_x,
            editor_y,
            text_viewport_w,
            editor_height,
            white_uv,
            self.config.theme.editor_bg,
        );

        let start_idx = self.scroll_y;
        let end_idx = (start_idx + visible_lines).min(buffer.len());

        for line_idx in start_idx..end_idx {
            let row_y = editor_y + (line_idx - start_idx) as f32 * self.buffer_line_height;
            let baseline_y = (row_y + self.buffer_font_ascent).round();

            // Active line highlight
            if line_idx == cursor.line {
                self.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    row_y,
                    text_viewport_w,
                    self.buffer_line_height,
                    white_uv,
                    self.config.theme.active_line_bg,
                );
            }

            // Draw line numbers
            let line_num_str = format!("{:>width$}", line_idx + 1, width = max_line_digits);
            let num_color = if line_idx == cursor.line {
                self.config.theme.line_number_active
            } else {
                self.config.theme.line_number_inactive
            };
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &line_num_str,
                activity_bar_width + self.sidebar_width + self.buffer_char_width,
                baseline_y,
                num_color,
                self.buffer_font_size,
                self.buffer_char_width,
            );

            // Draw selection ranges
            if let Some((s_line, s_col, e_line, e_col)) = cursor.selection_range() {
                if line_idx >= s_line && line_idx <= e_line {
                    let line_chars_count = buffer.lines()[line_idx].chars().count();
                    let col_start = if line_idx == s_line { s_col } else { 0 };
                    let col_end = if line_idx == e_line { e_col } else { line_chars_count };

                    // Adjust for scroll_x
                    let visible_start = col_start.saturating_sub(self.scroll_x);
                    let visible_end = col_end.saturating_sub(self.scroll_x);

                    if visible_start < visible_end {
                        let sel_x = text_area_x + visible_start as f32 * self.buffer_char_width;
                        let mut sel_w = ((visible_end - visible_start) as f32).max(0.5) * self.buffer_char_width;
                        if sel_x < minimap_x {
                            if sel_x + sel_w > minimap_x {
                                sel_w = minimap_x - sel_x;
                            }
                            self.push_quad(
                                vertices,
                                indices,
                                sel_x,
                                row_y,
                                sel_w,
                                self.buffer_line_height,
                                white_uv,
                                self.config.theme.selection_bg,
                            );
                        }
                    }
                }
            }

            // Draw source code text characters (with custom Rust syntax highlighting)
            let line_text = &buffer.lines()[line_idx];
            let mut pen_x = text_area_x;
            let char_colors = self.get_line_char_colors(line_text);
            
            for (char_idx, c) in line_text.chars().enumerate() {
                if char_idx < self.scroll_x {
                    continue;
                }
                // Stop rendering if we go past the minimap/scrollbar area to prevent overlap/overflow
                if pen_x + self.buffer_char_width > minimap_x {
                    break;
                }
                let char_color = char_colors.get(char_idx).copied().unwrap_or(self.config.theme.syntax_default);
                pen_x += self.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color, self.buffer_font_size, self.buffer_char_width);
            }

            // Draw Git Blame inline annotation at the end of the active line
            if self.config.show_git_blame && line_idx == cursor.line {
                if let Some(blame_str) = self.get_or_update_blame(active_file_path, line_idx) {
                    let line_len = line_text.chars().count();
                    for (c_idx, c) in blame_str.chars().enumerate() {
                        let v_idx = line_len + 4 + c_idx;
                        if v_idx < self.scroll_x {
                            continue;
                        }
                        let blame_char_x = text_area_x + (v_idx - self.scroll_x) as f32 * self.buffer_char_width;
                        if blame_char_x + self.buffer_char_width > minimap_x {
                            break;
                        }
                        self.push_char(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            c,
                            blame_char_x,
                            baseline_y,
                            self.config.theme.syntax_comment,
                            self.buffer_font_size,
                            self.buffer_char_width,
                        );
                    }
                }
            }
        }

        // Draw active cursor
        if cursor.line >= self.scroll_y && cursor.line < self.scroll_y + visible_lines {
            let cur_row_y = editor_y + (cursor.line - self.scroll_y) as f32 * self.buffer_line_height;
            let cur_x = text_area_x + (cursor.col as isize - self.scroll_x as isize) as f32 * self.buffer_char_width;
            
            if cursor.col >= self.scroll_x && cur_x + 2.0 <= minimap_x {
                self.push_quad(
                    vertices,
                    indices,
                    cur_x,
                    cur_row_y + 1.0,
                    2.0,
                    self.buffer_line_height - 2.0,
                    white_uv,
                    self.config.theme.cursor_color,
                );
            }
        }

        // --- 4. Draw Scrollbar ---
        let is_sb_hovered = self.active_modal.is_none() && mouse_x >= sb_x && mouse_y >= editor_y && mouse_y < editor_y + editor_height;

        // Scrollbar Track background
        self.push_quad(
            vertices,
            indices,
            sb_x,
            editor_y,
            scrollbar_width,
            total_editor_height,
            white_uv,
            self.config.theme.scrollbar_track,
        );
        // Vertical track separator (left of scrollbar)
        self.push_quad(
            vertices,
            indices,
            sb_x - 1.0,
            editor_y,
            1.0,
            total_editor_height,
            white_uv,
            self.config.theme.scrollbar_border,
        );

        let track_h = editor_height;
        let ratio = visible_lines as f32 / buffer.len() as f32;
        let thumb_h = (track_h * ratio).clamp(20.0, track_h);
        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
        let scroll_ratio = if max_scroll > 0.0 { self.scroll_y as f32 / max_scroll } else { 0.0 };
        let thumb_y = editor_y + scroll_ratio * (track_h - thumb_h);

        let thumb_color = if is_sb_hovered {
            self.config.theme.scrollbar_thumb_hover
        } else {
            self.config.theme.scrollbar_thumb
        };

        // Draw Scrollbar Thumb
        self.push_quad(
            vertices,
            indices,
            sb_x + 2.0,
            thumb_y,
            scrollbar_width - 4.0,
            thumb_h,
            white_uv,
            thumb_color,
        );

        // --- 4.1 Draw Horizontal Scrollbar ---
        let max_line_len = self.get_max_line_len(buffer, active_file_path, cursor.line);
        let visible_cols = (text_viewport_w / self.buffer_char_width).floor() as usize;
        
        let hs_y = editor_y + editor_height;
        let hs_h = 14.0f32;
        let is_hs_hovered = self.active_modal.is_none()
            && mouse_x >= text_area_x
            && mouse_x < minimap_x
            && mouse_y >= hs_y
            && mouse_y < hs_y + hs_h;

        // Draw Horizontal Scrollbar Track Background
        self.push_quad(
            vertices,
            indices,
            text_area_x,
            hs_y,
            text_viewport_w,
            hs_h,
            white_uv,
            self.config.theme.scrollbar_track,
        );

        // Draw horizontal track border separator (top of horizontal scrollbar)
        self.push_quad(
            vertices,
            indices,
            text_area_x,
            hs_y,
            text_viewport_w,
            1.0,
            white_uv,
            self.config.theme.scrollbar_border,
        );

        if max_line_len > visible_cols {
            // Calculate horizontal scrollbar thumb
            let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
            let thumb_w = (text_viewport_w * ratio_x).clamp(20.0, text_viewport_w);
            let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
            let scroll_ratio_x = if max_scroll_x > 0.0 { self.scroll_x as f32 / max_scroll_x } else { 0.0 };
            let thumb_x = text_area_x + scroll_ratio_x * (text_viewport_w - thumb_w);

            let thumb_color_x = if is_hs_hovered {
                self.config.theme.scrollbar_thumb_hover
            } else {
                self.config.theme.scrollbar_thumb
            };

            // Draw Horizontal Scrollbar Thumb (height 10.0, padded by 2.0 from top and bottom)
            self.push_quad(
                vertices,
                indices,
                thumb_x,
                hs_y + 2.0,
                thumb_w,
                10.0,
                white_uv,
                thumb_color_x,
            );
        }

        // --- 4.5. Draw Minimap ---
        // Draw Minimap Track background
        self.push_quad(
            vertices,
            indices,
            minimap_x,
            editor_y,
            minimap_width,
            total_editor_height,
            white_uv,
            self.config.theme.editor_bg,
        );
        // Vertical border separating editor and minimap
        self.push_quad(
            vertices,
            indices,
            minimap_x - 1.0,
            editor_y,
            1.0,
            total_editor_height,
            white_uv,
            self.config.theme.scrollbar_border,
        );

        let minimap_line_height = (self.buffer_font_size * 0.22).round().max(2.0);
        let minimap_char_w = minimap_line_height * 0.5;
        let minimap_quad_h = (minimap_line_height - 1.0).max(1.0);

        let minimap_total_h = buffer.len() as f32 * minimap_line_height;
        let minimap_offset_y = if minimap_total_h > editor_height {
            scroll_ratio * (minimap_total_h - editor_height)
        } else {
            0.0
        };

        // Determine visible lines in the minimap to optimize rendering
        let start_line = ((minimap_offset_y - 2.0) / minimap_line_height).floor().max(0.0) as usize;
        let end_line = ((editor_height + minimap_offset_y) / minimap_line_height).ceil().max(0.0) as usize;
        let end_line = end_line.min(buffer.len());

        for line_idx in start_line..end_line {
            let row_y = editor_y + line_idx as f32 * minimap_line_height - minimap_offset_y;
            
            let line_text = &buffer.lines()[line_idx];
            let char_colors = self.get_line_char_colors(line_text);
            
            let mut current_col = 0.0f32;
            let mut start_x = 0.0f32;
            let mut current_color = None;
            let mut block_w = 0.0f32;
            
            for (char_idx, c) in line_text.chars().enumerate() {
                let char_w = if c == '\t' { 4.0 * minimap_char_w } else { minimap_char_w };
                let color = char_colors.get(char_idx).copied().unwrap_or(self.config.theme.syntax_default);
                let is_whitespace = c == ' ' || c == '\t';
                
                if is_whitespace {
                    if let Some(col) = current_color {
                        let draw_w = block_w.min(minimap_width - start_x);
                        if draw_w > 0.0 {
                            self.push_quad(
                                vertices,
                                indices,
                                minimap_x + start_x,
                                row_y,
                                draw_w,
                                minimap_quad_h,
                                white_uv,
                                col,
                            );
                        }
                        current_color = None;
                    }
                    current_col += char_w;
                } else {
                    if let Some(col) = current_color {
                        if col == color {
                            block_w += char_w;
                        } else {
                            let draw_w = block_w.min(minimap_width - start_x);
                            if draw_w > 0.0 {
                                self.push_quad(
                                    vertices,
                                    indices,
                                    minimap_x + start_x,
                                    row_y,
                                    draw_w,
                                    minimap_quad_h,
                                    white_uv,
                                    col,
                                );
                            }
                            start_x = current_col;
                            current_color = Some(color);
                            block_w = char_w;
                        }
                    } else {
                        start_x = current_col;
                        current_color = Some(color);
                        block_w = char_w;
                    }
                    current_col += char_w;
                }
                if current_col >= minimap_width {
                    break;
                }
            }
            if let Some(col) = current_color {
                let draw_w = block_w.min(minimap_width - start_x);
                if draw_w > 0.0 {
                    self.push_quad(
                        vertices,
                        indices,
                        minimap_x + start_x,
                        row_y,
                        draw_w,
                        minimap_quad_h,
                        white_uv,
                        col,
                    );
                }
            }
        }

        // Draw Viewport Indicator highlight overlay
        let highlight_y_start = self.scroll_y as f32 * minimap_line_height - minimap_offset_y;
        let highlight_h = (visible_lines as f32 * minimap_line_height).min(editor_height);
        
        let highlight_color = if self.config.theme.editor_bg[0] > 0.5 {
            [0.0, 0.0, 0.0, 0.08] // Light theme -> dark highlight
        } else {
            [1.0, 1.0, 1.0, 0.08] // Dark theme -> light highlight
        };

        self.push_quad(
            vertices,
            indices,
            minimap_x,
            editor_y + highlight_y_start,
            minimap_width,
            highlight_h,
            white_uv,
            highlight_color,
        );

        // --- 4.5. Draw Bottom Dock ---
        if self.show_dock {
            let dock_w = width - self.sidebar_width;
            let dock_h = (height - self.status_height - dock_start_y).max(0.0);
            
            // 1. Draw top border
            self.push_quad(
                vertices,
                indices,
                self.sidebar_width,
                dock_start_y,
                dock_w,
                1.0,
                white_uv,
                self.config.theme.tabbar_border,
            );

            // 2. Draw dock tab bar
            let dock_tabbar_h = 28.0f32;
            self.push_quad(
                vertices,
                indices,
                self.sidebar_width,
                dock_start_y + 1.0,
                dock_w,
                dock_tabbar_h - 1.0,
                white_uv,
                self.config.theme.tabbar_bg,
            );
            // 2.5. Calculate active terminal tab layout details beforehand
            let mut active_dock_x = 0.0f32;
            let mut active_dock_w = 0.0f32;
            let mut has_active_dock = false;
            let mut temp_x = self.sidebar_width + 10.0f32;
            for idx in 0..terminals.len() {
                let term_name = format!("terminal-{}", idx + 1);
                let term_name_w = term_name.chars().count() as f32 * self.ui_char_width * 0.9;
                let icon_sz = 12.0f32;
                let close_sz = 10.0f32;
                let tab_w = 12.0 + icon_sz + 6.0 + term_name_w + 8.0 + close_sz + 10.0;
                if idx == self.active_dock_tab {
                    active_dock_x = temp_x;
                    active_dock_w = tab_w;
                    has_active_dock = true;
                }
                temp_x += tab_w;
            }

            // Draw dock tabbar bottom border (skipping the active tab)
            let dock_tabbar_border_y = dock_start_y + dock_tabbar_h;
            let border_start_x = self.sidebar_width;
            if has_active_dock {
                if active_dock_x > border_start_x {
                    self.push_quad(
                        vertices,
                        indices,
                        border_start_x,
                        dock_tabbar_border_y,
                        active_dock_x - border_start_x,
                        1.0,
                        white_uv,
                        self.config.theme.tabbar_border,
                    );
                }
                let border_end_x = active_dock_x + active_dock_w;
                if border_end_x < width {
                    self.push_quad(
                        vertices,
                        indices,
                        border_end_x,
                        dock_tabbar_border_y,
                        width - border_end_x,
                        1.0,
                        white_uv,
                        self.config.theme.tabbar_border,
                    );
                }
            } else {
                self.push_quad(
                    vertices,
                    indices,
                    border_start_x,
                    dock_tabbar_border_y,
                    dock_w,
                    1.0,
                    white_uv,
                    self.config.theme.tabbar_border,
                );
            }

            // 3. Draw active/inactive terminal tabs
            let mut cur_x = self.sidebar_width + 10.0f32;
            let tab_y = dock_start_y + 1.0;
            let tab_h = dock_tabbar_h - 1.0;
            let tab_font_sz = self.ui_font_size * 0.9;

            for idx in 0..terminals.len() {
                let is_active = idx == self.active_dock_tab;
                let term_name = format!("terminal-{}", idx + 1);
                let term_name_w = term_name.chars().count() as f32 * self.ui_char_width * 0.9;
                let icon_sz = 12.0f32;
                let close_sz = 10.0f32;
                let tab_w = 12.0 + icon_sz + 6.0 + term_name_w + 8.0 + close_sz + 10.0;
                
                // Draw tab background
                let bg_color = if is_active {
                    self.config.theme.tab_active_bg
                } else {
                    self.config.theme.tabbar_bg
                };
                let current_tab_h = if is_active {
                    dock_tabbar_h
                } else {
                    dock_tabbar_h - 1.0
                };
                self.push_quad(vertices, indices, cur_x, tab_y, tab_w, current_tab_h, white_uv, bg_color);
                
                // Draw separators/borders like in editor tabbar
                if idx > 0 {
                    self.push_quad(
                        vertices,
                        indices,
                        cur_x,
                        tab_y,
                        1.0,
                        dock_tabbar_h,
                        white_uv,
                        self.config.theme.tabbar_border,
                    );
                }
                self.push_quad(
                    vertices,
                    indices,
                    cur_x + tab_w - 1.0,
                    tab_y,
                    1.0,
                    dock_tabbar_h,
                    white_uv,
                    self.config.theme.tabbar_border,
                );

                // Draw terminal icon
                let icon_color = if is_active {
                    self.config.theme.tab_text
                } else {
                    let mut c = self.config.theme.tab_text;
                    c[3] *= 0.6;
                    c
                };
                let cur_tab_h_for_calc = dock_tabbar_h - 1.0;
                self.push_icon(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "terminal",
                    cur_x + 8.0,
                    (tab_y + (cur_tab_h_for_calc - icon_sz) / 2.0).round(),
                    icon_color,
                    icon_sz,
                );

                // Draw text
                let tab_baseline = (tab_y + cur_tab_h_for_calc / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round();
                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &term_name,
                    cur_x + 8.0 + icon_sz + 6.0,
                    tab_baseline,
                    icon_color,
                    tab_font_sz,
                    self.ui_char_width * 0.9,
                );

                // Draw tab close button
                let close_x = cur_x + tab_w - 8.0 - close_sz;
                let close_y = (tab_y + (cur_tab_h_for_calc - close_sz) / 2.0).round();
                
                let is_close_hover = self.active_modal.is_none() && mouse_x >= close_x - 3.0 && mouse_x < close_x + close_sz + 3.0 && mouse_y >= close_y - 3.0 && mouse_y < close_y + close_sz + 3.0;
                let close_color = if is_close_hover {
                    [1.0, 0.3, 0.3, 1.0]
                } else {
                    let mut c = icon_color;
                    c[3] *= 0.5;
                    c
                };

                self.push_icon(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "close",
                    close_x,
                    close_y,
                    close_color,
                    close_sz,
                );

                cur_x += tab_w;
            }

            // Draw '+' button to add new terminal
            let add_btn_w = 28.0f32;
            let add_btn_x = cur_x;
            let is_add_hover = self.active_modal.is_none() && mouse_x >= add_btn_x && mouse_x < add_btn_x + add_btn_w && mouse_y >= tab_y && mouse_y < tab_y + tab_h;
            let add_bg = if is_add_hover {
                self.config.theme.titlebar_hover_bg
            } else {
                self.config.theme.tabbar_bg
            };
            self.push_quad(vertices, indices, add_btn_x, tab_y, add_btn_w, tab_h, white_uv, add_bg);
            
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                "+",
                add_btn_x + 10.0,
                (tab_y + tab_h / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                self.config.theme.tab_text,
                self.ui_font_size,
                self.ui_char_width,
            );

            // Draw Close dock button
            let close_dock_w = 28.0f32;
            let close_dock_x = width - 10.0 - close_dock_w;
            let is_close_dock_hover = self.active_modal.is_none() && mouse_x >= close_dock_x && mouse_x < close_dock_x + close_dock_w && mouse_y >= tab_y && mouse_y < tab_y + tab_h;
            let close_dock_bg = if is_close_dock_hover {
                self.config.theme.titlebar_hover_bg
            } else {
                self.config.theme.tabbar_bg
            };
            self.push_quad(vertices, indices, close_dock_x, tab_y, close_dock_w, tab_h, white_uv, close_dock_bg);
            self.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                "close",
                close_dock_x + 8.0,
                (tab_y + (tab_h - 12.0) / 2.0).round(),
                self.config.theme.tab_text,
                12.0,
            );

            // 4. Draw Terminal Content Area
            let content_y = dock_start_y + dock_tabbar_h + 1.0;
            let content_h = dock_h - dock_tabbar_h - 1.0;
            self.push_quad(
                vertices,
                indices,
                self.sidebar_width,
                content_y,
                dock_w,
                content_h,
                white_uv,
                self.config.theme.editor_bg,
            );

            // 5. Draw active terminal grid cells
            if !terminals.is_empty() {
                let term = &terminals[self.active_dock_tab.min(terminals.len() - 1)];
                let grid = &term.grid;
                
                let term_font_sz = self.buffer_font_size;
                let term_char_w = self.buffer_char_width;
                let term_line_h = self.buffer_line_height;
                let term_font_ascent = self.buffer_font_ascent;

                let term_pad_x = 8.0f32;
                let term_pad_y = 6.0f32;

                for ty in 0..grid.rows {
                    let cell_y = content_y + term_pad_y + ty as f32 * term_line_h;
                    if cell_y + term_line_h > content_y + content_h {
                        break;
                    }
                    
                    let cell_baseline = (cell_y + term_font_ascent).round();

                    for tx in 0..grid.cols {
                        let cell_x = self.sidebar_width + term_pad_x + tx as f32 * term_char_w;
                        if cell_x + term_char_w > width {
                            break;
                        }

                        let cell = grid.cells[ty * grid.cols + tx];

                        // Draw non-default background
                        if cell.bg != crate::terminal::DEFAULT_BG {
                            self.push_quad(
                                vertices,
                                indices,
                                cell_x,
                                cell_y,
                                term_char_w,
                                term_line_h,
                                white_uv,
                                cell.bg,
                            );
                        }

                        // Draw character if not space
                        if cell.c != ' ' {
                            let mut color = cell.fg;
                            if grid.bold && color == crate::terminal::DEFAULT_FG {
                                color = [1.0, 1.0, 1.0, 1.0];
                            }
                            
                            let mut buf = [0u8; 4];
                            let c_str = cell.c.encode_utf8(&mut buf);
                            
                            self.push_str(
                                vertices,
                                indices,
                                atlas,
                                queue,
                                c_str,
                                cell_x,
                                cell_baseline,
                                color,
                                term_font_sz,
                                term_char_w,
                            );
                        }
                    }
                }

                // Draw Cursor
                let cursor_x = self.sidebar_width + term_pad_x + grid.cursor_x as f32 * term_char_w;
                let cursor_y = content_y + term_pad_y + grid.cursor_y as f32 * term_line_h;
                
                if cursor_x + term_char_w <= width && cursor_y + term_line_h <= content_y + content_h {
                    if terminal_focus {
                        self.push_quad(
                            vertices,
                            indices,
                            cursor_x,
                            cursor_y,
                            term_char_w,
                            term_line_h,
                            white_uv,
                            [0.7, 0.7, 0.7, 0.6],
                        );
                    } else {
                        self.push_quad(vertices, indices, cursor_x, cursor_y, term_char_w, 1.5, white_uv, [0.6, 0.6, 0.6, 0.8]);
                        self.push_quad(vertices, indices, cursor_x, cursor_y + term_line_h - 1.5, term_char_w, 1.5, white_uv, [0.6, 0.6, 0.6, 0.8]);
                        self.push_quad(vertices, indices, cursor_x, cursor_y, 1.5, term_line_h, white_uv, [0.6, 0.6, 0.6, 0.8]);
                        self.push_quad(vertices, indices, cursor_x + term_char_w - 1.5, cursor_y, 1.5, term_line_h, white_uv, [0.6, 0.6, 0.6, 0.8]);
                    }
                }
            }
        }

        // TODO: refactor, rework this shit
        // --- 5. Draw Statusbar ---
        let status_y = height - self.status_height;
        self.push_quad(
            vertices,
            indices,
            0.0,
            status_y,
            width,
            self.status_height,
            white_uv,
            self.config.theme.statusbar_bg,
        );
        self.push_quad(
            vertices,
            indices,
            0.0,
            status_y,
            width,
            1.0,
            white_uv,
            self.config.theme.statusbar_border,
        );

        let status_left = format!(" GARAGE | Line {}, Col {}", cursor.line + 1, cursor.col + 1);
        let status_right = format!("Lines: {} | UTF-8 | LF ", buffer.len());
        let baseline_y = (status_y + self.status_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round();
        let text_color = self.config.theme.statusbar_text;
        
        let mut pen_x = 10.0;
        pen_x += self.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &status_left,
            pen_x,
            baseline_y,
            text_color,
            self.ui_font_size,
            self.ui_char_width,
        );

        if self.config.show_git_branch {
            if let Some(ref branch) = self.git_branch {
                pen_x += self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    " | ",
                    pen_x,
                    baseline_y,
                    text_color,
                    self.ui_font_size,
                    self.ui_char_width,
                );
                
                // Draw branch icon
                let icon_sz = (self.ui_font_size * 0.9).round().max(12.0);
                let icon_y_center = baseline_y - (self.ui_font_ascent * 0.33).round();
                let icon_y = icon_y_center - (icon_sz / 2.0).round();
                self.push_icon(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "branch",
                    pen_x,
                    icon_y,
                    text_color,
                    icon_sz,
                );
                pen_x += icon_sz + 4.0; // Space after icon
                
                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    branch,
                    pen_x,
                    baseline_y,
                    text_color,
                    self.ui_font_size,
                    self.ui_char_width,
                );
            }
        }

        let right_text_width = status_right.chars().count() as f32 * self.ui_char_width;
        let right_x = width - right_text_width - 15.0 - 36.0;
        if right_x > width / 2.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &status_right,
                right_x,
                (status_y + self.status_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                self.config.theme.statusbar_text,
                self.ui_font_size,
                self.ui_char_width,
            );
        }

        // Draw statusbar action buttons in the bottom right corner
        let sb_btn_w = 26.0f32;
        let sb_btn_h = self.status_height - 1.0;
        let icon_sz = 14.0f32;
        let icon_y = status_y + (sb_btn_h - icon_sz) / 2.0;

        let term_btn_x = width - 10.0 - sb_btn_w;

        // Check hovers
        let is_term_hover = self.active_modal.is_none() && mouse_y >= status_y && mouse_x >= term_btn_x && mouse_x < term_btn_x + sb_btn_w;

        // 1. Terminal Button
        let term_bg = if self.show_dock {
            [0.2, 0.5, 0.8, 0.35] // blue tint when open
        } else if is_term_hover {
            self.config.theme.titlebar_hover_bg
        } else {
            self.config.theme.statusbar_bg
        };
        self.push_quad(vertices, indices, term_btn_x, status_y + 1.0, sb_btn_w, sb_btn_h, white_uv, term_bg);
        let term_color = if self.show_dock { [0.3, 0.6, 0.95, 1.0] } else { self.config.theme.statusbar_text };
        self.push_icon(vertices, indices, atlas, queue, "terminal", term_btn_x + (sb_btn_w - icon_sz) / 2.0, icon_y, term_color, icon_sz);

        // --- 6. Draw Context Dropdown Menus (On top of everything) ---
        if let Some(menu) = self.active_menu {
            let items = match menu {
                MenuType::Garage => vec!["Settings", "About", "Exit"],
                MenuType::File => vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
                MenuType::Edit => vec!["Undo (Ctrl+Z)", "Redo (Ctrl+Y)"],
                MenuType::Selection => vec!["Select All", "Clear Selection"],
                MenuType::View => vec!["Toggle Sidebar", "Command Palette (Ctrl+Shift+P)"],
            };
            
            // Calculate dynamic menu_x matching the header position
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

            // Draw Dropdown Card Background
            self.push_quad(
                vertices,
                indices,
                menu_x,
                self.titlebar_height,
                dropdown_w,
                dropdown_h,
                white_uv,
                self.config.theme.modal_bg,
            );

            // Draw Item Hovers and text
            for (idx, label) in items.iter().enumerate() {
                let row_y = self.titlebar_height + idx as f32 * item_height;
                let is_hovered = mouse_x >= menu_x && mouse_x < menu_x + dropdown_w && mouse_y >= row_y && mouse_y < row_y + item_height;

                if is_hovered {
                    self.push_quad(
                        vertices,
                        indices,
                        menu_x,
                        row_y,
                        dropdown_w,
                        item_height,
                        white_uv,
                        self.config.theme.dropdown_hover_bg,
                    );
                }

                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    label,
                    menu_x + 12.0,
                    (row_y + item_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                    if is_hovered { [0.0, 0.0, 0.0, 1.0] } else { self.config.theme.modal_text_normal },
                    self.ui_font_size,
                    self.ui_char_width,
                );
            }

            // Draw Card Borders on top of everything (left, right, bottom)
            self.push_quad(
                vertices,
                indices,
                menu_x,
                self.titlebar_height,
                1.0,
                dropdown_h,
                white_uv,
                self.config.theme.modal_border,
            );
            self.push_quad(
                vertices,
                indices,
                menu_x + dropdown_w - 1.0,
                self.titlebar_height,
                1.0,
                dropdown_h,
                white_uv,
                self.config.theme.modal_border,
            );
            self.push_quad(
                vertices,
                indices,
                menu_x,
                self.titlebar_height + dropdown_h - 1.0,
                dropdown_w,
                1.0,
                white_uv,
                self.config.theme.modal_border,
            );
        }

        // --- 7. Draw Modal Dialogs (On top of dropdowns/everything) ---
        if let Some(modal) = self.active_modal {
            // Semi-transparent black background overlay
            self.push_quad(
                vertices,
                indices,
                0.0,
                0.0,
                width,
                height,
                white_uv,
                [0.0, 0.0, 0.0, 0.4],
            );
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

            // Draw Modal Box Background
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y,
                modal_w,
                modal_h,
                white_uv,
                self.config.theme.modal_bg,
            );
            // Draw modal borders
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y,
                modal_w,
                1.0,
                white_uv,
                self.config.theme.modal_border,
            );
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y + modal_h - 1.0,
                modal_w,
                1.0,
                white_uv,
                self.config.theme.modal_border,
            );
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y,
                1.0,
                modal_h,
                white_uv,
                self.config.theme.modal_border,
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + modal_w - 1.0,
                modal_y,
                1.0,
                modal_h,
                white_uv,
                self.config.theme.modal_border,
            );

            match modal {
                ModalType::CommandPalette => {
                    let input_y = modal_y + 15.0;
                    let prefix = "> ";
                    let mut input_text = prefix.to_string();
                    input_text.push_str(&self.command_palette_query);
                    
                    let text_color = self.config.theme.modal_text_normal;
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &input_text,
                        modal_x + 20.0,
                        (input_y + self.ui_font_ascent).round(),
                        text_color,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // Draw caret in the input box
                    let query_len = prefix.chars().count() + self.command_palette_query.chars().count();
                    let caret_x = modal_x + 20.0 + query_len as f32 * self.ui_char_width;
                    self.push_quad(
                        vertices,
                        indices,
                        caret_x,
                        input_y + 2.0,
                        2.0,
                        self.ui_line_height - 4.0,
                        white_uv,
                        self.config.theme.cursor_color,
                    );

                    // Draw horizontal separator below input
                    let sep_y = input_y + self.ui_line_height + 15.0;
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x,
                        sep_y,
                        modal_w,
                        1.0,
                        white_uv,
                        self.config.theme.modal_border,
                    );

                    // Draw Filtered List of Commands
                    let list_y = sep_y + 1.0;
                    let item_height = (self.ui_line_height * 1.6).round().max(26.0);
                    let max_visible_items = ((modal_y + modal_h - 15.0 - list_y) / item_height).floor() as usize;

                    let filtered = self.get_filtered_commands();
                    
                    // Automatically scroll selection into view
                    if max_visible_items > 0 {
                        if self.command_palette_selected < self.command_palette_scroll {
                            self.command_palette_scroll = self.command_palette_selected;
                        } else if self.command_palette_selected >= self.command_palette_scroll + max_visible_items {
                            self.command_palette_scroll = self.command_palette_selected + 1 - max_visible_items;
                        }
                    }

                    // Clamp scroll offset to valid bounds
                    let max_scroll = filtered.len().saturating_sub(max_visible_items);
                    self.command_palette_scroll = self.command_palette_scroll.min(max_scroll);

                    let start_idx = self.command_palette_scroll;
                    let end_idx = (self.command_palette_scroll + max_visible_items).min(filtered.len());

                    for idx in start_idx..end_idx {
                        let item = filtered[idx];
                        let item_y = list_y + (idx - self.command_palette_scroll) as f32 * item_height;
                        let is_selected = idx == self.command_palette_selected;

                        // Highlight selected command row
                        if is_selected {
                            self.push_quad(
                                vertices,
                                indices,
                                modal_x + 1.0,
                                item_y,
                                modal_w - 2.0,
                                item_height,
                                white_uv,
                                self.config.theme.sidebar_hover_bg,
                            );
                        }

                        // Left text: display name
                        let display_name = item.0;
                        let item_text_color = if is_selected {
                            self.config.theme.modal_text_title
                        } else {
                            self.config.theme.modal_text_normal
                        };

                        self.push_str(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            display_name,
                            modal_x + 20.0,
                            (item_y + item_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                            item_text_color,
                            self.ui_font_size,
                            self.ui_char_width,
                        );

                        // Right text: description (if room fits)
                        let desc = item.1;
                        let desc_color = self.config.theme.modal_text_muted;
                        let desc_len = desc.chars().count() as f32;
                        let desc_w = desc_len * self.ui_char_width;
                        let right_margin = if filtered.len() > max_visible_items { 25.0 } else { 20.0 };
                        let desc_x = modal_x + modal_w - right_margin - desc_w;
                        
                        let name_len = display_name.chars().count() as f32;
                        let name_w = name_len * self.ui_char_width;
                        
                        if desc_x > modal_x + name_w + 40.0 {
                            self.push_str(
                                vertices,
                                indices,
                                atlas,
                                queue,
                                desc,
                                desc_x,
                                (item_y + item_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                                desc_color,
                                self.ui_font_size,
                                self.ui_char_width,
                            );
                        }
                    }

                    // Draw scrollbar for command palette if needed
                    if filtered.len() > max_visible_items {
                        let track_x = modal_x + modal_w - 8.0;
                        let track_w = 4.0f32;
                        let track_h = max_visible_items as f32 * item_height;
                        
                        // Scrollbar track
                        self.push_quad(
                            vertices,
                            indices,
                            track_x,
                            list_y,
                            track_w,
                            track_h,
                            white_uv,
                            self.config.theme.scrollbar_track,
                        );
                        
                        let ratio = max_visible_items as f32 / filtered.len() as f32;
                        let thumb_h = (track_h * ratio).clamp(15.0, track_h);
                        let scroll_ratio = self.command_palette_scroll as f32 / max_scroll as f32;
                        let thumb_y = list_y + scroll_ratio * (track_h - thumb_h);
                        
                        // Scrollbar thumb
                        self.push_quad(
                            vertices,
                            indices,
                            track_x,
                            thumb_y,
                            track_w,
                            thumb_h,
                            white_uv,
                            self.config.theme.scrollbar_thumb,
                        );
                    }
                }
                ModalType::UnsavedChanges => {
                    let file_name = self.tab_to_close
                        .and_then(|idx| tab_paths.get(idx).cloned())
                        .flatten()
                        .and_then(|p| Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_else(|| "untitled.txt".to_string());

                    let title_text = "Unsaved Changes";
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        title_text,
                        modal_x + 20.0,
                        modal_y + 35.0,
                        self.config.theme.modal_text_title,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    let mut truncated_name = file_name.clone();
                    if truncated_name.chars().count() > 20 {
                        let prefix: String = truncated_name.chars().take(17).collect();
                        truncated_name = format!("{}...", prefix);
                    }
                    let msg_text = format!("'{}' has unsaved changes.", truncated_name);
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &msg_text,
                        modal_x + 20.0,
                        modal_y + 70.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    let msg_text_2 = "Save changes before closing?";
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        msg_text_2,
                        modal_x + 20.0,
                        modal_y + 92.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    let btn_w = 130.0f32;
                    let btn_h = 34.0f32;
                    let spacing = 15.0f32;

                    let total_btn_block_w = 3.0 * btn_w + 2.0 * spacing;
                    let start_btn_x = modal_x + ((modal_w - total_btn_block_w) / 2.0).round();
                    let btn_y = modal_y + modal_h - btn_h - 20.0;

                    let btn_labels = ["Save", "Don't Save", "Cancel"];
                    for i in 0..3 {
                        let bx = start_btn_x + i as f32 * (btn_w + spacing);
                        let is_btn_hovered = mouse_x >= bx && mouse_x <= bx + btn_w && mouse_y >= btn_y && mouse_y <= btn_y + btn_h;

                        self.push_quad(
                            vertices,
                            indices,
                            bx,
                            btn_y,
                            btn_w,
                            btn_h,
                            white_uv,
                            if is_btn_hovered { self.config.theme.button_hover_bg } else { self.config.theme.button_bg },
                        );
                        self.push_quad(vertices, indices, bx, btn_y, btn_w, 1.0, white_uv, self.config.theme.button_border);
                        self.push_quad(vertices, indices, bx, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, self.config.theme.button_border);
                        self.push_quad(vertices, indices, bx, btn_y, 1.0, btn_h, white_uv, self.config.theme.button_border);
                        self.push_quad(vertices, indices, bx + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, self.config.theme.button_border);

                        let label = btn_labels[i];
                        let label_w = label.chars().count() as f32 * self.ui_char_width;
                        let tx = bx + ((btn_w - label_w) / 2.0).round();
                        let ty = (btn_y + btn_h / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round();

                        self.push_str(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            label,
                            tx,
                            ty,
                            self.config.theme.button_text,
                            self.ui_font_size,
                            self.ui_char_width,
                        );
                    }
                }
                ModalType::About => {
                    let title = "Garage";
                    let title_font_sz = self.ui_font_size * 1.5;
                    let title_char_w = self.ui_char_width * 1.5;
                    let title_w = title.chars().count() as f32 * title_char_w;
                    let title_x = modal_x + ((modal_w - title_w) / 2.0).round();
                    
                    // 1. Draw Title "Garage"
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        title,
                        title_x,
                        modal_y + 35.0,
                        self.config.theme.modal_text_title,
                        title_font_sz,
                        title_char_w,
                    );

                    // 2. Draw thin divider line
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 30.0,
                        modal_y + 55.0,
                        modal_w - 60.0,
                        1.0,
                        white_uv,
                        self.config.theme.tabbar_border,
                    );

                    // 3. Draw description
                    let desc = "A supercharged GPU-accelerated text editor.";
                    let desc_w = desc.chars().count() as f32 * self.ui_char_width;
                    let desc_x = modal_x + ((modal_w - desc_w) / 2.0).round();
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        desc,
                        desc_x,
                        modal_y + 80.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // 4. Version
                    let version = "Version 0.1.0 (main)";
                    let version_w = version.chars().count() as f32 * self.ui_char_width * 0.9;
                    let version_x = modal_x + ((modal_w - version_w) / 2.0).round();
                    let mut muted_text_color = self.config.theme.modal_text_normal;
                    muted_text_color[3] *= 0.6; // Mute color alpha
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        version,
                        version_x,
                        modal_y + 115.0,
                        muted_text_color,
                        self.ui_font_size * 0.9,
                        self.ui_char_width * 0.9,
                    );
                }
                ModalType::Settings => {
                    let row_height = (self.ui_line_height * 2.2).round();
                    let control_x = modal_x + 24.0 * self.ui_char_width;
                    let btn_h = (self.ui_line_height * 1.3).round().max(24.0);
                    let btn_w = (self.ui_char_width * 3.0).round().max(24.0);
                    let backend_btn_w = (self.ui_char_width * 10.0).round().max(80.0);
                    let theme_btn_w = (self.ui_char_width * 16.0).round().max(140.0);
                    let padding_x = 2.0 * self.ui_char_width;

                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "SETTINGS",
                        modal_x + padding_x,
                        modal_y + (self.ui_line_height * 1.8).round(),
                        self.config.theme.modal_text_title,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // Helper closure to draw button container with borders and label
                    let draw_button = |
                        vertices: &mut Vec<Vertex>,
                        indices: &mut Vec<u16>,
                        atlas: &mut FontAtlas,
                        queue: &wgpu::Queue,
                        text: &str,
                        bx: f32,
                        by: f32,
                        bw: f32,
                        bh: f32,
                        is_selected: bool,
                        is_hovered: bool,
                        theme: &crate::config::Theme,
                        white_uv: [f32; 2],
                        ui_char_width: f32,
                        ui_font_ascent: f32,
                        ui_font_size: f32,
                    | {
                        let bg_color = if is_selected {
                            theme.cursor_color // brand color
                        } else if is_hovered {
                            theme.button_hover_bg
                        } else {
                            theme.button_bg
                        };
                        let border_color = if is_selected {
                            theme.cursor_color
                        } else {
                            theme.button_border
                        };
                        let text_color = if is_selected {
                            [1.0, 1.0, 1.0, 1.0]
                        } else {
                            theme.button_text
                        };

                        // Draw background
                        self.push_quad(vertices, indices, bx, by, bw, bh, white_uv, bg_color);
                        // Draw borders (contiguous)
                        self.push_quad(vertices, indices, bx, by, bw, 1.0, white_uv, border_color); // Top
                        self.push_quad(vertices, indices, bx, by + bh - 1.0, bw, 1.0, white_uv, border_color); // Bottom
                        self.push_quad(vertices, indices, bx, by, 1.0, bh, white_uv, border_color); // Left
                        self.push_quad(vertices, indices, bx + bw - 1.0, by, 1.0, bh, white_uv, border_color); // Right

                        // Draw text centered
                        let text_w = text.chars().count() as f32 * ui_char_width;
                        let text_x = bx + ((bw - text_w) / 2.0).round();
                        let text_y = (by + bh / 2.0 + ui_font_ascent / 2.0 - 2.0).round();
                        self.push_str(vertices, indices, atlas, queue, text, text_x, text_y, text_color, ui_font_size, ui_char_width);
                    };

                    // 1. Editor Font Size Settings
                    let row1_y = modal_y + row_height * 1.0;
                    let btn1_y = row1_y + ((self.ui_line_height - btn_h) / 2.0).round();
                    let font_size_str = format!("Editor Font: {:.1} px", self.buffer_font_size);
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &font_size_str,
                        modal_x + padding_x,
                        row1_y + self.ui_font_ascent,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let dec_hover = mouse_x >= control_x && mouse_x <= control_x + btn_w && mouse_y >= btn1_y && mouse_y <= btn1_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "-", control_x, btn1_y, btn_w, btn_h, false, dec_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);
                    let inc_btn_x = control_x + btn_w + self.ui_char_width;
                    let inc_hover = mouse_x >= inc_btn_x && mouse_x <= inc_btn_x + btn_w && mouse_y >= btn1_y && mouse_y <= btn1_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "+", inc_btn_x, btn1_y, btn_w, btn_h, false, inc_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 2. UI Font Size Settings
                    let row2_y = modal_y + row_height * 2.0;
                    let btn2_y = row2_y + ((self.ui_line_height - btn_h) / 2.0).round();
                    let ui_size_str = format!("UI Font:     {:.1} px", self.ui_font_size);
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &ui_size_str,
                        modal_x + padding_x,
                        row2_y + self.ui_font_ascent,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let ui_dec_hover = mouse_x >= control_x && mouse_x <= control_x + btn_w && mouse_y >= btn2_y && mouse_y <= btn2_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "-", control_x, btn2_y, btn_w, btn_h, false, ui_dec_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);
                    let ui_inc_hover = mouse_x >= inc_btn_x && mouse_x <= inc_btn_x + btn_w && mouse_y >= btn2_y && mouse_y <= btn2_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "+", inc_btn_x, btn2_y, btn_w, btn_h, false, ui_inc_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 3. Backend Selection
                    let row3_y = modal_y + row_height * 3.0;
                    let btn3_y = row3_y + ((self.ui_line_height - btn_h) / 2.0).round();
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Backend:",
                        modal_x + padding_x,
                        row3_y + self.ui_font_ascent,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let is_vulkan = self.config.backend == "Vulkan";
                    let vulkan_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn3_y && mouse_y <= btn3_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "Vulkan", control_x, btn3_y, backend_btn_w, btn_h, is_vulkan, vulkan_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let is_opengl = self.config.backend == "OpenGL";
                    let opengl_btn_x = control_x + backend_btn_w + self.ui_char_width;
                    let opengl_hover = mouse_x >= opengl_btn_x && mouse_x <= opengl_btn_x + backend_btn_w && mouse_y >= btn3_y && mouse_y <= btn3_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "OpenGL", opengl_btn_x, btn3_y, backend_btn_w, btn_h, is_opengl, opengl_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 4. Theme Selection (Cycle Toggle Selector)
                    let row4_y = modal_y + row_height * 4.0;
                    let btn4_y = row4_y + ((self.ui_line_height - btn_h) / 2.0).round();
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Theme:",
                        modal_x + padding_x,
                        row4_y + self.ui_font_ascent,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let display_theme = format!("{}  ▼", self.config.theme.name);
                    let theme_hover = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= btn4_y && mouse_y <= btn4_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, &display_theme, control_x, btn4_y, theme_btn_w, btn_h, false, theme_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 5. Git Blame Selection
                    let row5_y = modal_y + row_height * 5.0;
                    let btn5_y = row5_y + ((self.ui_line_height - btn_h) / 2.0).round();
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Git Blame:",
                        modal_x + padding_x,
                        row5_y + self.ui_font_ascent,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let blame_enabled_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn5_y && mouse_y <= btn5_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "Enabled", control_x, btn5_y, backend_btn_w, btn_h, self.config.show_git_blame, blame_enabled_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let blame_disabled_x = control_x + backend_btn_w + self.ui_char_width;
                    let blame_disabled_hover = mouse_x >= blame_disabled_x && mouse_x <= blame_disabled_x + backend_btn_w && mouse_y >= btn5_y && mouse_y <= btn5_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "Disabled", blame_disabled_x, btn5_y, backend_btn_w, btn_h, !self.config.show_git_blame, blame_disabled_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 6. Git Branch Selection
                    let row6_y = modal_y + row_height * 6.0;
                    let btn6_y = row6_y + ((self.ui_line_height - btn_h) / 2.0).round();
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Git Branch:",
                        modal_x + padding_x,
                        row6_y + self.ui_font_ascent,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let branch_enabled_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn6_y && mouse_y <= btn6_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "Enabled", control_x, btn6_y, backend_btn_w, btn_h, self.config.show_git_branch, branch_enabled_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let branch_disabled_x = control_x + backend_btn_w + self.ui_char_width;
                    let branch_disabled_hover = mouse_x >= branch_disabled_x && mouse_x <= branch_disabled_x + backend_btn_w && mouse_y >= btn6_y && mouse_y <= btn6_y + btn_h;
                    draw_button(vertices, indices, atlas, queue, "Disabled", branch_disabled_x, btn6_y, backend_btn_w, btn_h, !self.config.show_git_branch, branch_disabled_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 7. Draw Active backend and GPU info
                    let row7_y = modal_y + row_height * 7.0;
                    let backend_str = match current_backend {
                        wgpu::Backend::Vulkan => "Vulkan",
                        wgpu::Backend::Gl => "OpenGL",
                        other => &format!("{:?}", other),
                    };
                    let is_fallback = (self.config.backend == "OpenGL" && current_backend != wgpu::Backend::Gl) ||
                                      (self.config.backend == "Vulkan" && current_backend != wgpu::Backend::Vulkan);
                    let active_info_str = if is_fallback {
                        format!("Active: {} (fallback) ({})", backend_str, self.active_device_name)
                    } else {
                        format!("Active: {} ({})", backend_str, self.active_device_name)
                    };
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &active_info_str,
                        modal_x + padding_x,
                        row7_y + self.ui_font_ascent,
                        self.config.theme.modal_text_muted,
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // 6. Draw Theme Selection Dropdown (if open) - rendered on top of active info
                    if self.theme_dropdown_open {
                        let dropdown_y = btn4_y + btn_h;
                        let item_height = (self.ui_line_height * 1.5).round().max(24.0);
                        let dropdown_h = 2.0 * item_height;

                        // Draw Dropdown background
                        self.push_quad(vertices, indices, control_x, dropdown_y, theme_btn_w, dropdown_h, white_uv, self.config.theme.modal_bg);
                        // Draw Dropdown borders
                        self.push_quad(vertices, indices, control_x, dropdown_y, theme_btn_w, 1.0, white_uv, self.config.theme.modal_border);
                        self.push_quad(vertices, indices, control_x, dropdown_y + dropdown_h - 1.0, theme_btn_w, 1.0, white_uv, self.config.theme.modal_border);
                        self.push_quad(vertices, indices, control_x, dropdown_y, 1.0, dropdown_h, white_uv, self.config.theme.modal_border);
                        self.push_quad(vertices, indices, control_x + theme_btn_w - 1.0, dropdown_y, 1.0, dropdown_h, white_uv, self.config.theme.modal_border);

                        let themes = ["Light Theme", "Dark Theme"];
                        for (idx, t_name) in themes.iter().enumerate() {
                            let item_y = dropdown_y + idx as f32 * item_height;
                            let is_item_hovered = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= item_y && mouse_y < item_y + item_height;

                            if is_item_hovered {
                                self.push_quad(vertices, indices, control_x + 1.0, item_y + 1.0, theme_btn_w - 2.0, item_height - 2.0, white_uv, self.config.theme.button_hover_bg);
                            }

                            let text_y = (item_y + item_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round();
                            self.push_str(
                                vertices,
                                indices,
                                atlas,
                                queue,
                                t_name,
                                control_x + 10.0,
                                text_y,
                                self.config.theme.modal_text_normal,
                                self.ui_font_size,
                                self.ui_char_width,
                            );
                        }
                    }
                }
            }

            if modal != ModalType::CommandPalette && modal != ModalType::UnsavedChanges {
                // Draw generic Close Button (centered horizontally)
                let btn_w = (12.0 * self.ui_char_width).max(100.0).round();
                let btn_h = (self.ui_line_height * 1.6).max(30.0).round();
                let btn_x = modal_x + ((modal_w - btn_w) / 2.0).round();
                let btn_y = modal_y + modal_h - btn_h - (self.ui_line_height * 1.0).round();

                let close_btn_hover = mouse_x >= btn_x && mouse_x <= btn_x + btn_w && mouse_y >= btn_y && mouse_y <= btn_y + btn_h;
                self.push_quad(
                    vertices,
                    indices,
                    btn_x,
                    btn_y,
                    btn_w,
                    btn_h,
                    white_uv,
                    if close_btn_hover { self.config.theme.button_hover_bg } else { self.config.theme.button_bg },
                );
                // Draw borders
                self.push_quad(vertices, indices, btn_x, btn_y, btn_w, 1.0, white_uv, self.config.theme.button_border);
                self.push_quad(vertices, indices, btn_x, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, self.config.theme.button_border);
                self.push_quad(vertices, indices, btn_x, btn_y, 1.0, btn_h, white_uv, self.config.theme.button_border);
                self.push_quad(vertices, indices, btn_x + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, self.config.theme.button_border);

                let close_text = "Close";
                let close_text_w = close_text.chars().count() as f32 * self.ui_char_width;
                let close_text_x = btn_x + ((btn_w - close_text_w) / 2.0).round();
                let close_text_y = (btn_y + btn_h / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round();

                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    close_text,
                    close_text_x,
                    close_text_y,
                    self.config.theme.button_text,
                    self.ui_font_size,
                    self.ui_char_width,
                );
            }
        }
    }
}
