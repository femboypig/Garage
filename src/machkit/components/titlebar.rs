use crate::machkit::{MenuType, UiState};
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;

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

    // ── Menu items ────────────────────────────────────────────────────────
    let menu_items_raw = [
        ("Garage", MenuType::Garage),
        ("File", MenuType::File),
        ("Edit", MenuType::Edit),
        ("Selection", MenuType::Selection),
        ("View", MenuType::View),
    ];

    // On macOS the native traffic-light buttons (close/minimize/zoom) are
    // rendered by AppKit at x≈[8..70]. We start our menu items at x=80
    // so they never overlap the buttons. If we are in fullscreen mode,
    // the buttons are hidden, so we start at x=0.
    let menu_start_x: f32 = if cfg!(target_os = "macos") && !ui.is_fullscreen { 80.0 } else { 0.0 };

    let mut menu_positions = Vec::new();
    let mut current_x = menu_start_x;
    for (i, (label, menu_type)) in menu_items_raw.iter().enumerate() {
        let label_len = label.chars().count() as f32;
        let text_w = label_len * ui.ui_char_width;
        let (left_pad, right_pad) = if i == 0 { (14.0, 10.0) } else { (10.0, 10.0) };
        let item_w = text_w + left_pad + right_pad;
        let x_min = current_x;
        let x_max = current_x + item_w;
        menu_positions.push((*label, x_min, x_max, left_pad, *menu_type));
        current_x = x_max;
    }

    for (label, x_min, x_max, left_pad, menu_type) in &menu_positions {
        let is_hovered = ui.active_modal.is_none()
            && mouse_y < ui.titlebar_height
            && mouse_x >= *x_min
            && mouse_x < *x_max;
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
    let file_name = ui
        .selected_file
        .as_ref()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
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
}
