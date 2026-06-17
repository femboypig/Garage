use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;

pub fn draw_minimap(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    buffer: &Buffer,
    editor_y: f32,
    editor_height: f32,
    total_editor_height: f32,
    minimap_x: f32,
    minimap_width: f32,
    visible_lines: usize,
    _active_file_path: Option<&str>,
) {
    let white_uv = atlas.white_pixel_uv();
    
    // Draw Minimap Track background
    ui.push_quad(
        vertices,
        indices,
        minimap_x,
        editor_y,
        minimap_width,
        total_editor_height,
        white_uv,
        ui.config.theme.editor_bg,
    );
    // Vertical border separating editor and minimap
    ui.push_quad(
        vertices,
        indices,
        minimap_x - 1.0,
        editor_y,
        1.0,
        total_editor_height,
        white_uv,
        ui.config.theme.scrollbar_border,
    );

    let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
    let minimap_char_w = minimap_line_height * 0.5;
    let minimap_quad_h = (minimap_line_height - 1.0).max(1.0);

    let minimap_total_h = buffer.len() as f32 * minimap_line_height;
    let max_scroll_f = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
    let scroll_ratio = if max_scroll_f > 0.0 { ui.scroll_y as f32 / max_scroll_f } else { 0.0 };
    
    let minimap_offset_y = if minimap_total_h > editor_height {
        scroll_ratio * (minimap_total_h - editor_height)
    } else {
        0.0
    };

    // Determine visible lines in the minimap to optimize rendering
    let start_line = ((minimap_offset_y - 2.0) / minimap_line_height).floor().max(0.0) as usize;
    let end_line = ((editor_height + minimap_offset_y) / minimap_line_height).ceil().max(0.0) as usize;
    let end_line = end_line.min(buffer.len());

    let default_color = ui.config.theme.syntax_default;

    for line_idx in start_line..end_line {
        let row_y = editor_y + line_idx as f32 * minimap_line_height - minimap_offset_y;
        
        let line_text = &buffer.lines()[line_idx];
        
        let mut current_col = 0.0f32;
        let mut start_x = 0.0f32;
        let mut current_color = None;
        let mut block_w = 0.0f32;
        
        for (_char_idx, c) in line_text.chars().enumerate() {
            let char_w = if c == '\t' { 4.0 * minimap_char_w } else { minimap_char_w };
            let color = default_color;
            let is_whitespace = c == ' ' || c == '\t';
            
            if is_whitespace {
                if let Some(col) = current_color {
                    let draw_w = block_w.min(minimap_width - start_x);
                    if draw_w > 0.0 {
                        ui.push_quad(
                            vertices,
                            indices,
                            minimap_x + start_x,
                            row_y,
                            draw_w,
                            minimap_quad_h,
                            white_uv,
                            col,
                        );
                    }
                    current_color = None;
                }
                current_col += char_w;
            } else {
                if let Some(col) = current_color {
                    if col == color {
                        block_w += char_w;
                    } else {
                        let draw_w = block_w.min(minimap_width - start_x);
                        if draw_w > 0.0 {
                            ui.push_quad(
                                vertices,
                                indices,
                                minimap_x + start_x,
                                row_y,
                                draw_w,
                                minimap_quad_h,
                                white_uv,
                                col,
                            );
                        }
                        start_x = current_col;
                        current_color = Some(color);
                        block_w = char_w;
                    }
                } else {
                    start_x = current_col;
                    current_color = Some(color);
                    block_w = char_w;
                }
                current_col += char_w;
            }
            if current_col >= minimap_width {
                break;
            }
        }
        if let Some(col) = current_color {
            let draw_w = block_w.min(minimap_width - start_x);
            if draw_w > 0.0 {
                ui.push_quad(
                    vertices,
                    indices,
                    minimap_x + start_x,
                    row_y,
                    draw_w,
                    minimap_quad_h,
                    white_uv,
                    col,
                );
            }
        }
    }

    // Draw Viewport Indicator highlight overlay
    let highlight_y_start = ui.scroll_y as f32 * minimap_line_height - minimap_offset_y;
    let highlight_h = (visible_lines as f32 * minimap_line_height).min(editor_height);
    
    let highlight_color = if ui.config.theme.editor_bg[0] > 0.5 {
        [0.0, 0.0, 0.0, 0.08] // Light theme -> dark highlight
    } else {
        [1.0, 1.0, 1.0, 0.08] // Dark theme -> light highlight
    };

    ui.push_quad(
        vertices,
        indices,
        minimap_x,
        editor_y + highlight_y_start,
        minimap_width,
        highlight_h,
        white_uv,
        highlight_color,
    );
}
