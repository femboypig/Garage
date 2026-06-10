use crate::ui::{UiState, Vertex, FontAtlas};
use std::path::Path;

pub fn draw_tab_bar(
    ui: &UiState,
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
) {
    let white_uv = atlas.white_pixel_uv();
    
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
}
