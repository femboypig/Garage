use crate::machkit::{UiState, Vertex};
use crate::renderer::atlas::FontAtlas;

pub fn draw_global_search(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
    modal_h: f32,
    white_uv: [f32; 2],
) {
    let input_y = modal_y + 15.0;
    let prefix = "Search: ";
    let mut input_text = prefix.to_string();
    input_text.push_str(&ui.global_search_query);

    let text_color = ui.config.theme.modal_text_normal;
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &input_text,
        modal_x + 20.0,
        (input_y + ui.ui_font_ascent).round(),
        text_color,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw caret in the input box
    let query_len = prefix.chars().count() + ui.global_search_query.chars().count();
    let caret_x = modal_x + 20.0 + query_len as f32 * ui.ui_char_width;
    ui.push_quad(
        vertices,
        indices,
        caret_x,
        input_y + 2.0,
        2.0,
        ui.ui_line_height - 4.0,
        white_uv,
        ui.config.theme.cursor_color,
    );

    // Draw horizontal separator below input
    let sep_y = input_y + ui.ui_line_height + 15.0;
    ui.push_quad(
        vertices,
        indices,
        modal_x,
        sep_y,
        modal_w,
        1.0,
        white_uv,
        ui.config.theme.modal_border,
    );

    let list_y = sep_y + 1.0;
    let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
    let max_visible_items = ((modal_y + modal_h - list_y) / item_height).floor() as usize;

    if ui.is_searching_globally && ui.global_search_results.is_empty() {
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            "Searching...",
            modal_x + 20.0,
            (list_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            ui.config.theme.modal_text_muted,
            ui.ui_font_size,
            ui.ui_char_width,
        );
        return;
    }

    if ui.global_search_results.is_empty() {
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            "No results found",
            modal_x + 20.0,
            (list_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            ui.config.theme.modal_text_muted,
            ui.ui_font_size,
            ui.ui_char_width,
        );
        return;
    }

    // Automatically scroll selection into view
    if max_visible_items > 0 {
        if ui.global_search_selected < ui.global_search_scroll {
            ui.global_search_scroll = ui.global_search_selected;
        } else if ui.global_search_selected >= ui.global_search_scroll + max_visible_items {
            ui.global_search_scroll = ui.global_search_selected + 1 - max_visible_items;
        }
    }

    // Clamp scroll offset to valid bounds
    let max_scroll = ui
        .global_search_results
        .len()
        .saturating_sub(max_visible_items);
    ui.global_search_scroll = ui.global_search_scroll.min(max_scroll);

    let start_idx = ui.global_search_scroll;
    let end_idx = (ui.global_search_scroll + max_visible_items).min(ui.global_search_results.len());

    for idx in start_idx..end_idx {
        let (path, line_idx, line_content) = &ui.global_search_results[idx];
        let item_y = list_y + (idx - ui.global_search_scroll) as f32 * item_height;
        let is_selected = idx == ui.global_search_selected;

        // Highlight selected command row
        if is_selected {
            ui.push_quad(
                vertices,
                indices,
                modal_x + 1.0,
                item_y,
                modal_w - 2.0,
                item_height,
                white_uv,
                ui.config.theme.sidebar_hover_bg,
            );
        }

        let item_text_color = if is_selected {
            ui.config.theme.modal_text_title
        } else {
            ui.config.theme.modal_text_normal
        };

        // File path and line number: e.g. "src/main.rs:45"
        let path_str = path.to_string_lossy().to_string();
        let display_path = format!(
            "{}:{}",
            path_str.strip_prefix("./").unwrap_or(&path_str),
            line_idx + 1
        );

        let path_x = modal_x + 20.0;
        let text_baseline = (item_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

        let path_len = display_path.chars().count();
        let path_width = path_len as f32 * ui.ui_char_width;

        // Draw path
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &display_path,
            path_x,
            text_baseline,
            if is_selected {
                ui.config.theme.modal_text_title
            } else {
                ui.config.theme.modal_text_muted
            },
            ui.ui_font_size,
            ui.ui_char_width,
        );

        // Draw snippet line content
        let snippet_x = path_x + path_width + 15.0;
        let max_snippet_w = modal_x + modal_w - snippet_x - 25.0;
        let max_snippet_chars = (max_snippet_w / ui.ui_char_width).floor().max(1.0) as usize;

        let mut snippet = line_content.clone();
        if snippet.chars().count() > max_snippet_chars {
            snippet = snippet
                .chars()
                .take(max_snippet_chars.saturating_sub(3))
                .collect::<String>()
                + "...";
        }

        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &snippet,
            snippet_x,
            text_baseline,
            item_text_color,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }

    // Draw scrollbar if needed
    if ui.global_search_results.len() > max_visible_items {
        let track_x = modal_x + modal_w - 8.0;
        let track_w = 4.0f32;
        let track_h = max_visible_items as f32 * item_height;

        // Scrollbar track
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

        // Scrollbar thumb
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
