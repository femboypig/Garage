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
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuType {
    File,
    Edit,
    Selection,
    View,
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
    pub target_sidebar_width: f32, // to animate or toggle
    
    // Project Tree State
    pub expanded_dirs: HashSet<PathBuf>,
    pub visible_nodes: Vec<FileNode>,
    pub selected_file: Option<PathBuf>,
    
    // Menu State
    pub active_menu: Option<MenuType>,
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
                    
                    // Skip hidden files/folders and target directories to stay optimized
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

    /// Handle click coordinates to determine if a menu or tree item was clicked
    pub fn handle_click(
        &mut self,
        mx: f32,
        my: f32,
        _width: f32,
        height: f32,
        buffer: &mut Buffer,
        cursor: &mut Cursor,
    ) -> UiAction {
        // 1. Check Titlebar Menu Clicks
        if my < self.titlebar_height {
            // File: 10..50
            if mx >= 10.0 && mx <= 60.0 {
                self.active_menu = if self.active_menu == Some(MenuType::File) { None } else { Some(MenuType::File) };
                return UiAction::None;
            }
            // Edit: 70..110
            if mx >= 70.0 && mx <= 120.0 {
                self.active_menu = if self.active_menu == Some(MenuType::Edit) { None } else { Some(MenuType::Edit) };
                return UiAction::None;
            }
            // Selection: 130..210
            if mx >= 130.0 && mx <= 210.0 {
                self.active_menu = if self.active_menu == Some(MenuType::Selection) { None } else { Some(MenuType::Selection) };
                return UiAction::None;
            }
            // View: 220..260
            if mx >= 220.0 && mx <= 270.0 {
                self.active_menu = if self.active_menu == Some(MenuType::View) { None } else { Some(MenuType::View) };
                return UiAction::None;
            }
            self.active_menu = None;
            return UiAction::None;
        }

        // 2. Check Dropdown Clicks (if active)
        if let Some(menu) = self.active_menu {
            let menu_action = match menu {
                MenuType::File => {
                    // Box: x: 10..160, y: 32..122
                    if mx >= 10.0 && mx <= 180.0 && my >= 32.0 && my <= 122.0 {
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
                    // Box: x: 70..220, y: 32..92
                    if mx >= 70.0 && mx <= 220.0 && my >= 32.0 && my <= 92.0 {
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
                    // Box: x: 130..280, y: 32..92
                    if mx >= 130.0 && mx <= 280.0 && my >= 32.0 && my <= 92.0 {
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
                    // Box: x: 220..370, y: 32..62
                    if mx >= 220.0 && mx <= 370.0 && my >= 32.0 && my <= 62.0 {
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
        let start = vertices.len() as u16;
        vertices.push(Vertex {
            position: [x, y],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [x + w, y],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [x + w, y + h],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [x, y + h],
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

            let x = pen_x + info.bearing_x;
            let y = baseline_y - info.bearing_y - info.height;
            let w = info.width;
            let h = info.height;

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

    /// Build entire UI frame (Titlebar, Sidebar, Dropdowns, Text Area, Gutter, Statusbar)
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

        // Smoothly expand/collapse sidebar width for a polished feel
        let step = 20.0;
        if self.sidebar_width < self.target_sidebar_width {
            self.sidebar_width = (self.sidebar_width + step).min(self.target_sidebar_width);
        } else if self.sidebar_width > self.target_sidebar_width {
            self.sidebar_width = (self.sidebar_width - step).max(self.target_sidebar_width);
        }

        // Calculate dynamic line number gutter width
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * self.char_width;
        let text_area_x = self.sidebar_width + gutter_width;

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
            ("File", 10.0, 60.0, MenuType::File),
            ("Edit", 70.0, 120.0, MenuType::Edit),
            ("Selection", 130.0, 210.0, MenuType::Selection),
            ("View", 220.0, 270.0, MenuType::View),
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
                    [0.16, 0.16, 0.22, 1.0], // Hover background
                );
            }
            
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                label,
                *x_min,
                20.0,
                if is_active { [1.0, 1.0, 1.0, 1.0] } else { [0.75, 0.75, 0.8, 1.0] },
            );
        }

        // Display current open file title in titlebar center
        let file_name = self.selected_file.as_ref()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        let title_str = format!("Garage - {}", file_name);
        let title_width = title_str.chars().count() as f32 * self.char_width;
        let title_x = (width - title_width) / 2.0;
        if title_x > 300.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &title_str,
                title_x,
                20.0,
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
                [0.05, 0.05, 0.07, 1.0], // Darker sidebar background
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

            // Render project tree files/folders
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

                // Draw tree structure indents
                let indent_x = 10.0 + node.depth as f32 * 12.0;
                let icon = if node.is_dir {
                    if self.expanded_dirs.contains(&node.path) { "▼ " } else { "▶ " }
                } else {
                    "  "
                };

                let text_color = if node.is_dir {
                    [0.8, 0.8, 0.85, 1.0] // Light gray folder
                } else {
                    [0.65, 0.65, 0.7, 1.0] // Off-white file
                };

                let node_text = format!("{}{}", icon, node.name);
                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &node_text,
                    indent_x,
                    row_y + self.font_ascent,
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
            [0.07, 0.07, 0.09, 1.0], // Editor sidebar background
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
            let baseline_y = row_y + self.font_ascent;

            // Current active line highlight
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

            // Draw selections
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
                            [0.15, 0.25, 0.42, 0.6], // Semitransparent blue highlight
                        );
                    }
                }
            }

            // Draw source code text
            let line_text = &buffer.lines()[line_idx];
            let mut pen_x = text_area_x;
            
            for c in line_text.chars() {
                let char_color = match c {
                    '0'..='9' => [0.85, 0.6, 0.35, 1.0], // Numbers
                    '{' | '}' | '(' | ')' | '[' | ']' => [0.8, 0.8, 0.3, 1.0], // Brackets
                    _ => [0.85, 0.85, 0.9, 1.0], // Normal characters
                };
                pen_x += self.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color);
            }
        }

        // Draw active text cursor
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
                [0.0, 0.9, 0.8, 1.0], // Teal cursor
            );
        }

        // --- 4. Draw Statusbar ---
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
            status_y + self.font_ascent + 2.0,
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
                status_y + self.font_ascent + 2.0,
                [0.5, 0.5, 0.55, 1.0],
            );
        }

        // --- 5. Draw Context Dropdown Menus (On top of everything) ---
        if let Some(menu) = self.active_menu {
            let (menu_x, dropdown_w, dropdown_h, items) = match menu {
                MenuType::File => (
                    10.0,
                    170.0,
                    90.0,
                    vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
                ),
                MenuType::Edit => (
                    70.0,
                    150.0,
                    60.0,
                    vec!["Undo (Ctrl+Z)", "Redo (Ctrl+Y)"],
                ),
                MenuType::Selection => (
                    130.0,
                    150.0,
                    60.0,
                    vec!["Select All", "Clear Selection"],
                ),
                MenuType::View => (
                    220.0,
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
                [0.08, 0.08, 0.1, 0.98], // Translucent dark background
            );
            // Draw thin card border
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
                    row_y + 20.0,
                    if is_hovered { [1.0, 1.0, 1.0, 1.0] } else { [0.75, 0.75, 0.8, 1.0] },
                );
            }
        }
    }
}
