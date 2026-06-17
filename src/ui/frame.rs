use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;
use crate::terminal::TerminalInstance;
use super::{UiState, UiAction, ModalType, CommandPaletteMode};

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

    pub fn draw_split_preview(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        white_uv: [f32; 2],
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        // Fill with selection bg
        self.push_quad(
            vertices,
            indices,
            x,
            y,
            w,
            h,
            white_uv,
            self.config.theme.selection_bg,
        );
        // 2px solid border using cursor_color (theme's primary accent color)
        let border_color = self.config.theme.cursor_color;
        self.push_quad(vertices, indices, x, y, w, 2.0, white_uv, border_color); // top
        self.push_quad(vertices, indices, x, y + h - 2.0, w, 2.0, white_uv, border_color); // bottom
        self.push_quad(vertices, indices, x, y, 2.0, h, white_uv, border_color); // left
        self.push_quad(vertices, indices, x + w - 2.0, y, 2.0, h, white_uv, border_color); // right
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
        crate::machkit::context::push_quad_raw(vertices, indices, white_uv, x, y, w, h, color);
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
        let white_uv = atlas.white_pixel_uv();
        let mut ctx = crate::machkit::UiContext {
            vertices,
            indices,
            atlas,
            queue,
            mouse_x: 0.0,
            mouse_y: 0.0,
            theme: &self.config.theme,
            white_uv,
            ui_font_size: self.ui_font_size,
            ui_char_width: self.ui_char_width,
            ui_font_ascent: self.ui_font_ascent,
            ui_line_height: self.ui_line_height,
            buffer_font_size: self.buffer_font_size,
            buffer_font_ascent: self.buffer_font_ascent,
            buffer_line_height: self.buffer_line_height,
        };
        ctx.push_char(c, pen_x, baseline_y, color, font_size, char_width)
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
        let white_uv = atlas.white_pixel_uv();
        let mut ctx = crate::machkit::UiContext {
            vertices,
            indices,
            atlas,
            queue,
            mouse_x: 0.0,
            mouse_y: 0.0,
            theme: &self.config.theme,
            white_uv,
            ui_font_size: self.ui_font_size,
            ui_char_width: self.ui_char_width,
            ui_font_ascent: self.ui_font_ascent,
            ui_line_height: self.ui_line_height,
            buffer_font_size: self.buffer_font_size,
            buffer_font_ascent: self.buffer_font_ascent,
            buffer_line_height: self.buffer_line_height,
        };
        ctx.push_icon(icon_path, x, y, color, size)
    }

    pub fn push_str(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        font_size: f32,
        char_width: f32,
    ) -> f32 {
        let white_uv = atlas.white_pixel_uv();
        let mut ctx = crate::machkit::UiContext {
            vertices,
            indices,
            atlas,
            queue,
            mouse_x: 0.0,
            mouse_y: 0.0,
            theme: &self.config.theme,
            white_uv,
            ui_font_size: self.ui_font_size,
            ui_char_width: self.ui_char_width,
            ui_font_ascent: self.ui_font_ascent,
            ui_line_height: self.ui_line_height,
            buffer_font_size: self.buffer_font_size,
            buffer_font_ascent: self.buffer_font_ascent,
            buffer_line_height: self.buffer_line_height,
        };
        ctx.push_str_spaced(text, x, y, color, font_size, char_width)
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

    pub fn run_global_search(&mut self, query: String) {
        self.project_search_file_cache.clear();
        if query.is_empty() {
            self.global_search_results.clear();
            self.global_search_selected = 0;
            self.global_search_scroll = 0;
            self.is_searching_globally = false;
            return;
        }
        self.is_searching_globally = true;
        let tx = self.global_search_tx.clone();
        let proxy = self.event_loop_proxy.clone();
        
        let case_sensitive = self.global_search_case_sensitive;
        let whole_word = self.global_search_whole_word;
        let is_regex = self.global_search_regex;

        std::thread::spawn(move || {
            let mut results = Vec::new();
            
            let pattern = if is_regex {
                query.clone()
            } else {
                regex::escape(&query)
            };
            
            let pattern = if whole_word {
                format!(r"\b{}\b", pattern)
            } else {
                pattern
            };
            
            let mut builder = regex::RegexBuilder::new(&pattern);
            builder.case_insensitive(!case_sensitive);
            
            if let Ok(re) = builder.build() {
                let walker = ignore::WalkBuilder::new(".")
                    .hidden(true)
                    .git_ignore(true)
                    .parents(true)
                    .build();
                    
                for result in walker {
                    if let Ok(entry) = result {
                        if entry.file_type().map_or(false, |t| t.is_file()) {
                            let path = entry.path().to_path_buf();
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                for (line_idx, line) in content.lines().enumerate() {
                                    if re.is_match(line) {
                                        results.push((path.clone(), line_idx, line.trim().to_string()));
                                        if results.len() >= 100 {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if results.len() >= 100 {
                        break;
                    }
                }
            }
            let _ = tx.send(results);
            let _ = proxy.send_event(());
        });
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
        match self.command_palette_mode {
            CommandPaletteMode::Commands => vec![
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
                ("Split Editor: Vertical", "Split the active editor vertically"),
                ("Split Editor: Horizontal", "Split the active editor horizontally"),
            ],
            CommandPaletteMode::Languages => vec![
                ("Rust", ""),
                ("Python", ""),
                ("JavaScript", ""),
                ("TypeScript", ""),
                ("HTML", ""),
                ("CSS", ""),
                ("JSON", ""),
                ("TOML", ""),
                ("C", ""),
                ("C++", ""),
                ("Go", ""),
                ("Plain Text", ""),
            ],
            CommandPaletteMode::Encodings => vec![
                ("UTF-8", ""),
                ("UTF-16", ""),
                ("ASCII", ""),
                ("ISO-8859-1", ""),
            ],
            CommandPaletteMode::LineEndings => vec![
                ("LF", "Unix line endings"),
                ("CRLF", "Windows line endings"),
            ],
        }
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
        buffer: &mut Buffer,
        _cursor: &mut Cursor,
        active_path: Option<&str>,
    ) -> UiAction {
        let path_key = active_path.unwrap_or("").to_string();
        self.command_palette_mode = CommandPaletteMode::Commands;
        match cmd.0 {
            "Split Editor: Vertical" => UiAction::SplitVertical,
            "Split Editor: Horizontal" => UiAction::SplitHorizontal,
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
            // Languages
            "Rust" => { self.forced_languages.insert(path_key, "rs".to_string()); UiAction::None }
            "Python" => { self.forced_languages.insert(path_key, "py".to_string()); UiAction::None }
            "JavaScript" => { self.forced_languages.insert(path_key, "js".to_string()); UiAction::None }
            "TypeScript" => { self.forced_languages.insert(path_key, "ts".to_string()); UiAction::None }
            "HTML" => { self.forced_languages.insert(path_key, "html".to_string()); UiAction::None }
            "CSS" => { self.forced_languages.insert(path_key, "css".to_string()); UiAction::None }
            "JSON" => { self.forced_languages.insert(path_key, "json".to_string()); UiAction::None }
            "TOML" => { self.forced_languages.insert(path_key, "toml".to_string()); UiAction::None }
            "C" => { self.forced_languages.insert(path_key, "c".to_string()); UiAction::None }
            "C++" => { self.forced_languages.insert(path_key, "cpp".to_string()); UiAction::None }
            "Go" => { self.forced_languages.insert(path_key, "go".to_string()); UiAction::None }
            "Plain Text" => { self.forced_languages.insert(path_key, "".to_string()); UiAction::None }
            // Encodings
            "UTF-8" => { self.forced_encodings.insert(path_key, "UTF-8".to_string()); UiAction::None }
            "UTF-16" => { self.forced_encodings.insert(path_key, "UTF-16".to_string()); UiAction::None }
            "ASCII" => { self.forced_encodings.insert(path_key, "ASCII".to_string()); UiAction::None }
            "ISO-8859-1" => { self.forced_encodings.insert(path_key, "ISO-8859-1".to_string()); UiAction::None }
            // Line Endings
            "LF" => {
                self.forced_line_endings.insert(path_key, "LF".to_string());
                buffer.line_ending = "LF".to_string();
                UiAction::None
            }
            "CRLF" => {
                self.forced_line_endings.insert(path_key, "CRLF".to_string());
                buffer.line_ending = "CRLF".to_string();
                UiAction::None
            }
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
        secondary_cursors: &[Cursor],
        width: f32,
        height: f32,
        mouse_x: f32,
        mouse_y: f32,
        current_backend: wgpu::Backend,
        tab_paths: &[Option<String>],
        tab_modified: &[bool],
        active_tab_idx: usize,
        dragged_tab_idx: Option<usize>,
        inactive_panes: &[crate::app::state::Pane],
        active_pane_idx: usize,
        is_split_horizontal: bool,
        terminals: &[TerminalInstance],
        active_terminal_idx: usize,
        terminal_focus: bool,
        _is_window_maximized: bool,
        tab_scroll_x: f32,
    ) {
        self.tab_scroll_x = tab_scroll_x;
        self.active_dock_tab = active_terminal_idx;

        let active_file_path = tab_paths.get(active_tab_idx).and_then(|p| p.as_deref());
        let is_project_search = active_file_path == Some("search://project");
        if is_project_search {
            if self.global_show_replace {
                self.breadcrumb_height = 64.0;
            } else {
                self.breadcrumb_height = 34.0;
            }
        } else if self.show_search_panel {
            if self.show_replace {
                self.breadcrumb_height = 84.0;
            } else {
                self.breadcrumb_height = 52.0;
            }
        } else {
            self.breadcrumb_height = (self.ui_line_height * 1.3).round().max(22.0);
        }

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

        // Drain global search channel
        if let Some(ref rx) = self.global_search_rx {
            while let Ok(results) = rx.try_recv() {
                self.global_search_results = results;
                self.is_searching_globally = false;
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
                secondary_cursors,
                width,
                mouse_x,
                mouse_y,
                tab_paths,
                tab_modified,
                active_tab_idx,
                status_y,
                dragged_tab_idx,
                self.tab_scroll_x,
                true,
            );
        } else if is_split_horizontal {
            let editor_area_height = status_y - main_y;
            let pane_height = (editor_area_height / 2.0).round();
            
            // Draw Top Pane (Pane 0)
            let (p0_buffer, p0_cursor, p0_secondary, p0_paths, p0_modified, p0_active_idx) = if active_pane_idx == 0 {
                (buffer, cursor, secondary_cursors, tab_paths.to_vec(), tab_modified.to_vec(), active_tab_idx)
            } else {
                let inactive_pane = &inactive_panes[0];
                let in_paths: Vec<Option<String>> = inactive_pane.tabs.iter().map(|t| t.path.clone()).collect();
                let in_modified: Vec<bool> = inactive_pane.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let in_active_idx = inactive_pane.active_tab_idx.min(inactive_pane.tabs.len().saturating_sub(1));
                let active_tab = &inactive_pane.tabs[in_active_idx];
                (&active_tab.buffer, &active_tab.cursor, &active_tab.secondary_cursors[..], in_paths, in_modified, in_active_idx)
            };
            
            let orig_scroll_x = self.scroll_x;
            let orig_scroll_y = self.scroll_y;
            if active_pane_idx != 0 {
                let inactive_pane = &inactive_panes[0];
                if let Some(tab) = inactive_pane.tabs.get(p0_active_idx) {
                    self.scroll_x = tab.scroll_x;
                    self.scroll_y = tab.scroll_y;
                }
            }
            let p0_scroll_x = if active_pane_idx == 0 { self.tab_scroll_x } else { inactive_panes[0].tab_scroll_x };
 
            crate::ui::components::editor_view::draw_editor_view(
                self,
                vertices,
                indices,
                atlas,
                queue,
                p0_buffer,
                p0_cursor,
                p0_secondary,
                width,
                mouse_x,
                mouse_y,
                &p0_paths,
                &p0_modified,
                p0_active_idx,
                main_y + pane_height,
                if active_pane_idx == 0 { dragged_tab_idx } else { None },
                p0_scroll_x,
                active_pane_idx == 0,
            );
            
            self.scroll_x = orig_scroll_x;
            self.scroll_y = orig_scroll_y;
            
            // Draw Bottom Pane (Pane 1)
            let (p1_buffer, p1_cursor, p1_secondary, p1_paths, p1_modified, p1_active_idx) = if active_pane_idx == 1 {
                (buffer, cursor, secondary_cursors, tab_paths.to_vec(), tab_modified.to_vec(), active_tab_idx)
            } else {
                let inactive_pane = &inactive_panes[0];
                let in_paths: Vec<Option<String>> = inactive_pane.tabs.iter().map(|t| t.path.clone()).collect();
                let in_modified: Vec<bool> = inactive_pane.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let in_active_idx = inactive_pane.active_tab_idx.min(inactive_pane.tabs.len().saturating_sub(1));
                let active_tab = &inactive_pane.tabs[in_active_idx];
                (&active_tab.buffer, &active_tab.cursor, &active_tab.secondary_cursors[..], in_paths, in_modified, in_active_idx)
            };
            
            let orig_scroll_x = self.scroll_x;
            let orig_scroll_y = self.scroll_y;
            if active_pane_idx != 1 {
                let inactive_pane = &inactive_panes[0];
                if let Some(tab) = inactive_pane.tabs.get(p1_active_idx) {
                    self.scroll_x = tab.scroll_x;
                    self.scroll_y = tab.scroll_y;
                }
            }
            let p1_scroll_x = if active_pane_idx == 1 { self.tab_scroll_x } else { inactive_panes[0].tab_scroll_x };
 
            // Temporarily shift titlebar_height to start bottom pane at main_y + pane_height
            let orig_titlebar_height = self.titlebar_height;
            self.titlebar_height = main_y + pane_height;
            
            crate::ui::components::editor_view::draw_editor_view(
                self,
                vertices,
                indices,
                atlas,
                queue,
                p1_buffer,
                p1_cursor,
                p1_secondary,
                width,
                mouse_x,
                mouse_y,
                &p1_paths,
                &p1_modified,
                p1_active_idx,
                status_y,
                if active_pane_idx == 1 { dragged_tab_idx } else { None },
                p1_scroll_x,
                active_pane_idx == 1,
            );

            self.scroll_x = orig_scroll_x;
            self.scroll_y = orig_scroll_y;
            self.titlebar_height = orig_titlebar_height;
            
            // Draw horizontal split separator border line between the two panes
            let white_uv = atlas.white_pixel_uv();
            self.push_quad(
                vertices,
                indices,
                sidebar_original,
                main_y + pane_height - 1.0,
                width - sidebar_original,
                1.0,
                white_uv,
                self.config.theme.modal_border,
            );
        } else {
            let editor_area_width = width - sidebar_original;
            let pane_width = editor_area_width / 2.0;
            
            // Draw Left Pane (Pane 0)
            let (p0_buffer, p0_cursor, p0_secondary, p0_paths, p0_modified, p0_active_idx) = if active_pane_idx == 0 {
                (buffer, cursor, secondary_cursors, tab_paths.to_vec(), tab_modified.to_vec(), active_tab_idx)
            } else {
                let inactive_pane = &inactive_panes[0];
                let in_paths: Vec<Option<String>> = inactive_pane.tabs.iter().map(|t| t.path.clone()).collect();
                let in_modified: Vec<bool> = inactive_pane.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let in_active_idx = inactive_pane.active_tab_idx.min(inactive_pane.tabs.len().saturating_sub(1));
                let active_tab = &inactive_pane.tabs[in_active_idx];
                (&active_tab.buffer, &active_tab.cursor, &active_tab.secondary_cursors[..], in_paths, in_modified, in_active_idx)
            };
            
            // Left pane ends at sidebar_original + pane_width
            let left_pane_width = sidebar_original + pane_width;
            
            let orig_scroll_x = self.scroll_x;
            let orig_scroll_y = self.scroll_y;
            if active_pane_idx != 0 {
                let inactive_pane = &inactive_panes[0];
                if let Some(tab) = inactive_pane.tabs.get(p0_active_idx) {
                    self.scroll_x = tab.scroll_x;
                    self.scroll_y = tab.scroll_y;
                }
            }
            let p0_scroll_x = if active_pane_idx == 0 { self.tab_scroll_x } else { inactive_panes[0].tab_scroll_x };
 
            crate::ui::components::editor_view::draw_editor_view(
                self,
                vertices,
                indices,
                atlas,
                queue,
                p0_buffer,
                p0_cursor,
                p0_secondary,
                left_pane_width,
                mouse_x,
                mouse_y,
                &p0_paths,
                &p0_modified,
                p0_active_idx,
                status_y,
                if active_pane_idx == 0 { dragged_tab_idx } else { None },
                p0_scroll_x,
                active_pane_idx == 0,
            );
            
            self.scroll_x = orig_scroll_x;
            self.scroll_y = orig_scroll_y;
            
            // Draw Right Pane (Pane 1)
            let (p1_buffer, p1_cursor, p1_secondary, p1_paths, p1_modified, p1_active_idx) = if active_pane_idx == 1 {
                (buffer, cursor, secondary_cursors, tab_paths.to_vec(), tab_modified.to_vec(), active_tab_idx)
            } else {
                let inactive_pane = &inactive_panes[0];
                let in_paths: Vec<Option<String>> = inactive_pane.tabs.iter().map(|t| t.path.clone()).collect();
                let in_modified: Vec<bool> = inactive_pane.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let in_active_idx = inactive_pane.active_tab_idx.min(inactive_pane.tabs.len().saturating_sub(1));
                let active_tab = &inactive_pane.tabs[in_active_idx];
                (&active_tab.buffer, &active_tab.cursor, &active_tab.secondary_cursors[..], in_paths, in_modified, in_active_idx)
            };
            
            let orig_scroll_x = self.scroll_x;
            let orig_scroll_y = self.scroll_y;
            if active_pane_idx != 1 {
                let inactive_pane = &inactive_panes[0];
                if let Some(tab) = inactive_pane.tabs.get(p1_active_idx) {
                    self.scroll_x = tab.scroll_x;
                    self.scroll_y = tab.scroll_y;
                }
            }
            let p1_scroll_x = if active_pane_idx == 1 { self.tab_scroll_x } else { inactive_panes[0].tab_scroll_x };
 
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
                p1_secondary,
                width,
                mouse_x,
                mouse_y,
                &p1_paths,
                &p1_modified,
                p1_active_idx,
                status_y,
                if active_pane_idx == 1 { dragged_tab_idx } else { None },
                p1_scroll_x,
                active_pane_idx == 1,
            );

            self.scroll_x = orig_scroll_x;
            self.scroll_y = orig_scroll_y;
            
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

        // --- 8. Draw Tab Drag and Drop Overlay ---
        if dragged_tab_idx.is_some() {
            let main_y = self.titlebar_height;
            let mut dock_start_y = height - self.status_height;
            if self.show_dock {
                dock_start_y = (height - self.status_height - self.dock_height).max(main_y + self.tabbar_height + self.breadcrumb_height + 50.0);
            }
            let editor_bottom_limit = if self.show_dock {
                dock_start_y
            } else {
                height - self.status_height
            };
            
            let is_outside = mouse_x < 0.0 || mouse_x >= width || mouse_y < 0.0 || mouse_y >= height;
            let is_in_tabbar = if !inactive_panes.is_empty() && is_split_horizontal {
                let editor_area_height = editor_bottom_limit - main_y;
                let pane_height = (editor_area_height / 2.0).round();
                (mouse_y >= main_y && mouse_y < main_y + self.tabbar_height)
                    || (mouse_y >= main_y + pane_height && mouse_y < main_y + pane_height + self.tabbar_height)
            } else {
                mouse_y >= main_y && mouse_y < main_y + self.tabbar_height
            };
            let is_in_editor_area = mouse_y >= main_y + self.tabbar_height && mouse_y < editor_bottom_limit;
            let white_uv = atlas.white_pixel_uv();
            let mut overlay_color = self.config.theme.tab_active_bg;
            overlay_color[3] = 0.25; // Premium semi-transparent theme active tab highlight

            if is_outside {
                // Draw full editor area overlay with message
                let mut overlay_rect_color = self.config.theme.tabbar_bg;
                overlay_rect_color[3] = 0.2; // Theme tabbar_bg highlight
                let editor_area_width = width - sidebar_original;
                let editor_area_height = editor_bottom_limit - (main_y + self.tabbar_height);
                self.push_quad(
                    vertices,
                    indices,
                    sidebar_original,
                    main_y + self.tabbar_height,
                    editor_area_width,
                    editor_area_height,
                    white_uv,
                    overlay_rect_color,
                );
                
                // Draw a nice border around the editor area to highlight the drop target using theme active tab bg
                let border_color = self.config.theme.tabbar_border;
                let area_x = sidebar_original;
                let area_y = main_y + self.tabbar_height;
                let area_w = editor_area_width;
                let area_h = editor_area_height;
                self.push_quad(vertices, indices, area_x, area_y, area_w, 2.0, white_uv, border_color);
                self.push_quad(vertices, indices, area_x, area_y + area_h - 2.0, area_w, 2.0, white_uv, border_color);
                self.push_quad(vertices, indices, area_x, area_y, 2.0, area_h, white_uv, border_color);
                self.push_quad(vertices, indices, area_x + area_w - 2.0, area_y, 2.0, area_h, white_uv, border_color);
                
                // Draw floating tab-like banner in the center
                let msg = "Drop outside to open in a new window";
                let msg_w = msg.chars().count() as f32 * self.ui_char_width;
                let pill_w = msg_w + 32.0; // padding of 16px on each side (no decorative icons)
                let pill_h = self.tabbar_height + 4.0; // matching tab height
                let pill_x = (sidebar_original + (editor_area_width - pill_w) / 2.0).round();
                let pill_y = (main_y + self.tabbar_height + (editor_area_height - pill_h) / 2.0).round();
                
                // 1. Tab-like background
                self.push_quad(
                    vertices,
                    indices,
                    pill_x,
                    pill_y,
                    pill_w,
                    pill_h,
                    white_uv,
                    self.config.theme.tab_active_bg,
                );
                
                // 2. Tab borders
                self.push_quad(vertices, indices, pill_x, pill_y, pill_w, 1.0, white_uv, border_color); // top border
                self.push_quad(vertices, indices, pill_x, pill_y + pill_h - 1.0, pill_w, 1.0, white_uv, border_color); // bottom border
                self.push_quad(vertices, indices, pill_x, pill_y, 1.0, pill_h, white_uv, border_color); // left border
                self.push_quad(vertices, indices, pill_x + pill_w - 1.0, pill_y, 1.0, pill_h, white_uv, border_color); // right border
                
                // 3. Draw the text (properly centered vertically and horizontally)
                let text_x = (pill_x + (pill_w - msg_w) / 2.0).round();
                let text_y = (pill_y + pill_h / 2.0 + self.ui_font_ascent / 2.0 - 3.5).round();
                self.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    msg,
                    text_x,
                    text_y,
                    self.config.theme.tab_text,
                    self.ui_font_size,
                    self.ui_char_width,
                );
            } else if is_in_editor_area {
                if inactive_panes.is_empty() {
                    // Split editor visual cue
                    let editor_area_width = width - sidebar_original;
                    let editor_area_height = editor_bottom_limit - (main_y + self.tabbar_height);
                    
                    if mouse_y < main_y + self.tabbar_height + editor_area_height * 0.25 {
                        // Top pane preview split
                        self.draw_split_preview(
                            vertices,
                            indices,
                            white_uv,
                            sidebar_original,
                            main_y + self.tabbar_height,
                            editor_area_width,
                            editor_area_height * 0.5,
                        );
                    } else if mouse_y >= main_y + self.tabbar_height + editor_area_height * 0.75 {
                        // Bottom pane preview split
                        self.draw_split_preview(
                            vertices,
                            indices,
                            white_uv,
                            sidebar_original,
                            main_y + self.tabbar_height + editor_area_height * 0.5,
                            editor_area_width,
                            editor_area_height * 0.5,
                        );
                    } else if mouse_x < sidebar_original + editor_area_width * 0.5 {
                        // Left pane preview split
                        self.draw_split_preview(
                            vertices,
                            indices,
                            white_uv,
                            sidebar_original,
                            main_y + self.tabbar_height,
                            editor_area_width * 0.5,
                            editor_area_height,
                        );
                    } else {
                        // Right pane preview split
                        self.draw_split_preview(
                            vertices,
                            indices,
                            white_uv,
                            sidebar_original + editor_area_width * 0.5,
                            main_y + self.tabbar_height,
                            editor_area_width * 0.5,
                            editor_area_height,
                        );
                    }
                } else {
                    // Two panes, highlight hovered pane
                    if is_split_horizontal {
                        let editor_area_height = editor_bottom_limit - main_y;
                        let pane_height = (editor_area_height / 2.0).round();
                        
                        if mouse_y < main_y + pane_height {
                            // Top pane highlight
                            self.draw_split_preview(
                                vertices,
                                indices,
                                white_uv,
                                sidebar_original,
                                main_y + self.tabbar_height,
                                width - sidebar_original,
                                pane_height - self.tabbar_height,
                            );
                        } else {
                            // Bottom pane highlight
                            self.draw_split_preview(
                                vertices,
                                indices,
                                white_uv,
                                sidebar_original,
                                main_y + pane_height + self.tabbar_height,
                                width - sidebar_original,
                                editor_bottom_limit - (main_y + pane_height + self.tabbar_height),
                            );
                        }
                    } else {
                        let editor_area_width = width - sidebar_original;
                        let pane_width = editor_area_width / 2.0;
                        
                        if mouse_x < sidebar_original + pane_width {
                            self.draw_split_preview(
                                vertices,
                                indices,
                                white_uv,
                                sidebar_original,
                                main_y + self.tabbar_height,
                                pane_width,
                                editor_bottom_limit - (main_y + self.tabbar_height),
                            );
                        } else {
                            self.draw_split_preview(
                                vertices,
                                indices,
                                white_uv,
                                sidebar_original + pane_width,
                                main_y + self.tabbar_height,
                                pane_width,
                                editor_bottom_limit - (main_y + self.tabbar_height),
                            );
                        }
                    }
                }
            } else if is_in_tabbar && !inactive_panes.is_empty() {
                // Highlight hovered pane's tabbar
                if is_split_horizontal {
                    let editor_area_height = editor_bottom_limit - main_y;
                    let pane_height = (editor_area_height / 2.0).round();
                    if mouse_y < main_y + pane_height {
                        self.push_quad(
                            vertices,
                            indices,
                            sidebar_original,
                            main_y,
                            width - sidebar_original,
                            self.tabbar_height,
                            white_uv,
                            overlay_color,
                        );
                    } else {
                        self.push_quad(
                            vertices,
                            indices,
                            sidebar_original,
                            main_y + pane_height,
                            width - sidebar_original,
                            self.tabbar_height,
                            white_uv,
                            overlay_color,
                        );
                    }
                } else {
                    let editor_area_width = width - sidebar_original;
                    let pane_width = editor_area_width / 2.0;
                    if mouse_x < sidebar_original + pane_width {
                        self.push_quad(
                            vertices,
                            indices,
                            sidebar_original,
                            main_y,
                            pane_width,
                            self.tabbar_height,
                            white_uv,
                            overlay_color,
                        );
                    } else {
                        self.push_quad(
                            vertices,
                            indices,
                            sidebar_original + pane_width,
                            main_y,
                            pane_width,
                            self.tabbar_height,
                            white_uv,
                            overlay_color,
                        );
                    }
                }
            }

            // Draw floating tab preview under mouse cursor
            if let Some(dragged_idx) = dragged_tab_idx {
                let path_str = tab_paths.get(dragged_idx).and_then(|p| p.as_deref());
                let is_modified = tab_modified.get(dragged_idx).copied().unwrap_or(false);
                crate::ui::components::editor::tab_bar::draw_floating_tab(
                    self,
                    vertices,
                    indices,
                    atlas,
                    queue,
                    mouse_x,
                    mouse_y,
                    path_str,
                    is_modified,
                );
            }
        }
    }
}
