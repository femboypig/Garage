use std::path::Path;
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use super::{UiState, UiAction, MenuType, ModalType};

impl UiState {
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
        terminals: &[crate::terminal::TerminalInstance],
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
            let mut cur_x = self.sidebar_width;
            let tab_y = dock_start_y + 1.0;
            let tab_h = dock_tabbar_h - 1.0;
            
            for idx in 0..terminals.len() {
                let term_name = if terminals[idx].grid.title.is_empty() {
                    format!("terminal-{}", idx + 1)
                } else {
                    let mut name = terminals[idx].grid.title.clone();
                    if name.chars().count() > 20 {
                        let prefix: String = name.chars().take(17).collect();
                        name = format!("{}...", prefix);
                    }
                    name
                };
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
}
