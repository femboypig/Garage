use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;

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
    CloseModal,
    ChangeBufferFontSize(f32),
    ChangeUiFontSize(f32),
    ChangeBackend(wgpu::Backend),
    ChangeSidebarWidth(f32),
    ChangeTheme(String),
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

    pub config: crate::config::AppConfig,
    pub active_device_name: String,
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

        let titlebar_height = (ui_line_height * 1.8).round().max(32.0);
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
            config,
            active_device_name: String::new(),
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
        self.titlebar_height = (self.ui_line_height * 1.8).round().max(32.0);
        self.status_height = (self.ui_line_height * 1.5).round().max(24.0);
        self.tabbar_height = (self.ui_line_height * 1.6).round().max(30.0);
        self.breadcrumb_height = (self.ui_line_height * 1.3).round().max(22.0);
    }

    pub fn scroll_to_cursor(&mut self, cursor: &Cursor, buffer_len: usize, height: f32) {
        let editor_height = height - self.titlebar_height - self.status_height - self.tabbar_height - self.breadcrumb_height;
        let visible_lines = (editor_height / self.buffer_line_height).floor() as usize;
        if visible_lines == 0 {
            return;
        }
        if cursor.line < self.scroll_y {
            self.scroll_y = cursor.line;
        } else if cursor.line >= self.scroll_y + visible_lines {
            self.scroll_y = cursor.line - visible_lines + 1;
        }
        let max_scroll = (buffer_len as isize - visible_lines as isize).max(0) as usize;
        self.scroll_y = self.scroll_y.min(max_scroll);
    }

    /// Re-scan the directory to populate the project tree
    pub fn rebuild_tree(&mut self) {
        self.visible_nodes.clear();
        self.scan_dir_recursive(Path::new("."), 0);
    }

    fn scan_dir_recursive(&mut self, dir: &Path, depth: usize) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut entries_vec = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    
                    // Skip the .git directory to keep the explorer clean
                    if name == ".git" {
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
    ) -> UiAction {
        // If a modal is open, check click boundaries and buttons
        if let Some(modal) = self.active_modal {
            let modal_w = match modal {
                ModalType::Settings => 500.0,
                ModalType::About => 400.0,
            };
            let modal_h = match modal {
                ModalType::Settings => 360.0,
                ModalType::About => 240.0,
            };
            let modal_x = ((width - modal_w) / 2.0).round();
            let modal_y = ((height - modal_h) / 2.0).round();

            // Check if clicked close button (centered horizontally)
            let btn_x = modal_x + (modal_w - 120.0) / 2.0;
            let inside_close_btn = mx >= btn_x && mx <= btn_x + 120.0 && my >= modal_y + modal_h - 60.0 && my <= modal_y + modal_h - 25.0;
            let clicked_outside = mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h;

            if inside_close_btn || clicked_outside {
                self.active_modal = None;
                return UiAction::CloseModal;
            }

            if modal == ModalType::Settings {
                // Row 1: Editor Font Size [-] and [+]
                // Decrease [-] at 240..270, 55..80
                if mx >= modal_x + 240.0 && mx <= modal_x + 270.0 && my >= modal_y + 55.0 && my <= modal_y + 80.0 {
                    return UiAction::ChangeBufferFontSize(-1.0);
                }
                // Increase [+] at 280..310, 55..80
                if mx >= modal_x + 280.0 && mx <= modal_x + 310.0 && my >= modal_y + 55.0 && my <= modal_y + 80.0 {
                    return UiAction::ChangeBufferFontSize(1.0);
                }

                // Row 2: UI Font Size [-] and [+]
                // Decrease [-] at 240..270, 95..120
                if mx >= modal_x + 240.0 && mx <= modal_x + 270.0 && my >= modal_y + 95.0 && my <= modal_y + 120.0 {
                    return UiAction::ChangeUiFontSize(-1.0);
                }
                // Increase [+] at 280..310, 95..120
                if mx >= modal_x + 280.0 && mx <= modal_x + 310.0 && my >= modal_y + 95.0 && my <= modal_y + 120.0 {
                    return UiAction::ChangeUiFontSize(1.0);
                }

                // Row 3: Sidebar Width [-] and [+]
                // Decrease [-] at 240..270, 135..160
                if mx >= modal_x + 240.0 && mx <= modal_x + 270.0 && my >= modal_y + 135.0 && my <= modal_y + 160.0 {
                    return UiAction::ChangeSidebarWidth(-20.0);
                }
                // Increase [+] at 280..310, 135..160
                if mx >= modal_x + 280.0 && mx <= modal_x + 310.0 && my >= modal_y + 135.0 && my <= modal_y + 160.0 {
                    return UiAction::ChangeSidebarWidth(20.0);
                }

                // Row 4: Backend Selection
                // Vulkan Button at 240..330, 180..210
                if mx >= modal_x + 240.0 && mx <= modal_x + 330.0 && my >= modal_y + 180.0 && my <= modal_y + 210.0 {
                    return UiAction::ChangeBackend(wgpu::Backend::Vulkan);
                }
                // OpenGL Button at 340..430, 180..210
                if mx >= modal_x + 340.0 && mx <= modal_x + 430.0 && my >= modal_y + 180.0 && my <= modal_y + 210.0 {
                    return UiAction::ChangeBackend(wgpu::Backend::Gl);
                }

                // Row 5: Theme Selection
                // Light at 110..190, 230..260
                if mx >= modal_x + 110.0 && mx <= modal_x + 190.0 && my >= modal_y + 230.0 && my <= modal_y + 260.0 {
                    return UiAction::ChangeTheme("Light Theme".to_string());
                }
                // Dark at 200..280, 230..260
                if mx >= modal_x + 200.0 && mx <= modal_x + 280.0 && my >= modal_y + 230.0 && my <= modal_y + 260.0 {
                    return UiAction::ChangeTheme("Dark Theme".to_string());
                }
                // Solarized at 290..380, 230..260
                if mx >= modal_x + 290.0 && mx <= modal_x + 380.0 && my >= modal_y + 230.0 && my <= modal_y + 260.0 {
                    return UiAction::ChangeTheme("Solarized Dark".to_string());
                }
                // Cyberpunk at 390..480, 230..260
                if mx >= modal_x + 390.0 && mx <= modal_x + 480.0 && my >= modal_y + 230.0 && my <= modal_y + 260.0 {
                    return UiAction::ChangeTheme("Cyberpunk".to_string());
                }
            }

            return UiAction::None;
        }

        // 1. Check Titlebar Menu Clicks (Contiguous adjacent layout)
        if my < self.titlebar_height {
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
                MenuType::View => vec!["Toggle Sidebar"],
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

        // 3. Check Tabbar Clicks (including control buttons on the right)
        let main_y = self.titlebar_height;
        if my >= main_y && my < main_y + self.tabbar_height {
            let control_btns_raw = [
                ("About", UiAction::ShowAbout),
                ("Settings", UiAction::ShowSettings),
                ("Search", UiAction::None),
            ];
            let mut btn_x = width - 15.0;
            for (label, action) in &control_btns_raw {
                let label_len = label.chars().count() as f32;
                let text_w = label_len * self.ui_char_width;
                let item_w = text_w + 16.0;
                btn_x -= item_w;
                if mx >= btn_x && mx < btn_x + item_w {
                    self.active_menu = None;
                    return action.clone();
                }
            }
            self.active_menu = None;
            return UiAction::None;
        }

        // 4. Check Sidebar Clicks
        if self.sidebar_width > 0.0 && mx < self.sidebar_width && my > main_y && my < height - self.status_height {
            let tree_y = my - main_y;
            let row_idx = (tree_y / self.ui_line_height).floor() as usize;
            if row_idx >= 1 {
                let node_idx = row_idx - 1;
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
    ) {
        for c in text.chars() {
            x += self.push_char(vertices, indices, atlas, queue, c, x, y, color, font_size, char_width);
        }
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
    ) {
        let white_uv = atlas.white_pixel_uv();
        let main_y = self.titlebar_height;
        let main_height = height - self.titlebar_height - self.status_height;

        // Instant expand/collapse sidebar width (no animation delay)
        self.sidebar_width = self.target_sidebar_width;

        // Calculate dynamic layouts
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * self.buffer_char_width;
        let text_area_x = self.sidebar_width + gutter_width;
        
        let scrollbar_width = 12.0;
        let _text_viewport_w = width - text_area_x - scrollbar_width;

        let editor_y = main_y + self.tabbar_height + self.breadcrumb_height;
        let editor_height = main_height - self.tabbar_height - self.breadcrumb_height;
        let visible_lines = (editor_height / self.buffer_line_height).floor() as usize;
        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as usize;
        self.scroll_y = self.scroll_y.min(max_scroll);

        // --- 1. Draw Titlebar Menu Headers (Light Theme) ---
        self.push_quad(
            vertices,
            indices,
            0.0,
            0.0,
            width,
            self.titlebar_height,
            white_uv,
            self.config.theme.titlebar_bg,
        );
        self.push_quad(
            vertices,
            indices,
            0.0,
            self.titlebar_height - 1.0,
            width,
            1.0,
            white_uv,
            self.config.theme.titlebar_border,
        );

        let menu_items_raw = [
            ("Garage", MenuType::Garage),
            ("File", MenuType::File),
            ("Edit", MenuType::Edit),
            ("Selection", MenuType::Selection),
            ("View", MenuType::View),
        ];

        let mut menu_positions = Vec::new();
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
            menu_positions.push((*label, x_min, x_max, left_pad, *menu_type));
            current_x = x_max;
        }

        for (label, x_min, x_max, left_pad, menu_type) in &menu_positions {
            let is_hovered = self.active_modal.is_none() && mouse_y < self.titlebar_height && mouse_x >= *x_min && mouse_x < *x_max;
            let is_active = self.active_menu == Some(*menu_type);

            if is_hovered || is_active {
                self.push_quad(
                    vertices,
                    indices,
                    *x_min,
                    0.0,
                    *x_max - *x_min,
                    self.titlebar_height - 1.0,
                    white_uv,
                    self.config.theme.titlebar_hover_bg,
                );
            }
            
            let label_color = if *menu_type == MenuType::Garage {
                self.config.theme.titlebar_brand_text
            } else if is_active || is_hovered {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                self.config.theme.titlebar_text
            };

            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                label,
                *x_min + *left_pad,
                (self.titlebar_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                label_color,
                self.ui_font_size,
                self.ui_char_width,
            );
        }

        // Display current open file title in titlebar center
        let file_name = self.selected_file.as_ref()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        let title_text = format!("Garage Code Editor - {}", file_name);
        let title_len = title_text.chars().count() as f32;
        let title_x = ((width - title_len * self.ui_char_width) / 2.0).round();
        if title_x > current_x + 20.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &title_text,
                title_x,
                (self.titlebar_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                self.config.theme.titlebar_text,
                self.ui_font_size,
                self.ui_char_width,
            );
        }

        // --- 2. Draw Sidebar Panel (Light Theme) ---
        if self.sidebar_width > 0.0 {
            self.push_quad(
                vertices,
                indices,
                0.0,
                main_y,
                self.sidebar_width,
                main_height,
                white_uv,
                self.config.theme.sidebar_bg,
            );
            self.push_quad(
                vertices,
                indices,
                self.sidebar_width - 1.0,
                main_y,
                1.0,
                main_height,
                white_uv,
                self.config.theme.sidebar_border,
            );            // Draw sidebar title header (root project directory name in uppercase)
            let root_name = std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string().to_uppercase()))
                .unwrap_or_else(|| "PROJECT".to_string());
            let sidebar_header_text = format!(" {}", root_name);

            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &sidebar_header_text,
                10.0,
                (main_y + self.ui_line_height / 2.0 + self.ui_font_ascent / 2.0 - 1.0).round(),
                self.config.theme.sidebar_text_dir,
                self.ui_font_size,
                self.ui_char_width,
            );
            for (idx, node) in self.visible_nodes.iter().enumerate() {
                // Shift down by 1 row to accommodate the sidebar header
                let row_y = main_y + (idx + 1) as f32 * self.ui_line_height;
                if row_y + self.ui_line_height > main_y + main_height {
                    break;
                }

                let is_hovered = self.active_modal.is_none() && mouse_x < self.sidebar_width && mouse_y >= row_y && mouse_y < row_y + self.ui_line_height;
                let is_selected = self.selected_file.as_ref() == Some(&node.path);

                if is_hovered || is_selected {
                    self.push_quad(
                        vertices,
                        indices,
                        0.0,
                        row_y,
                        self.sidebar_width - 1.0,
                        self.ui_line_height,
                        white_uv,
                        if is_selected { self.config.theme.sidebar_selected_bg } else { self.config.theme.sidebar_hover_bg },
                    );
                }

                let indent_x = 10.0 + node.depth as f32 * 12.0;
                let icon = if node.is_dir {
                    if self.expanded_dirs.contains(&node.path) { "▼ " } else { "▶ " }
                } else {
                    "  "
                };

                let text_color = if node.is_dir {
                    self.config.theme.sidebar_text_dir
                } else {
                    self.config.theme.sidebar_text_file
                };

                let node_text = format!("{}{}", icon, node.name);
                let max_w = self.sidebar_width - indent_x - 10.0;
                if max_w > 0.0 {
                    let max_chars = (max_w / self.ui_char_width).floor() as usize;
                    let truncated_text: String = if node_text.chars().count() > max_chars {
                        if max_chars > 3 {
                            let mut s: String = node_text.chars().take(max_chars - 3).collect();
                            s.push_str("...");
                            s
                        } else {
                            node_text.chars().take(max_chars).collect()
                        }
                    } else {
                        node_text
                    };
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &truncated_text,
                        indent_x,
                        (row_y + self.ui_line_height / 2.0 + self.ui_font_ascent / 2.0 - 1.0).round(),
                        text_color,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                }
            }
        }

        // --- Tab Bar & Control Buttons (New) ---
        // Tab Bar background (gray)
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            main_y,
            width - self.sidebar_width,
            self.tabbar_height,
            white_uv,
            self.config.theme.tabbar_bg,
        );
        // Tab bar bottom border
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            main_y + self.tabbar_height - 1.0,
            width - self.sidebar_width,
            1.0,
            white_uv,
            self.config.theme.tabbar_border,
        );

        // Draw active file tab
        let tab_w = (file_name.chars().count() as f32 * self.ui_char_width + 40.0).max(120.0);
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            main_y,
            tab_w,
            self.tabbar_height - 1.0,
            white_uv,
            self.config.theme.tab_active_bg,
        );
        // Active tab right border
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width + tab_w - 1.0,
            main_y,
            1.0,
            self.tabbar_height,
            white_uv,
            self.config.theme.tabbar_border,
        );
        // Active tab label
        self.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &file_name,
            self.sidebar_width + 15.0,
            (main_y + self.tabbar_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
            self.config.theme.tab_text,
            self.ui_font_size,
            self.ui_char_width,
        );

        // Draw control buttons on the right of the tab bar
        // Layout buttons: Search, Settings, About from right to left
        let control_btns_raw = [
            ("About", UiAction::ShowAbout),
            ("Settings", UiAction::ShowSettings),
            ("Search", UiAction::None),
        ];

        let mut btn_x = width - 15.0;
        let mut control_btns = Vec::new();
        for (label, action) in &control_btns_raw {
            let label_len = label.chars().count() as f32;
            let text_w = label_len * self.ui_char_width;
            let item_w = text_w + 16.0; // 8px left and right padding
            btn_x -= item_w;
            control_btns.push((*label, btn_x, item_w, action.clone()));
        }

        for (btn_label, x_pos, item_w, _btn_action) in &control_btns {
            let is_hovered = self.active_modal.is_none() && mouse_x >= *x_pos && mouse_x < *x_pos + *item_w && mouse_y >= main_y && mouse_y < main_y + self.tabbar_height - 1.0;
            self.push_quad(
                vertices,
                indices,
                *x_pos,
                main_y,
                *item_w,
                self.tabbar_height - 1.0,
                white_uv,
                if is_hovered { self.config.theme.titlebar_hover_bg } else { [0.0, 0.0, 0.0, 0.0] },
            );
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                btn_label,
                *x_pos + 8.0,
                (main_y + self.tabbar_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                self.config.theme.tab_text,
                self.ui_font_size,
                self.ui_char_width,
            );
        }

        // --- Breadcrumb Bar (New) ---
        // Breadcrumb bar background (white)
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            main_y + self.tabbar_height,
            width - self.sidebar_width,
            self.breadcrumb_height,
            white_uv,
            self.config.theme.breadcrumb_bg,
        );
        // Breadcrumb bottom border
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            main_y + self.tabbar_height + self.breadcrumb_height - 1.0,
            width - self.sidebar_width,
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
            self.sidebar_width + 15.0,
            (main_y + self.tabbar_height + self.breadcrumb_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
            self.config.theme.breadcrumb_text,
            self.ui_font_size,
            self.ui_char_width,
        );

        // --- 3. Draw Editor Text Area & Gutter (Light Theme) ---
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            editor_y,
            gutter_width,
            editor_height,
            white_uv,
            self.config.theme.gutter_bg,
        );
        self.push_quad(
            vertices,
            indices,
            text_area_x - 1.0,
            editor_y,
            1.0,
            editor_height,
            white_uv,
            self.config.theme.gutter_border,
        );

        // Draw main editor background area
        self.push_quad(
            vertices,
            indices,
            text_area_x,
            editor_y,
            width - text_area_x - scrollbar_width,
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
                    width - text_area_x - scrollbar_width,
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
                self.sidebar_width + self.buffer_char_width,
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

                    if col_start < col_end || (s_line != e_line && line_idx < e_line) {
                        let sel_x = text_area_x + col_start as f32 * self.buffer_char_width;
                        let sel_w = ((col_end - col_start) as f32).max(0.5) * self.buffer_char_width;
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

            // Draw source code text characters (with custom Rust syntax highlighting)
            let line_text = &buffer.lines()[line_idx];
            let mut pen_x = text_area_x;
            let char_colors = self.get_line_char_colors(line_text);
            
            for (char_idx, c) in line_text.chars().enumerate() {
                let char_color = char_colors.get(char_idx).copied().unwrap_or(self.config.theme.syntax_default);
                pen_x += self.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color, self.buffer_font_size, self.buffer_char_width);
            }
        }

        // Draw active cursor
        if cursor.line >= self.scroll_y && cursor.line < self.scroll_y + visible_lines {
            let cur_row_y = editor_y + (cursor.line - self.scroll_y) as f32 * self.buffer_line_height;
            let cur_x = text_area_x + cursor.col as f32 * self.buffer_char_width;
            
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

        // --- 4. Draw Scrollbar (on the right edge) ---
        let sb_x = width - scrollbar_width;
        let is_sb_hovered = self.active_modal.is_none() && mouse_x >= sb_x && mouse_y >= editor_y && mouse_y < editor_y + editor_height;

        // Scrollbar Track background
        self.push_quad(
            vertices,
            indices,
            sb_x,
            editor_y,
            scrollbar_width,
            editor_height,
            white_uv,
            self.config.theme.scrollbar_track,
        );
        // Vertical track separator
        self.push_quad(
            vertices,
            indices,
            sb_x - 1.0,
            editor_y,
            1.0,
            editor_height,
            white_uv,
            self.config.theme.scrollbar_border,
        );

        let ratio = visible_lines as f32 / buffer.len() as f32;
        let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
        let scroll_ratio = if max_scroll > 0.0 { self.scroll_y as f32 / max_scroll } else { 0.0 };
        let thumb_y = editor_y + scroll_ratio * (editor_height - thumb_h);

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
        self.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &status_left,
            10.0,
            (status_y + self.status_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
            self.config.theme.statusbar_text,
            self.ui_font_size,
            self.ui_char_width,
        );

        let right_text_width = status_right.chars().count() as f32 * self.ui_char_width;
        let right_x = width - right_text_width - 15.0;
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

        // --- 6. Draw Context Dropdown Menus (On top of everything) ---
        if let Some(menu) = self.active_menu {
            let items = match menu {
                MenuType::Garage => vec!["Settings", "About", "Exit"],
                MenuType::File => vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
                MenuType::Edit => vec!["Undo (Ctrl+Z)", "Redo (Ctrl+Y)"],
                MenuType::Selection => vec!["Select All", "Clear Selection"],
                MenuType::View => vec!["Toggle Sidebar"],
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
                ModalType::Settings => 500.0,
                ModalType::About => 400.0,
            };
            let modal_h = match modal {
                ModalType::Settings => 360.0,
                ModalType::About => 240.0,
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
                ModalType::About => {
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "GARAGE CODE EDITOR",
                        modal_x + 20.0,
                        modal_y + 35.0,
                        self.config.theme.modal_text_title,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "A supercharged GPU-accelerated",
                        modal_x + 20.0,
                        modal_y + 70.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "text editor written in Rust.",
                        modal_x + 20.0,
                        modal_y + 95.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Version: 0.1.0 (main)",
                        modal_x + 20.0,
                        modal_y + 130.0,
                        self.config.theme.modal_text_muted,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                  }
                ModalType::Settings => {
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "SETTINGS",
                        modal_x + 20.0,
                        modal_y + 35.0,
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
                    let font_size_str = format!("Editor Font: {:.1} px", self.buffer_font_size);
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &font_size_str,
                        modal_x + 20.0,
                        modal_y + 70.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let dec_hover = mouse_x >= modal_x + 240.0 && mouse_x <= modal_x + 270.0 && mouse_y >= modal_y + 55.0 && mouse_y <= modal_y + 80.0;
                    draw_button(vertices, indices, atlas, queue, "-", modal_x + 240.0, modal_y + 55.0, 30.0, 25.0, false, dec_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);
                    let inc_hover = mouse_x >= modal_x + 280.0 && mouse_x <= modal_x + 310.0 && mouse_y >= modal_y + 55.0 && mouse_y <= modal_y + 80.0;
                    draw_button(vertices, indices, atlas, queue, "+", modal_x + 280.0, modal_y + 55.0, 30.0, 25.0, false, inc_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 2. UI Font Size Settings
                    let ui_size_str = format!("UI Font:     {:.1} px", self.ui_font_size);
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &ui_size_str,
                        modal_x + 20.0,
                        modal_y + 110.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let ui_dec_hover = mouse_x >= modal_x + 240.0 && mouse_x <= modal_x + 270.0 && mouse_y >= modal_y + 95.0 && mouse_y <= modal_y + 120.0;
                    draw_button(vertices, indices, atlas, queue, "-", modal_x + 240.0, modal_y + 95.0, 30.0, 25.0, false, ui_dec_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);
                    let ui_inc_hover = mouse_x >= modal_x + 280.0 && mouse_x <= modal_x + 310.0 && mouse_y >= modal_y + 95.0 && mouse_y <= modal_y + 120.0;
                    draw_button(vertices, indices, atlas, queue, "+", modal_x + 280.0, modal_y + 95.0, 30.0, 25.0, false, ui_inc_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 3. Sidebar Width Settings
                    let sidebar_size_str = format!("Sidebar:     {:.0} px", self.sidebar_width);
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &sidebar_size_str,
                        modal_x + 20.0,
                        modal_y + 150.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let sb_dec_hover = mouse_x >= modal_x + 240.0 && mouse_x <= modal_x + 270.0 && mouse_y >= modal_y + 135.0 && mouse_y <= modal_y + 160.0;
                    draw_button(vertices, indices, atlas, queue, "-", modal_x + 240.0, modal_y + 135.0, 30.0, 25.0, false, sb_dec_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);
                    let sb_inc_hover = mouse_x >= modal_x + 280.0 && mouse_x <= modal_x + 310.0 && mouse_y >= modal_y + 135.0 && mouse_y <= modal_y + 160.0;
                    draw_button(vertices, indices, atlas, queue, "+", modal_x + 280.0, modal_y + 135.0, 30.0, 25.0, false, sb_inc_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 4. Backend Selection
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Backend:",
                        modal_x + 20.0,
                        modal_y + 195.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let is_vulkan = self.config.backend == "Vulkan";
                    let vulkan_hover = mouse_x >= modal_x + 240.0 && mouse_x <= modal_x + 330.0 && mouse_y >= modal_y + 180.0 && mouse_y <= modal_y + 210.0;
                    draw_button(vertices, indices, atlas, queue, "Vulkan", modal_x + 240.0, modal_y + 180.0, 90.0, 30.0, is_vulkan, vulkan_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let is_opengl = self.config.backend == "OpenGL";
                    let opengl_hover = mouse_x >= modal_x + 340.0 && mouse_x <= modal_x + 430.0 && mouse_y >= modal_y + 180.0 && mouse_y <= modal_y + 210.0;
                    draw_button(vertices, indices, atlas, queue, "OpenGL", modal_x + 340.0, modal_y + 180.0, 90.0, 30.0, is_opengl, opengl_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 5. Theme Selection
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Theme:",
                        modal_x + 20.0,
                        modal_y + 245.0,
                        self.config.theme.modal_text_normal,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    let is_light_t = self.config.theme.name == "Light Theme";
                    let light_hover = mouse_x >= modal_x + 110.0 && mouse_x <= modal_x + 190.0 && mouse_y >= modal_y + 230.0 && mouse_y <= modal_y + 260.0;
                    draw_button(vertices, indices, atlas, queue, "Light", modal_x + 110.0, modal_y + 230.0, 80.0, 30.0, is_light_t, light_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let is_dark_t = self.config.theme.name == "Dark Theme";
                    let dark_hover = mouse_x >= modal_x + 200.0 && mouse_x <= modal_x + 280.0 && mouse_y >= modal_y + 230.0 && mouse_y <= modal_y + 260.0;
                    draw_button(vertices, indices, atlas, queue, "Dark", modal_x + 200.0, modal_y + 230.0, 80.0, 30.0, is_dark_t, dark_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let is_sol_t = self.config.theme.name == "Solarized Dark";
                    let sol_hover = mouse_x >= modal_x + 290.0 && mouse_x <= modal_x + 380.0 && mouse_y >= modal_y + 230.0 && mouse_y <= modal_y + 260.0;
                    draw_button(vertices, indices, atlas, queue, "Solarized", modal_x + 290.0, modal_y + 230.0, 90.0, 30.0, is_sol_t, sol_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    let is_cyb_t = self.config.theme.name == "Cyberpunk";
                    let cyb_hover = mouse_x >= modal_x + 390.0 && mouse_x <= modal_x + 480.0 && mouse_y >= modal_y + 230.0 && mouse_y <= modal_y + 260.0;
                    draw_button(vertices, indices, atlas, queue, "Cyberpunk", modal_x + 390.0, modal_y + 230.0, 90.0, 30.0, is_cyb_t, cyb_hover, &self.config.theme, white_uv, self.ui_char_width, self.ui_font_ascent, self.ui_font_size);

                    // 6. Draw Active backend and GPU info
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
                        modal_x + 20.0,
                        modal_y + 285.0,
                        self.config.theme.modal_text_muted,
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                }
            }

            // Draw generic Close Button (centered horizontally)
            let btn_w = 120.0;
            let btn_h = 35.0;
            let btn_x = modal_x + ((modal_w - btn_w) / 2.0).round();
            let btn_y = modal_y + modal_h - 60.0;

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
