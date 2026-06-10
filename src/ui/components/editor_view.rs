use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use std::path::Path;

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
    let white_uv = atlas.white_pixel_uv();
    let main_y = ui.titlebar_height;

    // Sidebar Navigator (Activity Bar) Width
    let activity_bar_width = 0.0;

    // Calculate dynamic layouts
    let max_line_digits = buffer.len().to_string().len().max(3);
    let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
    let text_area_x = activity_bar_width + ui.sidebar_width + gutter_width;
    
    let scrollbar_width = ui.scrollbar_width();
    let minimap_width = ui.minimap_width();
    let sb_x = width - scrollbar_width;
    let minimap_x = sb_x - minimap_width;
    let text_viewport_w = minimap_x - text_area_x;

    let editor_y = main_y + ui.tabbar_height + ui.breadcrumb_height;
    let total_editor_height = status_y - editor_y;
    let editor_height = total_editor_height - 14.0;
    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
    let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as usize;
    ui.scroll_y = ui.scroll_y.min(max_scroll);

    // --- Tab Bar & Control Buttons (New) ---
    // Tab Bar background (gray)
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        main_y,
        width - (activity_bar_width + ui.sidebar_width),
        ui.tabbar_height,
        white_uv,
        ui.config.theme.tabbar_bg,
    );
    // Pre-calculate active tab X and width to omit the border underneath it
    let mut active_tab_x = 0.0f32;
    let mut active_tab_w = 0.0f32;
    let mut has_active_tab = false;
    let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);

    let mut temp_x = activity_bar_width + ui.sidebar_width;
    for idx in 0..tab_paths.len() {
        let path_opt = &tab_paths[idx];
        let file_name = path_opt.as_ref()
            .and_then(|p| Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled.txt".to_string());
        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
        let dot_reserved = 18.0f32;
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
        if idx == active_tab_idx {
            active_tab_x = temp_x;
            active_tab_w = tab_w;
            has_active_tab = true;
        }
        temp_x += tab_w;
    }

    // Tab bar bottom border
    let tabbar_start_x = activity_bar_width + ui.sidebar_width;
    if has_active_tab {
        if active_tab_x > tabbar_start_x {
            ui.push_quad(
                vertices,
                indices,
                tabbar_start_x,
                main_y + ui.tabbar_height - 1.0,
                active_tab_x - tabbar_start_x,
                1.0,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
        let right_start_x = active_tab_x + active_tab_w;
        if right_start_x < width {
            ui.push_quad(
                vertices,
                indices,
                right_start_x,
                main_y + ui.tabbar_height - 1.0,
                width - right_start_x,
                1.0,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
    } else {
        ui.push_quad(
            vertices,
            indices,
            tabbar_start_x,
            main_y + ui.tabbar_height - 1.0,
            width - tabbar_start_x,
            1.0,
            white_uv,
            ui.config.theme.tabbar_border,
        );
    }

    // Draw active/inactive file tabs
    let mut current_tab_x = activity_bar_width + ui.sidebar_width;
    let tab_baseline = (main_y + ui.tabbar_height / 2.0 + ui.ui_font_ascent / 2.0 - 3.5).round();

    for idx in 0..tab_paths.len() {
        let path_opt = &tab_paths[idx];
        let is_active = idx == active_tab_idx;
        let is_modified = tab_modified.get(idx).copied().unwrap_or(false);

        let file_name = path_opt.as_ref()
            .and_then(|p| Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled.txt".to_string());

        // Compute tab width
        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
        let dot_reserved = 18.0f32;
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);

        // Draw tab background
        let bg_color = if is_active {
            ui.config.theme.tab_active_bg
        } else {
            ui.config.theme.tabbar_bg
        };
        let tab_h = if is_active {
            ui.tabbar_height
        } else {
            ui.tabbar_height - 1.0
        };
        ui.push_quad(
            vertices,
            indices,
            current_tab_x,
            main_y,
            tab_w,
            tab_h,
            white_uv,
            bg_color,
        );

        // Draw separators/borders
        if idx > 0 {
            ui.push_quad(
                vertices,
                indices,
                current_tab_x,
                main_y,
                1.0,
                ui.tabbar_height,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
        ui.push_quad(
            vertices,
            indices,
            current_tab_x + tab_w - 1.0,
            main_y,
            1.0,
            ui.tabbar_height,
            white_uv,
            ui.config.theme.tabbar_border,
        );

        // Draw unsaved circle icon if modified
        if is_modified {
            let dot_size = (ui.ui_font_size * 0.55).round().max(7.0);
            let dot_x = (current_tab_x + 10.0).round();
            let dot_y = (main_y + ui.tabbar_height / 2.0 - dot_size / 2.0).round();
            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                "circle",
                dot_x,
                dot_y,
                ui.config.theme.tab_text,
                dot_size,
            );
        }

        // Draw tab label
        let label_x = current_tab_x + 12.0 + dot_reserved;
        let label_color = if is_active {
            ui.config.theme.tab_text
        } else {
            let mut c = ui.config.theme.tab_text;
            c[3] *= 0.6;
            c
        };
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &file_name,
            label_x,
            tab_baseline,
            label_color,
            ui.ui_font_size,
            ui.ui_char_width,
        );

        let is_tab_hovered = ui.active_modal.is_none() && mouse_x >= current_tab_x && mouse_x < current_tab_x + tab_w && mouse_y >= main_y && mouse_y < main_y + ui.tabbar_height;
        if is_tab_hovered {
            // Draw close button SVG icon
            let close_x = current_tab_x + tab_w - 10.0 - tab_close_icon_sz;
            let close_y = (main_y + ui.tabbar_height / 2.0 - tab_close_icon_sz / 2.0).round();

            let is_close_hovered = ui.active_modal.is_none() && mouse_x >= close_x - 3.0 && mouse_x < close_x + tab_close_icon_sz + 3.0 && mouse_y >= close_y - 3.0 && mouse_y < close_y + tab_close_icon_sz + 3.0;
            let close_color = if is_close_hovered {
                [1.0, 0.3, 0.3, 1.0]
            } else {
                let mut c = ui.config.theme.tab_text;
                c[3] *= 0.4;
                c
            };

            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                "close",
                close_x,
                close_y,
                close_color,
                tab_close_icon_sz,
            );
        }

        current_tab_x += tab_w;
    }

    // --- Breadcrumb Bar (New) ---
    // Breadcrumb bar background (white)
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        main_y + ui.tabbar_height,
        width - (activity_bar_width + ui.sidebar_width),
        ui.breadcrumb_height,
        white_uv,
        ui.config.theme.breadcrumb_bg,
    );
    // Breadcrumb bottom border
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        main_y + ui.tabbar_height + ui.breadcrumb_height - 1.0,
        width - (activity_bar_width + ui.sidebar_width),
        1.0,
        white_uv,
        ui.config.theme.breadcrumb_border,
    );
    
    // Construct breadcrumb text: relative_path > current_function
    let relative_path = ui.selected_file.as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());
    
    let current_fn = ui.find_current_function(buffer, cursor.line);
    let breadcrumb_text = if let Some(ref func) = current_fn {
        format!("{} > {}", relative_path, func)
    } else {
        relative_path
    };
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &breadcrumb_text,
        activity_bar_width + ui.sidebar_width + 15.0,
        (main_y + ui.tabbar_height + ui.breadcrumb_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
        ui.config.theme.breadcrumb_text,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // --- 3. Draw Editor Text Area & Gutter (Light Theme) ---
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

    let start_idx = ui.scroll_y;
    let end_idx = (start_idx + visible_lines).min(buffer.len());

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

        // Draw line numbers
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

    // --- 4. Draw Scrollbar ---
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

    // --- 4.1 Draw Horizontal Scrollbar ---
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

    // --- 4.5. Draw Minimap ---
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
    let minimap_offset_y = if minimap_total_h > editor_height {
        scroll_ratio * (minimap_total_h - editor_height)
    } else {
        0.0
    };

    // Determine visible lines in the minimap to optimize rendering
    let start_line = ((minimap_offset_y - 2.0) / minimap_line_height).floor().max(0.0) as usize;
    let end_line = ((editor_height + minimap_offset_y) / minimap_line_height).ceil().max(0.0) as usize;
    let end_line = end_line.min(buffer.len());

    for line_idx in start_line..end_line {
        let row_y = editor_y + line_idx as f32 * minimap_line_height - minimap_offset_y;
        
        let line_text = &buffer.lines()[line_idx];
        let char_colors = ui.get_line_char_colors(line_text);
        
        let mut current_col = 0.0f32;
        let mut start_x = 0.0f32;
        let mut current_color = None;
        let mut block_w = 0.0f32;
        
        for (char_idx, c) in line_text.chars().enumerate() {
            let char_w = if c == '\t' { 4.0 * minimap_char_w } else { minimap_char_w };
            let color = char_colors.get(char_idx).copied().unwrap_or(ui.config.theme.syntax_default);
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
