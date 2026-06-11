use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::ui::{UiState, UiAction};
use crate::editor::buffer::Buffer;
use crate::renderer::atlas::FontAtlas;
use crate::app::state::AppState;
use crate::app::handler::handle_action;


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

    // Update diagnostic hover detection
    let mut hover_reset = true;
    if !state.tabs.is_empty() && state.active_tab_idx < state.tabs.len() {
        let active_tab = &state.tabs[state.active_tab_idx];
        if let Some(ref active_path) = active_tab.path {
            if !active_path.starts_with("diagnostics://") {
                let main_y = ui.titlebar_height;
                let editor_top = main_y + ui.tabbar_height + ui.breadcrumb_height;
                let status_y = (size.height as f32 - ui.status_height).round();
                let editor_bottom_limit = status_y;
                
                let max_line_digits = active_tab.buffer.len().to_string().len().max(3);
                let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                let text_area_x = ui.sidebar_width + gutter_width;
                
                let scrollbar_width = ui.scrollbar_width();
                let minimap_width = ui.minimap_width();
                let sb_x = size.width as f32 - scrollbar_width;
                let minimap_x = sb_x - minimap_width;
                
                if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit - 14.0 {
                    let line_idx = (((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y).min(active_tab.buffer.len().saturating_sub(1));
                    let col_idx = (((state.mouse_x - text_area_x) / ui.buffer_char_width).floor() as usize + ui.scroll_x).min(active_tab.buffer.lines()[line_idx].chars().count());
                    
                    let new_pos = (line_idx, col_idx);
                    if ui.hover_pos != Some(new_pos) {
                        ui.hover_pos = Some(new_pos);
                        ui.hover_start = Some(std::time::Instant::now());
                        ui.hovered_diagnostic = None;
                    }
                    hover_reset = false;
                }
            }
        }
    }
    if hover_reset {
        ui.hover_pos = None;
        ui.hover_start = None;
        ui.hovered_diagnostic = None;
    }
 
    if state.is_dragging_sidebar {
        let new_width = if state.mouse_x < 30.0 { 0.0 } else { state.mouse_x.clamp(50.0, 600.0) };
        ui.sidebar_width = new_width;
        ui.target_sidebar_width = new_width;
    } else if ui.tab_scroll_is_dragging {
        let tabbar_start_x = ui.sidebar_width;
        let visible_width = size.width as f32 - tabbar_start_x;
        
        let mut total_tabs_width = 0.0f32;
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = path_opt.as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string());
            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
            let dot_reserved = 18.0f32;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
            total_tabs_width += tab_w;
        }

        let ratio = visible_width / total_tabs_width;
        let thumb_w = (visible_width * ratio).clamp(20.0, visible_width);
        let max_scroll_x = (total_tabs_width - visible_width).max(0.0);

        if max_scroll_x > 0.0 {
            let target_thumb_x = state.mouse_x - state.scroll_drag_offset_x;
            let scroll_range = visible_width - thumb_w;
            let scroll_ratio = if scroll_range > 0.0 {
                ((target_thumb_x - tabbar_start_x) / scroll_range).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.tab_scroll_x = scroll_ratio * max_scroll_x;
        } else {
            ui.tab_scroll_x = 0.0;
        }
    } else if state.is_dragging_dock_border {
        let main_y = ui.titlebar_height;
        let max_y = size.height as f32 - ui.status_height - 50.0;
        let min_y = main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0;
        let target_y = state.mouse_y.clamp(min_y, max_y);
        let new_height = size.height as f32 - ui.status_height - target_y;
        ui.dock_height = new_height;
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
        
        let active_tab = &mut state.tabs[state.active_tab_idx];
        let clicked_line = if minimap_total_h > total_editor_height {
            let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
            (scroll_ratio * (active_tab.buffer.len() - 1) as f32).round() as usize
        } else {
            (relative_y / minimap_line_height).floor() as usize
        };
        let clicked_line = clicked_line.min(active_tab.buffer.len() - 1);
        
        active_tab.cursor.line = clicked_line;
        let line_chars = active_tab.buffer.lines()[clicked_line].chars().count();
        active_tab.cursor.col = active_tab.cursor.col.min(line_chars);
        active_tab.cursor.intended_col = active_tab.cursor.col;
        
        ui.scroll_y = clicked_line.saturating_sub(visible_lines / 2).min(max_scroll as usize);
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
 
    let any_dragging = state.is_dragging_sidebar
        || state.is_dragging_dock_border
        || state.is_dragging_scroll
        || state.is_dragging_horizontal_scroll
        || state.is_dragging_minimap
        || state.is_dragging
        || ui.tab_scroll_is_dragging;

    if any_dragging {
        window.request_redraw();
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
            // Check if click is on tab scrollbar
            let tabbar_start_x = ui.sidebar_width;
            let visible_width = size.width as f32 - tabbar_start_x;
            
            // Calculate total width of all tabs
            let mut total_tabs_width = 0.0f32;
            let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
            let close_reserved = 8.0f32 + tab_close_icon_sz;
            let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
            for idx in 0..tab_paths.len() {
                let path_opt = &tab_paths[idx];
                let file_name = path_opt.as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "untitled.txt".to_string());
                let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                let dot_reserved = 18.0f32;
                let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                total_tabs_width += tab_w;
            }

            if total_tabs_width > visible_width {
                let main_y = ui.titlebar_height;
                let is_on_scrollbar = state.mouse_y >= main_y + ui.tabbar_height - 6.0
                    && state.mouse_y < main_y + ui.tabbar_height
                    && state.mouse_x >= tabbar_start_x
                    && state.mouse_x < size.width as f32;

                if is_on_scrollbar {
                    let ratio = visible_width / total_tabs_width;
                    let thumb_w = (visible_width * ratio).clamp(20.0, visible_width);
                    let max_scroll_x = total_tabs_width - visible_width;
                    let scroll_ratio_x = if max_scroll_x > 0.0 { ui.tab_scroll_x / max_scroll_x } else { 0.0 };
                    let thumb_x = tabbar_start_x + scroll_ratio_x * (visible_width - thumb_w);

                    ui.tab_scroll_is_dragging = true;
                    let is_on_thumb = state.mouse_x >= thumb_x && state.mouse_x < thumb_x + thumb_w;
                    if is_on_thumb {
                        state.scroll_drag_offset_x = state.mouse_x - thumb_x;
                    } else {
                        state.scroll_drag_offset_x = thumb_w / 2.0;
                        let target_thumb_x = state.mouse_x - state.scroll_drag_offset_x;
                        let target_ratio = if visible_width - thumb_w > 0.0 {
                            (target_thumb_x - tabbar_start_x) / (visible_width - thumb_w)
                        } else {
                            0.0
                        };
                        ui.tab_scroll_x = (target_ratio * max_scroll_x).clamp(0.0, max_scroll_x);
                    }
                    window.request_redraw();
                    return;
                }
            }

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
                        &state.dock_terminals,
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
                            &state.dock_terminals,
                        )
                    };
 
                    match action_res {
                        UiAction::None => {
                            let active_tab_idx = state.active_tab_idx;
                            let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                            
                            let main_y = ui.titlebar_height;
                            let mut dock_start_y = size.height as f32 - ui.status_height;
                            if ui.show_dock {
                                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
                            }
                            let editor_bottom_limit = if ui.show_dock {
                                dock_start_y
                            } else {
                                size.height as f32 - ui.status_height
                            };
                            let editor_height = editor_bottom_limit - editor_top - 14.0;
                            
                            let active_tab_len = state.tabs[active_tab_idx].buffer.len();
                            let max_line_digits = active_tab_len.to_string().len().max(3);
                            let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                            let text_area_x = ui.sidebar_width + gutter_width;
                            let scrollbar_width = ui.scrollbar_width();
                            let minimap_width = ui.minimap_width();
                            let sb_x = size.width as f32 - scrollbar_width;
                            let minimap_x = sb_x - minimap_width;
                            let text_viewport_w = (minimap_x - text_area_x).max(10.0);

                            // 1. Check if click is on minimap
                            if state.mouse_x >= minimap_x && state.mouse_x < sb_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit {
                                state.is_dragging_minimap = true;
                                let total_editor_height = editor_bottom_limit - editor_top;
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                let max_scroll = (active_tab_len as isize - visible_lines as isize).max(0) as f32;
                                let relative_y = state.mouse_y - editor_top;
                                
                                let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
                                let minimap_total_h = active_tab_len as f32 * minimap_line_height;
                                
                                let clicked_line = if minimap_total_h > total_editor_height {
                                    let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
                                    (scroll_ratio * (active_tab_len - 1) as f32).round() as usize
                                } else {
                                    (relative_y / minimap_line_height).floor() as usize
                                };
                                let clicked_line = clicked_line.min(active_tab_len - 1);
                                
                                let active_tab = &mut state.tabs[active_tab_idx];
                                active_tab.cursor.line = clicked_line;
                                let line_chars = active_tab.buffer.lines()[clicked_line].chars().count();
                                active_tab.cursor.col = active_tab.cursor.col.min(line_chars);
                                active_tab.cursor.intended_col = active_tab.cursor.col;
                                
                                ui.scroll_y = clicked_line.saturating_sub(visible_lines / 2).min(max_scroll as usize);
                                window.request_redraw();
                            }
                            // 2. Check if click is on scrollbar
                            else if state.mouse_x >= sb_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit {
                                state.is_dragging_scroll = true;
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                let ratio = visible_lines as f32 / active_tab_len as f32;
                                let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
                                let max_scroll = (active_tab_len as isize - visible_lines as isize).max(0) as f32;
                                
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
                            else if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= editor_bottom_limit - 14.0 && state.mouse_y < editor_bottom_limit {
                                state.is_dragging_horizontal_scroll = true;
                                let active_tab = &mut state.tabs[active_tab_idx];
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
                                 if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit - 14.0 {
                                     // 1. Check if copy button of hover diagnostic was clicked
                                     let mut copy_clicked = false;
                                     if let Some(diag) = &ui.hovered_diagnostic {
                                         if let Some((line_idx, col_idx)) = ui.hover_pos {
                                             let char_x = text_area_x + (col_idx as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;
                                             let char_y = editor_top + (line_idx as isize - ui.scroll_y as isize) as f32 * ui.buffer_line_height;
                                             
                                             let label = match diag.severity {
                                                 1 => "Syntax Error",
                                                 2 => "Warning",
                                                 _ => "Info",
                                             };
                                             let max_w = 400.0f32;
                                             let max_chars_per_line = (max_w / ui.ui_char_width).floor() as usize;
                                             let full_message = if diag.message.is_empty() { label.to_string() } else { format!("{}: {}", label, diag.message) };
                                             
                                             let mut lines = Vec::new();
                                             let words: Vec<&str> = full_message.split_whitespace().collect();
                                             let mut current_line = String::new();
                                             for word in words {
                                                 if current_line.is_empty() {
                                                     current_line = word.to_string();
                                                 } else if (current_line.chars().count() + 1 + word.chars().count()) <= max_chars_per_line {
                                                     current_line.push(' ');
                                                     current_line.push_str(word);
                                                 } else {
                                                     lines.push(current_line);
                                                     current_line = word.to_string();
                                                 }
                                             }
                                             if !current_line.is_empty() { lines.push(current_line); }
                                             
                                             let line_count = lines.len();
                                             let popup_w = (lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * ui.ui_char_width + 24.0 + 20.0).max(120.0);
                                             let popup_h = line_count as f32 * ui.ui_line_height + 16.0;
                                             
                                             let mut popup_x = char_x.round();
                                             let mut popup_y = (char_y + ui.buffer_line_height + 4.0).round();
                                             
                                             if popup_x + popup_w > minimap_x {
                                                 popup_x = (minimap_x - popup_w - 10.0).max(text_area_x + 10.0);
                                             }
                                             if popup_y + popup_h > editor_top + editor_height {
                                                 popup_y = (char_y - popup_h - 4.0).max(editor_top + 4.0);
                                             }
                                             
                                             let copy_x = (popup_x + popup_w - 24.0).round();
                                             let copy_y = (popup_y + (popup_h - 11.0) / 2.0).round();
                                             
                                             if state.mouse_x >= copy_x - 4.0 && state.mouse_x < copy_x + 15.0 && state.mouse_y >= copy_y - 4.0 && state.mouse_y < copy_y + 15.0 {
                                                 state.internal_clipboard = diag.message.clone();
                                                 ui.hovered_diagnostic = None;
                                                 ui.hover_start = None;
                                                 ui.hover_pos = None;
                                                 copy_clicked = true;
                                                 window.request_redraw();
                                             }
                                         }
                                     }
                                     if copy_clicked {
                                         return;
                                     }

                                      // 2. Check if virtual diagnostics tab item was clicked
                                      let mut clicked_info = None;
                                      if state.tabs[active_tab_idx].path.as_deref() == Some("diagnostics://project") {
                                          let clicked_target = ui.diagnostics_click_targets.iter().find(|t| {
                                              state.mouse_x >= t.0 && state.mouse_x <= t.2 && state.mouse_y >= t.1 && state.mouse_y <= t.3
                                          }).cloned();
                                          if let Some((target_path, target_line, target_col)) = clicked_target.map(|t| (t.4, t.5, t.6)) {
                                              clicked_info = Some((target_path, target_line, target_col));
                                          }
                                      }
                                      
                                      if let Some((target_path, target_line, target_col)) = clicked_info {
                                          // Open the file
                                          let open_action = crate::ui::UiAction::OpenFile(std::path::PathBuf::from(target_path));
                                          crate::app::handler::handle_action(
                                              ui,
                                              state,
                                              open_action,
                                              window,
                                              elwt,
                                              gpu,
                                              atlas,
                                              font_bytes,
                                          );
                                          
                                          // Set cursor on new active tab
                                          let new_active_tab = &mut state.tabs[state.active_tab_idx];
                                          new_active_tab.cursor.line = target_line;
                                          new_active_tab.cursor.col = target_col;
                                          new_active_tab.cursor.intended_col = target_col;
                                          new_active_tab.cursor.selection_anchor = Some((target_line, target_col));
                                          ui.scroll_to_cursor(&new_active_tab.cursor, new_active_tab.buffer.len(), size.width as f32, size.height as f32);
                                          window.request_redraw();
                                          return;
                                      }

                                     // Normal click
                                     let active_tab = &mut state.tabs[active_tab_idx];
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
            // Button Released
            ui.tab_scroll_is_dragging = false;
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
 
    let size = window.inner_size();

    // Handle Tab Bar Scroll
    let tabbar_start_x = ui.sidebar_width;
    if state.mouse_y >= ui.titlebar_height 
        && state.mouse_y < ui.titlebar_height + ui.tabbar_height
        && state.mouse_x >= tabbar_start_x
        && state.mouse_x < size.width as f32 
    {
        let mut total_tabs_width = 0.0f32;
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = path_opt.as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string());
            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
            let dot_reserved = 18.0f32;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
            total_tabs_width += tab_w;
        }

        let visible_width = size.width as f32 - tabbar_start_x;
        let max_scroll_x = (total_tabs_width - visible_width).max(0.0);

        let scroll_amount = match delta {
            MouseScrollDelta::LineDelta(dx, dy) => {
                let val = if dx.abs() > dy.abs() { dx } else { -dy };
                val * 24.0
            }
            MouseScrollDelta::PixelDelta(pos) => {
                let val = if pos.x.abs() > pos.y.abs() { pos.x } else { -pos.y };
                val as f32
            }
        };

        ui.tab_scroll_x = (ui.tab_scroll_x + scroll_amount).clamp(0.0, max_scroll_x);
        window.request_redraw();
        return;
    }

    // Handle Terminal Dock Scroll
    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let is_mouse_over_terminal = ui.show_dock 
        && state.mouse_y >= dock_start_y + 28.0 
        && state.mouse_y < size.height as f32 - ui.status_height
        && state.mouse_x >= ui.sidebar_width;

    if is_mouse_over_terminal && !state.dock_terminals.is_empty() {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => dy as isize * 3,
            MouseScrollDelta::PixelDelta(pos) => ((pos.y / 15.0) * 3.0) as isize,
        };
        let active_term = &mut state.dock_terminals[state.active_terminal_idx];
        let max_scroll = active_term.grid.scrollback.len() as isize;
        let new_offset = active_term.grid.scroll_offset as isize + scroll_lines;
        active_term.grid.scroll_offset = new_offset.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }

    // Handle Sidebar Scroll
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
 
        let max_line_len = state.tabs[state.active_tab_idx].buffer.lines().iter().map(|l: &String| l.chars().count()).max().unwrap_or(0);
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
