use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::ui::{UiState, UiAction, MenuType};

impl UiState {
    pub fn handle_menu_click(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        buffer: &mut Buffer,
        cursor: &mut Cursor,
    ) -> Option<UiAction> {
        // 1. Check Titlebar Menu Clicks (Contiguous adjacent layout)
        if my < self.titlebar_height {
            if !self.is_tiling_wm() {
                let btn_w = 45.0f32;
                if mx >= width - btn_w {
                    return Some(UiAction::Exit);
                } else if mx >= width - btn_w * 2.0 && mx < width - btn_w {
                    return Some(UiAction::MaximizeWindow);
                } else if mx >= width - btn_w * 3.0 && mx < width - btn_w * 2.0 {
                    return Some(UiAction::MinimizeWindow);
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
                    return Some(UiAction::None);
                }
                current_x = x_max;
            }
            self.active_menu = None;
            return Some(UiAction::None);
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
                return Some(action);
            }
            return Some(UiAction::None);
        }

        None
    }
}
