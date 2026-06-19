use crate::machkit::{UiState, Vertex};
use crate::renderer::atlas::FontAtlas;
use std::path::Path;

pub fn draw_unsaved_changes(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    mouse_x: f32,
    mouse_y: f32,
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
    modal_h: f32,
    white_uv: [f32; 2],
    tab_paths: &[Option<String>],
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

    let file_name = ui
        .tab_to_close
        .and_then(|idx| tab_paths.get(idx).cloned())
        .flatten()
        .and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "untitled.txt".to_string());

    let title_text = "Unsaved Changes";
    ctx.push_str(
        title_text,
        modal_x + 20.0,
        modal_y + 35.0,
        ctx.theme.modal_text_title,
        ctx.ui_font_size,
    );

    let mut truncated_name = file_name.clone();
    if truncated_name.chars().count() > 20 {
        let prefix: String = truncated_name.chars().take(17).collect();
        truncated_name = format!("{}...", prefix);
    }
    let msg_text = format!("'{}' has unsaved changes.", truncated_name);
    ctx.push_str(
        &msg_text,
        modal_x + 20.0,
        modal_y + 70.0,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );

    let msg_text_2 = "Save changes before closing?";
    ctx.push_str(
        msg_text_2,
        modal_x + 20.0,
        modal_y + 92.0,
        ctx.theme.modal_text_normal,
        ctx.ui_font_size,
    );

    let btn_w = 130.0f32;
    let btn_h = 34.0f32;
    let spacing = 15.0f32;

    let total_btn_block_w = 3.0 * btn_w + 2.0 * spacing;
    let start_btn_x = modal_x + ((modal_w - total_btn_block_w) / 2.0).round();
    let btn_y = modal_y + modal_h - btn_h - 20.0;

    let btn_labels = ["Save", "Don't Save", "Cancel"];
    for i in 0..3 {
        let bx = start_btn_x + i as f32 * (btn_w + spacing);
        let label = btn_labels[i];

        crate::machkit::Button::new()
            .text(label)
            .border(true)
            .bg_color(ctx.theme.button_bg)
            .draw(&mut ctx, bx, btn_y, btn_w, btn_h);
    }
}
