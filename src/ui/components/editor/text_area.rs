use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

pub fn draw_text_area(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    cursor: &Cursor,
    editor_y: f32,
    editor_height: f32,
    text_area_x: f32,
    text_viewport_w: f32,
    minimap_x: f32,
    start_idx: usize,
    end_idx: usize,
    visible_lines: usize,
    active_file_path: Option<&str>,
) {
    let white_uv = atlas.white_pixel_uv();
    
    // Draw main editor background area
    ui.push_quad(
        vertices,
        indices,
        text_area_x,
        editor_y,
        text_viewport_w,
        editor_height,
        white_uv,
        ui.config.theme.editor_bg,
    );

    for line_idx in start_idx..end_idx {
        let row_y = editor_y + (line_idx - start_idx) as f32 * ui.buffer_line_height;
        let baseline_y = (row_y + ui.buffer_font_ascent).round();

        // Active line highlight
        if line_idx == cursor.line {
            ui.push_quad(
                vertices,
                indices,
                text_area_x,
                row_y,
                text_viewport_w,
                ui.buffer_line_height,
                white_uv,
                ui.config.theme.active_line_bg,
            );
        }

        // Draw selection ranges
        if let Some((s_line, s_col, e_line, e_col)) = cursor.selection_range() {
            if line_idx >= s_line && line_idx <= e_line {
                let line_chars_count = buffer.lines()[line_idx].chars().count();
                let col_start = if line_idx == s_line { s_col } else { 0_usize };
                let col_end = if line_idx == e_line { e_col } else { line_chars_count };

                // Adjust for scroll_x
                let visible_start = col_start.saturating_sub(ui.scroll_x);
                let visible_end = col_end.saturating_sub(ui.scroll_x);

                if visible_start < visible_end {
                    let sel_x = text_area_x + visible_start as f32 * ui.buffer_char_width;
                    let mut sel_w = ((visible_end - visible_start) as f32).max(0.5) * ui.buffer_char_width;
                    if sel_x < minimap_x {
                        if sel_x + sel_w > minimap_x {
                            sel_w = minimap_x - sel_x;
                        }
                        ui.push_quad(
                            vertices,
                            indices,
                            sel_x,
                            row_y,
                            sel_w,
                            ui.buffer_line_height,
                            white_uv,
                            ui.config.theme.selection_bg,
                        );
                    }
                }
            }
        }

        // Draw source code text characters (with custom Rust syntax highlighting)
        let line_text = &buffer.lines()[line_idx];
        let mut pen_x = text_area_x;
        let char_colors = ui.get_line_char_colors(line_text);
        
        for (char_idx, c) in line_text.chars().enumerate() {
            if char_idx < ui.scroll_x {
                continue;
            }
            // Stop rendering if we go past the minimap/scrollbar area to prevent overlap/overflow
            if pen_x + ui.buffer_char_width > minimap_x {
                break;
            }
            let char_color = char_colors.get(char_idx).copied().unwrap_or(ui.config.theme.syntax_default);
            pen_x += ui.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color, ui.buffer_font_size, ui.buffer_char_width);
        }

        // Draw Git Blame inline annotation at the end of the active line
        if ui.config.show_git_blame && line_idx == cursor.line {
            if let Some(blame_str) = ui.get_or_update_blame(active_file_path, line_idx) {
                if blame_str != "Loading blame..." && !blame_str.is_empty() {
                    let line_len = line_text.chars().count();
                    for (c_idx, c) in blame_str.chars().enumerate() {
                        let v_idx = line_len + 4 + c_idx;
                        if v_idx < ui.scroll_x {
                            continue;
                        }
                        let blame_char_x = text_area_x + (v_idx - ui.scroll_x) as f32 * ui.buffer_char_width;
                        if blame_char_x + ui.buffer_char_width > minimap_x {
                            break;
                        }
                        ui.push_char(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            c,
                            blame_char_x,
                            baseline_y,
                            ui.config.theme.syntax_comment,
                            ui.buffer_font_size,
                            ui.buffer_char_width,
                        );
                    }
                }
            }
        }
    }

    // Draw active cursor
    if cursor.line >= ui.scroll_y && cursor.line < ui.scroll_y + visible_lines {
        let cur_row_y = editor_y + (cursor.line - ui.scroll_y) as f32 * ui.buffer_line_height;
        let cur_x = text_area_x + (cursor.col as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;
        
        if cursor.col >= ui.scroll_x && cur_x + 2.0 <= minimap_x {
            ui.push_quad(
                vertices,
                indices,
                cur_x,
                cur_row_y + 1.0,
                2.0,
                ui.buffer_line_height - 2.0,
                white_uv,
                ui.config.theme.cursor_color,
            );
        }
    }
}
