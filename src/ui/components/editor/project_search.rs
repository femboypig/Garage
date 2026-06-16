use crate::ui::{UiState, Vertex, FontAtlas};
use std::path::PathBuf;

enum SearchRenderItem {
    FileHeader { path: PathBuf },
    Match { path: PathBuf, line_idx: usize, content: String, result_idx: usize },
}

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
    let input_y = editor_y + ((input_panel_h - input_h) / 2.0).round();
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
    let prefix_w = prefix.chars().count() as f32 * ui.ui_char_width;
    let max_query_w = input_w - 20.0 - prefix_w;
    let max_query_chars = (max_query_w / ui.ui_char_width).floor().max(1.0) as usize;
    let display_query = if ui.global_search_query.chars().count() > max_query_chars {
        ui.global_search_query.chars().skip(ui.global_search_query.chars().count() - max_query_chars).collect::<String>()
    } else {
        ui.global_search_query.clone()
    };
    let mut input_text = prefix.to_string();
    input_text.push_str(&display_query);

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
    let query_len = display_query.chars().count();
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

    // 3. Construct flat rendering items (grouped by file)
    let mut render_items = Vec::new();
    let mut last_path = None;
    for (idx, (path, line_idx, content)) in ui.global_search_results.iter().enumerate() {
        if last_path.as_ref() != Some(path) {
            render_items.push(SearchRenderItem::FileHeader { path: path.clone() });
            last_path = Some(path.clone());
        }
        render_items.push(SearchRenderItem::Match {
            path: path.clone(),
            line_idx: *line_idx,
            content: content.clone(),
            result_idx: idx,
        });
    }

    let list_y = sep_y + 1.0;
    let item_height = ui.buffer_line_height;
    let max_visible_items = ((editor_y + editor_height - list_y) / item_height).floor() as usize;

    if render_items.is_empty() {
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

    // Find render index of the selected match
    let selected_render_idx = render_items.iter().position(|item| match item {
        SearchRenderItem::Match { result_idx, .. } => *result_idx == ui.global_search_selected,
        _ => false,
    }).unwrap_or(0);

    // Scroll selection into view
    if max_visible_items > 0 {
        if selected_render_idx < ui.global_search_scroll {
            ui.global_search_scroll = selected_render_idx;
        } else if selected_render_idx >= ui.global_search_scroll + max_visible_items {
            ui.global_search_scroll = selected_render_idx + 1 - max_visible_items;
        }
    }
    let max_scroll = render_items.len().saturating_sub(max_visible_items);
    ui.global_search_scroll = ui.global_search_scroll.min(max_scroll);

    let start_idx = ui.global_search_scroll;
    let end_idx = (ui.global_search_scroll + max_visible_items).min(render_items.len());

    for idx in start_idx..end_idx {
        let item_y = list_y + (idx - ui.global_search_scroll) as f32 * item_height;
        let text_baseline = (item_y + ui.buffer_font_ascent).round();

        match &render_items[idx] {
            SearchRenderItem::FileHeader { path } => {
                // Draw a nice subtle header background
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    item_y,
                    text_viewport_w,
                    item_height,
                    white_uv,
                    ui.config.theme.tabbar_bg,
                );
                // Draw file path text
                let path_str = path.to_string_lossy().to_string();
                let display_path = path_str.strip_prefix("./").unwrap_or(&path_str).to_string();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &display_path,
                    text_area_x + 10.0,
                    text_baseline,
                    ui.config.theme.syntax_keyword, // Highlight filepath nicely
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );
            }
            SearchRenderItem::Match { line_idx, content, result_idx, .. } => {
                let is_selected = *result_idx == ui.global_search_selected;

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

                // Draw line number aligned
                let line_str = format!("{}", line_idx + 1);
                let line_w = line_str.chars().count() as f32 * ui.buffer_char_width;
                // Indent 24px, right align line number in a 35px column
                let line_x = text_area_x + 24.0 + (35.0 - line_w).max(0.0);
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &line_str,
                    line_x,
                    text_baseline,
                    ui.config.theme.modal_text_muted,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );

                // Draw snippet content at a fixed starting point!
                let snippet_x = text_area_x + 24.0 + 35.0 + 15.0; // Total indent = 74px
                let max_snippet_w = text_area_x + text_viewport_w - snippet_x - 30.0;
                let max_snippet_chars = (max_snippet_w / ui.buffer_char_width).floor().max(1.0) as usize;

                let mut snippet = content.clone();
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
        }
    }

    // Draw scrollbar if results exceed view height
    if render_items.len() > max_visible_items {
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

        let ratio = max_visible_items as f32 / render_items.len() as f32;
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
