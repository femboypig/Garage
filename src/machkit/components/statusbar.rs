use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::UiState;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;

pub fn draw_statusbar(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    height: f32,
    _buffer: &Buffer,
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
            let icon_sz = (ui.ui_font_size * 1.15).round().max(15.0);
            let icon_y = (status_y + (ui.status_height - icon_sz) / 2.0).round();
            ui.push_icon(
                vertices, indices, atlas, queue, "branch", pen_x, icon_y, text_color, icon_sz,
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

    // 2. Draw Diagnostics Indicators
    let mut err_count = 0;
    let mut warn_count = 0;
    for (e, w) in ui.lsp_diagnostics.values() {
        err_count += *e;
        warn_count += *w;
    }

    let err_val_str = format!("{}", err_count);
    let warn_val_str = format!("{}", warn_count);

    let err_icon_sz = 14.0f32;
    let warn_icon_sz = 14.0f32;
    let err_text_w = err_val_str.chars().count() as f32 * ui.ui_char_width;
    let warn_text_w = warn_val_str.chars().count() as f32 * ui.ui_char_width;
    let diag_w = err_icon_sz + 4.0 + err_text_w + 12.0 + warn_icon_sz + 4.0 + warn_text_w;

    let is_diag_hovered = ui.active_modal.is_none()
        && mouse_y >= status_y
        && mouse_x >= pen_x
        && mouse_x < pen_x + diag_w;

    if is_diag_hovered {
        ui.push_quad(
            vertices,
            indices,
            pen_x - 4.0,
            status_y + 1.0,
            diag_w + 8.0,
            ui.status_height - 1.0,
            white_uv,
            ui.config.theme.titlebar_hover_bg,
        );
    }

    let err_color = if err_count > 0 {
        [0.95, 0.25, 0.25, 1.0]
    } else {
        ui.config.theme.statusbar_text
    };
    let err_icon_y = (status_y + (ui.status_height - err_icon_sz) / 2.0).round();
    ui.push_icon(
        vertices,
        indices,
        atlas,
        queue,
        "error",
        pen_x,
        err_icon_y,
        err_color,
        err_icon_sz,
    );
    pen_x += err_icon_sz + 4.0;

    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &err_val_str,
        pen_x,
        baseline_y,
        err_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );
    pen_x += 12.0;

    let warn_color = if warn_count > 0 {
        [0.95, 0.70, 0.15, 1.0]
    } else {
        ui.config.theme.statusbar_text
    };
    let warn_icon_y = (status_y + (ui.status_height - warn_icon_sz) / 2.0).round();
    ui.push_icon(
        vertices,
        indices,
        atlas,
        queue,
        "warning",
        pen_x,
        warn_icon_y,
        warn_color,
        warn_icon_sz,
    );
    pen_x += warn_icon_sz + 4.0;

    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &warn_val_str,
        pen_x,
        baseline_y,
        warn_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    if !ui.external_change_warnings.is_empty() {
        pen_x += 15.0; // spacer
        let warn_icon_sz = 14.0f32;
        let warn_icon_y = (status_y + (ui.status_height - warn_icon_sz) / 2.0).round();
        let warning_color = [0.95, 0.70, 0.15, 1.0];
        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "warning",
            pen_x,
            warn_icon_y,
            warning_color,
            warn_icon_sz,
        );
        pen_x += warn_icon_sz + 4.0;
        pen_x += ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            "File changed on disk",
            pen_x,
            baseline_y,
            warning_color,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }

    // 3. Right Side Components (drawn from right to left)
    let sb_btn_w = 28.0f32;
    let sb_btn_h = ui.status_height - 1.0;
    let icon_sz = 16.0f32;
    let icon_y = (status_y + (ui.status_height - icon_sz) / 2.0).round();
    let term_btn_x = width - 10.0 - sb_btn_w;

    // Detect file type / extension to show programming language
    let mut extension = active_path
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string();

    if let Some(path) = active_path {
        if let Some(forced_ext) = ui.forced_languages.get(path) {
            extension = forced_ext.clone();
        }
    }

    let language = ui
        .languages
        .get(&extension)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if extension.is_empty() {
                "Plain Text".to_string()
            } else {
                let mut chars = extension.chars();
                match chars.next() {
                    None => "Plain Text".to_string(),
                    Some(first) => {
                        let mut s = first.to_uppercase().to_string();
                        s.push_str(&chars.as_str().to_lowercase());
                        s
                    }
                }
            }
        });

    let encoding = active_path
        .and_then(|path| ui.forced_encodings.get(path))
        .map(|s| s.as_str())
        .unwrap_or("UTF-8");

    let line_ending = active_path
        .and_then(|path| ui.forced_line_endings.get(path))
        .map(|s| s.as_str())
        .unwrap_or("LF");

    let cursor_str = format!("Ln {}, Col {}", cursor.line + 1, cursor.col + 1);

    let right_components = [
        cursor_str.as_str(),
        language.as_str(),
        encoding,
        line_ending,
    ];

    let mut cur_right_x = term_btn_x - 10.0;
    for (i, comp) in right_components.iter().enumerate() {
        let comp_len = comp.chars().count() as f32;
        let comp_w = comp_len * ui.ui_char_width;
        let item_left = cur_right_x - comp_w - 16.0;
        let item_right = cur_right_x;
        cur_right_x -= comp_w + 16.0;

        if cur_right_x > pen_x {
            // Index 1 is language, Index 2 is encoding, Index 3 is line ending
            let is_hoverable = i == 1 || i == 2 || i == 3;
            let is_hovered = is_hoverable
                && ui.active_modal.is_none()
                && mouse_y >= status_y
                && mouse_x >= item_left
                && mouse_x < item_right;

            if is_hovered {
                ui.push_quad(
                    vertices,
                    indices,
                    item_left + 1.0,
                    status_y + 1.0,
                    item_right - item_left - 1.0,
                    ui.status_height - 1.0,
                    white_uv,
                    ui.config.theme.titlebar_hover_bg,
                );
            }

            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                comp,
                item_left + 8.0,
                baseline_y,
                text_color,
                ui.ui_font_size,
                ui.ui_char_width,
            );

            // Draw a vertical separator line on the left side of the component
            ui.push_quad(
                vertices,
                indices,
                item_left,
                status_y + 6.0,
                1.0,
                ui.status_height - 12.0,
                white_uv,
                ui.config.theme.statusbar_border,
            );
        }
    }

    // 4. Draw Terminal Toggle Button
    let is_term_hover = ui.active_modal.is_none()
        && mouse_y >= status_y
        && mouse_x >= term_btn_x
        && mouse_x < term_btn_x + sb_btn_w;
    let term_bg = if is_term_hover {
        ui.config.theme.titlebar_hover_bg
    } else {
        ui.config.theme.statusbar_bg
    };
    ui.push_quad(
        vertices,
        indices,
        term_btn_x,
        status_y + 1.0,
        sb_btn_w,
        sb_btn_h,
        white_uv,
        term_bg,
    );
    let term_color = if ui.show_dock {
        [0.38, 0.69, 0.94, 1.0]
    } else {
        ui.config.theme.statusbar_text
    };
    ui.push_icon(
        vertices,
        indices,
        atlas,
        queue,
        "terminal",
        term_btn_x + (sb_btn_w - icon_sz) / 2.0,
        icon_y,
        term_color,
        icon_sz,
    );
}
