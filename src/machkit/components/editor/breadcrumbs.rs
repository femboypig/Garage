use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::{UiState, Vertex};
use crate::renderer::atlas::FontAtlas;

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

    // Breadcrumb Bar background
    ctx.push_quad(bar_x, bar_y, bar_w, bar_h, ctx.theme.breadcrumb_bg);
    // Breadcrumb bottom border
    ctx.push_quad(
        bar_x,
        bar_y + bar_h - 1.0,
        bar_w,
        1.0,
        ctx.theme.breadcrumb_border,
    );

    let is_project_search = active_file_path == Some("search://project");
    if is_active_pane && (ui.show_search_panel || is_project_search) {
        let is_local = !is_project_search;
        let search_query = if is_local {
            &ui.search_query
        } else {
            &ui.global_search_query
        };
        let replace_query = if is_local {
            &ui.replace_query
        } else {
            &ui.global_replace_query
        };
        let case_sensitive = if is_local {
            ui.search_case_sensitive
        } else {
            ui.global_search_case_sensitive
        };
        let whole_word = if is_local {
            ui.search_whole_word
        } else {
            ui.global_search_whole_word
        };
        let regex = if is_local {
            ui.search_regex
        } else {
            ui.global_search_regex
        };
        let is_replace_focused = if is_local {
            ui.search_focus_replace
        } else {
            ui.global_search_focus_replace
        };
        let is_find_focused = !is_replace_focused;
        let show_replace = if is_local {
            ui.show_replace
        } else {
            ui.global_show_replace
        };

        let count_w = if is_local { 70.0f32 } else { 75.0f32 };
        let btn_prev_w = 24.0f32;
        let btn_next_w = 24.0f32;
        let close_btn_w = 24.0f32;

        let btn_rep_toggle_w = if is_local { 0.0f32 } else { 24.0f32 };
        let btn_filter_w = if is_local { 0.0f32 } else { 24.0f32 };

        let input_h = 26.0f32;
        let (input_y_1, input_y_2) = if is_local {
            let path_h = 20.0f32;
            let remaining_h = bar_h - path_h;
            let row_h = if show_replace {
                remaining_h / 2.0
            } else {
                remaining_h
            };
            let y1 = bar_y + path_h + (row_h - input_h) / 2.0;
            let y2 = bar_y + path_h + row_h + (row_h - input_h) / 2.0;
            (y1, y2)
        } else {
            let row_h = if show_replace { bar_h / 2.0 } else { bar_h };
            let y1 = bar_y + (row_h - input_h) / 2.0;
            let y2 = bar_y + row_h + (row_h - input_h) / 2.0;
            (y1, y2)
        };

        let l_baseline_1 = (input_y_1 + input_h / 2.0 + ctx.ui_font_ascent / 2.0 - 2.0).round();

        let close_x = bar_x + bar_w - 10.0 - close_btn_w;
        let next_x = close_x - 8.0 - btn_next_w;
        let prev_x = next_x - 4.0 - btn_prev_w;

        let (rep_toggle_x, filter_x, count_x) = if is_local {
            let count_x = prev_x - 8.0 - count_w;
            (prev_x, prev_x, count_x)
        } else {
            let rep_toggle_x = prev_x - 8.0 - btn_rep_toggle_w;
            let filter_x = rep_toggle_x - 4.0 - btn_filter_w;
            let count_x = filter_x - 8.0 - count_w;
            (rep_toggle_x, filter_x, count_x)
        };

        let toggle_btn_w = 24.0f32;
        let toggle_btn_x = bar_x + 10.0;
        let input_start_x = toggle_btn_x + toggle_btn_w + 6.0;
        let input_find_w = (count_x - 10.0 - input_start_x).max(50.0);

        // Draw path text above the search inputs in local search
        if is_local {
            let relative_path = active_file_path.unwrap_or("Untitled");
            let current_fn = ui.find_current_function(buffer, cursor.line);
            let breadcrumb_text = if let Some(ref func) = current_fn {
                format!("{} > {}", relative_path, func)
            } else {
                relative_path.to_string()
            };
            ctx.push_str(
                &breadcrumb_text,
                bar_x + 15.0,
                (bar_y + 10.0 + ctx.ui_font_ascent / 2.0).round(),
                ctx.theme.breadcrumb_text,
                ctx.ui_font_size,
            );
        }

        // --- ROW 1: FIND ---
        // 0. Toggle Replace button / Collapse All button
        let toggle_icon_name = if is_local {
            if show_replace {
                "chevron_up"
            } else {
                "chevron_down"
            }
        } else {
            "list_collapse"
        };
        crate::machkit::Button::new()
            .icon(toggle_icon_name)
            .text_color(ctx.theme.modal_text_muted)
            .draw(&mut ctx, toggle_btn_x, input_y_1, toggle_btn_w, input_h);

        // 1. Find Input text box
        let placeholder = if is_local {
            "Search query..."
        } else {
            "Search in project..."
        };
        let opt_btn_w = 22.0f32;
        let options_w = 3.0 * opt_btn_w + 10.0;

        crate::machkit::Input::new()
            .value(search_query)
            .placeholder(placeholder)
            .focused(is_find_focused)
            .icon("search")
            .right_padding(options_w)
            .draw(&mut ctx, input_start_x, input_y_1, input_find_w, input_h);

        // Option buttons inside Find Input
        let opt_y = input_y_1 + 2.0;
        let opt_h = input_h - 4.0;
        let opt_regex_x = input_start_x + input_find_w - 5.0 - opt_btn_w;
        let opt_word_x = opt_regex_x - 2.0 - opt_btn_w;
        let opt_case_x = opt_word_x - 2.0 - opt_btn_w;

        // Aa option
        crate::machkit::Button::new()
            .icon("case_sensitive")
            .active(case_sensitive)
            .text_color(ctx.theme.modal_text_normal)
            .draw(&mut ctx, opt_case_x, opt_y, opt_btn_w, opt_h);

        // W option
        crate::machkit::Button::new()
            .icon("whole_word")
            .active(whole_word)
            .text_color(ctx.theme.modal_text_normal)
            .draw(&mut ctx, opt_word_x, opt_y, opt_btn_w, opt_h);

        // .* option
        crate::machkit::Button::new()
            .icon("regex")
            .active(regex)
            .text_color(ctx.theme.modal_text_normal)
            .draw(&mut ctx, opt_regex_x, opt_y, opt_btn_w, opt_h);

        // 2. Match counts
        let count_str = if is_local {
            if ui.search_matches.is_empty() {
                "0 of 0".to_string()
            } else {
                format!(
                    "{} of {}",
                    ui.active_search_match_idx + 1,
                    ui.search_matches.len()
                )
            }
        } else {
            if ui.global_search_results.is_empty() {
                if ui.is_searching_globally {
                    "Searching...".to_string()
                } else {
                    "0/0".to_string()
                }
            } else {
                format!(
                    "{}/{}",
                    ui.global_search_selected + 1,
                    ui.global_search_results.len()
                )
            }
        };
        let count_text_len = count_str.chars().count() as f32;
        let count_text_x = count_x + ((count_w - count_text_len * ctx.ui_char_width) / 2.0).round();
        ctx.push_str(
            &count_str,
            count_text_x,
            l_baseline_1,
            ctx.theme.modal_text_muted,
            ctx.ui_font_size,
        );

        // 2.5 Optional Filter and Replace Toggle buttons for Project Search
        if !is_local {
            // Filter Button
            crate::machkit::Button::new().icon("filter").draw(
                &mut ctx,
                filter_x,
                input_y_1,
                btn_filter_w,
                input_h,
            );

            // Replace Toggle Button
            crate::machkit::Button::new()
                .icon("replace")
                .active(show_replace)
                .draw(&mut ctx, rep_toggle_x, input_y_1, btn_rep_toggle_w, input_h);
        }

        // 3. Prev Button (Chevron Left)
        crate::machkit::Button::new()
            .icon("chevron_left")
            .draw(&mut ctx, prev_x, input_y_1, btn_prev_w, input_h);

        // 4. Next Button (Chevron Right)
        crate::machkit::Button::new()
            .icon("chevron_right")
            .draw(&mut ctx, next_x, input_y_1, btn_next_w, input_h);

        // 5. Close Button (✕)
        let close_hover = ctx.mouse_x >= close_x
            && ctx.mouse_x < close_x + close_btn_w
            && ctx.mouse_y >= input_y_1
            && ctx.mouse_y < input_y_1 + input_h;
        let close_color = if close_hover {
            ctx.theme.modal_text_title
        } else {
            ctx.theme.modal_text_muted
        };
        crate::machkit::Button::new()
            .icon("close")
            .text_color(close_color)
            .draw(&mut ctx, close_x, input_y_1, close_btn_w, input_h);

        // --- ROW 2: REPLACE ---
        if show_replace {
            crate::machkit::Input::new()
                .value(replace_query)
                .placeholder("Replace with...")
                .focused(is_replace_focused)
                .icon("replace")
                .draw(&mut ctx, input_start_x, input_y_2, input_find_w, input_h);

            // 7. Replace Button (Icon: replace)
            crate::machkit::Button::new()
                .icon("replace")
                .draw(&mut ctx, prev_x, input_y_2, btn_prev_w, input_h);

            // 8. Replace All Button (Icon: replace_all)
            crate::machkit::Button::new()
                .icon("replace_all")
                .draw(&mut ctx, next_x, input_y_2, btn_next_w, input_h);
        }
    } else {
        // Construct breadcrumb text: relative_path > current_function
        let relative_path = active_file_path.unwrap_or("Untitled");

        let current_fn = ui.find_current_function(buffer, cursor.line);
        let breadcrumb_text = if let Some(ref func) = current_fn {
            format!("{} > {}", relative_path, func)
        } else {
            relative_path.to_string()
        };
        ctx.push_str(
            &breadcrumb_text,
            bar_x + 15.0,
            (bar_y + bar_h / 2.0 + ctx.ui_font_ascent / 2.0 - 2.0).round(),
            ctx.theme.breadcrumb_text,
            ctx.ui_font_size,
        );
    }
}
