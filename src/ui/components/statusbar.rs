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
            let icon_sz = (ui.ui_font_size * 0.9).round().max(12.0);
            let icon_y = (status_y + (ui.status_height - icon_sz) / 2.0).round();
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

    // 2. Draw Diagnostics Indicators
    let mut err_count = 0;
    let mut warn_count = 0;
    if let Some(path) = active_path {
        let abs_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else if let Ok(current_dir) = std::env::current_dir() {
            current_dir.join(path)
        } else {
            std::path::PathBuf::from(path)
        };
        
        let abs_path_str = abs_path.to_string_lossy().to_string();
        let abs_path_str_canon = std::fs::canonicalize(&abs_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| abs_path_str.clone());

        if let Some((e, w)) = ui.lsp_diagnostics.get(&abs_path_str)
            .or_else(|| ui.lsp_diagnostics.get(&abs_path_str_canon))
        {
            err_count = *e;
            warn_count = *w;
        } else {
            let active_suffix = format!("/{}", path.replace("./", ""));
            for (key, val) in &ui.lsp_diagnostics {
                if key.ends_with(&active_suffix) {
                    err_count = val.0;
                    warn_count = val.1;
                    break;
                }
            }
        }
    }

    let err_str = format!("⊗ {}  ", err_count);
    let err_color = if err_count > 0 { [0.95, 0.25, 0.25, 1.0] } else { ui.config.theme.statusbar_text };
    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &err_str,
        pen_x,
        baseline_y,
        err_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    let warn_str = format!("⚠ {}", warn_count);
    let warn_color = if warn_count > 0 { [0.95, 0.70, 0.15, 1.0] } else { ui.config.theme.statusbar_text };
    pen_x += ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &warn_str,
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
    let icon_y = (status_y + (ui.status_height - icon_sz) / 2.0).round();
    let term_btn_x = width - 10.0 - sb_btn_w;

    // Detect file type / extension to show programming language
    let extension = active_path
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let language = ui.languages.get(extension)
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

    let cursor_str = format!("Ln {}, Col {}", cursor.line + 1, cursor.col + 1);
    let lsp_str = format!("LSP: {}", ui.lsp_status);

    let right_components = [
        cursor_str.as_str(),
        language.as_str(),
        lsp_str.as_str(),
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
