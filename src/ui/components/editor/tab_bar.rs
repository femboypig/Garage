use crate::ui::{UiState, Vertex, FontAtlas};

pub fn draw_tab_bar(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    mouse_x: f32,
    mouse_y: f32,
    tab_paths: &[Option<String>],
    tab_modified: &[bool],
    active_tab_idx: usize,
    main_y: f32,
    activity_bar_width: f32,
    dragged_tab_idx: Option<usize>,
    tab_scroll_x: f32,
) {
    let white_uv = atlas.white_pixel_uv();
    let tabbar_start_x = activity_bar_width + ui.sidebar_width;
    let visible_width = (width - tabbar_start_x).max(0.0);
    
    // Tab Bar background (gray)
    ui.push_quad(
        vertices,
        indices,
        tabbar_start_x,
        main_y,
        visible_width,
        ui.tabbar_height,
        white_uv,
        ui.config.theme.tabbar_bg,
    );

    let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);

    // Calculate total width of all tabs
    let mut total_tabs_width = 0.0f32;
    for idx in 0..tab_paths.len() {
        let path_opt = &tab_paths[idx];
        let file_name = ui.get_tab_name(path_opt.as_deref());
        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
        let dot_reserved = 18.0f32;
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
        total_tabs_width += tab_w;
    }

    // Clamp tab scroll offset
    let max_scroll_x = (total_tabs_width - visible_width).max(0.0);
    let tab_scroll_x = tab_scroll_x.clamp(0.0, max_scroll_x);

    // Pre-calculate active tab X and width to omit the border underneath it
    let mut active_tab_x = 0.0f32;
    let mut active_tab_w = 0.0f32;
    let mut has_active_tab = false;

    let mut temp_x = tabbar_start_x;
    for idx in 0..tab_paths.len() {
        let path_opt = &tab_paths[idx];
        let file_name = ui.get_tab_name(path_opt.as_deref());
        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
        let dot_reserved = 18.0f32;
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
        if idx == active_tab_idx && Some(idx) != dragged_tab_idx {
            active_tab_x = temp_x - tab_scroll_x;
            active_tab_w = tab_w;
            has_active_tab = true;
        }
        temp_x += tab_w;
    }

    // Tab bar bottom border
    let border_y = main_y + ui.tabbar_height - 1.0;
    if has_active_tab {
        let active_left = active_tab_x.clamp(tabbar_start_x, width);
        let active_right = (active_tab_x + active_tab_w).clamp(tabbar_start_x, width);
        
        if active_left > tabbar_start_x {
            ui.push_quad(
                vertices,
                indices,
                tabbar_start_x,
                border_y,
                active_left - tabbar_start_x,
                1.0,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
        if active_right < width {
            ui.push_quad(
                vertices,
                indices,
                active_right,
                border_y,
                width - active_right,
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
            border_y,
            width - tabbar_start_x,
            1.0,
            white_uv,
            ui.config.theme.tabbar_border,
        );
    }

    // Draw active/inactive file tabs
    let mut current_tab_x = tabbar_start_x;
    let tab_baseline = (main_y + ui.tabbar_height / 2.0 + ui.ui_font_ascent / 2.0 - 3.5).round();

    for idx in 0..tab_paths.len() {
        let path_opt = &tab_paths[idx];
        let is_active = idx == active_tab_idx;
        let is_modified = tab_modified.get(idx).copied().unwrap_or(false);
        let is_diagnostics = path_opt.as_deref() == Some("diagnostics://project");

        let file_name = ui.get_tab_name(path_opt.as_deref());

        // Compute tab width
        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
        let dot_reserved = 18.0f32;
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);

        let draw_x = current_tab_x - tab_scroll_x;
        let clip_left = draw_x.max(tabbar_start_x);
        let clip_right = (draw_x + tab_w).min(width);

        if clip_left < clip_right {
            // Draw tab background
            let mut bg_color = if is_active {
                ui.config.theme.tab_active_bg
            } else {
                ui.config.theme.tabbar_bg
            };
            if Some(idx) == dragged_tab_idx {
                bg_color[3] *= 0.15;
            }
            let tab_h = if is_active {
                ui.tabbar_height
            } else {
                ui.tabbar_height - 1.0
            };
            ui.push_quad(
                vertices,
                indices,
                clip_left,
                main_y,
                clip_right - clip_left,
                tab_h,
                white_uv,
                bg_color,
            );

            // Draw separators/borders
            if idx > 0 {
                let border_x = draw_x;
                if border_x >= tabbar_start_x && border_x < width {
                    ui.push_quad(
                        vertices,
                        indices,
                        border_x,
                        main_y,
                        1.0,
                        ui.tabbar_height,
                        white_uv,
                        ui.config.theme.tabbar_border,
                    );
                }
            }
            let end_border_x = draw_x + tab_w - 1.0;
            if end_border_x >= tabbar_start_x && end_border_x < width {
                ui.push_quad(
                    vertices,
                    indices,
                    end_border_x,
                    main_y,
                    1.0,
                    ui.tabbar_height,
                    white_uv,
                    ui.config.theme.tabbar_border,
                );
            }

            // Draw unsaved circle icon if modified
            if is_modified && !is_diagnostics && Some(idx) != dragged_tab_idx {
                let dot_size = (ui.ui_font_size * 0.55).round().max(7.0);
                let dot_x = (draw_x + 10.0).round();
                let dot_y = (main_y + ui.tabbar_height / 2.0 - dot_size / 2.0).round();
                if dot_x >= tabbar_start_x && dot_x + dot_size < clip_right {
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
            }

            // Draw tab label (with char-by-char clipping)
            let label_x = draw_x + 12.0 + dot_reserved;
            let mut label_color = if is_active {
                ui.config.theme.tab_text
            } else {
                let mut c = ui.config.theme.tab_text;
                c[3] *= 0.6;
                c
            };
            if Some(idx) == dragged_tab_idx {
                label_color[3] *= 0.15;
            }
            
            let mut cur_char_x = label_x;
            for (char_idx, c) in file_name.chars().enumerate() {
                if cur_char_x + ui.ui_char_width > clip_right - 18.0 {
                    break;
                }
                if cur_char_x >= tabbar_start_x {
                    if is_diagnostics && char_idx == 0 {
                        // Draw circle representing status of diagnostics (red for errors, yellow for warnings, gray for clean)
                        let mut err_count = 0;
                        let mut warn_count = 0;
                        for (e, w) in ui.lsp_diagnostics.values() {
                            err_count += *e;
                            warn_count += *w;
                        }
                        let dot_color = if err_count > 0 {
                            [0.95, 0.25, 0.25, 1.0] // Red
                        } else if warn_count > 0 {
                            [0.95, 0.70, 0.15, 1.0] // Yellow
                        } else {
                            [0.5, 0.5, 0.5, 0.6] // Muted gray
                        };
                        let dot_size = (ui.ui_font_size * 0.65).round().max(8.0);
                        let dot_y = (main_y + ui.tabbar_height / 2.0 - dot_size / 2.0).round();
                        ui.push_icon(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            "circle",
                            cur_char_x,
                            dot_y,
                            dot_color,
                            dot_size,
                        );
                    } else {
                        ui.push_char(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            c,
                            cur_char_x,
                            tab_baseline,
                            label_color,
                            ui.ui_font_size,
                            ui.ui_char_width,
                        );
                    }
                }
                cur_char_x += ui.ui_char_width;
            }

            // Draw close button SVG icon if hovered
            let is_tab_hovered = ui.active_modal.is_none()
                && Some(idx) != dragged_tab_idx
                && mouse_x >= draw_x
                && mouse_x < draw_x + tab_w
                && mouse_y >= main_y
                && mouse_y < main_y + ui.tabbar_height;
            let is_tab_hovered_visible = is_tab_hovered && mouse_x >= tabbar_start_x && mouse_x < width;
            
            if is_tab_hovered_visible {
                let close_x = draw_x + tab_w - 10.0 - tab_close_icon_sz;
                let close_y = (main_y + ui.tabbar_height / 2.0 - tab_close_icon_sz / 2.0).round();

                let is_close_hovered = ui.active_modal.is_none()
                    && mouse_x >= close_x - 3.0
                    && mouse_x < close_x + tab_close_icon_sz + 3.0
                    && mouse_y >= close_y - 3.0
                    && mouse_y < close_y + tab_close_icon_sz + 3.0;
                let close_color = if is_close_hovered {
                    [1.0, 0.3, 0.3, 1.0]
                } else {
                    let mut c = ui.config.theme.tab_text;
                    c[3] *= 0.4;
                    c
                };

                if close_x >= tabbar_start_x && close_x + tab_close_icon_sz < width {
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
            }
        }

        current_tab_x += tab_w;
    }

    // Draw scrollbar if there is overflow and user is hovering tab bar or dragging
    if total_tabs_width > visible_width {
        let is_tabbar_hovered = ui.active_modal.is_none()
            && mouse_x >= tabbar_start_x
            && mouse_x < width
            && mouse_y >= main_y
            && mouse_y < main_y + ui.tabbar_height;

        if is_tabbar_hovered || ui.tab_scroll_is_dragging {
            let sb_y = main_y + ui.tabbar_height - 4.0;
            let sb_h = 4.0f32;
            
            // Track background
            ui.push_quad(
                vertices,
                indices,
                tabbar_start_x,
                sb_y,
                visible_width,
                sb_h,
                white_uv,
                [0.0, 0.0, 0.0, 0.15],
            );
            
            // Thumb
            let ratio = visible_width / total_tabs_width;
            let thumb_w = (visible_width * ratio).clamp(20.0_f32.min(visible_width), visible_width);
            let scroll_ratio_x = if max_scroll_x > 0.0 { tab_scroll_x / max_scroll_x } else { 0.0 };
            let thumb_x = tabbar_start_x + scroll_ratio_x * (visible_width - thumb_w);
            
            let is_thumb_hovered = mouse_x >= thumb_x && mouse_x < thumb_x + thumb_w && mouse_y >= sb_y && mouse_y < sb_y + sb_h;
            let thumb_color = if is_thumb_hovered || ui.tab_scroll_is_dragging {
                ui.config.theme.scrollbar_thumb_hover
            } else {
                ui.config.theme.scrollbar_thumb
            };
            
            ui.push_quad(
                vertices,
                indices,
                thumb_x,
                sb_y,
                thumb_w,
                sb_h,
                white_uv,
                thumb_color,
            );
        }
    }
}

pub fn draw_floating_tab(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    mouse_x: f32,
    mouse_y: f32,
    tab_path: Option<&str>,
    is_modified: bool,
) {
    let white_uv = atlas.white_pixel_uv();
    let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
    
    let is_diagnostics = tab_path == Some("diagnostics://project");
    let file_name = ui.get_tab_name(tab_path);
    
    let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
    let dot_reserved = 18.0f32;
    let close_reserved = 8.0f32 + tab_close_icon_sz;
    let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
    let tab_h = ui.tabbar_height;
    
    let draw_x = mouse_x - tab_w / 2.0;
    let draw_y = mouse_y - tab_h / 2.0;
    
    // Background (semi-transparent active tab color)
    let mut bg_color = ui.config.theme.tab_active_bg;
    bg_color[3] = 0.85; // slightly transparent
    
    ui.push_quad(
        vertices,
        indices,
        draw_x,
        draw_y,
        tab_w,
        tab_h,
        white_uv,
        bg_color,
    );
    
    // Subtle border around the floating tab
    let border_color = [0.25, 0.55, 0.95, 0.9]; // Premium blue border
    ui.push_quad(vertices, indices, draw_x, draw_y, tab_w, 1.0, white_uv, border_color);
    ui.push_quad(vertices, indices, draw_x, draw_y + tab_h - 1.0, tab_w, 1.0, white_uv, border_color);
    ui.push_quad(vertices, indices, draw_x, draw_y, 1.0, tab_h, white_uv, border_color);
    ui.push_quad(vertices, indices, draw_x + tab_w - 1.0, draw_y, 1.0, tab_h, white_uv, border_color);
    
    // Draw unsaved dot
    if is_modified && !is_diagnostics {
        let dot_size = (ui.ui_font_size * 0.55).round().max(7.0);
        let dot_x = (draw_x + 10.0).round();
        let dot_y = (draw_y + tab_h / 2.0 - dot_size / 2.0).round();
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
    
    // Draw label
    let label_x = draw_x + 12.0 + dot_reserved;
    let label_color = ui.config.theme.tab_text;
    let tab_baseline = (draw_y + tab_h / 2.0 + ui.ui_font_ascent / 2.0 - 3.5).round();
    
    let mut cur_char_x = label_x;
    for (char_idx, c) in file_name.chars().enumerate() {
        if cur_char_x + ui.ui_char_width > draw_x + tab_w - 18.0 {
            break;
        }
        if is_diagnostics && char_idx == 0 {
            let mut err_count = 0;
            let mut warn_count = 0;
            for (e, w) in ui.lsp_diagnostics.values() {
                err_count += *e;
                warn_count += *w;
            }
            let dot_color = if err_count > 0 {
                [0.95, 0.25, 0.25, 1.0]
            } else if warn_count > 0 {
                [0.95, 0.70, 0.15, 1.0]
            } else {
                [0.5, 0.5, 0.5, 0.6]
            };
            let dot_size = (ui.ui_font_size * 0.65).round().max(8.0);
            let dot_y = (draw_y + tab_h / 2.0 - dot_size / 2.0).round();
            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                "circle",
                cur_char_x,
                dot_y,
                dot_color,
                dot_size,
            );
        } else {
            ui.push_char(
                vertices,
                indices,
                atlas,
                queue,
                c,
                cur_char_x,
                tab_baseline,
                label_color,
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }
        cur_char_x += ui.ui_char_width;
    }
    
    // Draw close icon
    let close_x = draw_x + tab_w - 10.0 - tab_close_icon_sz;
    let close_y = (draw_y + tab_h / 2.0 - tab_close_icon_sz / 2.0).round();
    ui.push_icon(
        vertices,
        indices,
        atlas,
        queue,
        "close",
        close_x,
        close_y,
        ui.config.theme.tab_text,
        tab_close_icon_sz,
    );
}
