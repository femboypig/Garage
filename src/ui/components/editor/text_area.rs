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
    
    if active_file_path == Some("diagnostics://project") {
        // Clear click targets
        if start_idx == ui.scroll_y {
            ui.diagnostics_click_targets.clear();
        }

        // Draw background
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

        // Gather all diagnostics
        let mut all_diags = Vec::new();
        for (file_path, diags) in &ui.lsp_diagnostics_details {
            for d in diags {
                all_diags.push((file_path.clone(), d.clone()));
            }
        }
        // Sort by severity (1=error, 2=warning, etc.), then file path, then line
        all_diags.sort_by(|a, b| {
            a.1.severity.cmp(&b.1.severity)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.line.cmp(&b.1.line))
        });

        if all_diags.is_empty() {
            // Draw "No problems found" message in the center
            let msg = "No problems found in the workspace";
            let msg_w = msg.chars().count() as f32 * ui.buffer_char_width;
            let msg_x = (text_area_x + (text_viewport_w - msg_w) / 2.0).round();
            let msg_y = (editor_y + editor_height / 2.0).round();
            
            let mut pen_x = msg_x;
            for c in msg.chars() {
                pen_x += ui.push_char(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    c,
                    pen_x,
                    msg_y,
                    ui.config.theme.syntax_comment,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );
            }
            return;
        }

        // Render visible items
        let card_height = ui.buffer_line_height * 3.0; // each item is 3 lines tall (aligns with scrolling)
        let padding = 12.0f32;
        let card_w = text_viewport_w - padding * 2.0;
        
        for item_idx in start_idx..end_idx {
            if item_idx >= all_diags.len() {
                break;
            }
            let (file_path, diag) = &all_diags[item_idx];
            let relative_y = (item_idx - start_idx) as f32 * card_height;
            let card_y = editor_y + relative_y + 8.0;
            
            // Check if card fits in editor height bounds
            if card_y + card_height - 8.0 > editor_y + editor_height {
                break;
            }
            
            let card_x = text_area_x + padding;
            
            // Bounding box for clicks:
            ui.diagnostics_click_targets.push((
                card_x,
                card_y,
                card_x + card_w,
                card_y + card_height - 8.0,
                file_path.clone(),
                diag.line,
                diag.col,
            ));

            // Set colors based on severity
            let (bg_color, border_color, icon_name, icon_color, label) = match diag.severity {
                1 => (
                    [0.95, 0.25, 0.25, 0.08], // transparent red
                    [0.95, 0.25, 0.25, 1.0],  // solid red
                    "error",
                    [0.95, 0.25, 0.25, 1.0],
                    "Error",
                ),
                2 => (
                    [0.95, 0.6, 0.1, 0.08],  // transparent orange
                    [0.95, 0.6, 0.1, 1.0],   // solid orange
                    "circle",
                    [0.95, 0.6, 0.1, 1.0],
                    "Warning",
                ),
                _ => (
                    [0.2, 0.6, 0.9, 0.08],   // transparent blue
                    [0.2, 0.6, 0.9, 1.0],    // solid blue
                    "circle",
                    [0.2, 0.6, 0.9, 1.0],
                    "Info",
                ),
            };

            // Draw Card Background
            ui.push_quad(
                vertices,
                indices,
                card_x,
                card_y,
                card_w,
                card_height - 8.0,
                white_uv,
                bg_color,
            );

            // Draw Left Border stripe
            ui.push_quad(
                vertices,
                indices,
                card_x,
                card_y,
                3.0,
                card_height - 8.0,
                white_uv,
                border_color,
            );

            // Draw Severity Icon
            let icon_sz = (ui.buffer_font_size * 0.85).round().max(10.0);
            let icon_x = card_x + 12.0;
            let icon_y = (card_y + 8.0 + (ui.buffer_line_height - icon_sz) / 2.0).round();
            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                icon_name,
                icon_x,
                icon_y,
                icon_color,
                icon_sz,
            );

            // Get relative path from workspace root
            let current_dir = std::env::current_dir().unwrap_or_default().to_string_lossy().to_string();
            let display_path = if file_path.starts_with(&current_dir) {
                file_path[current_dir.len()..].trim_start_matches('/').to_string()
            } else {
                file_path.clone()
            };
            
            // Draw File Path & Location Header: e.g. "src/editor/lsp.rs:125"
            let header_text = format!("{} in {}:{}:{}", label, display_path, diag.line + 1, diag.col + 1);
            let mut pen_x = icon_x + icon_sz + 8.0;
            let header_y = (card_y + 8.0 + ui.buffer_font_ascent).round();
            for c in header_text.chars() {
                if pen_x + ui.buffer_char_width > card_x + card_w - 12.0 {
                    break;
                }
                pen_x += ui.push_char(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    c,
                    pen_x,
                    header_y,
                    ui.config.theme.syntax_type,
                    ui.buffer_font_size * 0.9,
                    ui.buffer_char_width,
                );
            }

            // Draw Diagnostic Message below
            let msg_y = (card_y + 8.0 + ui.buffer_line_height + ui.buffer_font_ascent).round();
            let mut msg_pen_x = card_x + 12.0;
            for c in diag.message.chars() {
                if msg_pen_x + ui.buffer_char_width > card_x + card_w - 12.0 {
                    break;
                }
                msg_pen_x += ui.push_char(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    c,
                    msg_pen_x,
                    msg_y,
                    ui.config.theme.syntax_default,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );
            }
        }

        return;
    }

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

        // 1. Draw LSP inline diagnostic annotation if present
        let mut inline_diag_w = 0.0f32;
        if let Some(diags) = ui.lsp_diagnostics_details.get(&abs_path) {
            let mut line_diagnostic: Option<&crate::editor::lsp::DiagnosticDetail> = None;
            for d in diags {
                if line_idx >= d.line && line_idx <= d.end_line {
                    if line_diagnostic.is_none() || d.severity < line_diagnostic.as_ref().unwrap().severity {
                        line_diagnostic = Some(d);
                    }
                }
            }
            if let Some(diag) = line_diagnostic {
                let msg = diag.message.split('\n').next().unwrap_or("").trim();
                let display_msg = format!("• {}", msg);
                let diag_color = match diag.severity {
                    1 => [0.95, 0.3, 0.3, 0.65],  // Error: Red
                    2 => [0.95, 0.6, 0.15, 0.65], // Warning: Orange
                    3 => [0.3, 0.6, 0.9, 0.55],   // Info: Blue
                    _ => [0.6, 0.6, 0.6, 0.55],   // Hint: Gray
                };
                let diag_x = pen_x + 30.0;
                if diag_x < minimap_x {
                    let max_w = (minimap_x - diag_x - 10.0).max(0.0);
                    let available_chars = (max_w / ui.buffer_char_width).floor() as usize;
                    if available_chars > 3 {
                        let final_msg = if display_msg.chars().count() > available_chars {
                            format!("{}...", &display_msg.chars().take(available_chars - 3).collect::<String>())
                        } else {
                            display_msg
                        };
                        ui.push_str(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            &final_msg,
                            diag_x,
                            baseline_y,
                            diag_color,
                            ui.buffer_font_size,
                            ui.buffer_char_width,
                        );
                        inline_diag_w = final_msg.chars().count() as f32 * ui.buffer_char_width + 20.0;
                    }
                }
            }
        }

        // 2. Draw Git Blame inline annotation at the end of the active line
        if ui.config.show_git_blame && line_idx == cursor.line {
            if let Some(blame_str) = ui.get_or_update_blame(active_file_path, line_idx) {
                if blame_str != "Loading blame..." && !blame_str.is_empty() {
                    let annotation_x = pen_x + 30.0 + inline_diag_w;
                    if annotation_x < minimap_x {
                        let max_w = (minimap_x - annotation_x - 10.0).max(0.0);
                        let available_chars = (max_w / ui.buffer_char_width).floor() as usize;
                        if available_chars > 3 {
                            let final_blame = if blame_str.chars().count() > available_chars {
                                format!("{}...", &blame_str.chars().take(available_chars - 3).collect::<String>())
                            } else {
                                blame_str
                            };
                            let mut annotation_color = ui.config.theme.syntax_comment;
                            annotation_color[3] *= 0.5; // Make it extra faint
                            
                            ui.push_str(
                                vertices,
                                indices,
                                atlas,
                                queue,
                                &final_blame,
                                annotation_x,
                                baseline_y,
                                annotation_color,
                                ui.buffer_font_size,
                                ui.buffer_char_width,
                            );
                        }
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

    // Draw hover diagnostic popup overlay
    if let Some(diag) = &ui.hovered_diagnostic {
        if let Some((line_idx, col_idx)) = ui.hover_pos {
            // Snapped coordinates of the hovered character
            let char_x = text_area_x + (col_idx as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;
            let char_y = editor_y + (line_idx as isize - ui.scroll_y as isize) as f32 * ui.buffer_line_height;
            
            // Background box colors based on severity
            let (bg_color, border_color, text_color, label) = match diag.severity {
                1 => (
                    [0.98, 0.88, 0.88, 0.97], // soft pink-red
                    [0.85, 0.25, 0.25, 1.0],  // red
                    [0.60, 0.10, 0.10, 1.0],  // dark red text
                    "Syntax Error",
                ),
                2 => (
                    [0.99, 0.94, 0.85, 0.97], // soft yellow-orange
                    [0.85, 0.55, 0.10, 1.0],  // orange
                    [0.60, 0.35, 0.05, 1.0],  // dark orange/brown text
                    "Warning",
                ),
                _ => (
                    [0.88, 0.94, 0.98, 0.97], // soft blue
                    [0.25, 0.60, 0.85, 1.0],  // blue
                    [0.10, 0.35, 0.60, 1.0],  // dark blue text
                    "Info",
                ),
            };

            // Parse lines with word wrapping
            let max_w = 400.0f32;
            let max_chars_per_line = (max_w / ui.ui_char_width).floor() as usize;
            let full_message = if diag.message.is_empty() {
                label.to_string()
            } else {
                format!("{}: {}", label, diag.message)
            };
            
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
            if !current_line.is_empty() {
                lines.push(current_line);
            }

            let line_count = lines.len();
            let popup_w = (lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * ui.ui_char_width + 24.0 + 20.0).max(120.0);
            let popup_h = line_count as f32 * ui.ui_line_height + 16.0;

            // Clamping coordinates
            let mut popup_x = char_x.round();
            let mut popup_y = (char_y + ui.buffer_line_height + 4.0).round();
            
            // Check overflow right
            if popup_x + popup_w > minimap_x {
                popup_x = (minimap_x - popup_w - 10.0).max(text_area_x + 10.0);
            }
            // Check overflow bottom
            if popup_y + popup_h > editor_y + editor_height {
                popup_y = (char_y - popup_h - 4.0).max(editor_y + 4.0);
            }

            // Draw shadow box (offset by 2.0)
            ui.push_quad(
                vertices,
                indices,
                popup_x + 2.0,
                popup_y + 2.0,
                popup_w,
                popup_h,
                white_uv,
                [0.0, 0.0, 0.0, 0.15],
            );

            // Draw Background Box
            ui.push_quad(
                vertices,
                indices,
                popup_x,
                popup_y,
                popup_w,
                popup_h,
                white_uv,
                bg_color,
            );

            // Draw Border Box (using thin quads)
            let border_thickness = 1.0f32;
            ui.push_quad(vertices, indices, popup_x, popup_y, popup_w, border_thickness, white_uv, border_color); // top
            ui.push_quad(vertices, indices, popup_x, popup_y + popup_h - border_thickness, popup_w, border_thickness, white_uv, border_color); // bottom
            ui.push_quad(vertices, indices, popup_x, popup_y, border_thickness, popup_h, white_uv, border_color); // left
            ui.push_quad(vertices, indices, popup_x + popup_w - border_thickness, popup_y, border_thickness, popup_h, white_uv, border_color); // right

            // Draw Text Lines
            for (i, line_text) in lines.iter().enumerate() {
                let text_y = (popup_y + 8.0 + i as f32 * ui.ui_line_height + ui.ui_font_ascent).round();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    line_text,
                    popup_x + 12.0,
                    text_y,
                    text_color,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
            }

            // Draw copy icon (overlapping squares)
            let copy_x = (popup_x + popup_w - 24.0).round();
            let copy_y = (popup_y + (popup_h - 11.0) / 2.0).round();
            
            // Back square
            ui.push_quad(vertices, indices, copy_x, copy_y, 8.0, 8.0, white_uv, border_color);
            ui.push_quad(vertices, indices, copy_x + 1.0, copy_y + 1.0, 6.0, 6.0, white_uv, bg_color);
            
            // Front square
            ui.push_quad(vertices, indices, copy_x + 3.0, copy_y + 3.0, 8.0, 8.0, white_uv, border_color);
            ui.push_quad(vertices, indices, copy_x + 4.0, copy_y + 4.0, 6.0, 6.0, white_uv, bg_color);
        }
    }
}
