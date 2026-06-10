use std::sync::Arc;
use winit::window::Window;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::event_loop::EventLoopWindowTarget;
use crate::renderer::gpu::GpuContext;
use std::time::Instant;
use crate::ui::{UiState, UiAction};
use crate::editor::buffer::Buffer;
use crate::renderer::atlas::FontAtlas;
use super::state::AppState;
use super::handler::handle_action;
use std::io::Write;

pub fn update_cursor_icon(window: &Window, ui: &UiState, buffer: &Buffer, mouse_x: f32, mouse_y: f32) {
    let size = window.inner_size();
    let max_line_digits = buffer.len().to_string().len().max(3);
    let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
    let text_area_x = ui.sidebar_width + gutter_width;
    
    let on_sidebar_border = ui.sidebar_width > 0.0 && (mouse_x - ui.sidebar_width).abs() <= 4.0;
    
    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let on_dock_border = ui.show_dock && (mouse_y - dock_start_y).abs() <= 4.0;
    
    if on_sidebar_border {
        window.set_cursor_icon(winit::window::CursorIcon::ColResize);
    } else if on_dock_border {
        window.set_cursor_icon(winit::window::CursorIcon::RowResize);
    } else {
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = ui.minimap_width();
        let sb_x = size.width as f32 - scrollbar_width;
        let minimap_x = sb_x - minimap_width;

        let is_in_editor = ui.active_modal.is_none()
            && ui.active_menu.is_none()
            && mouse_x >= text_area_x
            && mouse_x < minimap_x
            && mouse_y >= ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height
            && mouse_y < dock_start_y - 14.0;
            
        if is_in_editor {
            window.set_cursor_icon(winit::window::CursorIcon::Text);
        } else {
            window.set_cursor_icon(winit::window::CursorIcon::Default);
        }
    }
}

