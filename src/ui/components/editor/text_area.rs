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

        // Draw source code text characters (using LSP semantic tokens or custom fallback highlighting)
        let line_text = &buffer.lines()[line_idx];
        let mut pen_x = text_area_x;
        
        let abs_path = crate::editor::lsp::get_absolute_path(active_file_path.unwrap_or(""));
        let tokens_opt = ui.lsp_semantic_tokens.get(&abs_path);
        let default_color = ui.config.theme.syntax_default;
        let char_count = line_text.chars().count();
        let mut char_colors = vec![default_color; char_count];

        if let Some(tokens) = tokens_opt {
            for token in tokens {
                if token.line == line_idx {
                    let start = token.start_col;
                    let end = (token.start_col + token.length).min(char_count);
                    
                    let token_color = match token.token_type.as_str() {
                        "keyword" | "modifier" => ui.config.theme.syntax_keyword,
                        "string" => ui.config.theme.syntax_string,
                        "comment" => ui.config.theme.syntax_comment,
                        "number" => ui.config.theme.syntax_number,
                        "type" | "class" | "struct" | "interface" | "enum" | "typeParameter" => ui.config.theme.syntax_type,
                        "function" | "method" => ui.config.theme.syntax_attribute,
                        "macro" => ui.config.theme.syntax_macro,
                        "namespace" => ui.config.theme.syntax_namespace,
                        "enumMember" => ui.config.theme.syntax_enum_member,
                        "parameter" => ui.config.theme.syntax_parameter,
                        "variable" => ui.config.theme.syntax_variable,
                        "property" => ui.config.theme.syntax_property,
                        "operator" => ui.config.theme.syntax_operator,
                        _ => default_color,
                    };
                    
                    for c_idx in start..end {
                        char_colors[c_idx] = token_color;
                    }
                }
            }
        } else {
            char_colors = ui.get_line_char_colors(line_text);
        }
        
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

        // Draw LSP compilation warnings/errors underlines under characters
        if let Some(diags) = ui.lsp_diagnostics_details.get(&abs_path) {
            for d in diags {
                if line_idx >= d.line && line_idx <= d.end_line {
                    let start_char = if d.line == line_idx { d.col } else { 0 };
                    let end_char = if d.end_line == line_idx { d.end_col } else { char_count };
                    
                    if start_char < end_char && start_char < char_count {
                        let start_col_clamped = start_char.max(ui.scroll_x);
                        let end_col_clamped = end_char.min(char_count);
                        
                        if start_col_clamped < end_col_clamped {
                            let start_x = text_area_x + (start_col_clamped as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;
                            let end_x = text_area_x + (end_col_clamped as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;
                            
                            let start_x_clamped = start_x.max(text_area_x);
                            let end_x_clamped = end_x.min(minimap_x);
                            
                            if start_x_clamped < end_x_clamped {
                                let color = match d.severity {
                                    1 => [0.95, 0.25, 0.25, 0.9], // Error: Red
                                    2 => [0.95, 0.6, 0.1, 0.9],   // Warning: Yellow/Orange
                                    3 => [0.2, 0.6, 0.9, 0.7],    // Info: Blue
                                    _ => [0.5, 0.5, 0.5, 0.7],    // Hint: Gray
                                };
                                let wave_y = row_y + ui.buffer_line_height - 3.0;
                                let wave_height = 2.0f32;
                                let wave_period = 4.0f32;
                                let seg_width = 2.0f32;
                                
                                let mut wx = start_x_clamped;
                                let mut wave_up = true;
                                while wx < end_x_clamped {
                                    let seg_w = seg_width.min(end_x_clamped - wx);
                                    let seg_y = if wave_up { wave_y - wave_height * 0.5 } else { wave_y + wave_height * 0.5 };
                                    ui.push_quad(
                                        vertices,
                                        indices,
                                        wx,
                                        seg_y,
                                        seg_w,
                                        1.5,
                                        white_uv,
                                        color,
                                    );
                                    wx += wave_period * 0.5;
                                    wave_up = !wave_up;
                                }
                            }
                        }
                    }
                }
            }
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
