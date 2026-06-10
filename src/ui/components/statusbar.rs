use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;
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
    active_path: Option<&str>,
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

    let baseline_y = (status_y + ui.status_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
    let text_color = ui.config.theme.statusbar_text;
    
    let mut pen_x = 10.0;

    // 1. Draw Git Branch Info
    if ui.config.show_git_branch {
        if let Some(ref branch) = ui.git_branch {
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
            pen_x += icon_sz + 4.0;
            
            pen_x += ui.push_str(
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
            
            pen_x += 15.0; // spacing after branch name
        }
    }

    // 2. Draw Diagnostics Indicators (mocked for clean UI look)
    let err_color = [0.90, 0.30, 0.30, 1.0];
    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "⊗ 0  ",
        pen_x,
        baseline_y,
        err_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    let warn_color = [0.90, 0.70, 0.20, 1.0];
    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "⚠ 0",
        pen_x,
        baseline_y,
        warn_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // 3. Right Side Components (drawn from right to left)
    let sb_btn_w = 26.0f32;
    let sb_btn_h = ui.status_height - 1.0;
    let icon_sz = 14.0f32;
    let icon_y = status_y + (sb_btn_h - icon_sz) / 2.0;
    let term_btn_x = width - 10.0 - sb_btn_w;

    // Detect file type / extension to show programming language
    let extension = active_path
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let language = match extension {
        "rs" => "Rust",
        "json" => "JSON",
        "toml" => "TOML",
        "md" => "Markdown",
        "js" => "JavaScript",
        "ts" => "TypeScript",
        "html" => "HTML",
        "css" => "CSS",
        "wgsl" => "WGSL",
        "sh" => "Shell",
        _ => "Plain Text",
    };

    let cursor_str = format!("Ln {}, Col {}", cursor.line + 1, cursor.col + 1);
    let lsp_str = if language == "Rust" { "LSP: rust-analyzer" } else { "LSP: ready" };

    let right_components = [
        cursor_str.as_str(),
        language,
        lsp_str,
        "UTF-8",
        "LF",
    ];

    let mut cur_right_x = term_btn_x - 10.0;
    for comp in &right_components {
        let comp_len = comp.chars().count() as f32;
        let comp_w = comp_len * ui.ui_char_width;
        cur_right_x -= comp_w + 16.0;

        if cur_right_x > pen_x {
            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                comp,
                cur_right_x + 8.0,
                baseline_y,
                text_color,
                ui.ui_font_size,
                ui.ui_char_width,
            );

            // Draw a vertical separator line on the left side of the component
            ui.push_quad(
                vertices,
                indices,
                cur_right_x,
                status_y + 6.0,
                1.0,
                ui.status_height - 12.0,
                white_uv,
                ui.config.theme.statusbar_border,
            );
        }
    }

    // 4. Draw Terminal Toggle Button
    let is_term_hover = ui.active_modal.is_none() && mouse_y >= status_y && mouse_x >= term_btn_x && mouse_x < term_btn_x + sb_btn_w;
    let term_bg = if is_term_hover {
        ui.config.theme.titlebar_hover_bg
    } else {
        ui.config.theme.statusbar_bg
    };
    ui.push_quad(vertices, indices, term_btn_x, status_y + 1.0, sb_btn_w, sb_btn_h, white_uv, term_bg);
    let term_color = if ui.show_dock { [0.38, 0.69, 0.94, 1.0] } else { ui.config.theme.statusbar_text };
    ui.push_icon(vertices, indices, atlas, queue, "terminal", term_btn_x + (sb_btn_w - icon_sz) / 2.0, icon_y, term_color, icon_sz);
}
