use crate::machkit::{UiState, Vertex};
use crate::renderer::atlas::FontAtlas;
use std::path::PathBuf;

pub enum SearchRenderItem {
    FileHeader {
        path: PathBuf,
    },
    CodeLine {
        path: PathBuf,
        line_idx: usize,
        content: String,
        is_match: bool,
        result_idx: Option<usize>,
        is_first_in_range: bool,
        is_last_in_range: bool,
        start_line_of_range: usize,
        end_line_of_range: usize,
    },
    Separator {
        path: PathBuf,
    },
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

    // 2. Build search rendering items
    let render_items = build_search_render_items(ui);

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
        SearchRenderItem::CodeLine { result_idx: Some(res_idx), .. } => *res_idx == ui.global_search_selected,
        _ => false,
    }).unwrap_or(0);

    // Scroll selection into view
    if max_visible_items > 0 {
        if selected_render_idx < ui.scroll_y {
            ui.scroll_y = selected_render_idx;
        } else if selected_render_idx >= ui.scroll_y + max_visible_items {
            ui.scroll_y = selected_render_idx + 1 - max_visible_items;
        }
    }
    let max_scroll = render_items.len().saturating_sub(max_visible_items);
    ui.scroll_y = ui.scroll_y.min(max_scroll);

    let start_idx = ui.scroll_y;
    let end_idx = (ui.scroll_y + max_visible_items).min(render_items.len());

    for idx in start_idx..end_idx {
        let item_y = list_y + (idx - ui.scroll_y) as f32 * item_height;
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

                // Draw collapse chevron (right if collapsed, down if expanded)
                let is_collapsed = ui.collapsed_search_files.contains(path);
                let chevron_icon = if is_collapsed { "chevron_right" } else { "chevron_down" };
                let chevron_sz = 14.0f32;
                let chevron_y = (item_y + (item_height - chevron_sz) / 2.0).round();
                ui.push_icon(vertices, indices, atlas, queue, chevron_icon, text_area_x + 10.0, chevron_y, ui.config.theme.modal_text_muted, chevron_sz);

                // Draw file path text: File Name (Normal) + Parent Path (Muted)
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let parent_dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let parent_dir = parent_dir.strip_prefix("./").unwrap_or(&parent_dir).to_string();
                let parent_dir = if parent_dir.is_empty() { String::new() } else { format!(" {}/", parent_dir) };

                // Draw File Name (start at 30.0 to lay next to collapse chevron)
                let name_x = text_area_x + 30.0;
                let name_len = file_name.chars().count();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    file_name,
                    name_x,
                    text_baseline,
                    ui.config.theme.modal_text_title,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );

                // Draw Parent Path next to it
                if !parent_dir.is_empty() {
                    let parent_x = name_x + name_len as f32 * ui.buffer_char_width;
                    ui.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        &parent_dir,
                        parent_x,
                        text_baseline,
                        ui.config.theme.syntax_comment,
                        ui.buffer_font_size,
                        ui.buffer_char_width,
                    );
                }

                // Show button if row is hovered, or if active selection belongs to this file
                let selected_path = ui.global_search_results.get(ui.global_search_selected).map(|r| &r.0);
                let is_file_selected = Some(path) == selected_path;
                let show_open_btn = row_hover || is_file_selected;

                if show_open_btn {
                    let btn_text = "Open File Alt-Enter";
                    let btn_chars = btn_text.chars().count();
                    let btn_w = btn_chars as f32 * ui.ui_char_width + 16.0;
                    let btn_h = item_height - 6.0;
                    let btn_x = text_area_x + text_viewport_w - btn_w - 15.0;
                    let btn_y = item_y + 3.0;

                    let is_btn_hover = mouse_x >= btn_x && mouse_x < btn_x + btn_w && mouse_y >= btn_y && mouse_y < btn_y + btn_h;
                    let btn_bg = if is_btn_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg };

                    ui.push_quad(vertices, indices, btn_x, btn_y, btn_w, btn_h, white_uv, btn_bg);
                    ui.push_quad(vertices, indices, btn_x, btn_y, btn_w, 1.0, white_uv, ui.config.theme.button_border);
                    ui.push_quad(vertices, indices, btn_x, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, ui.config.theme.button_border);
                    ui.push_quad(vertices, indices, btn_x, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);
                    ui.push_quad(vertices, indices, btn_x + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);

                    let btn_baseline = (btn_y + btn_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
                    ui.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        btn_text,
                        btn_x + 8.0,
                        btn_baseline,
                        ui.config.theme.button_text,
                        ui.ui_font_size,
                        ui.ui_char_width,
                    );
                }
            }
            SearchRenderItem::CodeLine {
                line_idx,
                content,
                result_idx,
                ..
            } => {
                let is_selected = result_idx.map_or(false, |res_idx| res_idx == ui.global_search_selected);

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



                // Draw line number (centered/padded)
                let line_str = format!("{}", line_idx + 1);
                let line_w = line_str.chars().count() as f32 * ui.buffer_char_width;
                let line_x = text_area_x + 10.0 + (35.0 - line_w).max(0.0);
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &line_str,
                    line_x,
                    text_baseline,
                    ui.config.theme.syntax_comment,
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );

                // Draw code content
                let snippet_x = text_area_x + 60.0;
                let display_content = content.replace('\t', "    ");

                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &display_content,
                    snippet_x,
                    text_baseline,
                    if is_selected { ui.config.theme.modal_text_title } else { ui.config.theme.modal_text_normal },
                    ui.buffer_font_size,
                    ui.buffer_char_width,
                );

                if is_selected && !ui.global_search_focused {
                    let cursor_col_clamped = ui.global_search_col.min(display_content.chars().count());
                    let cursor_x = snippet_x + cursor_col_clamped as f32 * ui.buffer_char_width;
                    let mut cursor_ctx = crate::machkit::UiContext {
                        vertices,
                        indices,
                        atlas,
                        queue,
                        mouse_x: 0.0,
                        mouse_y: 0.0,
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
                    crate::machkit::Cursor::new()
                        .draw(&mut cursor_ctx, cursor_x, item_y + 2.0, item_height - 4.0);
                }

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
                        for m in re.find_iter(&display_content) {
                            let start_char = display_content[..m.start()].chars().count();
                            let end_char = display_content[..m.end()].chars().count();
                            
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
                                if is_selected {
                                    [0.2, 0.4, 0.8, 0.4] // Active match (blue highlight)
                                } else {
                                    [0.9, 0.7, 0.0, 0.35] // Muted match (yellow highlight)
                                },
                            );
                        }
                    }
                }
            }
            SearchRenderItem::Separator { .. } => {
                // Draw a subtle horizontal separator line across the code block
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x + 50.0,
                    (item_y + item_height / 2.0 - 0.5).round(),
                    text_viewport_w - 70.0,
                    1.0,
                    white_uv,
                    ui.config.theme.button_border,
                );
            }
        }
    }

}

