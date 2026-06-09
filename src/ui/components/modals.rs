use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::ui::{UiState, MenuType, ModalType};
use std::path::Path;

pub fn draw_modals(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    height: f32,
    mouse_x: f32,
    mouse_y: f32,
    current_backend: wgpu::Backend,
    tab_paths: &[Option<String>],
) {
    let white_uv = atlas.white_pixel_uv();

    // --- 6. Draw Context Dropdown Menus (On top of everything) ---
    if let Some(menu) = ui.active_menu {
        let items = match menu {
            MenuType::Garage => vec!["Settings", "About", "Exit"],
            MenuType::File => vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
            MenuType::Edit => vec!["Undo (Ctrl+Z)", "Redo (Ctrl+Y)"],
            MenuType::Selection => vec!["Select All", "Clear Selection"],
            MenuType::View => vec!["Toggle Sidebar", "Command Palette (Ctrl+Shift+P)"],
        };
        
        // Calculate dynamic menu_x matching the header position
        let menu_items_raw = [
            ("Garage", MenuType::Garage),
            ("File", MenuType::File),
            ("Edit", MenuType::Edit),
            ("Selection", MenuType::Selection),
            ("View", MenuType::View),
        ];
        let mut menu_x = 0.0;
        let mut current_x = 0.0;
        for (i, (label, m_type)) in menu_items_raw.iter().enumerate() {
            let label_len = label.chars().count() as f32;
            let text_w = label_len * ui.ui_char_width;
            let (left_pad, right_pad) = if i == 0 {
                (14.0, 10.0)
            } else {
                (10.0, 10.0)
            };
            let item_w = text_w + left_pad + right_pad;
            if m_type == &menu {
                menu_x = current_x;
                break;
            }
            current_x = current_x + item_w;
        }

        let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
        let dropdown_h = items.len() as f32 * item_height;
        let max_chars = items.iter().map(|s| s.chars().count()).max().unwrap_or(10) as f32;
        let dropdown_w = (max_chars * ui.ui_char_width + 30.0).round();

        // Draw Dropdown Card Background
        ui.push_quad(
            vertices,
            indices,
            menu_x,
            ui.titlebar_height,
            dropdown_w,
            dropdown_h,
            white_uv,
            ui.config.theme.modal_bg,
        );

        // Draw Item Hovers and text
        for (idx, label) in items.iter().enumerate() {
            let row_y = ui.titlebar_height + idx as f32 * item_height;
            let is_hovered = mouse_x >= menu_x && mouse_x < menu_x + dropdown_w && mouse_y >= row_y && mouse_y < row_y + item_height;

            if is_hovered {
                ui.push_quad(
                    vertices,
                    indices,
                    menu_x,
                    row_y,
                    dropdown_w,
                    item_height,
                    white_uv,
                    ui.config.theme.dropdown_hover_bg,
                );
            }

            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                label,
                menu_x + 12.0,
                (row_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
                if is_hovered { [0.0, 0.0, 0.0, 1.0] } else { ui.config.theme.modal_text_normal },
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }

        // Draw Card Borders on top of everything (left, right, bottom)
        ui.push_quad(
            vertices,
            indices,
            menu_x,
            ui.titlebar_height,
            1.0,
            dropdown_h,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            menu_x + dropdown_w - 1.0,
            ui.titlebar_height,
            1.0,
            dropdown_h,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            menu_x,
            ui.titlebar_height + dropdown_h - 1.0,
            dropdown_w,
            1.0,
            white_uv,
            ui.config.theme.modal_border,
        );
    }

    // --- 7. Draw Modal Dialogs (On top of dropdowns/everything) ---
    if let Some(modal) = ui.active_modal {
        // Semi-transparent black background overlay
        ui.push_quad(
            vertices,
            indices,
            0.0,
            0.0,
            width,
            height,
            white_uv,
            [0.0, 0.0, 0.0, 0.4],
        );
        let modal_w = match modal {
            ModalType::Settings => (45.0 * ui.ui_char_width).max(500.0).round(),
            ModalType::About => 520.0,
            ModalType::CommandPalette => (50.0 * ui.ui_char_width).max(500.0).round(),
            ModalType::UnsavedChanges => 520.0,
        };
        let modal_h = match modal {
            ModalType::Settings => {
                let row_height = (ui.ui_line_height * 2.2).round();
                (row_height * 8.2).max(430.0).round()
            }
            ModalType::About => 190.0,
            ModalType::CommandPalette => {
                let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
                let filtered_len = ui.get_filtered_commands().len();
                let visible_items = filtered_len.min(10);
                let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
                (header_h + visible_items as f32 * item_height + 15.0).round()
            }
            ModalType::UnsavedChanges => 200.0,
        };
        let modal_x = ((width - modal_w) / 2.0).round();
        let modal_y = ((height - modal_h) / 2.0).round();

        // Draw Modal Box Background
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            white_uv,
            ui.config.theme.modal_bg,
        );
        // Draw modal borders
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y,
            modal_w,
            1.0,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y + modal_h - 1.0,
            modal_w,
            1.0,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y,
            1.0,
            modal_h,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            modal_x + modal_w - 1.0,
            modal_y,
            1.0,
            modal_h,
            white_uv,
            ui.config.theme.modal_border,
        );

        match modal {
            ModalType::CommandPalette => {
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
            ModalType::UnsavedChanges => {
                let file_name = ui.tab_to_close
                    .and_then(|idx| tab_paths.get(idx).cloned())
                    .flatten()
                    .and_then(|p| Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_else(|| "untitled.txt".to_string());

                let title_text = "Unsaved Changes";
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    title_text,
                    modal_x + 20.0,
                    modal_y + 35.0,
                    ui.config.theme.modal_text_title,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );

                let mut truncated_name = file_name.clone();
                if truncated_name.chars().count() > 20 {
                    let prefix: String = truncated_name.chars().take(17).collect();
                    truncated_name = format!("{}...", prefix);
                }
                let msg_text = format!("'{}' has unsaved changes.", truncated_name);
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &msg_text,
                    modal_x + 20.0,
                    modal_y + 70.0,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );

                let msg_text_2 = "Save changes before closing?";
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    msg_text_2,
                    modal_x + 20.0,
                    modal_y + 92.0,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );

                let btn_w = 130.0f32;
                let btn_h = 34.0f32;
                let spacing = 15.0f32;

                let total_btn_block_w = 3.0 * btn_w + 2.0 * spacing;
                let start_btn_x = modal_x + ((modal_w - total_btn_block_w) / 2.0).round();
                let btn_y = modal_y + modal_h - btn_h - 20.0;

                let btn_labels = ["Save", "Don't Save", "Cancel"];
                for i in 0..3 {
                    let bx = start_btn_x + i as f32 * (btn_w + spacing);
                    let is_btn_hovered = mouse_x >= bx && mouse_x <= bx + btn_w && mouse_y >= btn_y && mouse_y <= btn_y + btn_h;

                    ui.push_quad(
                        vertices,
                        indices,
                        bx,
                        btn_y,
                        btn_w,
                        btn_h,
                        white_uv,
                        if is_btn_hovered { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg },
                    );
                    ui.push_quad(vertices, indices, bx, btn_y, btn_w, 1.0, white_uv, ui.config.theme.button_border);
                    ui.push_quad(vertices, indices, bx, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, ui.config.theme.button_border);
                    ui.push_quad(vertices, indices, bx, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);
                    ui.push_quad(vertices, indices, bx + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);

                    let label = btn_labels[i];
                    let label_w = label.chars().count() as f32 * ui.ui_char_width;
                    let tx = bx + ((btn_w - label_w) / 2.0).round();
                    let ty = (btn_y + btn_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

                    ui.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        label,
                        tx,
                        ty,
                        ui.config.theme.button_text,
                        ui.ui_font_size,
                        ui.ui_char_width,
                    );
                }
            }
            ModalType::About => {
                let title = "Garage";
                let title_font_sz = ui.ui_font_size * 1.5;
                let title_char_w = ui.ui_char_width * 1.5;
                let title_w = title.chars().count() as f32 * title_char_w;
                let title_x = modal_x + ((modal_w - title_w) / 2.0).round();
                
                // 1. Draw Title "Garage"
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    title,
                    title_x,
                    modal_y + 35.0,
                    ui.config.theme.modal_text_title,
                    title_font_sz,
                    title_char_w,
                );

                // 2. Draw thin divider line
                ui.push_quad(
                    vertices,
                    indices,
                    modal_x + 30.0,
                    modal_y + 55.0,
                    modal_w - 60.0,
                    1.0,
                    white_uv,
                    ui.config.theme.tabbar_border,
                );

                // 3. Draw description
                let desc = "A supercharged GPU-accelerated text editor.";
                let desc_w = desc.chars().count() as f32 * ui.ui_char_width;
                let desc_x = modal_x + ((modal_w - desc_w) / 2.0).round();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    desc,
                    desc_x,
                    modal_y + 80.0,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );

                // 4. Version
                let version = "Version 0.1.0 (main)";
                let version_w = version.chars().count() as f32 * ui.ui_char_width * 0.9;
                let version_x = modal_x + ((modal_w - version_w) / 2.0).round();
                let mut muted_text_color = ui.config.theme.modal_text_normal;
                muted_text_color[3] *= 0.6; // Mute color alpha
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    version,
                    version_x,
                    modal_y + 115.0,
                    muted_text_color,
                    ui.ui_font_size * 0.9,
                    ui.ui_char_width * 0.9,
                );
            }
            ModalType::Settings => {
                let row_height = (ui.ui_line_height * 2.2).round();
                let control_x = modal_x + 24.0 * ui.ui_char_width;
                let btn_h = (ui.ui_line_height * 1.3).round().max(24.0);
                let btn_w = (ui.ui_char_width * 3.0).round().max(24.0);
                let backend_btn_w = (ui.ui_char_width * 10.0).round().max(80.0);
                let theme_btn_w = (ui.ui_char_width * 16.0).round().max(140.0);
                let padding_x = 2.0 * ui.ui_char_width;

                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "SETTINGS",
                    modal_x + padding_x,
                    modal_y + (ui.ui_line_height * 1.8).round(),
                    ui.config.theme.modal_text_title,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );

                // Helper closure to draw button container with borders and label
                let mut draw_button = |
                    vertices: &mut Vec<Vertex>,
                    indices: &mut Vec<u16>,
                    atlas: &mut FontAtlas,
                    queue: &wgpu::Queue,
                    text: &str,
                    bx: f32,
                    by: f32,
                    bw: f32,
                    bh: f32,
                    is_selected: bool,
                    is_hovered: bool,
                    theme: &crate::config::Theme,
                    white_uv: [f32; 2],
                    ui_char_width: f32,
                    ui_font_ascent: f32,
                    ui_font_size: f32,
                | {
                    let bg_color = if is_selected {
                        theme.cursor_color // brand color
                    } else if is_hovered {
                        theme.button_hover_bg
                    } else {
                        theme.button_bg
                    };
                    let border_color = if is_selected {
                        theme.cursor_color
                    } else {
                        theme.button_border
                    };
                    let text_color = if is_selected {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        theme.button_text
                    };

                    // Draw background
                    ui.push_quad(vertices, indices, bx, by, bw, bh, white_uv, bg_color);
                    // Draw borders (contiguous)
                    ui.push_quad(vertices, indices, bx, by, bw, 1.0, white_uv, border_color); // Top
                    ui.push_quad(vertices, indices, bx, by + bh - 1.0, bw, 1.0, white_uv, border_color); // Bottom
                    ui.push_quad(vertices, indices, bx, by, 1.0, bh, white_uv, border_color); // Left
                    ui.push_quad(vertices, indices, bx + bw - 1.0, by, 1.0, bh, white_uv, border_color); // Right

                    // Draw text centered
                    let text_w = text.chars().count() as f32 * ui_char_width;
                    let text_x = bx + ((bw - text_w) / 2.0).round();
                    let text_y = (by + bh / 2.0 + ui_font_ascent / 2.0 - 2.0).round();
                    ui.push_str(vertices, indices, atlas, queue, text, text_x, text_y, text_color, ui_font_size, ui_char_width);
                };

                // 1. Editor Font Size Settings
                let row1_y = modal_y + row_height * 1.0;
                let btn1_y = row1_y + ((ui.ui_line_height - btn_h) / 2.0).round();
                let font_size_str = format!("Editor Font: {:.1} px", ui.buffer_font_size);
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &font_size_str,
                    modal_x + padding_x,
                    row1_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
                let dec_hover = mouse_x >= control_x && mouse_x <= control_x + btn_w && mouse_y >= btn1_y && mouse_y <= btn1_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "-", control_x, btn1_y, btn_w, btn_h, false, dec_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);
                let inc_btn_x = control_x + btn_w + ui.ui_char_width;
                let inc_hover = mouse_x >= inc_btn_x && mouse_x <= inc_btn_x + btn_w && mouse_y >= btn1_y && mouse_y <= btn1_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "+", inc_btn_x, btn1_y, btn_w, btn_h, false, inc_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                // 2. UI Font Size Settings
                let row2_y = modal_y + row_height * 2.0;
                let btn2_y = row2_y + ((ui.ui_line_height - btn_h) / 2.0).round();
                let ui_size_str = format!("UI Font:     {:.1} px", ui.ui_font_size);
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &ui_size_str,
                    modal_x + padding_x,
                    row2_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
                let ui_dec_hover = mouse_x >= control_x && mouse_x <= control_x + btn_w && mouse_y >= btn2_y && mouse_y <= btn2_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "-", control_x, btn2_y, btn_w, btn_h, false, ui_dec_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);
                let ui_inc_hover = mouse_x >= inc_btn_x && mouse_x <= inc_btn_x + btn_w && mouse_y >= btn2_y && mouse_y <= btn2_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "+", inc_btn_x, btn2_y, btn_w, btn_h, false, ui_inc_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                // 3. Backend Selection
                let row3_y = modal_y + row_height * 3.0;
                let btn3_y = row3_y + ((ui.ui_line_height - btn_h) / 2.0).round();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "Backend:",
                    modal_x + padding_x,
                    row3_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
                let is_vulkan = ui.config.backend == "Vulkan";
                let vulkan_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn3_y && mouse_y <= btn3_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "Vulkan", control_x, btn3_y, backend_btn_w, btn_h, is_vulkan, vulkan_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                let is_opengl = ui.config.backend == "OpenGL";
                let opengl_btn_x = control_x + backend_btn_w + ui.ui_char_width;
                let opengl_hover = mouse_x >= opengl_btn_x && mouse_x <= opengl_btn_x + backend_btn_w && mouse_y >= btn3_y && mouse_y <= btn3_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "OpenGL", opengl_btn_x, btn3_y, backend_btn_w, btn_h, is_opengl, opengl_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                // 4. Theme Selection (Cycle Toggle Selector)
                let row4_y = modal_y + row_height * 4.0;
                let btn4_y = row4_y + ((ui.ui_line_height - btn_h) / 2.0).round();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "Theme:",
                    modal_x + padding_x,
                    row4_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
                let display_theme = format!("{}  ▼", ui.config.theme.name);
                let theme_hover = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= btn4_y && mouse_y <= btn4_y + btn_h;
                draw_button(vertices, indices, atlas, queue, &display_theme, control_x, btn4_y, theme_btn_w, btn_h, false, theme_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                // 5. Git Blame Selection
                let row5_y = modal_y + row_height * 5.0;
                let btn5_y = row5_y + ((ui.ui_line_height - btn_h) / 2.0).round();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "Git Blame:",
                    modal_x + padding_x,
                    row5_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
                let blame_enabled_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn5_y && mouse_y <= btn5_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "Enabled", control_x, btn5_y, backend_btn_w, btn_h, ui.config.show_git_blame, blame_enabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                let blame_disabled_x = control_x + backend_btn_w + ui.ui_char_width;
                let blame_disabled_hover = mouse_x >= blame_disabled_x && mouse_x <= blame_disabled_x + backend_btn_w && mouse_y >= btn5_y && mouse_y <= btn5_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "Disabled", blame_disabled_x, btn5_y, backend_btn_w, btn_h, !ui.config.show_git_blame, blame_disabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                // 6. Git Branch Selection
                let row6_y = modal_y + row_height * 6.0;
                let btn6_y = row6_y + ((ui.ui_line_height - btn_h) / 2.0).round();
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    "Git Branch:",
                    modal_x + padding_x,
                    row6_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_normal,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
                let branch_enabled_hover = mouse_x >= control_x && mouse_x <= control_x + backend_btn_w && mouse_y >= btn6_y && mouse_y <= btn6_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "Enabled", control_x, btn6_y, backend_btn_w, btn_h, ui.config.show_git_branch, branch_enabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                let branch_disabled_x = control_x + backend_btn_w + ui.ui_char_width;
                let branch_disabled_hover = mouse_x >= branch_disabled_x && mouse_x <= branch_disabled_x + backend_btn_w && mouse_y >= btn6_y && mouse_y <= btn6_y + btn_h;
                draw_button(vertices, indices, atlas, queue, "Disabled", branch_disabled_x, btn6_y, backend_btn_w, btn_h, !ui.config.show_git_branch, branch_disabled_hover, &ui.config.theme, white_uv, ui.ui_char_width, ui.ui_font_ascent, ui.ui_font_size);

                // 7. Draw Active backend and GPU info
                let row7_y = modal_y + row_height * 7.0;
                let backend_str = match current_backend {
                    wgpu::Backend::Vulkan => "Vulkan",
                    wgpu::Backend::Gl => "OpenGL",
                    other => &format!("{:?}", other),
                };
                let is_fallback = (ui.config.backend == "OpenGL" && current_backend != wgpu::Backend::Gl) ||
                                  (ui.config.backend == "Vulkan" && current_backend != wgpu::Backend::Vulkan);
                let active_info_str = if is_fallback {
                    format!("Active: {} (fallback) ({})", backend_str, ui.active_device_name)
                } else {
                    format!("Active: {} ({})", backend_str, ui.active_device_name)
                };
                ui.push_str(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    &active_info_str,
                    modal_x + padding_x,
                    row7_y + ui.ui_font_ascent,
                    ui.config.theme.modal_text_muted,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );

                // 8. Draw Theme Selection Dropdown (if open) - rendered on top of active info
                if ui.theme_dropdown_open {
                    let dropdown_y = btn4_y + btn_h;
                    let item_height = (ui.ui_line_height * 1.5).round().max(24.0);
                    let dropdown_h = 2.0 * item_height;

                    // Draw Dropdown background
                    ui.push_quad(vertices, indices, control_x, dropdown_y, theme_btn_w, dropdown_h, white_uv, ui.config.theme.modal_bg);
                    // Draw Dropdown borders
                    ui.push_quad(vertices, indices, control_x, dropdown_y, theme_btn_w, 1.0, white_uv, ui.config.theme.modal_border);
                    ui.push_quad(vertices, indices, control_x, dropdown_y + dropdown_h - 1.0, theme_btn_w, 1.0, white_uv, ui.config.theme.modal_border);
                    ui.push_quad(vertices, indices, control_x, dropdown_y, 1.0, dropdown_h, white_uv, ui.config.theme.modal_border);
                    ui.push_quad(vertices, indices, control_x + theme_btn_w - 1.0, dropdown_y, 1.0, dropdown_h, white_uv, ui.config.theme.modal_border);

                    let themes = ["Light Theme", "Dark Theme"];
                    for (idx, t_name) in themes.iter().enumerate() {
                        let item_y = dropdown_y + idx as f32 * item_height;
                        let is_item_hovered = mouse_x >= control_x && mouse_x <= control_x + theme_btn_w && mouse_y >= item_y && mouse_y < item_y + item_height;

                        if is_item_hovered {
                            ui.push_quad(vertices, indices, control_x + 1.0, item_y + 1.0, theme_btn_w - 2.0, item_height - 2.0, white_uv, ui.config.theme.button_hover_bg);
                        }

                        let text_y = (item_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
                        ui.push_str(
                            vertices,
                            indices,
                            atlas,
                            queue,
                            t_name,
                            control_x + 10.0,
                            text_y,
                            ui.config.theme.modal_text_normal,
                            ui.ui_font_size,
                            ui.ui_char_width,
                        );
                    }
                }
            }
        }

        if modal != ModalType::CommandPalette && modal != ModalType::UnsavedChanges {
            // Draw generic Close Button (centered horizontally)
            let btn_w = (12.0 * ui.ui_char_width).max(100.0).round();
            let btn_h = (ui.ui_line_height * 1.6).max(30.0).round();
            let btn_x = modal_x + ((modal_w - btn_w) / 2.0).round();
            let btn_y = modal_y + modal_h - btn_h - (ui.ui_line_height * 1.0).round();

            let close_btn_hover = mouse_x >= btn_x && mouse_x <= btn_x + btn_w && mouse_y >= btn_y && mouse_y <= btn_y + btn_h;
            ui.push_quad(
                vertices,
                indices,
                btn_x,
                btn_y,
                btn_w,
                btn_h,
                white_uv,
                if close_btn_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg },
            );
            // Draw borders
            ui.push_quad(vertices, indices, btn_x, btn_y, btn_w, 1.0, white_uv, ui.config.theme.button_border);
            ui.push_quad(vertices, indices, btn_x, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, ui.config.theme.button_border);
            ui.push_quad(vertices, indices, btn_x, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);
            ui.push_quad(vertices, indices, btn_x + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);

            let close_text = "Close";
            let close_text_w = close_text.chars().count() as f32 * ui.ui_char_width;
            let close_text_x = btn_x + ((btn_w - close_text_w) / 2.0).round();
            let close_text_y = (btn_y + btn_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                close_text,
                close_text_x,
                close_text_y,
                ui.config.theme.button_text,
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }
    }
}
