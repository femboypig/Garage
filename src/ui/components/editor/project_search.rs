use crate::ui::{UiState, Vertex, FontAtlas};
use std::path::PathBuf;

enum SearchRenderItem {
    FileHeader { path: PathBuf },
    Match { line_idx: usize, content: String, result_idx: usize },
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
    mouse_x: f32,
    mouse_y: f32,
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

    // 2. Construct flat rendering items (grouped by file)
    let mut render_items = Vec::new();
    let mut last_path = None;
    for (idx, (path, line_idx, content)) in ui.global_search_results.iter().enumerate() {
        if last_path.as_ref() != Some(path) {
            render_items.push(SearchRenderItem::FileHeader { path: path.clone() });
            last_path = Some(path.clone());
        }
        render_items.push(SearchRenderItem::Match {
            line_idx: *line_idx,
            content: content.clone(),
            result_idx: idx,
        });
    }

    let list_y = editor_y;
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

        let row_hover = mouse_x >= text_area_x && mouse_x < text_area_x + text_viewport_w && mouse_y >= item_y && mouse_y < item_y + item_height;

        match &render_items[idx] {
            SearchRenderItem::FileHeader { path } => {
                // Draw header background
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x,
                    item_y,
                    text_viewport_w,
                    item_height,
                    white_uv,
                    if row_hover { ui.config.theme.titlebar_hover_bg } else { ui.config.theme.tabbar_bg },
                );

                // Detect file extension for icon
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let icon_name = match ext {
                    "rs" => "rust",
                    "toml" => "toml",
                    "json" => "json",
                    "md" => "md",
                    _ => "file",
                };
                
                // Draw icon
                let icon_sz = 14.0f32;
                let icon_y = (item_y + (item_height - icon_sz) / 2.0).round();
                ui.push_icon(vertices, indices, atlas, queue, icon_name, text_area_x + 10.0, icon_y, ui.config.theme.syntax_keyword, icon_sz);

                // Draw file path text
                let path_str = path.to_string_lossy().to_string();
                let display_path = path_str.strip_prefix("./").unwrap_or(&path_str).to_string();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &display_path,
                    text_area_x + 10.0 + icon_sz + 6.0,
                    text_baseline,
                    ui.config.theme.modal_text_title,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );
            }
            SearchRenderItem::Match { line_idx, content, result_idx, .. } => {
                let is_selected = *result_idx == ui.global_search_selected;

                let bg_color = if is_selected {
                    ui.config.theme.sidebar_hover_bg
                } else if row_hover {
                    ui.config.theme.titlebar_hover_bg
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                };
                if is_selected || row_hover {
                    ui.push_quad(
                        vertices,
                        indices,
                        text_area_x,
                        item_y,
                        text_viewport_w,
                        item_height,
                        white_uv,
                        bg_color,
                    );
                }

                // Draw line number
                let line_str = format!("{}", line_idx + 1);
                let line_w = line_str.chars().count() as f32 * ui.buffer_char_width;
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

                // Draw snippet
                let snippet_x = text_area_x + 24.0 + 35.0 + 15.0; // Total indent = 74px
                let max_snippet_w = text_area_x + text_viewport_w - snippet_x - 30.0;
                let max_snippet_chars = (max_snippet_w / ui.buffer_char_width).floor().max(1.0) as usize;

                let mut snippet = content.clone();
                if snippet.chars().count() > max_snippet_chars {
                    snippet = snippet.chars().take(max_snippet_chars.saturating_sub(3)).collect::<String>() + "...";
                }

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

                // Highlight matched query text
                let query = &ui.global_search_query;
                if !query.is_empty() {
                    let pattern = if ui.global_search_regex {
                        query.clone()
                    } else {
                        regex::escape(query)
                    };
                    let mut builder = regex::RegexBuilder::new(&pattern);
                    builder.case_insensitive(!ui.global_search_case_sensitive);
                    if let Ok(re) = builder.build() {
                        for m in re.find_iter(&snippet) {
                            let start_char = snippet[..m.start()].chars().count();
                            let end_char = snippet[..m.end()].chars().count();
                            
                            let highlight_x = snippet_x + start_char as f32 * ui.buffer_char_width;
                            let highlight_w = (end_char - start_char) as f32 * ui.buffer_char_width;

                            ui.push_quad(
                                vertices,
                                indices,
                                highlight_x,
                                item_y + 1.0,
                                highlight_w,
                                item_height - 2.0,
                                white_uv,
                                [0.9, 0.7, 0.0, 0.35],
                            );
                        }
                    }
                }
            }
        }
    }

    // Draw scrollbar
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

