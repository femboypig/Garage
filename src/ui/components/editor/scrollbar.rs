use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

pub fn draw_scrollbars(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    _queue: &wgpu::Queue,
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
    let white_uv = atlas.white_pixel_uv();
    
    // --- 1. Draw Vertical Scrollbar ---
    let is_sb_hovered = ui.active_modal.is_none() && mouse_x >= sb_x && mouse_y >= editor_y && mouse_y < editor_y + editor_height;

    // Scrollbar Track background
    ui.push_quad(
        vertices,
        indices,
        sb_x,
        editor_y,
        scrollbar_width,
        total_editor_height,
        white_uv,
        ui.config.theme.scrollbar_track,
    );
    // Vertical track separator (left of scrollbar)
    ui.push_quad(
        vertices,
        indices,
        sb_x - 1.0,
        editor_y,
        1.0,
        total_editor_height,
        white_uv,
        ui.config.theme.scrollbar_border,
    );

    let track_h = editor_height;
    let ratio = visible_lines as f32 / buffer.len() as f32;
    let thumb_h = (track_h * ratio).clamp(20.0, track_h);
    let max_scroll_f = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
    let scroll_ratio = if max_scroll_f > 0.0 { ui.scroll_y as f32 / max_scroll_f } else { 0.0 };
    let thumb_y = editor_y + scroll_ratio * (track_h - thumb_h);

    let thumb_color = if is_sb_hovered {
        ui.config.theme.scrollbar_thumb_hover
    } else {
        ui.config.theme.scrollbar_thumb
    };

    // Draw Scrollbar Thumb
    ui.push_quad(
        vertices,
        indices,
        sb_x + 2.0,
        thumb_y,
        scrollbar_width - 4.0,
        thumb_h,
        white_uv,
        thumb_color,
    );

    // --- 2. Draw Horizontal Scrollbar ---
    let max_line_len = ui.get_max_line_len(buffer, active_file_path, cursor.line);
    let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
    
    let hs_y = editor_y + editor_height;
    let hs_h = 14.0f32;
    let is_hs_hovered = ui.active_modal.is_none()
        && mouse_x >= text_area_x
        && mouse_x < minimap_x
        && mouse_y >= hs_y
        && mouse_y < hs_y + hs_h;

    // Draw Horizontal Scrollbar Track Background
    ui.push_quad(
        vertices,
        indices,
        text_area_x,
        hs_y,
        text_viewport_w,
        hs_h,
        white_uv,
        ui.config.theme.scrollbar_track,
    );

    // Draw horizontal track border separator (top of horizontal scrollbar)
    ui.push_quad(
        vertices,
        indices,
        text_area_x,
        hs_y,
        text_viewport_w,
        1.0,
        white_uv,
        ui.config.theme.scrollbar_border,
    );

    if max_line_len > visible_cols {
        // Calculate horizontal scrollbar thumb
        let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
        let thumb_w = (text_viewport_w * ratio_x).clamp(20.0, text_viewport_w);
        let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
        let scroll_ratio_x = if max_scroll_x > 0.0 { ui.scroll_x as f32 / max_scroll_x } else { 0.0 };
        let thumb_x = text_area_x + scroll_ratio_x * (text_viewport_w - thumb_w);

        let thumb_color_x = if is_hs_hovered {
            ui.config.theme.scrollbar_thumb_hover
        } else {
            ui.config.theme.scrollbar_thumb
        };

        // Draw Horizontal Scrollbar Thumb (height 10.0, padded by 2.0 from top and bottom)
        ui.push_quad(
            vertices,
            indices,
            thumb_x,
            hs_y + 2.0,
            thumb_w,
            10.0,
            white_uv,
            thumb_color_x,
        );
    }
}
