use winit::window::Window;
use crate::ui::UiState;
use crate::editor::buffer::Buffer;
use crate::app::state::AppState;

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
    ui.hover_pos = None;
    ui.hover_start = None;
    ui.hovered_diagnostic = None;
    ui.mouse_in_popup = false;
    ui.hovered_copy_button = false;
 
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
        let active_path = state.tabs[state.active_tab_idx].path.as_deref().unwrap_or("");
        let is_diagnostics = active_path.starts_with("diagnostics://");

        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
        let status_y = (size.height as f32 - ui.status_height).round();

        let show_horizontal_scrollbar = if is_diagnostics {
            false
        } else {
            let max_line_len = ui.get_max_line_len(&state.tabs[state.active_tab_idx].buffer, Some(active_path), state.tabs[state.active_tab_idx].cursor.line);
            let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
            let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
            let text_area_x = ui.sidebar_width + gutter_width;
            let scrollbar_width = ui.scrollbar_width();
            let minimap_width = ui.minimap_width();
            let sb_x = size.width as f32 - scrollbar_width;
            let minimap_x = sb_x - minimap_width;
            let text_viewport_w = (minimap_x - text_area_x).max(10.0);
            let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
            max_line_len > visible_cols
        };
        let hs_height = if show_horizontal_scrollbar { 14.0 } else { 0.0 };
        let editor_height = status_y - editor_top - hs_height;
        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;

        let virtual_len = if is_diagnostics {
            let mut count = 0;
            for (file_path, diags) in &ui.lsp_diagnostics_details {
                if diags.is_empty() {
                    continue;
                }
                let file_lines_len = ui.diagnostics_file_cache.get(file_path).map(|l| l.len()).unwrap_or(0);
                for diag in diags {
                    let start_line = diag.line.saturating_sub(3);
                    let end_line = if file_lines_len > 0 {
                        (diag.line + 3).min(file_lines_len - 1)
                    } else {
                        diag.line + 3
                    };
                    let num_code_lines = end_line - start_line + 1;
                    count += 1 + num_code_lines + 1; // Header + Code lines + Banner
                }
            }
            count.max(1)
        } else {
            state.tabs[state.active_tab_idx].buffer.len()
        };

        let ratio = visible_lines as f32 / virtual_len as f32;
        let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
        let max_scroll = (virtual_len as isize - visible_lines as isize).max(0) as f32;
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
        let is_diagnostics = state.tabs[state.active_tab_idx].path.as_deref().map_or(false, |p| p.starts_with("diagnostics://"));
        let max_line_digits = if is_diagnostics { 3 } else { state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3) };
        let gutter_width = if is_diagnostics { 0.0 } else { (max_line_digits as f32 + 2.0) * ui.buffer_char_width };
        let text_area_x = ui.sidebar_width + gutter_width;
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = if is_diagnostics { 0.0 } else { ui.minimap_width() };
        let sb_x = size.width as f32 - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
  
        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
        let raw_line_idx = if state.mouse_y >= editor_top {
            ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y
        } else {
            ui.scroll_y
        };

        let line_idx = if is_diagnostics {
            let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
            if visual_lines.is_empty() {
                0
            } else {
                raw_line_idx.min(visual_lines.len() - 1)
            }
        } else {
            raw_line_idx.min(state.tabs[state.active_tab_idx].buffer.len().saturating_sub(1))
        };
  
        let mouse_x_clamped = state.mouse_x.min(minimap_x);
        let col_idx = if mouse_x_clamped > text_area_x {
            ((mouse_x_clamped - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x
        } else {
            0
        };

        let line_chars = if is_diagnostics {
            let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
            visual_lines.get(line_idx).map_or(0, |vl| match vl {
                crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
            })
        } else {
            state.tabs[state.active_tab_idx].buffer.lines()[line_idx].chars().count()
        };
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
