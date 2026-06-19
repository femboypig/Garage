use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::{UiState, Vertex};
use crate::renderer::atlas::FontAtlas;

#[derive(Clone, Debug)]
pub enum VisualDiagnosticLine {
    Header {
        path: String,
        line: usize,
        col: usize,
    },
    Code {
        path: String,
        line_idx: usize,
        line_content: String,
        is_diagnostic_line: bool,
        diag: crate::editor::DiagnosticDetail,
    },
    Banner {
        path: String,
        diag: crate::editor::DiagnosticDetail,
    },
}

pub fn get_visual_diagnostic_lines(ui: &mut UiState) -> Vec<VisualDiagnosticLine> {
    // Gather all diagnostics files and sort them by highest severity
    let mut sorted_files: Vec<String> = ui.lsp_diagnostics_details.keys().cloned().collect();
    sorted_files.sort_by(|a, b| {
        let severity_a = ui
            .lsp_diagnostics_details
            .get(a)
            .and_then(|v| v.iter().map(|d| d.severity).min())
            .unwrap_or(4);
        let severity_b = ui
            .lsp_diagnostics_details
            .get(b)
            .and_then(|v| v.iter().map(|d| d.severity).min())
            .unwrap_or(4);
        severity_a.cmp(&severity_b).then_with(|| a.cmp(b))
    });

    let mut visual_lines = Vec::new();
    for file_path in &sorted_files {
        if let Some(diags) = ui.lsp_diagnostics_details.get(file_path) {
            if diags.is_empty() {
                continue;
            }

            // Cache the file lines if not present
            if !ui.diagnostics_file_cache.contains_key(file_path) {
                // Insert placeholder empty vector immediately to avoid spawning multiple threads
                ui.diagnostics_file_cache
                    .insert(file_path.clone(), Vec::new());

                let file_path_clone = file_path.clone();
                let tx = ui.diagnostics_file_tx.clone();
                let proxy = ui.event_loop_proxy.clone();

                std::thread::spawn(move || {
                    if let Ok(content) = std::fs::read_to_string(&file_path_clone) {
                        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                        let _ = tx.send((file_path_clone, lines));
                        let _ = proxy.send_event(());
                    }
                });
            }

            let file_lines = ui
                .diagnostics_file_cache
                .get(file_path)
                .cloned()
                .unwrap_or_default();
            if file_lines.is_empty() {
                continue;
            }

            let mut file_diags = diags.clone();
            file_diags.sort_by_key(|d| (d.line, d.col));

            if ui.collapsed_diagnostics.contains(file_path) {
                if let Some(first_diag) = file_diags.first() {
                    visual_lines.push(VisualDiagnosticLine::Header {
                        path: file_path.clone(),
                        line: first_diag.line,
                        col: first_diag.col,
                    });
                }
            } else {
                for diag in file_diags {
                    let start_line = diag.line.saturating_sub(3);
                    let end_line = (diag.line + 3).min(file_lines.len() - 1);

                    visual_lines.push(VisualDiagnosticLine::Header {
                        path: file_path.clone(),
                        line: diag.line,
                        col: diag.col,
                    });

                    for line_idx in start_line..=end_line {
                        let line_content = file_lines.get(line_idx).cloned().unwrap_or_default();
                        visual_lines.push(VisualDiagnosticLine::Code {
                            path: file_path.clone(),
                            line_idx,
                            line_content,
                            is_diagnostic_line: line_idx == diag.line,
                            diag: diag.clone(),
                        });

                        if line_idx == diag.line {
                            visual_lines.push(VisualDiagnosticLine::Banner {
                                path: file_path.clone(),
                                diag: diag.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    visual_lines
}

fn draw_diagnostics_area(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    cursor: &Cursor,
    editor_y: f32,
    editor_height: f32,
    text_area_x: f32,
    text_viewport_w: f32,
    start_idx: usize,
    end_idx: usize,
    tab_paths: &[Option<String>],
    tab_modified: &[bool],
    white_uv: [f32; 2],
) {
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

    let visual_lines = get_visual_diagnostic_lines(ui);

    if visual_lines.is_empty() {
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
    for item_idx in start_idx..end_idx {
        if item_idx >= visual_lines.len() {
            break;
        }

        let row_y = editor_y + (item_idx - start_idx) as f32 * ui.buffer_line_height;

        match &visual_lines[item_idx] {
            VisualDiagnosticLine::Header { path, line, col } => {
                // Draw excerpt subheader background (contrast color)
                let header_bg = [
                    ui.config.theme.editor_bg[0] * 0.96,
                    ui.config.theme.editor_bg[1] * 0.96,
                    ui.config.theme.editor_bg[2] * 0.96,
                    1.0,
                ];
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    row_y,
                    text_viewport_w,
                    ui.buffer_line_height,
                    white_uv,
                    header_bg,
                );

                // Draw header separator lines (top & bottom)
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    row_y,
                    text_viewport_w,
                    1.0,
                    white_uv,
                    ui.config.theme.scrollbar_border,
                );
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    row_y + ui.buffer_line_height - 1.0,
                    text_viewport_w,
                    1.0,
                    white_uv,
                    ui.config.theme.scrollbar_border,
                );

                let mut pen_x = text_area_x + 8.0;

                // 1. Draw collapse/expand toggle arrow
                let is_collapsed = ui.collapsed_diagnostics.contains(path);
                let toggle_char = if is_collapsed { '▶' } else { '▼' };
                pen_x += ui.push_char(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    toggle_char,
                    pen_x,
                    (row_y + ui.buffer_font_ascent).round(),
                    ui.config.theme.syntax_comment,
                    ui.buffer_font_size * 0.8,
                    ui.buffer_char_width,
                );
                pen_x += 4.0;

                // 2. Draw unsaved changes dot or empty space
                let mut is_modified = false;
                let target_path = std::path::Path::new(path);
                for (i, p_opt) in tab_paths.iter().enumerate() {
                    if let Some(p) = p_opt {
                        let p_buf = std::path::Path::new(p);
                        if p_buf == target_path
                            || (p_buf.is_relative() && target_path.ends_with(p_buf))
                            || (target_path.is_relative() && p_buf.ends_with(target_path))
                        {
                            is_modified = tab_modified.get(i).copied().unwrap_or(false);
                            break;
                        }
                    }
                }
                if is_modified {
                    let dot_size = (ui.ui_font_size * 0.55).round().max(7.0);
                    let dot_y = (row_y + (ui.buffer_line_height - dot_size) / 2.0).round();
                    ui.push_icon(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        "circle",
                        pen_x,
                        dot_y,
                        [0.95, 0.25, 0.25, 1.0], // Red modified dot
                        dot_size,
                    );
                    pen_x += ui.buffer_char_width;
                } else {
                    pen_x += ui.buffer_char_width;
                }
                pen_x += 4.0;

                // Format path: filename in default color, folder in muted comment color
                let path_buf = std::path::Path::new(path);
                let filename = path_buf
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path);
                let parent = path_buf.parent().and_then(|p| p.to_str()).unwrap_or("");

                let current_dir = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let display_parent = if parent.starts_with(&current_dir) {
                    parent[current_dir.len()..]
                        .trim_start_matches('/')
                        .to_string()
                } else {
                    parent.to_string()
                };

                for c in filename.chars() {
                    pen_x += ui.push_char(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        c,
                        pen_x,
                        (row_y + ui.buffer_font_ascent).round(),
                        ui.config.theme.syntax_default,
                        ui.buffer_font_size,
                        ui.buffer_char_width,
                    );
                }

                if !display_parent.is_empty() {
                    pen_x += 6.0;
                    let parent_slash = format!("{}/", display_parent);
                    for c in parent_slash.chars() {
                        pen_x += ui.push_char(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            c,
                            pen_x,
                            (row_y + ui.buffer_font_ascent).round(),
                            ui.config.theme.syntax_comment,
                            ui.buffer_font_size * 0.9,
                            ui.buffer_char_width,
                        );
                    }
                }

                // Click target for Header
                ui.diagnostics_click_targets.push((
                    text_area_x,
                    row_y,
                    text_area_x + text_viewport_w,
                    row_y + ui.buffer_line_height,
                    path.clone(),
                    *line,
                    *col,
                    "header".to_string(),
                ));
            }

            VisualDiagnosticLine::Code {
                path,
                line_idx,
                line_content,
                is_diagnostic_line,
                diag,
            } => {
                let gutter_w: f32 = 48.0;
                // Vertical gutter border line
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x + gutter_w - 6.0,
                    row_y,
                    1.0,
                    ui.buffer_line_height,
                    white_uv,
                    ui.config.theme.scrollbar_border,
                );

                // Line number text right-aligned
                let line_num_str = format!("{}", line_idx + 1);
                let num_len = line_num_str.chars().count();
                let num_x = text_area_x + gutter_w - 12.0 - (num_len as f32 * ui.buffer_char_width);
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &line_num_str,
                    num_x,
                    (row_y + ui.buffer_font_ascent).round(),
                    ui.config.theme.line_number_inactive,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );

                let code_start_x = text_area_x + gutter_w;
                // Render syntax highlighted code line
                let syntax_colors = ui.get_line_char_colors(line_content, Some(path));
                for (char_idx, c) in line_content.chars().enumerate() {
                    let char_color = syntax_colors
                        .get(char_idx)
                        .copied()
                        .unwrap_or(ui.config.theme.syntax_default);
                    ui.push_char(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        c,
                        code_start_x + char_idx as f32 * ui.buffer_char_width,
                        (row_y + ui.buffer_font_ascent).round(),
                        char_color,
                        ui.buffer_font_size,
                        ui.buffer_char_width,
                    );
                }

                // Draw squiggly red/yellow underline under diagnostic range
                if *is_diagnostic_line {
                    let start_char = diag.col;
                    let end_char = diag.end_col.max(diag.col + 1);
                    let start_x = code_start_x + start_char as f32 * ui.buffer_char_width;
                    let end_x = code_start_x + end_char as f32 * ui.buffer_char_width;

                    let color = match diag.severity {
                        1 => [0.95, 0.25, 0.25, 0.9],
                        2 => [0.95, 0.6, 0.1, 0.9],
                        _ => [0.2, 0.6, 0.9, 0.7],
                    };

                    let wave_y = row_y + ui.buffer_line_height - 3.0;
                    let wave_height: f32 = 2.0;
                    let wave_period: f32 = 4.0;

                    let mut wx = start_x;
                    while wx < end_x {
                        let seg_w = 2.0f32.min(end_x - wx);
                        let phase = (wx - start_x) * (2.0 * std::f32::consts::PI / wave_period);
                        let seg_y = wave_y + phase.sin() * (wave_height * 0.5);
                        ui.push_quad(vertices, indices, wx, seg_y, seg_w, 1.5, white_uv, color);
                        wx += 1.0;
                    }
                }

                // Click target for Code line
                ui.diagnostics_click_targets.push((
                    text_area_x,
                    row_y,
                    text_area_x + text_viewport_w,
                    row_y + ui.buffer_line_height,
                    path.clone(),
                    diag.line,
                    diag.col,
                    "code".to_string(),
                ));
            }

            VisualDiagnosticLine::Banner { path, diag } => {
                let gutter_w: f32 = 48.0;
                let code_start_x = text_area_x + gutter_w;

                let bg_color = match diag.severity {
                    1 => [0.95, 0.25, 0.25, 0.12], // Error: light red
                    2 => [0.95, 0.70, 0.15, 0.12], // Warning: light yellow/orange
                    _ => [0.2, 0.6, 0.9, 0.12],    // Info: light blue
                };

                // Draw full width banner background (excluding gutter)
                ui.push_quad(
                    vertices,
                    indices,
                    code_start_x,
                    row_y,
                    text_viewport_w - gutter_w,
                    ui.buffer_line_height,
                    white_uv,
                    bg_color,
                );

                // Draw solid left border indicator
                let border_color = match diag.severity {
                    1 => [0.95, 0.25, 0.25, 1.0],
                    2 => [0.95, 0.70, 0.15, 1.0],
                    _ => [0.2, 0.6, 0.9, 1.0],
                };
                ui.push_quad(
                    vertices,
                    indices,
                    code_start_x,
                    row_y,
                    3.0,
                    ui.buffer_line_height,
                    white_uv,
                    border_color,
                );

                // Render diagnostic message
                let mut pen_x = code_start_x + 12.0;
                let msg_color = border_color;
                for c in diag.message.chars() {
                    if pen_x + ui.buffer_char_width > text_area_x + text_viewport_w - 20.0 {
                        break;
                    }
                    pen_x += ui.push_char(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        c,
                        pen_x,
                        (row_y + ui.buffer_font_ascent).round(),
                        msg_color,
                        ui.buffer_font_size,
                        ui.buffer_char_width,
                    );
                }

                ui.diagnostics_click_targets.push((
                    text_area_x,
                    row_y,
                    text_area_x + text_viewport_w,
                    row_y + ui.buffer_line_height,
                    path.clone(),
                    diag.line,
                    diag.col,
                    "banner".to_string(),
                ));
            }
        }
    }

    // Draw cursor in diagnostics tab
    if cursor.line >= start_idx && cursor.line < end_idx {
        let cur_row_y = editor_y + (cursor.line - start_idx) as f32 * ui.buffer_line_height;
        if let Some(vl) = visual_lines.get(cursor.line) {
            let cur_x = match vl {
                VisualDiagnosticLine::Code { .. } => {
                    let gutter_w: f32 = 48.0;
                    text_area_x + gutter_w + cursor.col as f32 * ui.buffer_char_width
                }
                VisualDiagnosticLine::Header { .. } => {
                    text_area_x + 8.0 + cursor.col as f32 * ui.buffer_char_width
                }
                VisualDiagnosticLine::Banner { .. } => {
                    text_area_x + 12.0 + cursor.col as f32 * ui.buffer_char_width
                }
            };
            if cur_x + 2.0 <= text_area_x + text_viewport_w {
                let mut ctx = crate::machkit::UiContext {
                    vertices,
                    indices,
                    atlas,
                    queue,
                    mouse_x: 0.0,
                    mouse_y: 0.0,
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
                crate::machkit::Cursor::new().width(2.0).draw(
                    &mut ctx,
                    cur_x,
                    cur_row_y + 1.0,
                    ui.buffer_line_height - 2.0,
                );
            }
        }
    }
}

fn draw_selection_and_search_highlights(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    buffer: &Buffer,
    cursor: &Cursor,
    line_idx: usize,
    row_y: f32,
    text_area_x: f32,
    minimap_x: f32,
    white_uv: [f32; 2],
) {
    // Selection Range Highlight
    if let Some((s_line, s_col, e_line, e_col)) = cursor.selection_range()
        && line_idx >= s_line
        && line_idx <= e_line
    {
        let line_chars_count = buffer.lines()[line_idx].chars().count();
        let col_start = if line_idx == s_line { s_col } else { 0_usize };
        let col_end = if line_idx == e_line {
            e_col
        } else {
            line_chars_count
        };

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

    // Search Match Highlights
    if ui.show_search_panel && !ui.search_query.is_empty() {
        let query_len = ui.search_query.chars().count();
        for (match_idx, &(m_line, m_col)) in ui.search_matches.iter().enumerate() {
            if m_line == line_idx {
                let col_start = m_col;
                let col_end = m_col + query_len;
                let visible_start = col_start.saturating_sub(ui.scroll_x);
                let visible_end = col_end.saturating_sub(ui.scroll_x);
                if visible_start < visible_end {
                    let match_x = text_area_x + visible_start as f32 * ui.buffer_char_width;
                    let mut match_w = (visible_end - visible_start) as f32 * ui.buffer_char_width;
                    if match_x < minimap_x {
                        if match_x + match_w > minimap_x {
                            match_w = minimap_x - match_x;
                        }
                        let is_active = match_idx == ui.active_search_match_idx;
                        let highlight_color = if is_active {
                            [1.0, 0.5, 0.0, 0.45] // Bright orange
                        } else {
                            [0.9, 0.9, 0.0, 0.25] // Soft yellow
                        };
                        ui.push_quad(
                            vertices,
                            indices,
                            match_x,
                            row_y,
                            match_w,
                            ui.buffer_line_height,
                            white_uv,
                            highlight_color,
                        );

                        if is_active {
                            ui.push_quad(
                                vertices,
                                indices,
                                match_x,
                                row_y,
                                match_w,
                                1.0,
                                white_uv,
                                [1.0, 0.5, 0.0, 0.9],
                            );
                            ui.push_quad(
                                vertices,
                                indices,
                                match_x,
                                row_y + ui.buffer_line_height - 1.0,
                                match_w,
                                1.0,
                                white_uv,
                                [1.0, 0.5, 0.0, 0.9],
                            );
                            ui.push_quad(
                                vertices,
                                indices,
                                match_x,
                                row_y,
                                1.0,
                                ui.buffer_line_height,
                                white_uv,
                                [1.0, 0.5, 0.0, 0.9],
                            );
                            ui.push_quad(
                                vertices,
                                indices,
                                match_x + match_w - 1.0,
                                row_y,
                                1.0,
                                ui.buffer_line_height,
                                white_uv,
                                [1.0, 0.5, 0.0, 0.9],
                            );
                        }
                    }
                }
            }
        }
    }
}

fn draw_text_cursors(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    cursor: &Cursor,
    secondary_cursors: &[Cursor],
    editor_y: f32,
    text_area_x: f32,
    minimap_x: f32,
    visible_lines: usize,
    white_uv: [f32; 2],
) {
    let mut cursor_ctx = crate::machkit::UiContext {
        vertices,
        indices,
        atlas,
        queue,
        mouse_x: 0.0,
        mouse_y: 0.0,
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

    // Draw active cursor
    if cursor.line >= ui.scroll_y && cursor.line < ui.scroll_y + visible_lines {
        let cur_row_y = editor_y + (cursor.line - ui.scroll_y) as f32 * ui.buffer_line_height;
        let cur_x = text_area_x
            + (cursor.col as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;

        if cursor.col >= ui.scroll_x && cur_x + 2.0 <= minimap_x {
            crate::machkit::Cursor::new().width(2.0).draw(
                &mut cursor_ctx,
                cur_x,
                cur_row_y + 1.0,
                ui.buffer_line_height - 2.0,
            );
        }
    }

    // Draw secondary cursors
    for cur in secondary_cursors {
        if cur.line >= ui.scroll_y && cur.line < ui.scroll_y + visible_lines {
            let cur_row_y = editor_y + (cur.line - ui.scroll_y) as f32 * ui.buffer_line_height;
            let cur_x = text_area_x
                + (cur.col as isize - ui.scroll_x as isize) as f32 * ui.buffer_char_width;

            if cur.col >= ui.scroll_x && cur_x + 2.0 <= minimap_x {
                crate::machkit::Cursor::new().width(2.0).draw(
                    &mut cursor_ctx,
                    cur_x,
                    cur_row_y + 1.0,
                    ui.buffer_line_height - 2.0,
                );
            }
        }
    }
}

pub fn draw_text_area(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    cursor: &Cursor,
    secondary_cursors: &[Cursor],
    editor_y: f32,
    editor_height: f32,
    text_area_x: f32,
    text_viewport_w: f32,
    minimap_x: f32,
    start_idx: usize,
    end_idx: usize,
    visible_lines: usize,
    active_file_path: Option<&str>,
    tab_paths: &[Option<String>],
    tab_modified: &[bool],
    mouse_x: f32,
    mouse_y: f32,
) {
    let white_uv = atlas.white_pixel_uv();

    if active_file_path == Some("search://project") {
        super::project_search::draw_project_search(
            ui,
            vertices,
            indices,
            atlas,
            queue,
            text_area_x,
            editor_y,
            text_viewport_w,
            editor_height,
            mouse_x,
            mouse_y,
        );
        return;
    }

    if active_file_path == Some("diagnostics://project") {
        draw_diagnostics_area(
            ui,
            vertices,
            indices,
            atlas,
            queue,
            cursor,
            editor_y,
            editor_height,
            text_area_x,
            text_viewport_w,
            start_idx,
            end_idx,
            tab_paths,
            tab_modified,
            white_uv,
        );
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

        draw_selection_and_search_highlights(
            ui,
            vertices,
            indices,
            buffer,
            cursor,
            line_idx,
            row_y,
            text_area_x,
            minimap_x,
            white_uv,
        );

        // Draw source code text characters (plain style, no syntax highlighting)
        let line_text = &buffer.lines()[line_idx];
        let mut pen_x = text_area_x;

        let default_color = ui.config.theme.syntax_default;
        for (char_idx, c) in line_text.chars().enumerate() {
            if char_idx < ui.scroll_x {
                continue;
            }
            // Stop rendering if we go past the minimap/scrollbar area to prevent overlap/overflow
            if pen_x + ui.buffer_char_width > minimap_x {
                break;
            }
            pen_x += ui.push_char(
                vertices,
                indices,
                atlas,
                queue,
                c,
                pen_x,
                baseline_y,
                default_color,
                ui.buffer_font_size,
                ui.buffer_char_width,
            );
        }

        let inline_diag_w = 0.0f32;

        // 2. Draw Git Blame inline annotation at the end of the active line
        if ui.config.show_git_blame
            && line_idx == cursor.line
            && let Some(blame_str) = ui.get_or_update_blame(active_file_path, line_idx)
            && blame_str != "Loading blame..."
            && !blame_str.is_empty()
        {
            let annotation_x = pen_x + 30.0 + inline_diag_w;
            if annotation_x < minimap_x {
                let max_w = (minimap_x - annotation_x - 10.0).max(0.0);
                let available_chars = (max_w / ui.buffer_char_width).floor() as usize;
                if available_chars > 3 {
                    let final_blame = if blame_str.chars().count() > available_chars {
                        format!(
                            "{}...",
                            &blame_str
                                .chars()
                                .take(available_chars - 3)
                                .collect::<String>()
                        )
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

    draw_text_cursors(
        ui,
        vertices,
        indices,
        atlas,
        queue,
        cursor,
        secondary_cursors,
        editor_y,
        text_area_x,
        minimap_x,
        visible_lines,
        white_uv,
    );
}
