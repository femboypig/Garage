use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::{UiState, Vertex};
use crate::renderer::atlas::FontAtlas;

pub fn draw_scrollbars(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    cursor: &Cursor,
    editor_y: f32,
    editor_height: f32,
    total_editor_height: f32,
    text_area_x: f32,
    text_viewport_w: f32,
    minimap_x: f32,
    sb_x: f32,
    scrollbar_width: f32,
    visible_lines: usize,
    mouse_x: f32,
    mouse_y: f32,
    active_file_path: Option<&str>,
) {
    // --- 1. Draw Vertical Scrollbar ---
    let (virtual_len, visible_count) = if active_file_path == Some("search://project") {
        let items_len =
            crate::machkit::components::editor::project_search::build_search_render_items(ui).len();
        (items_len, visible_lines)
    } else if active_file_path.is_some_and(|p| p.starts_with("diagnostics://")) {
        let mut count = 0;
        for (file_path, diags) in &ui.lsp_diagnostics_details {
            if diags.is_empty() {
                continue;
            }
            let file_lines_len = ui
                .diagnostics_file_cache
                .get(file_path)
                .map(|l| l.len())
                .unwrap_or(0);
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
        (count.max(1), visible_lines)
    } else {
        (buffer.len(), visible_lines)
    };

    let max_line_len = ui.get_max_line_len(buffer, active_file_path, cursor.line);
    let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;

    let is_sb_hovered = ui.active_modal.is_none()
        && mouse_x >= sb_x
        && mouse_y >= editor_y
        && mouse_y < editor_y + editor_height;

    let white_uv = atlas.white_pixel_uv();
    let mut ctx = crate::machkit::UiContext {
        vertices,
        indices,
        atlas,
        queue,
        mouse_x,
        mouse_y,
        theme: &ui.config.theme,
        white_uv,
        ui_font_size: ui.ui_font_size,
        ui_char_width: ui.ui_char_width,
        ui_font_ascent: ui.ui_font_ascent,
        ui_line_height: ui.ui_line_height,
        buffer_font_size: ui.buffer_font_size,
        buffer_font_ascent: ui.buffer_font_ascent,
        buffer_line_height: ui.buffer_line_height,
    };

    crate::machkit::Scrollbar::new()
        .vertical(true)
        .virtual_len(virtual_len)
        .visible_count(visible_count)
        .scroll_pos(ui.scroll_y)
        .hovered(is_sb_hovered)
        .draw(
            &mut ctx,
            sb_x,
            editor_y,
            scrollbar_width,
            total_editor_height,
        );

    // --- 2. Draw Horizontal Scrollbar ---
    if active_file_path.is_some_and(|p| {
        p.starts_with("diagnostics://") || p == "search://project"
    }) {
        return;
    }

    let hs_y = editor_y + editor_height;
    let hs_h = 14.0f32;
    let is_hs_hovered = ui.active_modal.is_none()
        && mouse_x >= text_area_x
        && mouse_x < minimap_x
        && mouse_y >= hs_y
        && mouse_y < hs_y + hs_h;

    crate::machkit::Scrollbar::new()
        .vertical(false)
        .virtual_len(max_line_len)
        .visible_count(visible_cols)
        .scroll_pos(ui.scroll_x)
        .hovered(is_hs_hovered)
        .draw(&mut ctx, text_area_x, hs_y, text_viewport_w, hs_h);
}
