use crate::ui::{UiState, Vertex, FontAtlas};

pub fn draw_command_palette(
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
    let prefix = "> ";
    let mut input_text = prefix.to_string();
    input_text.push_str(&ui.command_palette_query);
    
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
    let query_len = prefix.chars().count() + ui.command_palette_query.chars().count();
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

    // Draw Filtered List of Commands
    let list_y = sep_y + 1.0;
    let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
    let max_visible_items = ((modal_y + modal_h - 15.0 - list_y) / item_height).floor() as usize;

    let filtered = ui.get_filtered_commands();
    
    // Automatically scroll selection into view
    if max_visible_items > 0 {
        if ui.command_palette_selected < ui.command_palette_scroll {
            ui.command_palette_scroll = ui.command_palette_selected;
        } else if ui.command_palette_selected >= ui.command_palette_scroll + max_visible_items {
            ui.command_palette_scroll = ui.command_palette_selected + 1 - max_visible_items;
        }
    }

    // Clamp scroll offset to valid bounds
    let max_scroll = filtered.len().saturating_sub(max_visible_items);
    ui.command_palette_scroll = ui.command_palette_scroll.min(max_scroll);

    let start_idx = ui.command_palette_scroll;
    let end_idx = (ui.command_palette_scroll + max_visible_items).min(filtered.len());

    for idx in start_idx..end_idx {
        let item = filtered[idx];
        let item_y = list_y + (idx - ui.command_palette_scroll) as f32 * item_height;
        let is_selected = idx == ui.command_palette_selected;

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

        // Left text: display name
        let display_name = item.0;
        let item_text_color = if is_selected {
            ui.config.theme.modal_text_title
        } else {
            ui.config.theme.modal_text_normal
        };

        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            display_name,
            modal_x + 20.0,
            (item_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            item_text_color,
            ui.ui_font_size,
            ui.ui_char_width,
        );

        // Right text: description (if room fits)
        let desc = item.1;
        let desc_color = ui.config.theme.modal_text_muted;
        let desc_len = desc.chars().count() as f32;
        let desc_w = desc_len * ui.ui_char_width;
        let right_margin = if filtered.len() > max_visible_items { 25.0 } else { 20.0 };
        let desc_x = modal_x + modal_w - right_margin - desc_w;
        
        let name_len = display_name.chars().count() as f32;
        let name_w = name_len * ui.ui_char_width;
        
        if desc_x > modal_x + name_w + 40.0 {
            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                desc,
                desc_x,
                (item_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
                desc_color,
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }
    }

    // Draw scrollbar for command palette if needed
    if filtered.len() > max_visible_items {
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
        
        let ratio = max_visible_items as f32 / filtered.len() as f32;
        let thumb_h = (track_h * ratio).clamp(15.0, track_h);
        let scroll_ratio = ui.command_palette_scroll as f32 / max_scroll as f32;
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
