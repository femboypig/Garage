use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;
use crate::terminal::TerminalInstance;
use super::{UiState, UiAction, ModalType};

impl UiState {
    pub fn get_max_line_len(&mut self, buffer: &Buffer, active_file_path: Option<&str>, cursor_line: usize) -> usize {
        let mut raw_max = buffer.max_line_len();
        if self.config.show_git_blame {
            if let Some(blame_str) = self.get_or_update_blame(active_file_path, cursor_line) {
                if blame_str != "Loading blame..." && !blame_str.is_empty() {
                    let cursor_line_len = buffer.lines().get(cursor_line).map_or(0, |l| l.chars().count()) + 4 + blame_str.chars().count();
                    if cursor_line_len > raw_max {
                        raw_max = cursor_line_len;
                    }
                }
            }
        }
        raw_max
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

    /// Basic syntax highlighting (currently returns default theme color)
    pub fn get_line_char_colors(&self, line_text: &str, _path_opt: Option<&str>) -> Vec<[f32; 4]> {
        vec![self.config.theme.syntax_default; line_text.chars().count()]
    }


    pub fn update_git_branch(&mut self) {
        crate::git::update_git_branch(self.git_branch_tx.clone(), self.event_loop_proxy.clone());
    }

    pub fn update_git_statuses(&mut self) {
        crate::git::update_git_statuses(self.git_status_tx.clone(), self.event_loop_proxy.clone());
    }

    pub fn update_git_diff(&mut self, file_path: Option<&str>) {
        if let Some(path) = file_path {
            crate::git::update_git_diff(
                path.to_string(),
                self.git_diff_tx.clone(),
                self.event_loop_proxy.clone(),
            );
        }
    }

    pub fn get_or_update_blame(&self, file_path: Option<&str>, line_idx: usize) -> Option<String> {
        let file_path = file_path?;
        if let Some(blame_map) = self.git_file_blames.get(file_path) {
            blame_map.get(&line_idx).cloned()
        } else {
            None
        }
    }

    pub fn update_git_file_blame(&mut self, file_path: Option<&str>) {
        if let Some(path) = file_path {
            crate::git::update_git_file_blame(
                path.to_string(),
                self.git_blame_file_tx.clone(),
                self.event_loop_proxy.clone(),
            );
        }
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
        inactive_panes: &[crate::app::state::Pane],
        active_pane_idx: usize,
        terminals: &[TerminalInstance],
        active_terminal_idx: usize,
        terminal_focus: bool,
        _is_window_maximized: bool,
    ) {
        self.active_dock_tab = active_terminal_idx;

        // Drain git branch channel
        if let Some(ref rx) = self.git_branch_rx {
            while let Ok(branch) = rx.try_recv() {
                self.git_branch = Some(branch);
            }
        }

        // Drain git file blame channel
        if let Some(ref rx) = self.git_blame_file_rx {
            while let Ok((file, blame_map)) = rx.try_recv() {
                self.git_file_blames.insert(file, blame_map);
            }
        }

        // Drain git status channel
        if let Some(ref rx) = self.git_status_rx {
            while let Ok(statuses) = rx.try_recv() {
                self.git_statuses = statuses;
            }
        }

        // Drain git diff channel
        if let Some(ref rx) = self.git_diff_rx {
            while let Ok((file, hunks)) = rx.try_recv() {
                self.git_diffs.insert(file, hunks);
            }
        }

        // Throttled git branch, status and diff check (every 1 second)
        if self.last_branch_check.is_none() || self.last_branch_check.unwrap().elapsed() > std::time::Duration::from_secs(1) {
            if self.config.show_git_branch {
                self.update_git_branch();
            }
            self.update_git_statuses();
            if active_tab_idx < tab_paths.len() {
                if let Some(ref file_path) = tab_paths[active_tab_idx] {
                    self.update_git_diff(Some(file_path));
                }
            }
            self.last_branch_check = Some(std::time::Instant::now());
        }

        if active_tab_idx < tab_paths.len() {
            if let Some(ref file_path) = tab_paths[active_tab_idx] {
                if !self.git_file_blames.contains_key(file_path) {
                    self.update_git_file_blame(Some(file_path));
                }
            }
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
        crate::ui::components::titlebar::draw_titlebar(
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
        crate::ui::components::sidebar::draw_sidebar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            main_y,
            main_height,
            mouse_x,
            mouse_y,
            tab_paths,
            tab_modified,
        );

        // --- 3. Draw Editor Tabbar, Breadcrumbs, Text Area, Gutter, Scrollbars & Minimap ---
        let sidebar_original = self.sidebar_width;
        
        if inactive_panes.is_empty() {
            crate::ui::components::editor_view::draw_editor_view(
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
        } else {
            let editor_area_width = width - sidebar_original;
            let pane_width = editor_area_width / 2.0;
            
            // Draw Left Pane (Pane 0)
            let (p0_buffer, p0_cursor, p0_paths, p0_modified, p0_active_idx) = if active_pane_idx == 0 {
                (buffer, cursor, tab_paths.to_vec(), tab_modified.to_vec(), active_tab_idx)
            } else {
                let inactive_pane = &inactive_panes[0];
                let in_paths: Vec<Option<String>> = inactive_pane.tabs.iter().map(|t| t.path.clone()).collect();
                let in_modified: Vec<bool> = inactive_pane.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let in_active_idx = inactive_pane.active_tab_idx;
                let active_tab = &inactive_pane.tabs[in_active_idx];
                (&active_tab.buffer, &active_tab.cursor, in_paths, in_modified, in_active_idx)
            };
            
            // Left pane ends at sidebar_original + pane_width
            let left_pane_width = sidebar_original + pane_width;
            
            crate::ui::components::editor_view::draw_editor_view(
                self,
                vertices,
                indices,
                atlas,
                queue,
                p0_buffer,
                p0_cursor,
                left_pane_width,
                mouse_x,
                mouse_y,
                &p0_paths,
                &p0_modified,
                p0_active_idx,
                status_y,
            );
            
            // Draw Right Pane (Pane 1)
            let (p1_buffer, p1_cursor, p1_paths, p1_modified, p1_active_idx) = if active_pane_idx == 1 {
                (buffer, cursor, tab_paths.to_vec(), tab_modified.to_vec(), active_tab_idx)
            } else {
                let inactive_pane = &inactive_panes[0];
                let in_paths: Vec<Option<String>> = inactive_pane.tabs.iter().map(|t| t.path.clone()).collect();
                let in_modified: Vec<bool> = inactive_pane.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let in_active_idx = inactive_pane.active_tab_idx;
                let active_tab = &inactive_pane.tabs[in_active_idx];
                (&active_tab.buffer, &active_tab.cursor, in_paths, in_modified, in_active_idx)
            };
            
            // Temporarily shift sidebar_width to start right pane at sidebar_original + pane_width
            self.sidebar_width = sidebar_original + pane_width;
            
            crate::ui::components::editor_view::draw_editor_view(
                self,
                vertices,
                indices,
                atlas,
                queue,
                p1_buffer,
                p1_cursor,
                width,
                mouse_x,
                mouse_y,
                &p1_paths,
                &p1_modified,
                p1_active_idx,
                status_y,
            );
            
            // Draw vertical split separator border line between the two panes
            let white_uv = atlas.white_pixel_uv();
            self.push_quad(
                vertices,
                indices,
                sidebar_original + pane_width - 1.0,
                self.titlebar_height,
                1.0,
                status_y - self.titlebar_height,
                white_uv,
                self.config.theme.modal_border,
            );
            
            // Restore sidebar_width
            self.sidebar_width = sidebar_original;
        }

        // --- 4.5. Draw Bottom Dock ---
        crate::ui::components::dock::draw_dock(
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
        let active_path = tab_paths.get(active_tab_idx).and_then(|p| p.as_deref());
        crate::ui::components::statusbar::draw_statusbar(
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
            active_path,
        );

        // --- 6. Draw Context Dropdown Menus & 7. Modal Dialogs ---
        crate::ui::components::modals::draw_modals(
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
