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
    pub char_width: f32,
    pub line_height: f32,
    pub font_ascent: f32,
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
    pub fn new(atlas: &mut FontAtlas, queue: &wgpu::Queue) -> Self {
        let glyph_m = atlas.get_or_rasterize(queue, 'm').expect("Failed to measure character");
        let char_width = glyph_m.width.max(8.0);

        let font_metrics = atlas.font.horizontal_line_metrics(atlas.font_size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: atlas.font_size * 0.8,
                descent: -atlas.font_size * 0.2,
                line_gap: atlas.font_size * 0.2,
                new_line_size: atlas.font_size * 1.2,
            });

        let line_height = font_metrics.new_line_size;
        let font_ascent = font_metrics.ascent;

        let mut expanded_dirs = HashSet::new();
        // Expand root by default
        expanded_dirs.insert(PathBuf::from("."));

        let mut state = Self {
            char_width,
            line_height,
            font_ascent,
            scroll_y: 0,
            scroll_x: 0,
            titlebar_height: 32.0,
            status_height: 24.0,
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
        // If a modal is open, any click outside the modal boundaries or on modal buttons closes it
        if let Some(_modal) = self.active_modal {
            let modal_w = 400.0;
            let modal_h = 240.0;
            let modal_x = (width - modal_w) / 2.0;
            let modal_y = (height - modal_h) / 2.0;

            // Check if clicked close button (e.g. at x: modal_x + 150..modal_x + 250, y: modal_y + 180..modal_y + 220)
            let inside_close_btn = mx >= modal_x + 140.0 && mx <= modal_x + 260.0 && my >= modal_y + 180.0 && my <= modal_y + 215.0;
            let clicked_outside = mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h;

            if inside_close_btn || clicked_outside {
                self.active_modal = None;
                return UiAction::CloseModal;
            }
            return UiAction::None;
        }

        // 1. Check Titlebar Menu Clicks
        if my < self.titlebar_height {
            // Garage Menu: 10..70
            if mx >= 10.0 && mx <= 70.0 {
                self.active_menu = if self.active_menu == Some(MenuType::Garage) { None } else { Some(MenuType::Garage) };
                return UiAction::None;
            }
            // File: 80..130
            if mx >= 80.0 && mx <= 130.0 {
                self.active_menu = if self.active_menu == Some(MenuType::File) { None } else { Some(MenuType::File) };
                return UiAction::None;
            }
            // Edit: 140..190
            if mx >= 140.0 && mx <= 190.0 {
                self.active_menu = if self.active_menu == Some(MenuType::Edit) { None } else { Some(MenuType::Edit) };
                return UiAction::None;
            }
            // Selection: 200..280
            if mx >= 200.0 && mx <= 280.0 {
                self.active_menu = if self.active_menu == Some(MenuType::Selection) { None } else { Some(MenuType::Selection) };
                return UiAction::None;
            }
            // View: 290..340
            if mx >= 290.0 && mx <= 340.0 {
                self.active_menu = if self.active_menu == Some(MenuType::View) { None } else { Some(MenuType::View) };
                return UiAction::None;
            }
            self.active_menu = None;
            return UiAction::None;
        }

        // 2. Check Dropdown Clicks (if active)
        if let Some(menu) = self.active_menu {
            let menu_action = match menu {
                MenuType::Garage => {
                    // Box: x: 10..180, y: 32..122
                    if mx >= 10.0 && mx <= 180.0 && my >= 32.0 && my <= 122.0 {
                        let idx = ((my - 32.0) / 30.0).floor() as usize;
                        match idx {
                            0 => Some(UiAction::ShowSettings),
                            1 => Some(UiAction::ShowAbout),
                            2 => Some(UiAction::Exit),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                MenuType::File => {
                    // Box: x: 80..250, y: 32..122
                    if mx >= 80.0 && mx <= 250.0 && my >= 32.0 && my <= 122.0 {
                        let idx = ((my - 32.0) / 30.0).floor() as usize;
                        match idx {
                            0 => Some(UiAction::SaveFile),
                            1 => Some(UiAction::ToggleSidebar),
                            2 => Some(UiAction::Exit),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                MenuType::Edit => {
                    // Box: x: 140..290, y: 32..92
                    if mx >= 140.0 && mx <= 290.0 && my >= 32.0 && my <= 92.0 {
                        let idx = ((my - 32.0) / 30.0).floor() as usize;
                        match idx {
                            0 => Some(UiAction::Undo),
                            1 => Some(UiAction::Redo),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                MenuType::Selection => {
                    // Box: x: 200..350, y: 32..92
                    if mx >= 200.0 && mx <= 350.0 && my >= 32.0 && my <= 92.0 {
                        let idx = ((my - 32.0) / 30.0).floor() as usize;
                        match idx {
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
                        }
                    } else {
                        None
                    }
                }
                MenuType::View => {
                    // Box: x: 290..440, y: 32..62
                    if mx >= 290.0 && mx <= 440.0 && my >= 32.0 && my <= 62.0 {
                        let idx = ((my - 32.0) / 30.0).floor() as usize;
                        match idx {
                            0 => Some(UiAction::ToggleSidebar),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
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
            let node_idx = (tree_y / self.line_height).floor() as usize;
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
    ) -> f32 {
        if let Some(info) = atlas.get_or_rasterize(queue, c) {
            if info.width == 0.0 || info.height == 0.0 {
                return self.char_width;
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
        self.char_width
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
    ) {
        for c in text.chars() {
            x += self.push_char(vertices, indices, atlas, queue, c, x, y, color);
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
    ) {
        let white_uv = atlas.white_pixel_uv();
        let main_y = self.titlebar_height;
        let main_height = height - self.titlebar_height - self.status_height;

        // Smoothly expand/collapse sidebar width
        let step = 20.0;
        if self.sidebar_width < self.target_sidebar_width {
            self.sidebar_width = (self.sidebar_width + step).min(self.target_sidebar_width);
        } else if self.sidebar_width > self.target_sidebar_width {
            self.sidebar_width = (self.sidebar_width - step).max(self.target_sidebar_width);
        }

        // Calculate dynamic layouts
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * self.char_width;
        let text_area_x = self.sidebar_width + gutter_width;
        
        let scrollbar_width = 12.0;
        // Text viewport width is limited by scrollbar
        let _text_viewport_w = width - text_area_x - scrollbar_width;

        // Ensure vertical scrolling viewport matches active cursor position
        let visible_lines = (main_height / self.line_height).floor() as usize;
        if cursor.line < self.scroll_y {
            self.scroll_y = cursor.line;
        } else if cursor.line >= self.scroll_y + visible_lines {
            self.scroll_y = cursor.line - visible_lines + 1;
        }

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

        let menu_items = [
            ("Garage", 10.0, 70.0, MenuType::Garage),
            ("File", 80.0, 130.0, MenuType::File),
            ("Edit", 140.0, 190.0, MenuType::Edit),
            ("Selection", 200.0, 280.0, MenuType::Selection),
            ("View", 290.0, 340.0, MenuType::View),
        ];

        for (label, x_min, x_max, menu_type) in &menu_items {
            let is_hovered = mouse_y < self.titlebar_height && mouse_x >= *x_min && mouse_x <= *x_max;
            let is_active = self.active_menu == Some(*menu_type);
            
            if is_hovered || is_active {
                self.push_quad(
                    vertices,
                    indices,
                    *x_min - 4.0,
                    4.0,
                    *x_max - *x_min + 8.0,
                    24.0,
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
                (self.titlebar_height / 2.0 + self.font_ascent / 2.0 - 2.0).round(),
                label_color,
            );
        }

        // Display current open file title in titlebar center
        let file_name = self.selected_file.as_ref()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        let title_str = format!("Garage - {}", file_name);
        let title_width = title_str.chars().count() as f32 * self.char_width;
        let title_x = (width - title_width) / 2.0;
        if title_x > 360.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &title_str,
                title_x,
                (self.titlebar_height / 2.0 + self.font_ascent / 2.0 - 2.0).round(),
                [0.5, 0.5, 0.55, 1.0],
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
                let row_y = main_y + idx as f32 * self.line_height;
                if row_y + self.line_height > main_y + main_height {
                    break;
                }

                let is_hovered = mouse_x < self.sidebar_width && mouse_y >= row_y && mouse_y < row_y + self.line_height;
                let is_selected = self.selected_file.as_ref() == Some(&node.path);

                if is_hovered || is_selected {
                    self.push_quad(
                        vertices,
                        indices,
                        0.0,
                        row_y,
                        self.sidebar_width - 1.0,
                        self.line_height,
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
                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &node_text,
                    indent_x,
                    (row_y + self.font_ascent).round(),
                    text_color,
                );
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
            let row_y = main_y + (line_idx - start_idx) as f32 * self.line_height;
            let baseline_y = (row_y + self.font_ascent).round();

            // Active line highlight
            if line_idx == cursor.line {
                self.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    row_y,
                    width - text_area_x,
                    self.line_height,
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
                self.sidebar_width + self.char_width,
                baseline_y,
                num_color,
            );

            // Draw selection ranges
            if let Some((s_line, s_col, e_line, e_col)) = cursor.selection_range() {
                if line_idx >= s_line && line_idx <= e_line {
                    let line_chars_count = buffer.lines()[line_idx].chars().count();
                    let col_start = if line_idx == s_line { s_col } else { 0 };
                    let col_end = if line_idx == e_line { e_col } else { line_chars_count };

                    if col_start < col_end || (s_line != e_line && line_idx < e_line) {
                        let sel_x = text_area_x + col_start as f32 * self.char_width;
                        let sel_w = ((col_end - col_start) as f32).max(0.5) * self.char_width;
                        self.push_quad(
                            vertices,
                            indices,
                            sel_x,
                            row_y,
                            sel_w,
                            self.line_height,
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
                pen_x += self.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color);
            }
        }

        // Draw active cursor
        if cursor.line >= self.scroll_y && cursor.line < self.scroll_y + visible_lines {
            let cur_row_y = main_y + (cursor.line - self.scroll_y) as f32 * self.line_height;
            let cur_x = text_area_x + cursor.col as f32 * self.char_width;
            
            self.push_quad(
                vertices,
                indices,
                cur_x,
                cur_row_y + 1.0,
                2.0,
                self.line_height - 2.0,
                white_uv,
                [0.0, 0.9, 0.8, 1.0],
            );
        }

        // --- 4. Draw Scrollbar (on the right edge) ---
        let sb_x = width - scrollbar_width;
        let is_sb_hovered = mouse_x >= sb_x && mouse_y >= main_y && mouse_y < main_y + main_height;

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
            (status_y + self.font_ascent / 2.0 + 2.0).round(),
            [0.6, 0.6, 0.65, 1.0],
        );

        let right_text_width = status_right.chars().count() as f32 * self.char_width;
        let right_x = width - right_text_width - 15.0;
        if right_x > width / 2.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &status_right,
                right_x,
                (status_y + self.font_ascent / 2.0 + 2.0).round(),
                [0.5, 0.5, 0.55, 1.0],
            );
        }

        // --- 6. Draw Context Dropdown Menus (On top of everything) ---
        if let Some(menu) = self.active_menu {
            let (menu_x, dropdown_w, dropdown_h, items) = match menu {
                MenuType::Garage => (
                    10.0,
                    150.0,
                    90.0,
                    vec!["Settings", "About", "Exit"],
                ),
                MenuType::File => (
                    80.0,
                    170.0,
                    90.0,
                    vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
                ),
                MenuType::Edit => (
                    140.0,
                    150.0,
                    60.0,
                    vec!["Undo (Ctrl+Z)", "Redo (Ctrl+Y)"],
                ),
                MenuType::Selection => (
                    200.0,
                    150.0,
                    60.0,
                    vec!["Select All", "Clear Selection"],
                ),
                MenuType::View => (
                    290.0,
                    150.0,
                    30.0,
                    vec!["Toggle Sidebar"],
                ),
            };

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
                let row_y = self.titlebar_height + idx as f32 * 30.0;
                let is_hovered = mouse_x >= menu_x - 4.0 && mouse_x <= menu_x - 4.0 + dropdown_w && mouse_y >= row_y && mouse_y < row_y + 30.0;

                if is_hovered {
                    self.push_quad(
                        vertices,
                        indices,
                        menu_x - 3.0,
                        row_y + 1.0,
                        dropdown_w - 2.0,
                        28.0,
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
                    (row_y + 20.0).round(),
                    if is_hovered { [1.0, 1.0, 1.0, 1.0] } else { [0.75, 0.75, 0.8, 1.0] },
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
            let modal_h = 240.0;
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
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        " Font Size:  16.0 px",
                        modal_x + 20.0,
                        modal_y + 75.0,
                        [0.8, 0.8, 0.85, 1.0],
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        " Font Family: IBM Plex Mono",
                        modal_x + 20.0,
                        modal_y + 105.0,
                        [0.8, 0.8, 0.85, 1.0],
                    );
                    self.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        " Tab Width:   4 spaces",
                        modal_x + 20.0,
                        modal_y + 135.0,
                        [0.8, 0.8, 0.85, 1.0],
                    );
                }
            }

            // Draw generic Close Button
            let close_btn_hover = mouse_x >= modal_x + 140.0 && mouse_x <= modal_x + 260.0 && mouse_y >= modal_y + 180.0 && mouse_y <= modal_y + 215.0;
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + 180.0,
                120.0,
                35.0,
                white_uv,
                if close_btn_hover { [0.18, 0.18, 0.22, 1.0] } else { [0.12, 0.12, 0.15, 1.0] },
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + 180.0,
                120.0,
                1.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + 214.0,
                120.0,
                1.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 140.0,
                modal_y + 180.0,
                1.0,
                35.0,
                white_uv,
                [0.25, 0.25, 0.3, 1.0],
            );
            self.push_quad(
                vertices,
                indices,
                modal_x + 259.0,
                modal_y + 180.0,
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
                modal_y + 203.0,
                [0.85, 0.85, 0.9, 1.0],
            );
        }
    }
}
