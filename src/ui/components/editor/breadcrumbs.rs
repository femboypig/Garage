use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

pub fn draw_breadcrumbs(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    cursor: &Cursor,
    width: f32,
    main_y: f32,
    activity_bar_width: f32,
    active_file_path: Option<&str>,
    is_active_pane: bool,
    mouse_x: f32,
    mouse_y: f32,
) {
    let white_uv = atlas.white_pixel_uv();
    let bar_x = activity_bar_width + ui.sidebar_width;
    let bar_w = width - bar_x;
    let bar_y = main_y + ui.tabbar_height;
    let bar_h = ui.breadcrumb_height;
    
    // Breadcrumb Bar background
    ui.push_quad(
        vertices,
        indices,
        bar_x,
        bar_y,
        bar_w,
        bar_h,
        white_uv,
        ui.config.theme.breadcrumb_bg,
    );
    // Breadcrumb bottom border
    ui.push_quad(
        vertices,
        indices,
        bar_x,
        bar_y + bar_h - 1.0,
        bar_w,
        1.0,
        white_uv,
        ui.config.theme.breadcrumb_border,
    );
    
    // If search panel is active on this pane and it's not the project search tab, draw search panel instead of breadcrumbs
    if is_active_pane && ui.show_search_panel && active_file_path != Some("search://project") {
        let label_find_w = 40.0f32;
        let input_find_w = 120.0f32;
        let count_w = 60.0f32;
        let btn_prev_w = 20.0f32;
        let btn_next_w = 20.0f32;
        let label_replace_w = 60.0f32;
        let input_replace_w = 120.0f32;
        let btn_replace_w = 60.0f32;
        let close_btn_w = 20.0f32;

        let mut cur_x = bar_x + 10.0;
        let input_h = bar_h - 6.0;
        let input_y = bar_y + 3.0;
        let l_baseline = (bar_y + bar_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

        // 1. "Find:" label
        ui.push_str(vertices, indices, atlas, queue, "Find:", cur_x, l_baseline, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);
        cur_x += label_find_w;

        // 2. Find Input text box
        let find_in_x = cur_x;
        let is_find_focused = !ui.search_focus_replace;
        ui.push_quad(vertices, indices, find_in_x, input_y, input_find_w, input_h, white_uv, ui.config.theme.editor_bg);
        let border_color = if is_find_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
        ui.push_quad(vertices, indices, find_in_x, input_y, input_find_w, 1.0, white_uv, border_color);
        ui.push_quad(vertices, indices, find_in_x, input_y + input_h - 1.0, input_find_w, 1.0, white_uv, border_color);
        ui.push_quad(vertices, indices, find_in_x, input_y, 1.0, input_h, white_uv, border_color);
        ui.push_quad(vertices, indices, find_in_x + input_find_w - 1.0, input_y, 1.0, input_h, white_uv, border_color);

        // Draw input content
        ui.push_str(vertices, indices, atlas, queue, &ui.search_query, find_in_x + 5.0, l_baseline, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
        if is_find_focused {
            let cursor_x = find_in_x + 5.0 + ui.search_query.chars().count() as f32 * ui.ui_char_width;
            if cursor_x < find_in_x + input_find_w - 5.0 {
                ui.push_quad(vertices, indices, cursor_x, input_y + 3.0, 1.5, input_h - 6.0, white_uv, ui.config.theme.cursor_color);
            }
        }
        cur_x += input_find_w + 10.0;

        // 3. Match counts ("1 of 2")
        let count_str = if ui.search_matches.is_empty() {
            "0 of 0".to_string()
        } else {
            format!("{} of {}", ui.active_search_match_idx + 1, ui.search_matches.len())
        };
        ui.push_str(vertices, indices, atlas, queue, &count_str, cur_x, l_baseline, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);
        cur_x += count_w;

        // 4. Prev Button (◀)
        let prev_x = cur_x;
        let prev_hover = mouse_x >= prev_x && mouse_x < prev_x + btn_prev_w && mouse_y >= input_y && mouse_y < input_y + input_h;
        ui.push_quad(vertices, indices, prev_x, input_y, btn_prev_w, input_h, white_uv, if prev_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, prev_x, input_y, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y + input_h - 1.0, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x + btn_prev_w - 1.0, input_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_str(vertices, indices, atlas, queue, "◀", prev_x + 4.0, l_baseline, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);
        cur_x += btn_prev_w + 4.0;

        // 5. Next Button (▶)
        let next_x = cur_x;
        let next_hover = mouse_x >= next_x && mouse_x < next_x + btn_next_w && mouse_y >= input_y && mouse_y < input_y + input_h;
        ui.push_quad(vertices, indices, next_x, input_y, btn_next_w, input_h, white_uv, if next_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, next_x, input_y, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y + input_h - 1.0, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x + btn_next_w - 1.0, input_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_str(vertices, indices, atlas, queue, "▶", next_x + 4.0, l_baseline, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);
        cur_x += btn_next_w + 15.0;

        // 6. "Replace:" label
        ui.push_str(vertices, indices, atlas, queue, "Replace:", cur_x, l_baseline, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);
        cur_x += label_replace_w;

        // 7. Replace Input text box
        let rep_in_x = cur_x;
        let is_replace_focused = ui.search_focus_replace;
        ui.push_quad(vertices, indices, rep_in_x, input_y, input_replace_w, input_h, white_uv, ui.config.theme.editor_bg);
        let border_color2 = if is_replace_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
        ui.push_quad(vertices, indices, rep_in_x, input_y, input_replace_w, 1.0, white_uv, border_color2);
        ui.push_quad(vertices, indices, rep_in_x, input_y + input_h - 1.0, input_replace_w, 1.0, white_uv, border_color2);
        ui.push_quad(vertices, indices, rep_in_x, input_y, 1.0, input_h, white_uv, border_color2);
        ui.push_quad(vertices, indices, rep_in_x + input_replace_w - 1.0, input_y, 1.0, input_h, white_uv, border_color2);

        ui.push_str(vertices, indices, atlas, queue, &ui.replace_query, rep_in_x + 5.0, l_baseline, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
        if is_replace_focused {
            let cursor_x = rep_in_x + 5.0 + ui.replace_query.chars().count() as f32 * ui.ui_char_width;
            if cursor_x < rep_in_x + input_replace_w - 5.0 {
                ui.push_quad(vertices, indices, cursor_x, input_y + 3.0, 1.5, input_h - 6.0, white_uv, ui.config.theme.cursor_color);
            }
        }
        cur_x += input_replace_w + 10.0;

        // 8. Replace Button
        let rep_x = cur_x;
        let rep_hover = mouse_x >= rep_x && mouse_x < rep_x + btn_replace_w && mouse_y >= input_y && mouse_y < input_y + input_h;
        ui.push_quad(vertices, indices, rep_x, input_y, btn_replace_w, input_h, white_uv, if rep_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, rep_x, input_y, btn_replace_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, rep_x, input_y + input_h - 1.0, btn_replace_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, rep_x, input_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, rep_x + btn_replace_w - 1.0, input_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_str(vertices, indices, atlas, queue, "Replace", rep_x + 6.0, l_baseline, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);

        // 9. Close Button (✕) on the right of the breadcrumb bar
        let close_x = bar_x + bar_w - 25.0;
        let close_hover = mouse_x >= close_x && mouse_x < close_x + close_btn_w && mouse_y >= input_y && mouse_y < input_y + input_h;
        ui.push_str(vertices, indices, atlas, queue, "✕", close_x + 4.0, l_baseline - 2.0, if close_hover { ui.config.theme.modal_text_title } else { ui.config.theme.modal_text_muted }, ui.ui_font_size, ui.ui_char_width);
    } else {
        // Construct breadcrumb text: relative_path > current_function
        let relative_path = active_file_path
            .unwrap_or("Untitled");
        
        let current_fn = ui.find_current_function(buffer, cursor.line);
        let breadcrumb_text = if let Some(ref func) = current_fn {
            format!("{} > {}", relative_path, func)
        } else {
            relative_path.to_string()
        };
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &breadcrumb_text,
            bar_x + 15.0,
            (bar_y + bar_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            ui.config.theme.breadcrumb_text,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }
}
