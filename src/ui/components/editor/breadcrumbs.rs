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
    
    let is_project_search = active_file_path == Some("search://project");
    if is_active_pane && (ui.show_search_panel || is_project_search) {
        let is_local = !is_project_search;
        let search_query = if is_local { &ui.search_query } else { &ui.global_search_query };
        let replace_query = if is_local { &ui.replace_query } else { &ui.global_replace_query };
        let case_sensitive = if is_local { ui.search_case_sensitive } else { ui.global_search_case_sensitive };
        let whole_word = if is_local { ui.search_whole_word } else { ui.global_search_whole_word };
        let regex = if is_local { ui.search_regex } else { ui.global_search_regex };
        let is_replace_focused = if is_local { ui.search_focus_replace } else { ui.global_search_focus_replace };
        let is_find_focused = !is_replace_focused;

        let count_w = 70.0f32;
        let btn_prev_w = 22.0f32;
        let btn_next_w = 22.0f32;
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
        let input_start_x = bar_x + 10.0;
        let input_find_w = (count_x - 10.0 - input_start_x).max(50.0);

        // --- ROW 1: FIND ---
        // 1. Find Input text box
        ui.push_quad(vertices, indices, input_start_x, input_y_1, input_find_w, input_h, white_uv, ui.config.theme.editor_bg);
        let border_color_1 = if is_find_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
        ui.push_quad(vertices, indices, input_start_x, input_y_1, input_find_w, 1.0, white_uv, border_color_1);
        ui.push_quad(vertices, indices, input_start_x, input_y_1 + input_h - 1.0, input_find_w, 1.0, white_uv, border_color_1);
        ui.push_quad(vertices, indices, input_start_x, input_y_1, 1.0, input_h, white_uv, border_color_1);
        ui.push_quad(vertices, indices, input_start_x + input_find_w - 1.0, input_y_1, 1.0, input_h, white_uv, border_color_1);

        // Magnifying glass icon inside input box
        let search_icon_sz = 12.0f32;
        let search_icon_y = (input_y_1 + (input_h - search_icon_sz) / 2.0).round();
        ui.push_icon(vertices, indices, atlas, queue, "search", input_start_x + 6.0, search_icon_y, ui.config.theme.modal_text_muted, search_icon_sz);

        // Option buttons inside Find Input
        let opt_btn_w = 20.0f32;
        let opt_y = input_y_1 + 2.0;
        let opt_h = input_h - 4.0;
        let opt_regex_x = input_start_x + input_find_w - 5.0 - opt_btn_w;
        let opt_word_x = opt_regex_x - 2.0 - opt_btn_w;
        let opt_case_x = opt_word_x - 2.0 - opt_btn_w;

        // Aa option
        let case_hover = mouse_x >= opt_case_x && mouse_x < opt_case_x + opt_btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
        let case_bg = if case_sensitive { ui.config.theme.selection_bg } else if case_hover { ui.config.theme.button_hover_bg } else { [0.0, 0.0, 0.0, 0.0] };
        ui.push_quad(vertices, indices, opt_case_x, opt_y, opt_btn_w, opt_h, white_uv, case_bg);
        let opt_icon_y = (opt_y + (opt_h - 12.0) / 2.0).round();
        ui.push_icon(vertices, indices, atlas, queue, "case_sensitive", opt_case_x + (opt_btn_w - 12.0) / 2.0, opt_icon_y, ui.config.theme.modal_text_normal, 12.0);

        // W option
        let word_hover = mouse_x >= opt_word_x && mouse_x < opt_word_x + opt_btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
        let word_bg = if whole_word { ui.config.theme.selection_bg } else if word_hover { ui.config.theme.button_hover_bg } else { [0.0, 0.0, 0.0, 0.0] };
        ui.push_quad(vertices, indices, opt_word_x, opt_y, opt_btn_w, opt_h, white_uv, word_bg);
        ui.push_icon(vertices, indices, atlas, queue, "whole_word", opt_word_x + (opt_btn_w - 12.0) / 2.0, opt_icon_y, ui.config.theme.modal_text_normal, 12.0);

        // .* option
        let regex_hover = mouse_x >= opt_regex_x && mouse_x < opt_regex_x + opt_btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
        let regex_bg = if regex { ui.config.theme.selection_bg } else if regex_hover { ui.config.theme.button_hover_bg } else { [0.0, 0.0, 0.0, 0.0] };
        ui.push_quad(vertices, indices, opt_regex_x, opt_y, opt_btn_w, opt_h, white_uv, regex_bg);
        ui.push_icon(vertices, indices, atlas, queue, "regex", opt_regex_x + (opt_btn_w - 12.0) / 2.0, opt_icon_y, ui.config.theme.modal_text_normal, 12.0);

        // Draw text inside find input
        let options_w = 3.0 * opt_btn_w + 10.0;
        let text_start_x = input_start_x + 22.0;
        let max_chars = ((input_find_w - 24.0 - options_w) / ui.ui_char_width).floor().max(1.0) as usize;
        let placeholder = if is_local { "Search query..." } else { "Search in project..." };
        
        if search_query.is_empty() {
            ui.push_str(vertices, indices, atlas, queue, placeholder, text_start_x, l_baseline_1, ui.config.theme.syntax_comment, ui.ui_font_size, ui.ui_char_width);
        } else {
            let display_query = if search_query.chars().count() > max_chars {
                if is_find_focused {
                    search_query.chars().skip(search_query.chars().count() - max_chars).collect::<String>()
                } else {
                    search_query.chars().take(max_chars).collect::<String>()
                }
            } else {
                search_query.clone()
            };
            ui.push_str(vertices, indices, atlas, queue, &display_query, text_start_x, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
            if is_find_focused {
                let cursor_x = text_start_x + display_query.chars().count() as f32 * ui.ui_char_width;
                if cursor_x < input_start_x + input_find_w - options_w {
                    ui.push_quad(vertices, indices, cursor_x, input_y_1 + 3.0, 1.5, input_h - 6.0, white_uv, ui.config.theme.cursor_color);
                }
            }
        }

        // 2. Match counts
        let count_str = if is_local {
            if ui.search_matches.is_empty() {
                "0 of 0".to_string()
            } else {
                format!("{} of {}", ui.active_search_match_idx + 1, ui.search_matches.len())
            }
        } else {
            if ui.global_search_results.is_empty() {
                if ui.is_searching_globally {
                    "Searching...".to_string()
                } else {
                    "0 results".to_string()
                }
            } else {
                format!("{} res", ui.global_search_results.len())
            }
        };
        let count_text_len = count_str.chars().count() as f32;
        let count_text_x = count_x + ((count_w - count_text_len * ui.ui_char_width) / 2.0).round();
        ui.push_str(vertices, indices, atlas, queue, &count_str, count_text_x, l_baseline_1, ui.config.theme.modal_text_muted, ui.ui_font_size, ui.ui_char_width);

        // 3. Prev Button (Chevron Up)
        let prev_hover = mouse_x >= prev_x && mouse_x < prev_x + btn_prev_w && mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h;
        ui.push_quad(vertices, indices, prev_x, input_y_1, btn_prev_w, input_h, white_uv, if prev_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, prev_x, input_y_1, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y_1 + input_h - 1.0, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x + btn_prev_w - 1.0, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        let chevron_y = (input_y_1 + (input_h - 12.0) / 2.0).round();
        ui.push_icon(vertices, indices, atlas, queue, "chevron_up", prev_x + (btn_prev_w - 12.0) / 2.0, chevron_y, ui.config.theme.button_text, 12.0);

        // 4. Next Button (Chevron Down)
        let next_hover = mouse_x >= next_x && mouse_x < next_x + btn_next_w && mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h;
        ui.push_quad(vertices, indices, next_x, input_y_1, btn_next_w, input_h, white_uv, if next_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, next_x, input_y_1, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y_1 + input_h - 1.0, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x + btn_next_w - 1.0, input_y_1, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        ui.push_icon(vertices, indices, atlas, queue, "chevron_down", next_x + (btn_next_w - 12.0) / 2.0, chevron_y, ui.config.theme.button_text, 12.0);

        // 5. Close Button (✕)
        let close_hover = mouse_x >= close_x && mouse_x < close_x + close_btn_w && mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h;
        if close_hover {
            ui.push_quad(vertices, indices, close_x, input_y_1, close_btn_w, input_h, white_uv, ui.config.theme.button_hover_bg);
        }
        let close_icon_y = (input_y_1 + (input_h - 12.0) / 2.0).round();
        ui.push_icon(vertices, indices, atlas, queue, "close", close_x + (close_btn_w - 12.0) / 2.0, close_icon_y, if close_hover { ui.config.theme.modal_text_title } else { ui.config.theme.modal_text_muted }, 12.0);

        // --- ROW 2: REPLACE ---
        // 6. Replace Input text box
        ui.push_quad(vertices, indices, input_start_x, input_y_2, input_find_w, input_h, white_uv, ui.config.theme.editor_bg);
        let border_color_2 = if is_replace_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
        ui.push_quad(vertices, indices, input_start_x, input_y_2, input_find_w, 1.0, white_uv, border_color_2);
        ui.push_quad(vertices, indices, input_start_x, input_y_2 + input_h - 1.0, input_find_w, 1.0, white_uv, border_color_2);
        ui.push_quad(vertices, indices, input_start_x, input_y_2, 1.0, input_h, white_uv, border_color_2);
        ui.push_quad(vertices, indices, input_start_x + input_find_w - 1.0, input_y_2, 1.0, input_h, white_uv, border_color_2);

        // Replace icon inside input box
        let replace_icon_sz = 12.0f32;
        let replace_icon_y = (input_y_2 + (input_h - replace_icon_sz) / 2.0).round();
        ui.push_icon(vertices, indices, atlas, queue, "replace", input_start_x + 6.0, replace_icon_y, ui.config.theme.modal_text_muted, replace_icon_sz);

        let max_replace_chars = ((input_find_w - 24.0) / ui.ui_char_width).floor().max(1.0) as usize;
        if replace_query.is_empty() {
            ui.push_str(vertices, indices, atlas, queue, "Replace with...", text_start_x, l_baseline_2, ui.config.theme.syntax_comment, ui.ui_font_size, ui.ui_char_width);
        } else {
            let display_replace = if replace_query.chars().count() > max_replace_chars {
                if is_replace_focused {
                    replace_query.chars().skip(replace_query.chars().count() - max_replace_chars).collect::<String>()
                } else {
                    replace_query.chars().take(max_replace_chars).collect::<String>()
                }
            } else {
                replace_query.clone()
            };
            ui.push_str(vertices, indices, atlas, queue, &display_replace, text_start_x, l_baseline_2, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
            if is_replace_focused {
                let cursor_x = text_start_x + display_replace.chars().count() as f32 * ui.ui_char_width;
                if cursor_x < input_start_x + input_find_w - 5.0 {
                    ui.push_quad(vertices, indices, cursor_x, input_y_2 + 3.0, 1.5, input_h - 6.0, white_uv, ui.config.theme.cursor_color);
                }
            }
        }

        // 7. Replace Button (Icon: replace)
        let rep_hover = mouse_x >= prev_x && mouse_x < prev_x + btn_prev_w && mouse_y >= input_y_2 && mouse_y < input_y_2 + input_h;
        ui.push_quad(vertices, indices, prev_x, input_y_2, btn_prev_w, input_h, white_uv, if rep_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, prev_x, input_y_2, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y_2 + input_h - 1.0, btn_prev_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, prev_x + btn_prev_w - 1.0, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        let rep_icon_y = (input_y_2 + (input_h - 12.0) / 2.0).round();
        ui.push_icon(vertices, indices, atlas, queue, "replace", prev_x + (btn_prev_w - 12.0) / 2.0, rep_icon_y, ui.config.theme.button_text, 12.0);

        // 8. Replace All Button (Icon: replace_all)
        let rep_all_hover = mouse_x >= next_x && mouse_x < next_x + btn_next_w && mouse_y >= input_y_2 && mouse_y < input_y_2 + input_h;
        ui.push_quad(vertices, indices, next_x, input_y_2, btn_next_w, input_h, white_uv, if rep_all_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
        ui.push_quad(vertices, indices, next_x, input_y_2, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y_2 + input_h - 1.0, btn_next_w, 1.0, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        ui.push_quad(vertices, indices, next_x + btn_next_w - 1.0, input_y_2, 1.0, input_h, white_uv, ui.config.theme.button_border);
        
        ui.push_icon(vertices, indices, atlas, queue, "replace_all", next_x + (btn_next_w - 12.0) / 2.0, rep_icon_y, ui.config.theme.button_text, 12.0);
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
