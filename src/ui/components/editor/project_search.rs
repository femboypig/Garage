use crate::ui::{UiState, Vertex, FontAtlas};

pub fn draw_project_search(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    text_area_x: f32,
    editor_y: f32,
    text_viewport_w: f32,
    editor_height: f32,
) {
    let white_uv = atlas.white_pixel_uv();

    // 1. Draw overall panel background
    ui.push_quad(
        vertices,
        indices,
        text_area_x,
        editor_y,
        text_viewport_w,
        editor_height,
        white_uv,
        ui.config.theme.editor_bg,
    );

    // 2. Input Box layout (centered at the top)
    let input_panel_h = 50.0f32;
    let input_h = ui.ui_line_height + 8.0;
    let input_y = editor_y + 10.0;
    let input_w = 400.0f32;
    let input_x = text_area_x + 20.0;

    // Draw input background
    ui.push_quad(
        vertices,
        indices,
        input_x,
        input_y,
        input_w,
        input_h,
        white_uv,
        ui.config.theme.tabbar_bg,
    );
    // Draw input borders
    ui.push_quad(vertices, indices, input_x, input_y, input_w, 1.0, white_uv, ui.config.theme.modal_border);
    ui.push_quad(vertices, indices, input_x, input_y + input_h - 1.0, input_w, 1.0, white_uv, ui.config.theme.modal_border);
    ui.push_quad(vertices, indices, input_x, input_y, 1.0, input_h, white_uv, ui.config.theme.modal_border);
    ui.push_quad(vertices, indices, input_x + input_w - 1.0, input_y, 1.0, input_h, white_uv, ui.config.theme.modal_border);

    // Draw "Search: " label and query inside input box
    let prefix = "Search: ";
    let mut input_text = prefix.to_string();
    input_text.push_str(&ui.global_search_query);

    let l_baseline = (input_y + input_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &input_text,
        input_x + 10.0,
        l_baseline,
        ui.config.theme.modal_text_normal,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw caret
    let prefix_len = prefix.chars().count();
    let query_len = ui.global_search_query.chars().count();
    let caret_x = input_x + 10.0 + (prefix_len + query_len) as f32 * ui.ui_char_width;
    if caret_x < input_x + input_w - 10.0 {
        ui.push_quad(
            vertices,
            indices,
            caret_x,
            input_y + 4.0,
            2.0,
            input_h - 8.0,
            white_uv,
            ui.config.theme.cursor_color,
        );
    }

    // Draw result count text on the right of the input box
    let count_str = if ui.global_search_results.is_empty() {
        if ui.is_searching_globally {
            "Searching...".to_string()
        } else {
            "0 results".to_string()
        }
    } else {
        format!("{} results", ui.global_search_results.len())
    };
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &count_str,
        input_x + input_w + 15.0,
        l_baseline,
        ui.config.theme.modal_text_muted,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw a separator line below the input panel
    let sep_y = editor_y + input_panel_h;
    ui.push_quad(
        vertices,
        indices,
        text_area_x,
        sep_y,
        text_viewport_w,
        1.0,
        white_uv,
        ui.config.theme.modal_border,
    );

    // 3. Results list drawing
    let list_y = sep_y + 1.0;
    let item_height = ui.buffer_line_height;
    let max_visible_items = ((editor_y + editor_height - list_y) / item_height).floor() as usize;

    if ui.global_search_results.is_empty() {
        if !ui.is_searching_globally {
            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                "No results found",
                text_area_x + 20.0,
                (list_y + 15.0 + ui.buffer_font_ascent).round(),
                ui.config.theme.syntax_comment,
                ui.buffer_font_size,
                ui.buffer_char_width,
            );
        }
        return;
    }

    // Scroll selection into view
    if max_visible_items > 0 {
        if ui.global_search_selected < ui.global_search_scroll {
            ui.global_search_scroll = ui.global_search_selected;
        } else if ui.global_search_selected >= ui.global_search_scroll + max_visible_items {
            ui.global_search_scroll = ui.global_search_selected + 1 - max_visible_items;
        }
    }
    let max_scroll = ui.global_search_results.len().saturating_sub(max_visible_items);
    ui.global_search_scroll = ui.global_search_scroll.min(max_scroll);

    let start_idx = ui.global_search_scroll;
    let end_idx = (ui.global_search_scroll + max_visible_items).min(ui.global_search_results.len());

    for idx in start_idx..end_idx {
        let (path, line_idx, line_content) = &ui.global_search_results[idx];
        let item_y = list_y + (idx - ui.global_search_scroll) as f32 * item_height;
        let is_selected = idx == ui.global_search_selected;

        // Draw selected row highlight background
        if is_selected {
            ui.push_quad(
                vertices,
                indices,
                text_area_x,
                item_y,
                text_viewport_w,
                item_height,
                white_uv,
                ui.config.theme.sidebar_hover_bg,
            );
        }

        // File path: e.g. "src/main.rs:45"
        let path_str = path.to_string_lossy().to_string();
        let display_path = format!("{}:{}", path_str.strip_prefix("./").unwrap_or(&path_str), line_idx + 1);
        let text_baseline = (item_y + ui.buffer_font_ascent).round();
        
        let path_x = text_area_x + 20.0;
        let path_w = display_path.chars().count() as f32 * ui.buffer_char_width;

        // Draw path
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &display_path,
            path_x,
            text_baseline,
            if is_selected { ui.config.theme.modal_text_title } else { ui.config.theme.modal_text_muted },
            ui.buffer_font_size,
            ui.buffer_char_width,
        );

        // Draw snippet line content on the right of the path
        let snippet_x = path_x + path_w + 20.0;
        let max_snippet_w = text_area_x + text_viewport_w - snippet_x - 30.0;
        let max_snippet_chars = (max_snippet_w / ui.buffer_char_width).floor().max(1.0) as usize;

        let mut snippet = line_content.clone();
        if snippet.chars().count() > max_snippet_chars {
            snippet = snippet.chars().take(max_snippet_chars.saturating_sub(3)).collect::<String>() + "...";
        }

        // Draw snippet text
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &snippet,
            snippet_x,
            text_baseline,
            if is_selected { ui.config.theme.modal_text_title } else { ui.config.theme.modal_text_normal },
            ui.buffer_font_size,
            ui.buffer_char_width,
        );

        // Highlight matched query text inside the snippet
        let query_lower = ui.global_search_query.to_lowercase();
        if !query_lower.is_empty() {
            let snippet_lower = snippet.to_lowercase();
            let mut search_start = 0;
            while let Some(match_offset) = snippet_lower[search_start..].find(&query_lower) {
                let actual_char_idx = snippet[..search_start + match_offset].chars().count();
                let match_char_len = query_lower.chars().count();
                
                let highlight_x = snippet_x + actual_char_idx as f32 * ui.buffer_char_width;
                let highlight_w = match_char_len as f32 * ui.buffer_char_width;

                ui.push_quad(
                    vertices,
                    indices,
                    highlight_x,
                    item_y,
                    highlight_w,
                    item_height,
                    white_uv,
                    [0.9, 0.7, 0.0, 0.3], // soft golden highlight
                );

                search_start += match_offset + query_lower.len().max(1);
            }
        }
    }

    // Draw scrollbar if results exceed view height
    if ui.global_search_results.len() > max_visible_items {
        let track_w = 4.0f32;
        let track_x = text_area_x + text_viewport_w - track_w - 4.0;
        let track_h = max_visible_items as f32 * item_height;

        ui.push_quad(
            vertices,
            indices,
            track_x,
            list_y,
            track_w,
            track_h,
            white_uv,
            ui.config.theme.scrollbar_track,
        );

        let ratio = max_visible_items as f32 / ui.global_search_results.len() as f32;
        let thumb_h = (track_h * ratio).clamp(15.0_f32.min(track_h), track_h);
        let scroll_ratio = ui.global_search_scroll as f32 / max_scroll as f32;
        let thumb_y = list_y + scroll_ratio * (track_h - thumb_h);

        ui.push_quad(
            vertices,
            indices,
            track_x,
            thumb_y,
            track_w,
            thumb_h,
            white_uv,
            ui.config.theme.scrollbar_thumb,
        );
    }
}
