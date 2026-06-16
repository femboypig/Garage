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
        let label_w = 65.0f32;
        let count_w = 70.0f32;
        let btn_prev_w = 22.0f32;
        let btn_next_w = 22.0f32;
        let btn_replace_w = 70.0f32;
        let btn_replace_all_w = 45.0f32;
        let close_btn_w = 22.0f32;

        let row_h = bar_h / 2.0;
        let input_h = row_h - 6.0;
        let input_y_1 = bar_y + 3.0;
        let input_y_2 = bar_y + row_h + 3.0;

        let l_baseline_1 = (bar_y + row_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
        let l_baseline_2 = (bar_y + row_h + row_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

        let close_x = bar_x + bar_w - 25.0;
        let next_x = close_x - 10.0 - btn_next_w;
        let prev_x = next_x - 4.0 - btn_prev_w;
        let count_x = prev_x - 10.0 - count_w;
        let input_start_x = bar_x + 10.0 + label_w;
        let input_find_w = (count_x - 10.0 - input_start_x).max(50.0);

        // --- ROW 1: FIND ---
        // 1. "Find:" label
        ui.push_str(vertices, indices, atlas, queue, "Find:", bar_x + 10.0, l_baseline_1, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);

        // 2. Find Input text box
        let is_find_focused = !ui.search_focus_replace;
        ui.push_quad(vertices, indices, input_start_x, input_y_1, input_find_w, input_h, white_uv, ui.config.theme.editor_bg);
        let border_color_1 = if is_find_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
        ui.push_quad(vertices, indices, input_start_x, input_y_1, input_find_w, 1.0, white_uv, border_color_1);
        ui.push_quad(vertices, indices, input_start_x, input_y_1 + input_h - 1.0, input_find_w, 1.0, white_uv, border_color_1);
        ui.push_quad(vertices, indices, input_start_x, input_y_1, 1.0, input_h, white_uv, border_color_1);
        ui.push_quad(vertices, indices, input_start_x + input_find_w - 1.0, input_y_1, 1.0, input_h, white_uv, border_color_1);

        // Option buttons inside Find Input
        let opt_btn_w = 20.0f32;
        let opt_y = input_y_1 + 2.0;
        let opt_h = input_h - 4.0;
        let opt_regex_x = input_start_x + input_find_w - 5.0 - opt_btn_w;
        let opt_word_x = opt_regex_x - 2.0 - opt_btn_w;
        let opt_case_x = opt_word_x - 2.0 - opt_btn_w;

        // Aa option
        let case_hover = mouse_x >= opt_case_x && mouse_x < opt_case_x + opt_btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
        let case_bg = if ui.search_case_sensitive { ui.config.theme.selection_bg } else if case_hover { ui.config.theme.button_hover_bg } else { [0.0, 0.0, 0.0, 0.0] };
        ui.push_quad(vertices, indices, opt_case_x, opt_y, opt_btn_w, opt_h, white_uv, case_bg);
        ui.push_str(vertices, indices, atlas, queue, "Aa", opt_case_x + 3.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);

        // W option
        let word_hover = mouse_x >= opt_word_x && mouse_x < opt_word_x + opt_btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
        let word_bg = if ui.search_whole_word { ui.config.theme.selection_bg } else if word_hover { ui.config.theme.button_hover_bg } else { [0.0, 0.0, 0.0, 0.0] };
        ui.push_quad(vertices, indices, opt_word_x, opt_y, opt_btn_w, opt_h, white_uv, word_bg);
        ui.push_str(vertices, indices, atlas, queue, "W", opt_word_x + 5.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);

        // .* option
        let regex_hover = mouse_x >= opt_regex_x && mouse_x < opt_regex_x + opt_btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
        let regex_bg = if ui.search_regex { ui.config.theme.selection_bg } else if regex_hover { ui.config.theme.button_hover_bg } else { [0.0, 0.0, 0.0, 0.0] };
        ui.push_quad(vertices, indices, opt_regex_x, opt_y, opt_btn_w, opt_h, white_uv, regex_bg);
        ui.push_str(vertices, indices, atlas, queue, ".*", opt_regex_x + 4.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);

        // Draw text inside find input
        let options_w = 3.0 * opt_btn_w + 10.0;
        let max_chars = ((input_find_w - 10.0 - options_w) / ui.ui_char_width).floor().max(1.0) as usize;
        
        if ui.search_query.is_empty() {
            ui.push_str(vertices, indices, atlas, queue, "Search query...", input_start_x + 5.0, l_baseline_1, ui.config.theme.syntax_comment, ui.ui_font_size, ui.ui_char_width);
        } else {
            let display_query = if ui.search_query.chars().count() > max_chars {
                if is_find_focused {
                    ui.search_query.chars().skip(ui.search_query.chars().count() - max_chars).collect::<String>()
                } else {
                    ui.search_query.chars().take(max_chars).collect::<String>()
                }
            } else {
                ui.search_query.clone()
            };
            ui.push_str(vertices, indices, atlas, queue, &display_query, input_start_x + 5.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
            if is_find_focused {
                let cursor_x = input_start_x + 5.0 + display_query.chars().count() as f32 * ui.ui_char_width;
                if cursor_x < input_start_x + input_find_w - options_w {
                    ui.push_quad(vertices, indices, cursor_x, input_y_1 + 3.0, 1.5, input_h - 6.0, white_uv, ui.config.theme.cursor_color);
                }
            }
        }

        // 3. Match counts ("1 of 2")
        let count_str = if ui.search_matches.is_empty() {
            "0 of 0".to_string()
        } else {
            format!("{} of {}", ui.active_search_match_idx + 1, ui.search_matches.len())
        };
        let count_text_len = count_str.chars().count() as f32;
        let count_text_x = count_x + ((count_w - count_text_len * ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, &count_str, count_text_x, l_baseline_1, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);

        // 4. Prev Button (◀)
        let prev_hover = mouse_x >= prev_x && mouse_x < prev_x + btn_prev_w && mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h;
        ui.push_quad(vertices, indices, prev_x, input_y_1, btn_prev_w, input_h, white_uv, if prev_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, prev_x, input_y_1, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y_1 + input_h - 1.0, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x + btn_prev_w - 1.0, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        let prev_text_x = prev_x + ((btn_prev_w - ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, "◀", prev_text_x, l_baseline_1, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);

        // 5. Next Button (▶)
        let next_hover = mouse_x >= next_x && mouse_x < next_x + btn_next_w && mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h;
        ui.push_quad(vertices, indices, next_x, input_y_1, btn_next_w, input_h, white_uv, if next_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, next_x, input_y_1, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y_1 + input_h - 1.0, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x + btn_next_w - 1.0, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        let next_text_x = next_x + ((btn_next_w - ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, "▶", next_text_x, l_baseline_1, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);

        // 6. Close Button (✕)
        let close_hover = mouse_x >= close_x && mouse_x < close_x + close_btn_w && mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h;
        if close_hover {
            ui.push_quad(vertices, indices, close_x, input_y_1, close_btn_w, input_h, white_uv, ui.config.theme.button_hover_bg);
        }
        let close_text_x = close_x + ((close_btn_w - ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, "✕", close_text_x, l_baseline_1, if close_hover { ui.config.theme.modal_text_title } else { ui.config.theme.modal_text_muted }, ui.ui_font_size, ui.ui_char_width);

        // --- ROW 2: REPLACE ---
        // 7. "Replace:" label
        ui.push_str(vertices, indices, atlas, queue, "Replace:", bar_x + 10.0, l_baseline_2, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);

        // 8. Replace Input text box
        let is_replace_focused = ui.search_focus_replace;
        ui.push_quad(vertices, indices, input_start_x, input_y_2, input_find_w, input_h, white_uv, ui.config.theme.editor_bg);
        let border_color_2 = if is_replace_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
        ui.push_quad(vertices, indices, input_start_x, input_y_2, input_find_w, 1.0, white_uv, border_color_2);
        ui.push_quad(vertices, indices, input_start_x, input_y_2 + input_h - 1.0, input_find_w, 1.0, white_uv, border_color_2);
        ui.push_quad(vertices, indices, input_start_x, input_y_2, 1.0, input_h, white_uv, border_color_2);
        ui.push_quad(vertices, indices, input_start_x + input_find_w - 1.0, input_y_2, 1.0, input_h, white_uv, border_color_2);

        let max_replace_chars = ((input_find_w - 10.0) / ui.ui_char_width).floor().max(1.0) as usize;
        if ui.replace_query.is_empty() {
            ui.push_str(vertices, indices, atlas, queue, "Replace with...", input_start_x + 5.0, l_baseline_2, ui.config.theme.syntax_comment, ui.ui_font_size, ui.ui_char_width);
        } else {
            let display_replace = if ui.replace_query.chars().count() > max_replace_chars {
                if is_replace_focused {
                    ui.replace_query.chars().skip(ui.replace_query.chars().count() - max_replace_chars).collect::<String>()
                } else {
                    ui.replace_query.chars().take(max_replace_chars).collect::<String>()
                }
            } else {
                ui.replace_query.clone()
            };
            ui.push_str(vertices, indices, atlas, queue, &display_replace, input_start_x + 5.0, l_baseline_2, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
            if is_replace_focused {
                let cursor_x = input_start_x + 5.0 + display_replace.chars().count() as f32 * ui.ui_char_width;
                if cursor_x < input_start_x + input_find_w - 5.0 {
                    ui.push_quad(vertices, indices, cursor_x, input_y_2 + 3.0, 1.5, input_h - 6.0, white_uv, ui.config.theme.cursor_color);
                }
            }
        }

        // 9. Replace Button
        let rep_hover = mouse_x >= count_x && mouse_x < count_x + btn_replace_w && mouse_y >= input_y_2 && mouse_y < input_y_2 + input_h;
        ui.push_quad(vertices, indices, count_x, input_y_2, btn_replace_w, input_h, white_uv, if rep_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, count_x, input_y_2, btn_replace_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, count_x, input_y_2 + input_h - 1.0, btn_replace_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, count_x, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, count_x + btn_replace_w - 1.0, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        let replace_btn_text = "Replace";
        let rep_text_len = replace_btn_text.chars().count() as f32;
        let rep_text_x = count_x + ((btn_replace_w - rep_text_len * ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, replace_btn_text, rep_text_x, l_baseline_2, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);

        // 10. Replace All Button
        let rep_all_x = count_x + btn_replace_w + 5.0;
        let rep_all_hover = mouse_x >= rep_all_x && mouse_x < rep_all_x + btn_replace_all_w && mouse_y >= input_y_2 && mouse_y < input_y_2 + input_h;
        ui.push_quad(vertices, indices, rep_all_x, input_y_2, btn_replace_all_w, input_h, white_uv, if rep_all_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, rep_all_x, input_y_2, btn_replace_all_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, rep_all_x, input_y_2 + input_h - 1.0, btn_replace_all_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, rep_all_x, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, rep_all_x + btn_replace_all_w - 1.0, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        let replace_all_text = "All";
        let rep_all_text_len = replace_all_text.chars().count() as f32;
        let rep_all_text_x = rep_all_x + ((btn_replace_all_w - rep_all_text_len * ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, replace_all_text, rep_all_text_x, l_baseline_2, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);
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
