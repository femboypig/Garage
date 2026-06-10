use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

pub fn draw_gutter(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    _buffer: &Buffer,
    cursor: &Cursor,
    editor_y: f32,
    total_editor_height: f32,
    gutter_width: f32,
    text_area_x: f32,
    activity_bar_width: f32,
    start_idx: usize,
    end_idx: usize,
    max_line_digits: usize,
) {
    let white_uv = atlas.white_pixel_uv();
    
    // Draw Gutter background
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        editor_y,
        gutter_width,
        total_editor_height,
        white_uv,
        ui.config.theme.gutter_bg,
    );
    // Draw Gutter border separator
    ui.push_quad(
        vertices,
        indices,
        text_area_x - 1.0,
        editor_y,
        1.0,
        total_editor_height,
        white_uv,
        ui.config.theme.gutter_border,
    );

    // Draw line numbers
    for line_idx in start_idx..end_idx {
        let row_y = editor_y + (line_idx - start_idx) as f32 * ui.buffer_line_height;
        let baseline_y = (row_y + ui.buffer_font_ascent).round();

        let line_num_str = format!("{:>width$}", line_idx + 1, width = max_line_digits);
        let num_color = if line_idx == cursor.line {
            ui.config.theme.line_number_active
        } else {
            ui.config.theme.line_number_inactive
        };
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &line_num_str,
            activity_bar_width + ui.sidebar_width + ui.buffer_char_width,
            baseline_y,
            num_color,
            ui.buffer_font_size,
            ui.buffer_char_width,
        );
    }
}
