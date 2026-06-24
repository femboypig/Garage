use crate::machkit::UiState;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;

pub fn draw_sidebar_input(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    mouse_x: f32,
    mouse_y: f32,
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
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

    let title = ui
        .sidebar_input_mode
        .map(|mode| mode.title())
        .unwrap_or("Input");

    let title_y = modal_y + 20.0;
    ctx.push_str(
        title,
        modal_x + 20.0,
        title_y + ctx.ui_font_ascent,
        ctx.theme.modal_text_title,
        ctx.ui_font_size,
    );

    let input_x = modal_x + 20.0;
    let input_y = title_y + ctx.ui_line_height + 15.0;
    let input_w = modal_w - 40.0;
    let input_h = ctx.ui_line_height + 8.0;

    crate::machkit::Input::new()
        .value(&ui.sidebar_input_value)
        .focused(true)
        .draw(&mut ctx, input_x, input_y, input_w, input_h);

    let btn_w = 80.0f32;
    let btn_h = 24.0f32;
    let cancel_x = modal_x + modal_w - 20.0 - btn_w * 2.0 - 10.0;
    let confirm_x = modal_x + modal_w - 20.0 - btn_w;
    let btn_y = input_y + input_h + 15.0;

    crate::machkit::Button::new()
        .text("Cancel")
        .border(true)
        .bg_color(ctx.theme.button_bg)
        .draw(&mut ctx, cancel_x, btn_y, btn_w, btn_h);

    crate::machkit::Button::new()
        .text("OK")
        .border(true)
        .bg_color(ctx.theme.button_bg)
        .draw(&mut ctx, confirm_x, btn_y, btn_w, btn_h);
}
