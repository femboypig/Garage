use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::ui::{UiState, MenuType};

pub fn draw_titlebar(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    mouse_x: f32,
    mouse_y: f32,
) {
    let white_uv = atlas.white_pixel_uv();

    // Draw Titlebar Background
    ui.push_quad(
        vertices,
        indices,
        0.0,
        0.0,
        width,
        ui.titlebar_height,
        white_uv,
        ui.config.theme.titlebar_bg,
    );

    // Draw Titlebar Border
    ui.push_quad(
        vertices,
        indices,
        0.0,
        ui.titlebar_height - 1.0,
        width,
        1.0,
        white_uv,
        ui.config.theme.titlebar_border,
    );

    let menu_items_raw = [
        ("Garage", MenuType::Garage),
        ("File", MenuType::File),
        ("Edit", MenuType::Edit),
        ("Selection", MenuType::Selection),
        ("View", MenuType::View),
    ];

    let mut menu_positions = Vec::new();
    let mut current_x = 0.0;
    for (i, (label, menu_type)) in menu_items_raw.iter().enumerate() {
        let label_len = label.chars().count() as f32;
        let text_w = label_len * ui.ui_char_width;
        let (left_pad, right_pad) = if i == 0 {
            (14.0, 10.0)
        } else {
            (10.0, 10.0)
        };
        let item_w = text_w + left_pad + right_pad;
        let x_min = current_x;
        let x_max = current_x + item_w;
        menu_positions.push((*label, x_min, x_max, left_pad, *menu_type));
        current_x = x_max;
    }

    for (label, x_min, x_max, left_pad, menu_type) in &menu_positions {
        let is_hovered = ui.active_modal.is_none() && mouse_y < ui.titlebar_height && mouse_x >= *x_min && mouse_x < *x_max;
        let is_active = ui.active_menu == Some(*menu_type);

        if is_hovered || is_active {
            ui.push_quad(
                vertices,
                indices,
                *x_min,
                0.0,
                *x_max - *x_min,
                ui.titlebar_height - 1.0,
                white_uv,
                ui.config.theme.titlebar_hover_bg,
            );
        }
        
        let label_color = if is_active || is_hovered {
            if ui.config.theme.name.contains("Dark") {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.0, 0.0, 0.0, 1.0]
            }
        } else {
            ui.config.theme.titlebar_text
        };

        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            label,
            *x_min + *left_pad,
            (ui.titlebar_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            label_color,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }

    // Display current open file title in titlebar center
    let file_name = ui.selected_file.as_ref()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());
    let title_text = format!("Garage - {}", file_name);
    let title_len = title_text.chars().count() as f32;
    let title_x = ((width - title_len * ui.ui_char_width) / 2.0).round();
    if title_x > current_x + 20.0 {
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &title_text,
            title_x,
            (ui.titlebar_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            ui.config.theme.titlebar_text,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }

    if !ui.is_tiling_wm() {
        // --- Window Controls (Client-Side Decorations) ---
        // Minimize, Maximize, Close buttons on the top right
        let btn_w = 45.0f32;
        let btn_h = ui.titlebar_height - 1.0;
        let control_y = 0.0f32;
        
        let close_x = width - btn_w;
        let max_x = width - btn_w * 2.0;
        let min_x = width - btn_w * 3.0;

        // Check hovers
        let is_close_hover = ui.active_modal.is_none() && mouse_y >= control_y && mouse_y < control_y + btn_h && mouse_x >= close_x && mouse_x < width;
        let is_max_hover = ui.active_modal.is_none() && mouse_y >= control_y && mouse_y < control_y + btn_h && mouse_x >= max_x && mouse_x < close_x;
        let is_min_hover = ui.active_modal.is_none() && mouse_y >= control_y && mouse_y < control_y + btn_h && mouse_x >= min_x && mouse_x < max_x;

        let hover_sz = 24.0f32;
        let hover_y = control_y + ((btn_h - hover_sz) / 2.0).round();

        // Draw Close button hover
        if is_close_hover {
            let close_bg = [0.85, 0.25, 0.25, 1.0]; // beautiful red hover
            let hover_x = close_x + ((btn_w - hover_sz) / 2.0).round();
            ui.push_quad(vertices, indices, hover_x, hover_y, hover_sz, hover_sz, white_uv, close_bg);
        }
        
        // Draw Maximize button hover
        if is_max_hover {
            let max_bg = ui.config.theme.titlebar_hover_bg;
            let hover_x = max_x + ((btn_w - hover_sz) / 2.0).round();
            ui.push_quad(vertices, indices, hover_x, hover_y, hover_sz, hover_sz, white_uv, max_bg);
        }

        // Draw Minimize button hover
        if is_min_hover {
            let min_bg = ui.config.theme.titlebar_hover_bg;
            let hover_x = min_x + ((btn_w - hover_sz) / 2.0).round();
            ui.push_quad(vertices, indices, hover_x, hover_y, hover_sz, hover_sz, white_uv, min_bg);
        }

        // Draw Icons
        let icon_sz = 14.0f32;
        let icon_y = (btn_h / 2.0 - icon_sz / 2.0).round();
        
        // Minimize icon
        let min_color = ui.config.theme.titlebar_text;
        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "minimize",
            (min_x + (btn_w - icon_sz) / 2.0).round(),
            icon_y,
            min_color,
            icon_sz,
        );

        // Maximize icon
        let max_color = ui.config.theme.titlebar_text;
        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "maximize",
            (max_x + (btn_w - icon_sz) / 2.0).round(),
            icon_y,
            max_color,
            icon_sz,
        );

        // Close icon
        let close_color = if is_close_hover {
            [1.0, 1.0, 1.0, 1.0] // White on red bg
        } else {
            ui.config.theme.titlebar_text
        };
        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "close",
            (close_x + (btn_w - icon_sz) / 2.0).round(),
            icon_y,
            close_color,
            icon_sz,
        );
    }
}
