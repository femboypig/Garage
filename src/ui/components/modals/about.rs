use crate::ui::{UiState, Vertex, FontAtlas};

pub fn draw_about(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
    _modal_h: f32,
    white_uv: [f32; 2],
) {
    let title = "Garage";
    let title_font_sz = ui.ui_font_size * 1.5;
    let title_char_w = ui.ui_char_width * 1.5;
    let title_w = title.chars().count() as f32 * title_char_w;
    let title_x = modal_x + ((modal_w - title_w) / 2.0).round();
    
    // 1. Draw Title "Garage"
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        title,
        title_x,
        modal_y + 35.0,
        ui.config.theme.modal_text_title,
        title_font_sz,
        title_char_w,
    );

    // 2. Draw thin divider line
    ui.push_quad(
        vertices,
        indices,
        modal_x + 30.0,
        modal_y + 55.0,
        modal_w - 60.0,
        1.0,
        white_uv,
        ui.config.theme.tabbar_border,
    );

    // 3. Draw description
    let desc = "A supercharged GPU-accelerated text editor.";
    let desc_w = desc.chars().count() as f32 * ui.ui_char_width;
    let desc_x = modal_x + ((modal_w - desc_w) / 2.0).round();
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        desc,
        desc_x,
        modal_y + 80.0,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // 4. Version
    let version = "Version 0.1.0 (main)";
    let version_w = version.chars().count() as f32 * ui.ui_char_width * 0.9;
    let version_x = modal_x + ((modal_w - version_w) / 2.0).round();
    let mut muted_text_color = ui.config.theme.modal_text_normal;
    muted_text_color[3] *= 0.6; // Mute color alpha
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        version,
        version_x,
        modal_y + 115.0,
        muted_text_color,
        ui.ui_font_size * 0.9,
        ui.ui_char_width * 0.9,
    );
}
