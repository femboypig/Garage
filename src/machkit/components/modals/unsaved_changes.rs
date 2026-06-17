use crate::ui::{UiState, Vertex, FontAtlas};
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
    let file_name = ui.tab_to_close
        .and_then(|idx| tab_paths.get(idx).cloned())
        .flatten()
        .and_then(|p| Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "untitled.txt".to_string());

    let title_text = "Unsaved Changes";
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        title_text,
        modal_x + 20.0,
        modal_y + 35.0,
        ui.config.theme.modal_text_title,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    let mut truncated_name = file_name.clone();
    if truncated_name.chars().count() > 20 {
        let prefix: String = truncated_name.chars().take(17).collect();
        truncated_name = format!("{}...", prefix);
    }
    let msg_text = format!("'{}' has unsaved changes.", truncated_name);
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &msg_text,
        modal_x + 20.0,
        modal_y + 70.0,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    let msg_text_2 = "Save changes before closing?";
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        msg_text_2,
        modal_x + 20.0,
        modal_y + 92.0,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
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
        let is_btn_hovered = mouse_x >= bx && mouse_x <= bx + btn_w && mouse_y >= btn_y && mouse_y <= btn_y + btn_h;

        ui.push_quad(
            vertices,
            indices,
            bx,
            btn_y,
            btn_w,
            btn_h,
            white_uv,
            if is_btn_hovered { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg },
        );
        ui.push_quad(vertices, indices, bx, btn_y, btn_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, bx, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, bx, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, bx + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);

        let label = btn_labels[i];
        let label_w = label.chars().count() as f32 * ui.ui_char_width;
        let tx = bx + ((btn_w - label_w) / 2.0).round();
        let ty = (btn_y + btn_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            label,
            tx,
            ty,
            ui.config.theme.button_text,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }
}