pub fn handle_cursor_moved(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    position_x: f32,
    position_y: f32,
) {
    state.mouse_x = position_x;
    state.mouse_y = position_y;

    let size = window.inner_size();

    if state.is_dragging_sidebar {
        let new_width = if state.mouse_x < 30.0 { 0.0 } else { state.mouse_x.clamp(50.0, 600.0) };
        ui.sidebar_width = new_width;
        ui.target_sidebar_width = new_width;
    } else if state.is_dragging_dock_border {
        let main_y = ui.titlebar_height;
        let max_y = size.height as f32 - ui.status_height - 50.0;
        let min_y = main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0;
        let target_y = state.mouse_y.clamp(min_y, max_y);
        let new_height = size.height as f32 - ui.status_height - target_y;
        ui.dock_height = new_height;
        
        // Resize active terminal PTY
        if !state.dock_terminals.is_empty() {
            let width_content = size.width as f32 - ui.sidebar_width - 16.0;
            let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
            let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
            let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
            let active_term = &mut state.dock_terminals[state.active_terminal_idx];
            active_term.grid.resize(cols, rows);
            active_term.resize_pty(cols, rows);
        }
    } else if state.is_dragging_scroll {
        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
        let status_y = (size.height as f32 - ui.status_height).round();
        let editor_height = status_y - editor_top - 14.0;
        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
        let ratio = visible_lines as f32 / state.tabs[state.active_tab_idx].buffer.len() as f32;
        let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
        let max_scroll = (state.tabs[state.active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0) as f32;
        let relative_y = state.mouse_y - editor_top - state.scroll_drag_offset_y;
        let scroll_range = editor_height - thumb_h;
        let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
        ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
    } else if state.is_dragging_horizontal_scroll {
        let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = ui.minimap_width();
        let sb_x = size.width as f32 - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
        let text_viewport_w = (minimap_x - text_area_x).max(10.0);

        let max_line_len = ui.get_max_line_len(&state.tabs[state.active_tab_idx].buffer, state.tabs[state.active_tab_idx].path.as_deref(), state.tabs[state.active_tab_idx].cursor.line);
        let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
        let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
        let thumb_w = (text_viewport_w * ratio_x).clamp(20.0, text_viewport_w);
        let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
        let relative_x = state.mouse_x - text_area_x - state.scroll_drag_offset_x;
        let scroll_range = text_viewport_w - thumb_w;
        let scroll_ratio = if scroll_range > 0.0 { (relative_x / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
        ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
    } else if state.is_dragging_minimap {
        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
        let status_y = (size.height as f32 - ui.status_height).round();
        let total_editor_height = status_y - editor_top;
        let editor_height = total_editor_height - 14.0;
        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
        let max_scroll = (state.tabs[state.active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0) as f32;
        let relative_y = state.mouse_y - editor_top;
        
        let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
        let minimap_total_h = state.tabs[state.active_tab_idx].buffer.len() as f32 * minimap_line_height;
        
        if minimap_total_h > total_editor_height {
            let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
            ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
        } else {
            let line_idx = (relative_y / minimap_line_height).floor() as usize;
            ui.scroll_y = line_idx.saturating_sub(visible_lines / 2).min(max_scroll as usize);
        }
    } else if state.is_dragging {
        let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = ui.minimap_width();
        let sb_x = size.width as f32 - scrollbar_width;
        let minimap_x = sb_x - minimap_width;

        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
        let line_idx = if state.mouse_y >= editor_top {
            ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y
        } else {
            ui.scroll_y
        };
        let line_idx = line_idx.min(state.tabs[state.active_tab_idx].buffer.len() - 1);

        let mouse_x_clamped = state.mouse_x.min(minimap_x);
        let col_idx = if mouse_x_clamped > text_area_x {
            ((mouse_x_clamped - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x
        } else {
            0
        };
        let line_chars = state.tabs[state.active_tab_idx].buffer.lines()[line_idx].chars().count();
        let col_idx = col_idx.min(line_chars);

        state.tabs[state.active_tab_idx].cursor.line = line_idx;
        state.tabs[state.active_tab_idx].cursor.col = col_idx;
        state.tabs[state.active_tab_idx].cursor.intended_col = col_idx;

        ui.scroll_to_cursor(&state.tabs[state.active_tab_idx].cursor, state.tabs[state.active_tab_idx].buffer.len(), size.width as f32, size.height as f32);
    }

    update_cursor_icon(window, ui, &state.tabs[state.active_tab_idx].buffer, state.mouse_x, state.mouse_y);
}

pub fn handle_mouse_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    input_state: ElementState,
    button: MouseButton,
) {
    if button == MouseButton::Left {
        let size = window.inner_size();
        if input_state == ElementState::Pressed {
            // Check if click is on custom titlebar drag zone (CSD)
            let menu_items = ["Garage", "File", "Edit", "Selection", "View"];
            let mut menu_width = 0.0f32;
            for (i, label) in menu_items.iter().enumerate() {
                let label_len = label.chars().count() as f32;
                let text_w = label_len * ui.ui_char_width;
                let (left_pad, right_pad) = if i == 0 { (14.0, 10.0) } else { (10.0, 10.0) };
                menu_width += text_w + left_pad + right_pad;
            }
            
            let max_drag_x = if ui.is_tiling_wm() {
                size.width as f32
            } else {
                size.width as f32 - 135.0
            };
            let is_titlebar_drag_zone = state.mouse_y < ui.titlebar_height
                && state.mouse_x >= menu_width
                && state.mouse_x < max_drag_x;

            if is_titlebar_drag_zone {
                let now = Instant::now();
                let is_double_click = if let Some(last) = state.last_click_time {
                    now.duration_since(last) < std::time::Duration::from_millis(300)
                } else {
                    false
                };
                state.last_click_time = Some(now);

                if is_double_click {
                    let is_max = window.is_maximized();
                    window.set_maximized(!is_max);
                } else {
                    let _ = window.drag_window();
                }
                return;
            }

            // Calculate dock border position
            let main_y = ui.titlebar_height;
            let mut dock_start_y = size.height as f32 - ui.status_height;
            if ui.show_dock {
                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
            }
            
            // Check if focus changes
            if ui.show_dock && state.mouse_x >= ui.sidebar_width && state.mouse_y >= dock_start_y {
                state.terminal_focus = true;
            } else {
                state.terminal_focus = false;
            }

            // Check if click is on dock resize border
            let on_dock_border = ui.show_dock && (state.mouse_y - dock_start_y).abs() <= 4.0;

            if on_dock_border {
                state.is_dragging_dock_border = true;
            } else if ui.active_modal.is_some() {
                let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                let tab_modified: Vec<bool> = state.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let action_res = {
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    ui.handle_click(
                        state.mouse_x,
                        state.mouse_y,
                        size.width as f32,
                        size.height as f32,
                        &mut active_tab.buffer,
                        &mut active_tab.cursor,
                        &tab_paths,
                        &tab_modified,
                        state.dock_terminals.len(),
                    )
                };
                handle_action(ui, state, action_res, window, elwt, gpu, atlas, font_bytes);
            } else {
                // Check if click is on sidebar resize border
                let on_sidebar_border = ui.sidebar_width > 0.0 && (state.mouse_x - ui.sidebar_width).abs() <= 4.0;
                if on_sidebar_border {
                    state.is_dragging_sidebar = true;
                } else {
                    let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                    let tab_modified: Vec<bool> = state.tabs.iter().map(|t| t.buffer.is_modified).collect();
                    let action_res = {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        ui.handle_click(
                            state.mouse_x,
                            state.mouse_y,
                            size.width as f32,
                            size.height as f32,
                            &mut active_tab.buffer,
                            &mut active_tab.cursor,
                            &tab_paths,
                            &tab_modified,
                            state.dock_terminals.len(),
                        )
                    };

                    match action_res {
                        UiAction::None => {
                            let active_tab = &mut state.tabs[state.active_tab_idx];
                            let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                            let status_y = (size.height as f32 - ui.status_height).round();
                            let editor_height = status_y - editor_top - 14.0;
                            
                            let max_line_digits = active_tab.buffer.len().to_string().len().max(3);
                            let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                            let text_area_x = ui.sidebar_width + gutter_width;
                            let scrollbar_width = ui.scrollbar_width();
                            let minimap_width = ui.minimap_width();
                            let sb_x = size.width as f32 - scrollbar_width;
                            let minimap_x = sb_x - minimap_width;
                            let text_viewport_w = (minimap_x - text_area_x).max(10.0);

                            // 1. Check if click is on minimap
                            if state.mouse_x >= minimap_x && state.mouse_x < sb_x && state.mouse_y >= editor_top && state.mouse_y < size.height as f32 - ui.status_height {
                                state.is_dragging_minimap = true;
                                let total_editor_height = status_y - editor_top;
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                let max_scroll = (active_tab.buffer.len() as isize - visible_lines as isize).max(0) as f32;
                                let relative_y = state.mouse_y - editor_top;
                                
                                let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
                                let minimap_total_h = active_tab.buffer.len() as f32 * minimap_line_height;
                                
                                if minimap_total_h > total_editor_height {
                                    let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
                                    ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                                } else {
                                    let line_idx = (relative_y / minimap_line_height).floor() as usize;
                                    ui.scroll_y = line_idx.saturating_sub(visible_lines / 2).min(max_scroll as usize);
                                }
                            }
                            // 2. Check if click is on scrollbar
                            else if state.mouse_x >= sb_x && state.mouse_y >= editor_top && state.mouse_y < size.height as f32 - ui.status_height {
                                state.is_dragging_scroll = true;
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                let ratio = visible_lines as f32 / active_tab.buffer.len() as f32;
                                let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
                                let max_scroll = (active_tab.buffer.len() as isize - visible_lines as isize).max(0) as f32;
                                
                                let scroll_ratio = if max_scroll > 0.0 { ui.scroll_y as f32 / max_scroll } else { 0.0 };
                                let thumb_y = editor_top + scroll_ratio * (editor_height - thumb_h);
                                
                                if state.mouse_y >= thumb_y && state.mouse_y < thumb_y + thumb_h {
                                    state.scroll_drag_offset_y = state.mouse_y - thumb_y;
                                } else {
                                    state.scroll_drag_offset_y = thumb_h / 2.0;
                                    let relative_y = state.mouse_y - editor_top - state.scroll_drag_offset_y;
                                    let scroll_range = editor_height - thumb_h;
                                    let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                                    ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                                }
                            }
                            // 3. Check if click is on horizontal scrollbar
                            else if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= size.height as f32 - ui.status_height - 14.0 && state.mouse_y < size.height as f32 - ui.status_height {
                                state.is_dragging_horizontal_scroll = true;
                                let max_line_len = ui.get_max_line_len(&active_tab.buffer, active_tab.path.as_deref(), active_tab.cursor.line);
                                let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
                                let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
                                let thumb_w = (text_viewport_w * ratio_x).clamp(20.0, text_viewport_w);
                                let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
                                
                                let scroll_ratio_x = if max_scroll_x > 0.0 { ui.scroll_x as f32 / max_scroll_x } else { 0.0 };
                                let thumb_x = text_area_x + scroll_ratio_x * (text_viewport_w - thumb_w);
                                
                                if state.mouse_x >= thumb_x && state.mouse_x < thumb_x + thumb_w {
                                    state.scroll_drag_offset_x = state.mouse_x - thumb_x;
                                } else {
                                    state.scroll_drag_offset_x = thumb_w / 2.0;
                                    let relative_x = state.mouse_x - text_area_x - state.scroll_drag_offset_x;
                                    let scroll_range = text_viewport_w - thumb_w;
                                    let scroll_ratio = if scroll_range > 0.0 { (relative_x / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                                    ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
                                }
                            } else {
                                 // Click inside editor area
                                 if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= editor_top && state.mouse_y < size.height as f32 - ui.status_height - 14.0 {
                                     active_tab.buffer.commit_transaction();
                                     state.is_dragging = true;

                                     let line_idx = ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y;
                                     let line_idx = line_idx.min(active_tab.buffer.len() - 1);

                                     let col_idx = ((state.mouse_x - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x;
                                     let line_chars = active_tab.buffer.lines()[line_idx].chars().count();
                                     let col_idx = col_idx.min(line_chars);

                                     let extend_selection = state.modifiers.shift_key();
                                     if extend_selection {
                                         if active_tab.cursor.selection_anchor.is_none() {
                                             active_tab.cursor.selection_anchor = Some((active_tab.cursor.line, active_tab.cursor.col));
                                         }
                                     } else {
                                         active_tab.cursor.selection_anchor = Some((line_idx, col_idx));
                                     }

                                     active_tab.cursor.line = line_idx;
                                     active_tab.cursor.col = col_idx;
                                     active_tab.cursor.intended_col = col_idx;

                                     ui.scroll_to_cursor(&active_tab.cursor, active_tab.buffer.len(), size.width as f32, size.height as f32);
                                }
                            }
                        }
                        act => {
                            handle_action(ui, state, act, window, elwt, gpu, atlas, font_bytes);
                        }
                    }
                }
            }
        } else {
            let was_dragging_sidebar = state.is_dragging_sidebar;
            state.is_dragging = false;
            state.is_dragging_scroll = false;
            state.is_dragging_horizontal_scroll = false;
            state.is_dragging_minimap = false;
            state.is_dragging_sidebar = false;
            state.is_dragging_dock_border = false;
            if was_dragging_sidebar {
                ui.config.sidebar_width = ui.sidebar_width;
                ui.config.save_in_background();
            }
            if let Some((s_l, s_c, e_l, e_c)) = state.tabs[state.active_tab_idx].cursor.selection_range() {
                if s_l == e_l && s_c == e_c {
                    state.tabs[state.active_tab_idx].cursor.clear_selection();
                }
            }
        }
        update_cursor_icon(window, ui, &state.tabs[state.active_tab_idx].buffer, state.mouse_x, state.mouse_y);
        window.request_redraw();
    }
}

pub fn handle_mouse_wheel(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    delta: MouseScrollDelta,
) {
    if ui.active_modal == Some(crate::ui::ModalType::CommandPalette) {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize,
            MouseScrollDelta::PixelDelta(pos) => (pos.y / 15.0) as isize * -1,
        };
        let filtered_len = ui.get_filtered_commands().len();
        let max_visible_items = 10;
        let max_scroll = (filtered_len as isize - max_visible_items as isize).max(0);
        let new_scroll = ui.command_palette_scroll as isize + scroll_lines;
        ui.command_palette_scroll = new_scroll.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }
    if ui.active_modal.is_some() {
        return;
    }

    // Handle Sidebar Scroll
    let size = window.inner_size();
    let sidebar_top = ui.titlebar_height;
    let sidebar_bottom = size.height as f32 - ui.status_height;
    if ui.sidebar_width > 0.0 && state.mouse_x >= 0.0 && state.mouse_x < ui.sidebar_width && state.mouse_y >= sidebar_top && state.mouse_y < sidebar_bottom {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
            MouseScrollDelta::PixelDelta(pos) => ((pos.y / (ui.ui_line_height as f64)) * 3.0) as isize * -1,
        };
        let total_rows = 1 + ui.visible_nodes.len();
        let main_height = sidebar_bottom - sidebar_top;
        let visible_rows = (main_height / ui.ui_line_height).floor() as usize;
        let max_scroll = (total_rows as isize - visible_rows as isize).max(0);
        let new_scroll = ui.sidebar_scroll as isize + scroll_lines;
        ui.sidebar_scroll = new_scroll.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }

    let is_shift = state.modifiers.shift_key();
    if is_shift {
        let scroll_cols = match delta {
            MouseScrollDelta::LineDelta(dx, _) if dx != 0.0 => -dx as isize * 3,
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
            MouseScrollDelta::PixelDelta(pos) => {
                let val = if pos.x.abs() > pos.y.abs() { pos.x } else { pos.y };
                ((val / (ui.buffer_char_width as f64)) * 3.0) as isize * -1
            }
        };
        let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = ui.minimap_width();
        let sb_x = size.width as f32 - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
        let text_viewport_w = (minimap_x - text_area_x).max(10.0);
        let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;

        let max_line_len = state.tabs[state.active_tab_idx].buffer.lines().iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let max_scroll = (max_line_len as isize - visible_cols as isize).max(0);
        let new_scroll = ui.scroll_x as isize + scroll_cols;
        ui.scroll_x = new_scroll.clamp(0, max_scroll) as usize;
        state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
        window.request_redraw();
        return;
    }

    let scroll_lines = match delta {
        MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
        MouseScrollDelta::PixelDelta(pos) => ((pos.y / (ui.buffer_line_height as f64)) * 3.0) as isize * -1,
    };

    let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
    let status_y = (window.inner_size().height as f32 - ui.status_height).round();
    let editor_height = status_y - editor_top - 14.0;
    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
    let max_scroll = (state.tabs[state.active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0);

    let new_scroll = ui.scroll_y as isize + scroll_lines;
    ui.scroll_y = new_scroll.clamp(0, max_scroll) as usize;

    window.request_redraw();
}

pub fn handle_keyboard_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: Key,
    physical_key: PhysicalKey,
) {
    if state.terminal_focus && !state.dock_terminals.is_empty() {
        let active_term = &mut state.dock_terminals[state.active_terminal_idx];
        let bytes_to_write: Option<Vec<u8>> = match &logical_key {
            Key::Character(text) => {
                let ctrl = state.modifiers.control_key();
                if ctrl && text.len() == 1 {
                    let c = text.chars().next().unwrap();
                    if c.is_ascii_alphabetic() {
                        let code = c.to_ascii_uppercase() as u8 - b'A' + 1;
                        Some(vec![code])
                    } else {
                        Some(text.as_bytes().to_vec())
                    }
                } else {
                    Some(text.as_bytes().to_vec())
                }
            }
            Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
            Key::Named(NamedKey::Space) => Some(vec![b' ']),
            Key::Named(NamedKey::Backspace) => Some(vec![127]),
            Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
            Key::Named(NamedKey::Escape) => Some(vec![27]),
            Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
            Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
            Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
            Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
            Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
            Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
            _ => None,
        };

        if let Some(bytes) = bytes_to_write {
            let _ = active_term.pty_writer.write_all(&bytes);
            let _ = active_term.pty_writer.flush();
        }
        window.request_redraw();
        return;
    }

    let ctrl = state.modifiers.control_key();
    let shift = state.modifiers.shift_key();
    let alt = state.modifiers.alt_key();

    // 1. If CommandPalette modal is active, handle it specifically
    if let Some(crate::ui::ModalType::CommandPalette) = ui.active_modal {
        match &logical_key {
            Key::Named(NamedKey::Escape) => {
                ui.active_modal = None;
                window.request_redraw();
            }
            Key::Named(NamedKey::ArrowDown) => {
                let items_count = ui.get_filtered_commands().len();
                if items_count > 0 {
                    ui.command_palette_selected = (ui.command_palette_selected + 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::ArrowUp) => {
                let items_count = ui.get_filtered_commands().len();
                if items_count > 0 {
                    ui.command_palette_selected = (ui.command_palette_selected + items_count - 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                let filtered = ui.get_filtered_commands();
                if ui.command_palette_selected < filtered.len() {
                    let cmd = filtered[ui.command_palette_selected];
                    ui.active_modal = None;
                    
                    let action_res = {
                         let active_tab = &mut state.tabs[state.active_tab_idx];
                         ui.execute_command(cmd, &mut active_tab.buffer, &mut active_tab.cursor)
                     };
                     handle_action(ui, state, action_res, window, elwt, gpu, atlas, font_bytes);
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Backspace) => {
                ui.command_palette_query.pop();
                ui.command_palette_selected = 0;
                window.request_redraw();
            }
            Key::Character(text) => {
                if text.chars().count() == 1 {
                    let c = text.chars().next().unwrap();
                    if c.is_ascii_graphic() || c == ' ' {
                        ui.command_palette_query.push(c);
                        ui.command_palette_selected = 0;
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // 2. If any other modal is active, Escape closes it
    if ui.active_modal.is_some() {
        if let Key::Named(NamedKey::Escape) = &logical_key {
            ui.active_modal = None;
            window.request_redraw();
        }
        return;
    }

    // 3. Otherwise map key input to Action
    if let Some(action) = crate::editor::keymap::map_key(&logical_key, physical_key, ctrl, shift, alt) {
        match action {
            crate::editor::actions::Action::ZoomIn => {
                let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
                ui.update_buffer_font_size(&atlas.font, new_size);
            }
            crate::editor::actions::Action::ZoomOut => {
                let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
                ui.update_buffer_font_size(&atlas.font, new_size);
            }
            crate::editor::actions::Action::CommandPalette => {
                ui.active_modal = Some(crate::ui::ModalType::CommandPalette);
                ui.command_palette_query.clear();
                ui.command_palette_selected = 0;
            }
            crate::editor::actions::Action::SaveFile => {
                handle_action(ui, state, UiAction::SaveFile, window, elwt, gpu, atlas, font_bytes);
            }
            crate::editor::actions::Action::Escape => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.clear_selection();
            }
            crate::editor::actions::Action::MoveLeft { select, word } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                if word {
                    active_tab.cursor.move_word_left(&active_tab.buffer, select);
                } else {
                    active_tab.cursor.move_left(&active_tab.buffer, select);
                }
            }
            crate::editor::actions::Action::MoveRight { select, word } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                if word {
                    active_tab.cursor.move_word_right(&active_tab.buffer, select);
                } else {
                    active_tab.cursor.move_right(&active_tab.buffer, select);
                }
            }
            crate::editor::actions::Action::MoveUp { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_up(&active_tab.buffer, select);
            }
            crate::editor::actions::Action::MoveDown { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_down(&active_tab.buffer, select);
            }
            crate::editor::actions::Action::MoveToLineStart { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_to_line_start(select);
            }
            crate::editor::actions::Action::MoveToLineEnd { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_to_line_end(&active_tab.buffer, select);
            }
            crate::editor::actions::Action::SelectAll => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.selection_anchor = Some((0, 0));
                active_tab.cursor.line = active_tab.buffer.len() - 1;
                active_tab.cursor.col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                active_tab.cursor.intended_col = active_tab.cursor.col;
            }
            crate::editor::actions::Action::Copy => {
                let active_tab = &state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    state.internal_clipboard = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
                }
            }
            crate::editor::actions::Action::Cut => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    state.internal_clipboard = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.intended_col = s_c;
                    active_tab.cursor.clear_selection();
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::Paste => {
                if !state.internal_clipboard.is_empty() {
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    active_tab.buffer.start_transaction();
                    if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                        active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                        active_tab.cursor.line = s_l;
                        active_tab.cursor.col = s_c;
                        active_tab.cursor.clear_selection();
                    }
                    active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &state.internal_clipboard);

                    let parts = state.internal_clipboard.split('\n').collect::<Vec<&str>>();
                    if parts.len() == 1 {
                        active_tab.cursor.col += state.internal_clipboard.chars().count();
                    } else {
                        active_tab.cursor.line += parts.len() - 1;
                        active_tab.cursor.col = parts.last().unwrap().chars().count();
                    }
                    active_tab.cursor.intended_col = active_tab.cursor.col;
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::Undo => {
                handle_action(ui, state, UiAction::Undo, window, elwt, gpu, atlas, font_bytes);
                state.tabs[state.active_tab_idx].cursor.intended_col = state.tabs[state.active_tab_idx].cursor.col;
            }
            crate::editor::actions::Action::Redo => {
                handle_action(ui, state, UiAction::Redo, window, elwt, gpu, atlas, font_bytes);
                state.tabs[state.active_tab_idx].cursor.intended_col = state.tabs[state.active_tab_idx].cursor.col;
            }
            crate::editor::actions::Action::DeleteLeft => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.intended_col = s_c;
                    active_tab.cursor.clear_selection();
                    active_tab.buffer.commit_transaction();
                } else if active_tab.cursor.col > 0 || active_tab.cursor.line > 0 {
                    active_tab.buffer.start_transaction();
                    let is_paired = if active_tab.cursor.col > 0 {
                        let line_chars: Vec<char> = active_tab.buffer.lines()[active_tab.cursor.line].chars().collect();
                        if active_tab.cursor.col < line_chars.len() {
                            let left_char = line_chars[active_tab.cursor.col - 1];
                            let right_char = line_chars[active_tab.cursor.col];
                            match (left_char, right_char) {
                                ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"') | ('\'', '\'') => true,
                                _ => false,
                            }
                        } else { false }
                    } else { false };

                    if is_paired {
                        active_tab.buffer.delete(active_tab.cursor.line, active_tab.cursor.col - 1, active_tab.cursor.line, active_tab.cursor.col + 1);
                        active_tab.cursor.col -= 1;
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    } else {
                        let mut prev_cursor = active_tab.cursor;
                        prev_cursor.move_left(&active_tab.buffer, false);
                        active_tab.buffer.delete(prev_cursor.line, prev_cursor.col, active_tab.cursor.line, active_tab.cursor.col);
                        active_tab.cursor = prev_cursor;
                    }
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::DeleteRight => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.intended_col = s_c;
                    active_tab.cursor.clear_selection();
                    active_tab.buffer.commit_transaction();
                } else {
                    let line_len = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                    if active_tab.cursor.col < line_len || active_tab.cursor.line < active_tab.buffer.len() - 1 {
                        active_tab.buffer.start_transaction();
                        let mut next_cursor = active_tab.cursor;
                        next_cursor.move_right(&active_tab.buffer, false);
                        active_tab.buffer.delete(active_tab.cursor.line, active_tab.cursor.col, next_cursor.line, next_cursor.col);
                        active_tab.buffer.commit_transaction();
                    }
                }
            }
            crate::editor::actions::Action::InsertNewLine => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.clear_selection();
                }
                active_tab.buffer.start_transaction();
                active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, "\n");
                active_tab.cursor.line += 1;
                active_tab.cursor.col = 0;
                active_tab.cursor.intended_col = 0;
                active_tab.buffer.commit_transaction();
            }
            crate::editor::actions::Action::InsertTab => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.clear_selection();
                }
                active_tab.buffer.start_transaction();
                active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, "    ");
                active_tab.cursor.col += 4;
                active_tab.cursor.intended_col = active_tab.cursor.col;
                active_tab.buffer.commit_transaction();
            }
            crate::editor::actions::Action::InsertChar(c) => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let step_over = if active_tab.cursor.selection_range().is_none() && (c == ')' || c == ']' || c == '}' || c == '"' || c == '\'') {
                    let line_chars: Vec<char> = active_tab.buffer.lines()[active_tab.cursor.line].chars().collect();
                    if active_tab.cursor.col < line_chars.len() && line_chars[active_tab.cursor.col] == c {
                        true
                    } else { false }
                } else { false };

                if step_over {
                    active_tab.cursor.col += 1;
                    active_tab.cursor.intended_col = active_tab.cursor.col;
                } else {
                    let wrapped = if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                        let matching_close = match c {
                            '(' => Some(')'),
                            '[' => Some(']'),
                            '{' => Some('}'),
                            '"' => Some('"'),
                            '\'' => Some('\''),
                            _ => None,
                        };

                        if let Some(close_char) = matching_close {
                            active_tab.buffer.start_transaction();
                            active_tab.buffer.insert(s_l, s_c, &c.to_string());
                            let adjusted_e_c = if s_l == e_l { e_c + 1 } else { e_c };
                            active_tab.buffer.insert(e_l, adjusted_e_c, &close_char.to_string());
                            
                            if active_tab.cursor.selection_anchor.unwrap().0 == s_l && active_tab.cursor.selection_anchor.unwrap().1 == s_c {
                                active_tab.cursor.selection_anchor = Some((s_l, s_c + 1));
                                active_tab.cursor.line = e_l;
                                active_tab.cursor.col = adjusted_e_c;
                            } else {
                                active_tab.cursor.selection_anchor = Some((e_l, adjusted_e_c));
                                active_tab.cursor.line = s_l;
                                active_tab.cursor.col = s_c + 1;
                            }
                            active_tab.cursor.intended_col = active_tab.cursor.col;
                            active_tab.buffer.commit_transaction();
                            true
                        } else { false }
                    } else { false };

                    if !wrapped {
                        let paired = if active_tab.cursor.selection_range().is_none() {
                            let matching_close = match c {
                                '(' => Some(')'),
                                '[' => Some(']'),
                                '{' => Some('}'),
                                '"' => Some('"'),
                                '\'' => Some('\''),
                                _ => None,
                            };

                            if let Some(close_char) = matching_close {
                                active_tab.buffer.start_transaction();
                                let pair_str = format!("{}{}", c, close_char);
                                active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &pair_str);
                                active_tab.cursor.col += 1;
                                active_tab.cursor.intended_col = active_tab.cursor.col;
                                active_tab.buffer.commit_transaction();
                                true
                            } else { false }
                        } else { false };

                        if !paired {
                            if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                                active_tab.buffer.start_transaction();
                                active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                                active_tab.cursor.line = s_l;
                                active_tab.cursor.col = s_c;
                                active_tab.cursor.clear_selection();
                            }
                            active_tab.buffer.start_transaction();
                            active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &c.to_string());
                            active_tab.cursor.col += 1;
                            active_tab.cursor.intended_col = active_tab.cursor.col;
                            active_tab.buffer.commit_transaction();
                        }
                    }
                }
            }
        }
    }
    
    let active_tab = &state.tabs[state.active_tab_idx];
    ui.scroll_to_cursor(&active_tab.cursor, active_tab.buffer.len(), window.inner_size().width as f32, window.inner_size().height as f32);
    update_cursor_icon(window, ui, &active_tab.buffer, state.mouse_x, state.mouse_y);
    window.request_redraw();
}
