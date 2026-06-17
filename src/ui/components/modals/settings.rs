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
    _modal_w: f32,
    _modal_h: f32,
    white_uv: [f32; 2],
) {
    let mut ctx = crate::machkit::UiContext {
        vertices,
        indices,
        atlas,
        queue,
        mouse_x,
        mouse_y,
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

    let row_height = (ctx.ui_line_height * 2.2).round();
    let control_x = modal_x + 24.0 * ctx.ui_char_width;
    let btn_h = (ctx.ui_line_height * 1.3).round().max(24.0);
    let btn_w = (ctx.ui_char_width * 3.0).round().max(24.0);
    let backend_btn_w = (ctx.ui_char_width * 10.0).round().max(80.0);
    let theme_btn_w = (ctx.ui_char_width * 16.0).round().max(140.0);
    let padding_x = 2.0 * ctx.ui_char_width;

    ctx.push_str(
        "SETTINGS",
        modal_x + padding_x,
        modal_y + (ctx.ui_line_height * 1.8).round(),
        ctx.theme.modal_text_title,
        ctx.ui_font_size,
    );

    // Helper closure to draw button container using machkit::Button
    let draw_button = |
        ctx: &mut crate::machkit::UiContext,
        text: &str,
        bx: f32,
        by: f32,
        bw: f32,
        bh: f32,
        is_selected: bool,
    | {
        let bg_color = if is_selected {
            ctx.theme.cursor_color // brand color
        } else {
            ctx.theme.button_bg
        };
        let border_color = if is_selected {
            ctx.theme.cursor_color
        } else {
            ctx.theme.button_border
        };
        let text_color = if is_selected {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            ctx.theme.button_text
        };

        crate::machkit::Button::new()
            .text(text)
            .bg_color(bg_color)
            .border_color(border_color)
            .text_color(text_color)
            .border(true)
            .draw(ctx, bx, by, bw, bh);
    };

    // 1. Editor Font Size Settings
    let row1_y = modal_y + row_height * 1.0;
    let btn1_y = row1_y + ((ctx.ui_line_height - btn_h) / 2.0).round();
    let font_size_str = format!("Editor Font: {:.1} px", ctx.buffer_font_size);
    ctx.push_str(
        &font_size_str,
        modal_x + padding_x,
        row1_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );
    draw_button(&mut ctx, "-", control_x, btn1_y, btn_w, btn_h, false);
    let inc_btn_x = control_x + btn_w + ctx.ui_char_width;
    draw_button(&mut ctx, "+", inc_btn_x, btn1_y, btn_w, btn_h, false);

    // 2. UI Font Size Settings
    let row2_y = modal_y + row_height * 2.0;
    let btn2_y = row2_y + ((ctx.ui_line_height - btn_h) / 2.0).round();
    let ui_size_str = format!("UI Font:     {:.1} px", ctx.ui_font_size);
    ctx.push_str(
        &ui_size_str,
        modal_x + padding_x,
        row2_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );
    draw_button(&mut ctx, "-", control_x, btn2_y, btn_w, btn_h, false);
    draw_button(&mut ctx, "+", inc_btn_x, btn2_y, btn_w, btn_h, false);

    // 3. Backend Selection
    let row3_y = modal_y + row_height * 3.0;
    let btn3_y = row3_y + ((ctx.ui_line_height - btn_h) / 2.0).round();
    ctx.push_str(
        "Backend:",
        modal_x + padding_x,
        row3_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );
    let is_vulkan = ui.config.backend == "Vulkan";
    draw_button(&mut ctx, "Vulkan", control_x, btn3_y, backend_btn_w, btn_h, is_vulkan);

    let is_opengl = ui.config.backend == "OpenGL";
    let opengl_btn_x = control_x + backend_btn_w + ctx.ui_char_width;
    draw_button(&mut ctx, "OpenGL", opengl_btn_x, btn3_y, backend_btn_w, btn_h, is_opengl);

    // 4. Theme Selection (Cycle Toggle Selector)
    let row4_y = modal_y + row_height * 4.0;
    let btn4_y = row4_y + ((ctx.ui_line_height - btn_h) / 2.0).round();
    ctx.push_str(
        "Theme:",
        modal_x + padding_x,
        row4_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );
    let display_theme = format!("{}  ▼", ui.config.theme.name);
    draw_button(&mut ctx, &display_theme, control_x, btn4_y, theme_btn_w, btn_h, false);

    // 5. Git Blame Selection
    let row5_y = modal_y + row_height * 5.0;
    let btn5_y = row5_y + ((ctx.ui_line_height - btn_h) / 2.0).round();
    ctx.push_str(
        "Git Blame:",
        modal_x + padding_x,
        row5_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );
    draw_button(&mut ctx, "Enabled", control_x, btn5_y, backend_btn_w, btn_h, ui.config.show_git_blame);

    let blame_disabled_x = control_x + backend_btn_w + ctx.ui_char_width;
    draw_button(&mut ctx, "Disabled", blame_disabled_x, btn5_y, backend_btn_w, btn_h, !ui.config.show_git_blame);

    // 6. Git Branch Selection
    let row6_y = modal_y + row_height * 6.0;
    let btn6_y = row6_y + ((ctx.ui_line_height - btn_h) / 2.0).round();
    ctx.push_str(
        "Git Branch:",
        modal_x + padding_x,
        row6_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );
    draw_button(&mut ctx, "Enabled", control_x, btn6_y, backend_btn_w, btn_h, ui.config.show_git_branch);

    let branch_disabled_x = control_x + backend_btn_w + ctx.ui_char_width;
    draw_button(&mut ctx, "Disabled", branch_disabled_x, btn6_y, backend_btn_w, btn_h, !ui.config.show_git_branch);

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
    ctx.push_str(
        &active_info_str,
        modal_x + padding_x,
        row7_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_muted,
        ctx.ui_font_size,
    );

    // 8. Draw Theme Selection Dropdown (if open) - rendered on top of active info
    if ui.theme_dropdown_open {
        let dropdown_y = btn4_y + btn_h;
        let item_height = (ctx.ui_line_height * 1.5).round().max(24.0);
        let dropdown_h = 2.0 * item_height;

        // Draw Dropdown background
        ctx.push_quad(control_x, dropdown_y, theme_btn_w, dropdown_h, ctx.theme.modal_bg);
        // Draw Dropdown borders
        ctx.push_quad(control_x, dropdown_y, theme_btn_w, 1.0, ctx.theme.modal_border);
        ctx.push_quad(control_x, dropdown_y + dropdown_h - 1.0, theme_btn_w, 1.0, ctx.theme.modal_border);
        ctx.push_quad(control_x, dropdown_y, 1.0, dropdown_h, ctx.theme.modal_border);
        ctx.push_quad(control_x + theme_btn_w - 1.0, dropdown_y, 1.0, dropdown_h, ctx.theme.modal_border);

        let themes = ["Light Theme", "Dark Theme"];
        for (idx, t_name) in themes.iter().enumerate() {
            let item_y = dropdown_y + idx as f32 * item_height;
            let is_item_hovered = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= item_y && mouse_y < item_y + item_height;

            if is_item_hovered {
                ctx.push_quad(control_x + 1.0, item_y + 1.0, theme_btn_w - 2.0, item_height - 2.0, ctx.theme.button_hover_bg);
            }

            let text_y = (item_y + item_height / 2.0 + ctx.ui_font_ascent / 2.0 - 2.0).round();
            ctx.push_str(
                t_name,
                control_x + 10.0,
                text_y,
                ctx.theme.modal_text_normal,
                ctx.ui_font_size,
            );
        }
    }
}
