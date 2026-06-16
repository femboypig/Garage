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

    // 2. Input Box layout (centered at the top)
    let input_panel_h = 90.0f32; // Increased height to support 2 rows!
    
    // Row 1: Search Query
    let input_h = 26.0;
    let row1_y = editor_y + 12.0;
    let label_x = text_area_x + 20.0;
    let input_x = text_area_x + 90.0;
    let input_w = 450.0f32;

    let l_baseline_1 = (row1_y + input_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

    // Draw "Search" label
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "Search",
        label_x,
        l_baseline_1,
        ui.config.theme.modal_text_muted,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw Search Input background
    let is_search_focused = !ui.global_search_focus_replace;
    ui.push_quad(
        vertices,
        indices,
        input_x,
        row1_y,
        input_w,
        input_h,
        white_uv,
        ui.config.theme.tabbar_bg,
    );
    let border_color_1 = if is_search_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
    ui.push_quad(vertices, indices, input_x, row1_y, input_w, 1.0, white_uv, border_color_1);
    ui.push_quad(vertices, indices, input_x, row1_y + input_h - 1.0, input_w, 1.0, white_uv, border_color_1);
    ui.push_quad(vertices, indices, input_x, row1_y, 1.0, input_h, white_uv, border_color_1);
    ui.push_quad(vertices, indices, input_x + input_w - 1.0, row1_y, 1.0, input_h, white_uv, border_color_1);

    // Draw option buttons inside search input (far right)
    let btn_w = 22.0f32;
    let opt_y = row1_y + 2.0;
    let opt_h = input_h - 4.0;
    let opt_regex_x = input_x + input_w - 5.0 - btn_w;
    let opt_word_x = opt_regex_x - 2.0 - btn_w;
    let opt_case_x = opt_word_x - 2.0 - btn_w;

    // Aa option button
    let case_hover = mouse_x >= opt_case_x && mouse_x < opt_case_x + btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
    let case_bg = if ui.global_search_case_sensitive {
        ui.config.theme.selection_bg
    } else if case_hover {
        ui.config.theme.button_hover_bg
    } else {
        [0.0, 0.0, 0.0, 0.0]
    };
    ui.push_quad(vertices, indices, opt_case_x, opt_y, btn_w, opt_h, white_uv, case_bg);
    ui.push_str(vertices, indices, atlas, queue, "Aa", opt_case_x + 4.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);

    // W option button
    let word_hover = mouse_x >= opt_word_x && mouse_x < opt_word_x + btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
    let word_bg = if ui.global_search_whole_word {
        ui.config.theme.selection_bg
    } else if word_hover {
        ui.config.theme.button_hover_bg
    } else {
        [0.0, 0.0, 0.0, 0.0]
    };
    ui.push_quad(vertices, indices, opt_word_x, opt_y, btn_w, opt_h, white_uv, word_bg);
    ui.push_str(vertices, indices, atlas, queue, "W", opt_word_x + 6.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);

    // .* option button
    let regex_hover = mouse_x >= opt_regex_x && mouse_x < opt_regex_x + btn_w && mouse_y >= opt_y && mouse_y < opt_y + opt_h;
    let regex_bg = if ui.global_search_regex {
        ui.config.theme.selection_bg
    } else if regex_hover {
        ui.config.theme.button_hover_bg
    } else {
        [0.0, 0.0, 0.0, 0.0]
    };
    ui.push_quad(vertices, indices, opt_regex_x, opt_y, btn_w, opt_h, white_uv, regex_bg);
    ui.push_str(vertices, indices, atlas, queue, ".*", opt_regex_x + 5.0, l_baseline_1, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);

    // Draw search text or placeholder
    let options_area_w = 3.0 * btn_w + 10.0;
    let max_query_w = input_w - 20.0 - options_area_w;
    let max_query_chars = (max_query_w / ui.ui_char_width).floor().max(1.0) as usize;
    
    if ui.global_search_query.is_empty() {
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            "Search in project...",
            input_x + 10.0,
            l_baseline_1,
            ui.config.theme.syntax_comment,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    } else {
        let display_query = if ui.global_search_query.chars().count() > max_query_chars {
            ui.global_search_query.chars().skip(ui.global_search_query.chars().count() - max_query_chars).collect::<String>()
        } else {
            ui.global_search_query.clone()
        };
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &display_query,
            input_x + 10.0,
            l_baseline_1,
            ui.config.theme.modal_text_normal,
            ui.ui_font_size,
            ui.ui_char_width,
        );
        // Draw caret
        if is_search_focused {
            let caret_x = input_x + 10.0 + display_query.chars().count() as f32 * ui.ui_char_width;
            if caret_x < input_x + input_w - options_area_w {
                ui.push_quad(vertices, indices, caret_x, row1_y + 4.0, 2.0, input_h - 8.0, white_uv, ui.config.theme.cursor_color);
            }
        }
    }

    // Row 2: Replace Query
    let row2_y = editor_y + 48.0;
    let l_baseline_2 = (row2_y + input_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

    // Draw "Replace" label
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "Replace",
        label_x,
        l_baseline_2,
        ui.config.theme.modal_text_muted,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw Replace Input background
    let is_replace_focused = ui.global_search_focus_replace;
    ui.push_quad(
        vertices,
        indices,
        input_x,
        row2_y,
        input_w,
        input_h,
        white_uv,
        ui.config.theme.tabbar_bg,
    );
    let border_color_2 = if is_replace_focused { ui.config.theme.cursor_color } else { ui.config.theme.modal_border };
    ui.push_quad(vertices, indices, input_x, row2_y, input_w, 1.0, white_uv, border_color_2);
    ui.push_quad(vertices, indices, input_x, row2_y + input_h - 1.0, input_w, 1.0, white_uv, border_color_2);
    ui.push_quad(vertices, indices, input_x, row2_y, 1.0, input_h, white_uv, border_color_2);
    ui.push_quad(vertices, indices, input_x + input_w - 1.0, row2_y, 1.0, input_h, white_uv, border_color_2);

    // Draw replace text or placeholder
    if ui.global_replace_query.is_empty() {
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            "Replace with...",
            input_x + 10.0,
            l_baseline_2,
            ui.config.theme.syntax_comment,
            ui.ui_font_size,
            ui.ui_char_width,
        );
    } else {
        let display_replace = if ui.global_replace_query.chars().count() > max_query_chars {
            ui.global_replace_query.chars().skip(ui.global_replace_query.chars().count() - max_query_chars).collect::<String>()
        } else {
            ui.global_replace_query.clone()
        };
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &display_replace,
            input_x + 10.0,
            l_baseline_2,
            ui.config.theme.modal_text_normal,
            ui.ui_font_size,
            ui.ui_char_width,
        );
        // Draw caret
        if is_replace_focused {
            let caret_x = input_x + 10.0 + display_replace.chars().count() as f32 * ui.ui_char_width;
            if caret_x < input_x + input_w - 10.0 {
                ui.push_quad(vertices, indices, caret_x, row2_y + 4.0, 2.0, input_h - 8.0, white_uv, ui.config.theme.cursor_color);
            }
        }
    }

    // Draw "Replace All" Button
    let btn_all_w = 90.0f32;
    let btn_all_x = input_x + input_w + 15.0;
    let all_hover = mouse_x >= btn_all_x && mouse_x < btn_all_x + btn_all_w && mouse_y >= row2_y && mouse_y < row2_y + input_h;
    
    ui.push_quad(vertices, indices, btn_all_x, row2_y, btn_all_w, input_h, white_uv, if all_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg });
    ui.push_quad(vertices, indices, btn_all_x, row2_y, btn_all_w, 1.0, white_uv, ui.config.theme.button_border);
    ui.push_quad(vertices, indices, btn_all_x, row2_y + input_h - 1.0, btn_all_w, 1.0, white_uv, ui.config.theme.button_border);
    ui.push_quad(vertices, indices, btn_all_x, row2_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
    ui.push_quad(vertices, indices, btn_all_x + btn_all_w - 1.0, row2_y, 1.0, input_h, white_uv, ui.config.theme.button_border);
    
    let btn_all_text = "Replace All";
    let text_all_w = btn_all_text.chars().count() as f32 * ui.ui_char_width;
    let text_all_x = btn_all_x + ((btn_all_w - text_all_w) / 2.0).round();
    ui.push_str(vertices, indices, atlas, queue, btn_all_text, text_all_x, l_baseline_2, ui.config.theme.button_text, ui.ui_font_size, ui.ui_char_width);

    // Draw search result count next to input on Row 1
    let count_str = if ui.global_search_results.is_empty() {
        if ui.is_searching_globally {
            "Searching...".to_string()
        } else {
            "0 results".to_string()
        }
    } else {
        let mut file_set = std::collections::HashSet::new();
        for (path, _, _) in &ui.global_search_results {
            file_set.insert(path.clone());
        }
        format!("{} results in {} files", ui.global_search_results.len(), file_set.len())
    };
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &count_str,
        input_x + input_w + 15.0,
        l_baseline_1,
        ui.config.theme.modal_text_muted,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw separator line below input panel
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
