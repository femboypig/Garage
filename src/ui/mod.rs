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
    
    // Project Tree State
    pub expanded_dirs: HashSet<PathBuf>,
    pub visible_nodes: Vec<FileNode>,
    pub selected_file: Option<PathBuf>,
    
    // Menu & Modal State
    pub active_menu: Option<MenuType>,
    pub active_modal: Option<ModalType>,
}

impl UiState {
    pub fn new(atlas: &mut FontAtlas, _queue: &wgpu::Queue, ui_font_size: f32, buffer_font_size: f32) -> Self {
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
            titlebar_height: (ui_line_height * 1.8).round().max(32.0),
            status_height: (ui_line_height * 1.5).round().max(24.0),
            sidebar_width: 200.0,
            target_sidebar_width: 200.0,
            expanded_dirs,
            visible_nodes: Vec::new(),
            selected_file: None,
            active_menu: None,
            active_modal: None,
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
    }

    pub fn scroll_to_cursor(&mut self, cursor: &Cursor, buffer_len: usize, height: f32) {
        let main_height = height - self.titlebar_height - self.status_height;
        let visible_lines = (main_height / self.buffer_line_height).floor() as usize;
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
                    
                    // Skip hidden files/folders and build target directories
                    if name.starts_with('.') || name == "target" || name == "Cargo.lock" {
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
            let modal_w = 400.0;
            let modal_h = match modal {
                ModalType::Settings => 280.0,
                ModalType::About => 240.0,
            };
            let modal_x = (width - modal_w) / 2.0;
            let modal_y = (height - modal_h) / 2.0;

            // Check if clicked close button
            let inside_close_btn = mx >= modal_x + 140.0 && mx <= modal_x + 260.0 && my >= modal_y + modal_h - 60.0 && my <= modal_y + modal_h - 25.0;
            let clicked_outside = mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h;

            if inside_close_btn || clicked_outside {
                self.active_modal = None;
                return UiAction::CloseModal;
            }

            if modal == ModalType::Settings {
                // Row 1: Editor Font Size [-] and [+]
                // Decrease [-] at 200..230, 55..80
                if mx >= modal_x + 200.0 && mx <= modal_x + 230.0 && my >= modal_y + 55.0 && my <= modal_y + 80.0 {
                    return UiAction::ChangeBufferFontSize(-1.0);
                }
                // Increase [+] at 240..270, 55..80
                if mx >= modal_x + 240.0 && mx <= modal_x + 270.0 && my >= modal_y + 55.0 && my <= modal_y + 80.0 {
                    return UiAction::ChangeBufferFontSize(1.0);
                }

                // Row 2: UI Font Size [-] and [+]
                // Decrease [-] at 200..230, 95..120
                if mx >= modal_x + 200.0 && mx <= modal_x + 230.0 && my >= modal_y + 95.0 && my <= modal_y + 120.0 {
                    return UiAction::ChangeUiFontSize(-1.0);
                }
                // Increase [+] at 240..270, 95..120
                if mx >= modal_x + 240.0 && mx <= modal_x + 270.0 && my >= modal_y + 95.0 && my <= modal_y + 120.0 {
                    return UiAction::ChangeUiFontSize(1.0);
                }

                // Row 3: Backend Selection
                // Vulkan Button at 110..200, 135..165
                if mx >= modal_x + 110.0 && mx <= modal_x + 200.0 && my >= modal_y + 135.0 && my <= modal_y + 165.0 {
                    return UiAction::ChangeBackend(wgpu::Backend::Vulkan);
                }
                // OpenGL Button at 210..300, 135..165
                if mx >= modal_x + 210.0 && mx <= modal_x + 300.0 && my >= modal_y + 135.0 && my <= modal_y + 165.0 {
                    return UiAction::ChangeBackend(wgpu::Backend::Gl);
                }
            }

            return UiAction::None;
        }

        // 1. Check Titlebar Menu Clicks
        if my < self.titlebar_height {
            let menu_items_raw = [
                ("Garage", MenuType::Garage),
                ("File", MenuType::File),
                ("Edit", MenuType::Edit),
                ("Selection", MenuType::Selection),
                ("View", MenuType::View),
            ];
            let mut current_x = 10.0;
            for (label, menu_type) in &menu_items_raw {
                let item_w = label.chars().count() as f32 * self.ui_char_width;
                let x_min = current_x - 6.0;
                let x_max = current_x + item_w + 6.0;
                if mx >= x_min && mx <= x_max {
                    self.active_menu = if self.active_menu == Some(*menu_type) { None } else { Some(*menu_type) };
                    return UiAction::None;
                }
                current_x = current_x + item_w + 24.0;
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
            
            // Calculate dynamic menu_x matching the drawn position
            let menu_items_raw = [
                ("Garage", MenuType::Garage),
                ("File", MenuType::File),
                ("Edit", MenuType::Edit),
                ("Selection", MenuType::Selection),
                ("View", MenuType::View),
            ];
            let mut menu_x = 10.0;
            let mut current_x = 10.0;
            for (label, m_type) in &menu_items_raw {
                let item_w = label.chars().count() as f32 * self.ui_char_width;
                if *m_type == menu {
                    menu_x = current_x;
                    break;
                }
                current_x = current_x + item_w + 24.0;
            }

            let item_height = (self.ui_line_height * 1.6).round().max(26.0);
            let dropdown_h = items.len() as f32 * item_height;
            let max_chars = items.iter().map(|s| s.chars().count()).max().unwrap_or(10) as f32;
            let dropdown_w = (max_chars * self.ui_char_width + 30.0).round();

            let menu_action = if mx >= menu_x - 4.0 && mx <= menu_x - 4.0 + dropdown_w && my >= self.titlebar_height && my <= self.titlebar_height + dropdown_h {
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

        // 3. Check Sidebar Clicks
        if self.sidebar_width > 0.0 && mx < self.sidebar_width && my > self.titlebar_height && my < height - self.status_height {
            let tree_y = my - self.titlebar_height;
            let node_idx = (tree_y / self.ui_line_height).floor() as usize;
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
        // Text viewport width is limited by scrollbar
        let _text_viewport_w = width - text_area_x - scrollbar_width;

        let visible_lines = (main_height / self.buffer_line_height).floor() as usize;
        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as usize;
        self.scroll_y = self.scroll_y.min(max_scroll);

        // --- 1. Draw Titlebar Menu Headers ---
        self.push_quad(
            vertices,
            indices,
            0.0,
            0.0,
            width,
            self.titlebar_height,
            white_uv,
            [0.07, 0.07, 0.09, 1.0],
        );
        self.push_quad(
            vertices,
            indices,
            0.0,
            self.titlebar_height - 1.0,
            width,
            1.0,
            white_uv,
            [0.16, 0.16, 0.2, 1.0],
        );

        let menu_items_raw = [
            ("Garage", MenuType::Garage),
            ("File", MenuType::File),
            ("Edit", MenuType::Edit),
            ("Selection", MenuType::Selection),
            ("View", MenuType::View),
        ];

        let mut menu_positions = Vec::new();
        let mut current_x = 10.0;
        for (label, menu_type) in &menu_items_raw {
            let label_len = label.chars().count() as f32;
            let item_w = label_len * self.ui_char_width;
            let x_min = current_x;
            let x_max = current_x + item_w;
            menu_positions.push((*label, x_min, x_max, *menu_type));
            current_x = x_max + 24.0; // Spacing between menus
        }

        for (label, x_min, x_max, menu_type) in &menu_positions {
            let is_hovered = self.active_modal.is_none() && mouse_y < self.titlebar_height && mouse_x >= *x_min - 6.0 && mouse_x <= *x_max + 6.0;
            let is_active = self.active_menu == Some(*menu_type);
            
            let ui_hover_height = (self.ui_line_height * 1.4).round();
            let hover_y = ((self.titlebar_height - ui_hover_height) / 2.0).round();

            if is_hovered || is_active {
                self.push_quad(
                    vertices,
                    indices,
                    *x_min - 6.0,
                    hover_y,
                    *x_max - *x_min + 12.0,
                    ui_hover_height,
                    white_uv,
                    [0.16, 0.16, 0.22, 1.0],
                );
            }
            
            // Bold brand title
            let label_color = if *menu_type == MenuType::Garage {
                [0.0, 0.9, 0.8, 1.0] // Teal brand highlights
            } else if is_active {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.75, 0.75, 0.8, 1.0]
            };

            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                label,
                *x_min,
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
        let title_str = format!("Garage - {}", file_name);
        let title_width = title_str.chars().count() as f32 * self.ui_char_width;
        let title_x = (width - title_width) / 2.0;
        if title_x > 360.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &title_str,
                title_x,
                (self.titlebar_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                [0.5, 0.5, 0.55, 1.0],
                self.ui_font_size,
                self.ui_char_width,
            );
        }

        // --- 2. Draw Sidebar Project Tree ---
        if self.sidebar_width > 0.0 {
            self.push_quad(
                vertices,
                indices,
                0.0,
                main_y,
                self.sidebar_width,
                main_height,
                white_uv,
                [0.05, 0.05, 0.07, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                self.sidebar_width - 1.0,
                main_y,
                1.0,
                main_height,
                white_uv,
                [0.15, 0.15, 0.18, 1.0],
            );

            for (idx, node) in self.visible_nodes.iter().enumerate() {
                let row_y = main_y + idx as f32 * self.ui_line_height;
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
                        if is_selected { [0.12, 0.16, 0.22, 1.0] } else { [0.08, 0.08, 0.11, 1.0] },
                    );
                }

                let indent_x = 10.0 + node.depth as f32 * 12.0;
                let icon = if node.is_dir {
                    if self.expanded_dirs.contains(&node.path) { "▼ " } else { "▶ " }
                } else {
                    "  "
                };

                let text_color = if node.is_dir {
                    [0.8, 0.8, 0.85, 1.0]
                } else {
                    [0.65, 0.65, 0.7, 1.0]
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

        // --- 3. Draw Editor Text Area & Gutter ---
        self.push_quad(
            vertices,
            indices,
            self.sidebar_width,
            main_y,
            gutter_width,
            main_height,
            white_uv,
            [0.07, 0.07, 0.09, 1.0],
        );
        self.push_quad(
            vertices,
            indices,
            text_area_x - 1.0,
            main_y,
            1.0,
            main_height,
            white_uv,
            [0.14, 0.14, 0.18, 1.0],
        );

        let start_idx = self.scroll_y;
        let end_idx = (start_idx + visible_lines).min(buffer.len());

        for line_idx in start_idx..end_idx {
            let row_y = main_y + (line_idx - start_idx) as f32 * self.buffer_line_height;
            let baseline_y = (row_y + self.buffer_font_ascent).round();

            // Active line highlight
            if line_idx == cursor.line {
                self.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    row_y,
                    width - text_area_x,
                    self.buffer_line_height,
                    white_uv,
                    [0.09, 0.09, 0.12, 1.0],
                );
            }

            // Draw line numbers
            let line_num_str = format!("{:>width$}", line_idx + 1, width = max_line_digits);
            let num_color = if line_idx == cursor.line {
                [0.75, 0.75, 0.8, 1.0]
            } else {
                [0.3, 0.3, 0.35, 1.0]
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
                            [0.15, 0.25, 0.42, 0.6],
                        );
                    }
                }
            }

            // Draw source code text characters
            let line_text = &buffer.lines()[line_idx];
            let mut pen_x = text_area_x;
            
            for c in line_text.chars() {
                let char_color = match c {
                    '0'..='9' => [0.85, 0.6, 0.35, 1.0],
                    '{' | '}' | '(' | ')' | '[' | ']' => [0.8, 0.8, 0.3, 1.0],
                    _ => [0.85, 0.85, 0.9, 1.0],
                };
                pen_x += self.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color, self.buffer_font_size, self.buffer_char_width);
            }
        }

        // Draw active cursor
        if cursor.line >= self.scroll_y && cursor.line < self.scroll_y + visible_lines {
            let cur_row_y = main_y + (cursor.line - self.scroll_y) as f32 * self.buffer_line_height;
            let cur_x = text_area_x + cursor.col as f32 * self.buffer_char_width;
            
            self.push_quad(
                vertices,
                indices,
                cur_x,
                cur_row_y + 1.0,
                2.0,
                self.buffer_line_height - 2.0,
                white_uv,
                [0.0, 0.9, 0.8, 1.0],
            );
        }

        // --- 4. Draw Scrollbar (on the right edge) ---
        let sb_x = width - scrollbar_width;
        let is_sb_hovered = self.active_modal.is_none() && mouse_x >= sb_x && mouse_y >= main_y && mouse_y < main_y + main_height;

        // Scrollbar Track background
        self.push_quad(
            vertices,
            indices,
            sb_x,
            main_y,
            scrollbar_width,
            main_height,
            white_uv,
            [0.08, 0.08, 0.1, 1.0],
        );
        // Vertical track separator
        self.push_quad(
            vertices,
            indices,
            sb_x - 1.0,
            main_y,
            1.0,
            main_height,
            white_uv,
            [0.15, 0.15, 0.18, 1.0],
        );

        let ratio = visible_lines as f32 / buffer.len() as f32;
        let thumb_h = (main_height * ratio).clamp(20.0, main_height);
        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
        let scroll_ratio = if max_scroll > 0.0 { self.scroll_y as f32 / max_scroll } else { 0.0 };
        let thumb_y = main_y + scroll_ratio * (main_height - thumb_h);

        let thumb_color = if is_sb_hovered {
            [0.32, 0.32, 0.38, 1.0] // Brighter thumb on hover
        } else {
            [0.22, 0.22, 0.26, 1.0]
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
            [0.08, 0.08, 0.1, 1.0],
        );
        self.push_quad(
            vertices,
            indices,
            0.0,
            status_y,
            width,
            1.0,
            white_uv,
            [0.15, 0.15, 0.18, 1.0],
        );

        let status_left = format!(" GARAGE | Line {}, Col {}", cursor.line + 1, cursor.col + 1);
        let status_right = format!("Lines: {} | IBM Plex Mono ", buffer.len());

        self.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &status_left,
            10.0,
            (status_y + self.status_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
            [0.6, 0.6, 0.65, 1.0],
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
                [0.5, 0.5, 0.55, 1.0],
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
            let mut menu_x = 10.0;
            let mut current_x = 10.0;
            for (label, m_type) in &menu_items_raw {
                let item_w = label.chars().count() as f32 * self.ui_char_width;
                if *m_type == menu {
                    menu_x = current_x;
                    break;
                }
                current_x = current_x + item_w + 24.0;
            }

            let item_height = (self.ui_line_height * 1.6).round().max(26.0);
            let dropdown_h = items.len() as f32 * item_height;
            let max_chars = items.iter().map(|s| s.chars().count()).max().unwrap_or(10) as f32;
            let dropdown_w = (max_chars * self.ui_char_width + 30.0).round();

            // Draw Dropdown Card Background
            self.push_quad(
                vertices,
                indices,
                menu_x - 4.0,
                self.titlebar_height,
                dropdown_w,
                dropdown_h,
                white_uv,
                [0.08, 0.08, 0.1, 0.98],
            );
            // Draw card borders
            self.push_quad(
                vertices,
                indices,
                menu_x - 4.0,
                self.titlebar_height,
                dropdown_w,
                1.0,
                white_uv,
                [0.2, 0.2, 0.25, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                menu_x - 4.0,
                self.titlebar_height + dropdown_h - 1.0,
                dropdown_w,
                1.0,
                white_uv,
                [0.2, 0.2, 0.25, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                menu_x - 4.0,
                self.titlebar_height,
                1.0,
                dropdown_h,
                white_uv,
                [0.2, 0.2, 0.25, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                menu_x - 4.0 + dropdown_w - 1.0,
                self.titlebar_height,
                1.0,
                dropdown_h,
                white_uv,
                [0.2, 0.2, 0.25, 1.0],
            );

            for (idx, label) in items.iter().enumerate() {
                let row_y = self.titlebar_height + idx as f32 * item_height;
                let is_hovered = mouse_x >= menu_x - 4.0 && mouse_x <= menu_x - 4.0 + dropdown_w && mouse_y >= row_y && mouse_y < row_y + item_height;

                if is_hovered {
                    self.push_quad(
                        vertices,
                        indices,
                        menu_x - 3.0,
                        row_y + 1.0,
                        dropdown_w - 2.0,
                        item_height - 2.0,
                        white_uv,
                        [0.18, 0.18, 0.24, 1.0],
                    );
                }

                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    label,
                    menu_x + 8.0,
                    (row_y + item_height / 2.0 + self.ui_font_ascent / 2.0 - 2.0).round(),
                    if is_hovered { [1.0, 1.0, 1.0, 1.0] } else { [0.75, 0.75, 0.8, 1.0] },
                    self.ui_font_size,
                    self.ui_char_width,
                );
            }
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
                [0.0, 0.0, 0.0, 0.6],
            );

            let modal_w = 400.0;
            let modal_h = match modal {
                ModalType::Settings => 280.0,
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
                [0.09, 0.09, 0.12, 1.0],
            );
            // Draw modal border
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y,
                modal_w,
                1.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y + modal_h - 1.0,
                modal_w,
                1.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x,
                modal_y,
                1.0,
                modal_h,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + modal_w - 1.0,
                modal_y,
                1.0,
                modal_h,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
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
                        [0.0, 0.9, 0.8, 1.0], // Teal title
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
                        [0.8, 0.8, 0.85, 1.0],
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
                        [0.8, 0.8, 0.85, 1.0],
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
                        [0.5, 0.5, 0.55, 1.0],
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
                        [1.0, 1.0, 1.0, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    
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
                        [0.8, 0.8, 0.85, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                    
                    // Decrease button [-] at x: 200..230, y: 55..80
                    let dec_hover = mouse_x >= modal_x + 200.0 && mouse_x <= modal_x + 230.0 && mouse_y >= modal_y + 55.0 && mouse_y <= modal_y + 80.0;
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 200.0,
                        modal_y + 55.0,
                        30.0,
                        25.0,
                        white_uv,
                        if dec_hover { [0.18, 0.18, 0.22, 1.0] } else { [0.12, 0.12, 0.15, 1.0] },
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "-",
                        modal_x + 211.0,
                        (modal_y + 72.0).round(),
                        [0.85, 0.85, 0.9, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // Increase button [+] at x: 240..270, y: 55..80
                    let inc_hover = mouse_x >= modal_x + 240.0 && mouse_x <= modal_x + 270.0 && mouse_y >= modal_y + 55.0 && mouse_y <= modal_y + 80.0;
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 240.0,
                        modal_y + 55.0,
                        30.0,
                        25.0,
                        white_uv,
                        if inc_hover { [0.18, 0.18, 0.22, 1.0] } else { [0.12, 0.12, 0.15, 1.0] },
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "+",
                        modal_x + 251.0,
                        (modal_y + 72.0).round(),
                        [0.85, 0.85, 0.9, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );

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
                        [0.8, 0.8, 0.85, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // Decrease button [-] at x: 200..230, y: 95..120
                    let ui_dec_hover = mouse_x >= modal_x + 200.0 && mouse_x <= modal_x + 230.0 && mouse_y >= modal_y + 95.0 && mouse_y <= modal_y + 120.0;
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 200.0,
                        modal_y + 95.0,
                        30.0,
                        25.0,
                        white_uv,
                        if ui_dec_hover { [0.18, 0.18, 0.22, 1.0] } else { [0.12, 0.12, 0.15, 1.0] },
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "-",
                        modal_x + 211.0,
                        (modal_y + 112.0).round(),
                        [0.85, 0.85, 0.9, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // Increase button [+] at x: 240..270, y: 95..120
                    let ui_inc_hover = mouse_x >= modal_x + 240.0 && mouse_x <= modal_x + 270.0 && mouse_y >= modal_y + 95.0 && mouse_y <= modal_y + 120.0;
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 240.0,
                        modal_y + 95.0,
                        30.0,
                        25.0,
                        white_uv,
                        if ui_inc_hover { [0.18, 0.18, 0.22, 1.0] } else { [0.12, 0.12, 0.15, 1.0] },
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "+",
                        modal_x + 251.0,
                        (modal_y + 112.0).round(),
                        [0.85, 0.85, 0.9, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // 3. Backend Selection
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Backend:",
                        modal_x + 20.0,
                        modal_y + 150.0,
                        [0.8, 0.8, 0.85, 1.0],
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // Vulkan Button at x: 110..200, y: 135..165
                    let is_vulkan = current_backend == wgpu::Backend::Vulkan;
                    let vulkan_hover = mouse_x >= modal_x + 110.0 && mouse_x <= modal_x + 200.0 && mouse_y >= modal_y + 135.0 && mouse_y <= modal_y + 165.0;
                    let vulkan_bg = if is_vulkan {
                        [0.0, 0.45, 0.4, 1.0]
                    } else if vulkan_hover {
                        [0.18, 0.18, 0.22, 1.0]
                    } else {
                        [0.12, 0.12, 0.15, 1.0]
                    };
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 110.0,
                        modal_y + 135.0,
                        90.0,
                        30.0,
                        white_uv,
                        vulkan_bg,
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "Vulkan",
                        modal_x + 125.0,
                        (modal_y + 155.0).round(),
                        if is_vulkan { [1.0, 1.0, 1.0, 1.0] } else { [0.85, 0.85, 0.9, 1.0] },
                        self.ui_font_size,
                        self.ui_char_width,
                    );

                    // OpenGL Button at x: 210..300, y: 135..165
                    let is_opengl = current_backend == wgpu::Backend::Gl;
                    let opengl_hover = mouse_x >= modal_x + 210.0 && mouse_x <= modal_x + 300.0 && mouse_y >= modal_y + 135.0 && mouse_y <= modal_y + 165.0;
                    let opengl_bg = if is_opengl {
                        [0.0, 0.45, 0.4, 1.0]
                    } else if opengl_hover {
                        [0.18, 0.18, 0.22, 1.0]
                    } else {
                        [0.12, 0.12, 0.15, 1.0]
                    };
                    self.push_quad(
                        vertices,
                        indices,
                        modal_x + 210.0,
                        modal_y + 135.0,
                        90.0,
                        30.0,
                        white_uv,
                        opengl_bg,
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "OpenGL",
                        modal_x + 225.0,
                        (modal_y + 155.0).round(),
                        if is_opengl { [1.0, 1.0, 1.0, 1.0] } else { [0.85, 0.85, 0.9, 1.0] },
                        self.ui_font_size,
                        self.ui_char_width,
                    );
                }
            }

            // Draw generic Close Button
            let close_btn_hover = mouse_x >= modal_x + 140.0 && mouse_x <= modal_x + 260.0 && mouse_y >= modal_y + modal_h - 60.0 && mouse_y <= modal_y + modal_h - 25.0;
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + modal_h - 60.0,
                120.0,
                35.0,
                white_uv,
                if close_btn_hover { [0.18, 0.18, 0.22, 1.0] } else { [0.12, 0.12, 0.15, 1.0] },
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + modal_h - 60.0,
                120.0,
                1.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + modal_h - 26.0,
                120.0,
                1.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + modal_h - 60.0,
                1.0,
                35.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 259.0,
                modal_y + modal_h - 60.0,
                1.0,
                35.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );

            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                "Close",
                modal_x + 180.0,
                modal_y + modal_h - 37.0,
                [0.85, 0.85, 0.9, 1.0],
                self.ui_font_size,
                self.ui_char_width,
            );
        }
    }
}
