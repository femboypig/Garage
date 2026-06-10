use crate::ui::{UiState, Vertex, FontAtlas};

pub fn draw_settings(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    mouse_x: f32,
    mouse_y: f32,
    current_backend: wgpu::Backend,
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
    modal_h: f32,
    white_uv: [f32; 2],
) {
    let row_height = (ui.ui_line_height * 2.2).round();
    let control_x = modal_x + 24.0 * ui.ui_char_width;
    let btn_h = (ui.ui_line_height * 1.3).round().max(24.0);
    let btn_w = (ui.ui_char_width * 3.0).round().max(24.0);
    let backend_btn_w = (ui.ui_char_width * 10.0).round().max(80.0);
    let theme_btn_w = (ui.ui_char_width * 16.0).round().max(140.0);
    let padding_x = 2.0 * ui.ui_char_width;

    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "SETTINGS",
        modal_x + padding_x,
        modal_y + (ui.ui_line_height * 1.8).round(),
        ui.config.theme.modal_text_title,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Helper closure to draw button container with borders and label
    let draw_button = |
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        text: &str,
        bx: f32,
        by: f32,
        bw: f32,
        bh: f32,
        is_selected: bool,
        is_hovered: bool,
        theme: &crate::config::Theme,
        white_uv: [f32; 2],
        ui_char_width: f32,
        ui_font_ascent: f32,
        ui_font_size: f32,
    | {
        let bg_color = if is_selected {
            theme.cursor_color // brand color
        } else if is_hovered {
            theme.button_hover_bg
        } else {
            theme.button_bg
        };
        let border_color = if is_selected {
            theme.cursor_color
        } else {
            theme.button_border
        };
        let text_color = if is_selected {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            theme.button_text
        };

        // Draw background
        ui.push_quad(vertices, indices, bx, by, bw, bh, white_uv, bg_color);
        // Draw borders (contiguous)
        ui.push_quad(vertices, indices, bx, by, bw, 1.0, white_uv, border_color); // Top
        ui.push_quad(vertices, indices, bx, by + bh - 1.0, bw, 1.0, white_uv, border_color); // Bottom
        ui.push_quad(vertices, indices, bx, by, 1.0, bh, white_uv, border_color); // Left
        ui.push_quad(vertices, indices, bx + bw - 1.0, by, 1.0, bh, white_uv, border_color); // Right

        // Draw text centered
        let text_w = text.chars().count() as f32 * ui_char_width;
        let text_x = bx + ((bw - text_w) / 2.0).round();
        let text_y = (by + bh / 2.0 + ui_font_ascent / 2.0 - 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, text, text_x, text_y, text_color, ui_font_size, ui_char_width);
    };

    // 1. Editor Font Size Settings
    let row1_y = modal_y + row_height * 1.0;
    let btn1_y = row1_y + ((ui.ui_line_height - btn_h) / 2.0).round();
    let font_size_str = format!("Editor Font: {:.1} px", ui.buffer_font_size);
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &font_size_str,
        modal_x + padding_x,
        row1_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    let dec_hover = mouse_x >= control_x && mouse_x <= control_x + btn_w && mouse_y >= btn1_y && mouse_y <= btn1_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "-", control_x, btn1_y, btn_w, btn_h, false, dec_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);
    let inc_btn_x = control_x + btn_w + ui.ui_char_width;
    let inc_hover = mouse_x >= inc_btn_x && mouse_x <= inc_btn_x + btn_w && mouse_y >= btn1_y && mouse_y <= btn1_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "+", inc_btn_x, btn1_y, btn_w, btn_h, false, inc_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    // 2. UI Font Size Settings
    let row2_y = modal_y + row_height * 2.0;
    let btn2_y = row2_y + ((ui.ui_line_height - btn_h) / 2.0).round();
    let ui_size_str = format!("UI Font:     {:.1} px", ui.ui_font_size);
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &ui_size_str,
        modal_x + padding_x,
        row2_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    let ui_dec_hover = mouse_x >= control_x && mouse_x <= control_x + btn_w && mouse_y >= btn2_y && mouse_y <= btn2_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "-", control_x, btn2_y, btn_w, btn_h, false, ui_dec_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);
    let ui_inc_hover = mouse_x >= inc_btn_x && mouse_x <= inc_btn_x + btn_w && mouse_y >= btn2_y && mouse_y <= btn2_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "+", inc_btn_x, btn2_y, btn_w, btn_h, false, ui_inc_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    // 3. Backend Selection
    let row3_y = modal_y + row_height * 3.0;
    let btn3_y = row3_y + ((ui.ui_line_height - btn_h) / 2.0).round();
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "Backend:",
        modal_x + padding_x,
        row3_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    let is_vulkan = ui.config.backend == "Vulkan";
    let vulkan_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn3_y && mouse_y <= btn3_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "Vulkan", control_x, btn3_y, backend_btn_w, btn_h, is_vulkan, vulkan_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    let is_opengl = ui.config.backend == "OpenGL";
    let opengl_btn_x = control_x + backend_btn_w + ui.ui_char_width;
    let opengl_hover = mouse_x >= opengl_btn_x && mouse_x <= opengl_btn_x + backend_btn_w && mouse_y >= btn3_y && mouse_y <= btn3_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "OpenGL", opengl_btn_x, btn3_y, backend_btn_w, btn_h, is_opengl, opengl_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    // 4. Theme Selection (Cycle Toggle Selector)
    let row4_y = modal_y + row_height * 4.0;
    let btn4_y = row4_y + ((ui.ui_line_height - btn_h) / 2.0).round();
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "Theme:",
        modal_x + padding_x,
        row4_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    let display_theme = format!("{}  ▼", ui.config.theme.name);
    let theme_hover = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= btn4_y && mouse_y <= btn4_y + btn_h;
    draw_button(vertices, indices, atlas, queue, &display_theme, control_x, btn4_y, theme_btn_w, btn_h, false, theme_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    // 5. Git Blame Selection
    let row5_y = modal_y + row_height * 5.0;
    let btn5_y = row5_y + ((ui.ui_line_height - btn_h) / 2.0).round();
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "Git Blame:",
        modal_x + padding_x,
        row5_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    let blame_enabled_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn5_y && mouse_y <= btn5_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "Enabled", control_x, btn5_y, backend_btn_w, btn_h, ui.config.show_git_blame, blame_enabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    let blame_disabled_x = control_x + backend_btn_w + ui.ui_char_width;
    let blame_disabled_hover = mouse_x >= blame_disabled_x && mouse_x <= blame_disabled_x + backend_btn_w && mouse_y >= btn5_y && mouse_y <= btn5_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "Disabled", blame_disabled_x, btn5_y, backend_btn_w, btn_h, !ui.config.show_git_blame, blame_disabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    // 6. Git Branch Selection
    let row6_y = modal_y + row_height * 6.0;
    let btn6_y = row6_y + ((ui.ui_line_height - btn_h) / 2.0).round();
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "Git Branch:",
        modal_x + padding_x,
        row6_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    let branch_enabled_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn6_y && mouse_y <= btn6_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "Enabled", control_x, btn6_y, backend_btn_w, btn_h, ui.config.show_git_branch, branch_enabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    let branch_disabled_x = control_x + backend_btn_w + ui.ui_char_width;
    let branch_disabled_hover = mouse_x >= branch_disabled_x && mouse_x <= branch_disabled_x + backend_btn_w && mouse_y >= btn6_y && mouse_y <= btn6_y + btn_h;
    draw_button(vertices, indices, atlas, queue, "Disabled", branch_disabled_x, btn6_y, backend_btn_w, btn_h, !ui.config.show_git_branch, branch_disabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

    // 7. Draw Active backend and GPU info
    let row7_y = modal_y + row_height * 7.0;
    let backend_str = match current_backend {
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Gl => "OpenGL",
        other => &format!("{:?}", other),
    };
    let is_fallback = (ui.config.backend == "OpenGL" && current_backend != wgpu::Backend::Gl) ||
                      (ui.config.backend == "Vulkan" && current_backend != wgpu::Backend::Vulkan);
    let active_info_str = if is_fallback {
        format!("Active: {} (fallback) ({})", backend_str, ui.active_device_name)
    } else {
        format!("Active: {} ({})", backend_str, ui.active_device_name)
    };
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &active_info_str,
        modal_x + padding_x,
        row7_y + ui.ui_font_ascent,
        ui.config.theme.modal_text_muted,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // 8. Draw Theme Selection Dropdown (if open) - rendered on top of active info
    if ui.theme_dropdown_open {
        let dropdown_y = btn4_y + btn_h;
        let item_height = (ui.ui_line_height * 1.5).round().max(24.0);
        let dropdown_h = 2.0 * item_height;

        // Draw Dropdown background
        ui.push_quad(vertices, indices, control_x, dropdown_y, theme_btn_w, dropdown_h, white_uv, ui.config.theme.modal_bg);
        // Draw Dropdown borders
        ui.push_quad(vertices, indices, control_x, dropdown_y, theme_btn_w, 1.0, white_uv, ui.config.theme.modal_border);
        ui.push_quad(vertices, indices, control_x, dropdown_y + dropdown_h - 1.0, theme_btn_w, 1.0, white_uv, ui.config.theme.modal_border);
        ui.push_quad(vertices, indices, control_x, dropdown_y, 1.0, dropdown_h, white_uv, ui.config.theme.modal_border);
        ui.push_quad(vertices, indices, control_x + theme_btn_w - 1.0, dropdown_y, 1.0, dropdown_h, white_uv, ui.config.theme.modal_border);

        let themes = ["Light Theme", "Dark Theme"];
        for (idx, t_name) in themes.iter().enumerate() {
            let item_y = dropdown_y + idx as f32 * item_height;
            let is_item_hovered = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= item_y && mouse_y < item_y + item_height;

            if is_item_hovered {
                ui.push_quad(vertices, indices, control_x + 1.0, item_y + 1.0, theme_btn_w - 2.0, item_height - 2.0, white_uv, ui.config.theme.button_hover_bg);
            }

            let text_y = (item_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                t_name,
                control_x + 10.0,
                text_y,
                ui.config.theme.modal_text_normal,
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }
    }
}
