use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::ui::UiState;

pub fn draw_statusbar(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    height: f32,
    buffer: &Buffer,
    cursor: &Cursor,
    mouse_x: f32,
    mouse_y: f32,
) {
    let white_uv = atlas.white_pixel_uv();
    let status_y = height - ui.status_height;

    // Draw Statusbar Background
    ui.push_quad(
        vertices,
        indices,
        0.0,
        status_y,
        width,
        ui.status_height,
        white_uv,
        ui.config.theme.statusbar_bg,
    );

    // Draw Statusbar Border
    ui.push_quad(
        vertices,
        indices,
        0.0,
        status_y,
        width,
        1.0,
        white_uv,
        ui.config.theme.statusbar_border,
    );

    let status_left = format!(" GARAGE | Line {}, Col {}", cursor.line + 1, cursor.col + 1);
    let status_right = format!("Lines: {} | UTF-8 | LF ", buffer.len());
    let baseline_y = (status_y + ui.status_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
    let text_color = ui.config.theme.statusbar_text;
    
    let mut pen_x = 10.0;
    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &status_left,
        pen_x,
        baseline_y,
        text_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    if ui.config.show_git_branch {
        if let Some(ref branch) = ui.git_branch {
            pen_x += ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                " | ",
                pen_x,
                baseline_y,
                text_color,
                ui.ui_font_size,
                ui.ui_char_width,
            );
            
            // Draw branch icon
            let icon_sz = (ui.ui_font_size * 0.9).round().max(12.0);
            let icon_y_center = baseline_y - (ui.ui_font_ascent * 0.33).round();
            let icon_y = icon_y_center - (icon_sz / 2.0).round();
            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                "branch",
                pen_x,
                icon_y,
                text_color,
                icon_sz,
            );
            pen_x += icon_sz + 4.0; // Space after icon
            
            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                branch,
                pen_x,
                baseline_y,
                text_color,
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }
    }

    let right_text_width = status_right.chars().count() as f32 * ui.ui_char_width;
    let right_x = width - right_text_width - 15.0 - 36.0;
    if right_x > width / 2.0 {
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &status_right,
            right_x,
            (status_y + ui.status_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            ui.config.theme.statusbar_text,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }

    // Draw statusbar action buttons in the bottom right corner
    let sb_btn_w = 26.0f32;
    let sb_btn_h = ui.status_height - 1.0;
    let icon_sz = 14.0f32;
    let icon_y = status_y + (sb_btn_h - icon_sz) / 2.0;

    let term_btn_x = width - 10.0 - sb_btn_w;

    // Check hovers
    let is_term_hover = ui.active_modal.is_none() && mouse_y >= status_y && mouse_x >= term_btn_x && mouse_x < term_btn_x + sb_btn_w;

    // Terminal Button
    let term_bg = if ui.show_dock {
        [0.2, 0.5, 0.8, 0.35] // blue tint when open
    } else if is_term_hover {
        ui.config.theme.titlebar_hover_bg
    } else {
        ui.config.theme.statusbar_bg
    };
    ui.push_quad(vertices, indices, term_btn_x, status_y + 1.0, sb_btn_w, sb_btn_h, white_uv, term_bg);
    let term_color = if ui.show_dock { [0.3, 0.6, 0.95, 1.0] } else { ui.config.theme.statusbar_text };
    ui.push_icon(vertices, indices, atlas, queue, "terminal", term_btn_x + (sb_btn_w - icon_sz) / 2.0, icon_y, term_color, icon_sz);
}
