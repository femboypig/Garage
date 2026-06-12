use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

use super::editor::{tab_bar, breadcrumbs, gutter, text_area, scrollbar, minimap};

pub fn draw_editor_view(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    cursor: &Cursor,
    width: f32,
    mouse_x: f32,
    mouse_y: f32,
    tab_paths: &[Option<String>],
    tab_modified: &[bool],
    active_tab_idx: usize,
    status_y: f32,
) {
    let active_file_path = tab_paths.get(active_tab_idx).and_then(|p| p.as_deref());
    let is_diagnostics = active_file_path.map_or(false, |p| p.starts_with("diagnostics://"));
    let main_y = ui.titlebar_height;

    // Sidebar Navigator (Activity Bar) Width
    let activity_bar_width = 0.0;

    // Calculate dynamic layouts
    let max_line_digits = buffer.len().to_string().len().max(3);
    let gutter_width = if is_diagnostics { 0.0 } else { (max_line_digits as f32 + 2.0) * ui.buffer_char_width };
    let text_area_x = activity_bar_width + ui.sidebar_width + gutter_width;
    
    let scrollbar_width = ui.scrollbar_width();
    let minimap_width = if is_diagnostics { 0.0 } else { ui.minimap_width() };
    let sb_x = width - scrollbar_width;
    let minimap_x = sb_x - minimap_width;
    let text_viewport_w = minimap_x - text_area_x;

    let editor_y = main_y + ui.tabbar_height + ui.breadcrumb_height;
    let total_editor_height = status_y - editor_y;
    let editor_height = total_editor_height - 14.0;
    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
    let (virtual_len, visible_count) = if is_diagnostics {
        let mut count = 0;
        for diags in ui.lsp_diagnostics_details.values() {
            if !diags.is_empty() {
                count += 1 + 2 * diags.len();
            }
        }
        (count.max(1), visible_lines)
    } else {
        (buffer.len(), visible_lines)
    };
    let max_scroll = (virtual_len as isize - visible_count as isize).max(0) as usize;
    ui.scroll_y = ui.scroll_y.min(max_scroll);

    let start_idx = ui.scroll_y;
    let end_idx = (start_idx + visible_count).min(virtual_len);

    // 1. Draw Tab Bar
    tab_bar::draw_tab_bar(
        ui,
        vertices,
        indices,
        atlas,
        queue,
        width,
        mouse_x,
        mouse_y,
        tab_paths,
        tab_modified,
        active_tab_idx,
        main_y,
        activity_bar_width,
    );

    // 2. Draw Breadcrumbs
    breadcrumbs::draw_breadcrumbs(
        ui,
        vertices,
        indices,
        atlas,
        queue,
        buffer,
        cursor,
        width,
        main_y,
        activity_bar_width,
    );

    // 3. Draw Gutter
    if !is_diagnostics {
        gutter::draw_gutter(
            ui,
            vertices,
            indices,
            atlas,
            queue,
            buffer,
            cursor,
            editor_y,
            total_editor_height,
            gutter_width,
            text_area_x,
            activity_bar_width,
            start_idx,
            end_idx,
            max_line_digits,
            active_file_path,
        );
    }

    // 4. Draw Text Area
    text_area::draw_text_area(
        ui,
        vertices,
        indices,
        atlas,
        queue,
        buffer,
        cursor,
        editor_y,
        editor_height,
        text_area_x,
        text_viewport_w,
        minimap_x,
        start_idx,
        end_idx,
        visible_lines,
        active_file_path,
    );

    // 5. Draw Scrollbars
    scrollbar::draw_scrollbars(
        ui,
        vertices,
        indices,
        atlas,
        queue,
        buffer,
        cursor,
        editor_y,
        editor_height,
        total_editor_height,
        text_area_x,
        text_viewport_w,
        minimap_x,
        sb_x,
        scrollbar_width,
        visible_lines,
        mouse_x,
        mouse_y,
        active_file_path,
    );

    // 6. Draw Minimap
    if !is_diagnostics {
        minimap::draw_minimap(
            ui,
            vertices,
            indices,
            atlas,
            buffer,
            editor_y,
            editor_height,
            total_editor_height,
            minimap_x,
            minimap_width,
            visible_lines,
        );
    }
}
