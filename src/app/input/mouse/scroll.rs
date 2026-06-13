use winit::window::Window;
use winit::event::MouseScrollDelta;

use crate::ui::UiState;
use crate::app::state::AppState;

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
 
    let active_path = state.tabs[state.active_tab_idx].path.as_deref().unwrap_or("");
    let is_diagnostics = active_path.starts_with("diagnostics://");

    let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
    let status_y = (window.inner_size().height as f32 - ui.status_height).round();

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

    let max_scroll = (virtual_len as isize - visible_lines as isize).max(0);
 
    let new_scroll = ui.scroll_y as isize + scroll_lines;
    ui.scroll_y = new_scroll.clamp(0, max_scroll) as usize;
 
    window.request_redraw();
}
