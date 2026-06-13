use std::path::Path;
use crate::ui::{UiState, UiAction};

impl UiState {
    pub fn handle_workspace_click(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
        tab_paths: &[Option<String>],
        tab_modified: &[bool],
        terminals: &[crate::terminal::TerminalInstance],
    ) -> UiAction {
        // 3. Check Tabbar Clicks
        let main_y = self.titlebar_height;
        if my >= main_y && my < main_y + self.tabbar_height {
            let activity_bar_width = 0.0;
            let tabbar_start_x = activity_bar_width + self.sidebar_width;

            // Check if clicking in the scrollbar area (bottom 6px of the tab bar)
            if my >= main_y + self.tabbar_height - 6.0 {
                return UiAction::None;
            }

            // Make sure the click is within the visible tab bar area
            if mx >= tabbar_start_x && mx < width {
                // Check actual file tabs
                let tab_close_icon_sz = (self.ui_font_size * 0.8).round().max(10.0);
                let mut current_tab_x = tabbar_start_x;
                let close_reserved = 8.0f32 + tab_close_icon_sz;

                for idx in 0..tab_paths.len() {
                    let path_opt = &tab_paths[idx];
                    let _is_modified = tab_modified.get(idx).copied().unwrap_or(false);
                    let dot_reserved = 18.0f32;
                    let is_diagnostics = path_opt.as_deref() == Some("diagnostics://project");
                    let file_name = if is_diagnostics {
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
                    } else {
                        path_opt.as_ref()
                            .and_then(|p| Path::new(p).file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "untitled.txt".to_string())
                    };

                    let name_w = file_name.chars().count() as f32 * self.ui_char_width;
                    let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);

                    let draw_x = current_tab_x - self.tab_scroll_x;
                    let clip_left = draw_x.max(tabbar_start_x);
                    let clip_right = (draw_x + tab_w).min(width);

                    if mx >= clip_left && mx < clip_right {
                        // Check if clicked the close button
                        let close_x = draw_x + tab_w - 10.0 - tab_close_icon_sz;
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
            }

            self.active_menu = None;
            return UiAction::None;
        }

        // 4. Check Sidebar Clicks
        let activity_bar_width = 0.0;
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
            let mut cur_x = self.sidebar_width;
            let tab_y = dock_start_y + 1.0;
            let tab_h = dock_tabbar_h - 1.0;
            
            for idx in 0..terminals.len() {
                let term_name = terminals[idx].get_display_name(idx);
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

            // Check if clicked the terminal toggle button
            if mx >= term_btn_x && mx < term_btn_x + sb_btn_w {
                return UiAction::ToggleDock;
            }

            // Check if clicked the diagnostics indicator
            let mut pen_x = 10.0;
            if self.config.show_git_branch {
                if let Some(ref branch) = self.git_branch {
                    let icon_sz = (self.ui_font_size * 0.9).round().max(12.0);
                    pen_x += icon_sz + 4.0;
                    let branch_len = branch.chars().count() as f32;
                    pen_x += branch_len * self.ui_char_width;
                    pen_x += 15.0;
                }
            }
            let mut err_count = 0;
            let mut warn_count = 0;
            for (e, w) in self.lsp_diagnostics.values() {
                err_count += *e;
                warn_count += *w;
            }
            let err_str = format!("⊗ {}  ", err_count);
            let warn_str = format!("⚠ {}", warn_count);
            let diag_w = (err_str.chars().count() + warn_str.chars().count()) as f32 * self.ui_char_width;

            if mx >= pen_x && mx <= pen_x + diag_w {
                return UiAction::OpenFile("diagnostics://project".into());
            }
        }

        UiAction::None
    }
}