pub fn build_search_render_items(ui: &mut UiState) -> Vec<SearchRenderItem> {
    let mut file_groups: Vec<(PathBuf, Vec<(usize, usize)>)> = Vec::new();
    for (result_idx, (path, line_idx, _)) in ui.global_search_results.iter().enumerate() {
        if let Some(pos) = file_groups.iter().position(|(p, _)| p == path) {
            file_groups[pos].1.push((*line_idx, result_idx));
        } else {
            file_groups.push((path.clone(), vec![(*line_idx, result_idx)]));
        }
    }

    let mut render_items = Vec::new();
    for (path, mut matches) in file_groups {
        render_items.push(SearchRenderItem::FileHeader { path: path.clone() });

        if ui.collapsed_search_files.contains(&path) {
            continue;
        }

        matches.sort_by_key(|m| m.0);

        let file_lines = ui.project_search_file_cache.entry(path.clone()).or_insert_with(|| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                content.lines().map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            }
        });

        let file_len = file_lines.len();
        if file_len == 0 {
            continue;
        }

        let mut ranges: Vec<(usize, usize, Vec<(usize, usize)>)> = Vec::new();
        for (line_idx, res_idx) in matches {
            let (extra_before, extra_after) = ui.global_search_expanded_margins
                .get(&(path.clone(), line_idx))
                .copied()
                .unwrap_or((2, 2));

            let start = line_idx.saturating_sub(extra_before);
            let end = (line_idx + extra_after).min(file_len - 1);

            if let Some(last) = ranges.last_mut() {
                if start <= last.1 + 1 {
                    last.1 = last.1.max(end);
                    last.2.push((line_idx, res_idx));
                    continue;
                }
            }
            ranges.push((start, end, vec![(line_idx, res_idx)]));
        }

        for (r_idx, (start, end, range_matches)) in ranges.iter().enumerate() {
            for i in *start..=*end {
                let is_match = range_matches.iter().any(|m| m.0 == i);
                let res_idx = range_matches.iter().find(|m| m.0 == i).map(|m| m.1);
                let content = file_lines.get(i).cloned().unwrap_or_default();
                render_items.push(SearchRenderItem::CodeLine {
                    path: path.clone(),
                    line_idx: i,
                    content,
                    is_match,
                    result_idx: res_idx,
                    is_first_in_range: i == *start,
                    is_last_in_range: i == *end,
                    start_line_of_range: *start,
                    end_line_of_range: *end,
                });
            }
            if r_idx + 1 < ranges.len() {
                render_items.push(SearchRenderItem::Separator { path: path.clone() });
            }
        }
    }
    render_items
}


