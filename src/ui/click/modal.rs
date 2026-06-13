use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::ui::{UiState, UiAction, ModalType};

impl UiState {
    pub fn handle_modal_click(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        _tab_paths: &[Option<String>],
        _tab_modified: &[bool],
        modal: ModalType,
    ) -> UiAction {
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

        UiAction::None
    }
}
