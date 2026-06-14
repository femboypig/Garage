use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::ui::{UiState, UiAction};
use crate::renderer::atlas::FontAtlas;
use crate::app::state::AppState;
use crate::app::handler::handle_action;

pub fn update_cursor_icon(window: &Window, ui: &UiState, state: &AppState) {
    if state.is_actually_dragging_tab() {
        window.set_cursor_icon(winit::window::CursorIcon::Grabbing);
        return;
    }
    let size = window.inner_size();
    let mouse_x = state.mouse_x;
    let mouse_y = state.mouse_y;

    // Check status bar first
    let status_y = size.height as f32 - ui.status_height;
    let is_on_statusbar_item = if ui.active_modal.is_none() && mouse_y >= status_y {
        let sb_btn_w = 28.0f32;
        let term_btn_x = size.width as f32 - 10.0 - sb_btn_w;

        // 1. Check Terminal toggle button
        if mouse_x >= term_btn_x && mouse_x < term_btn_x + sb_btn_w {
            true
        } else if state.tabs.is_empty() {
            false
        } else {
            // Calculate left side (diagnostics)
            let mut pen_x = 10.0;
            if ui.config.show_git_branch {
                if let Some(ref branch) = ui.git_branch {
                    let icon_sz = (ui.ui_font_size * 0.9).round().max(12.0);
                    pen_x += icon_sz + 4.0;
                    let branch_len = branch.chars().count() as f32;
                    pen_x += branch_len * ui.ui_char_width;
                    pen_x += 15.0;
                }
            }
            let mut err_count = 0;
            let mut warn_count = 0;
            for (e, w) in ui.lsp_diagnostics.values() {
                err_count += *e;
                warn_count += *w;
            }
            let err_val_str = format!("{}", err_count);
            let warn_val_str = format!("{}", warn_count);
            let err_icon_sz = 14.0f32;
            let warn_icon_sz = 14.0f32;
            let err_text_w = err_val_str.chars().count() as f32 * ui.ui_char_width;
            let warn_text_w = warn_val_str.chars().count() as f32 * ui.ui_char_width;
            let diag_w = err_icon_sz + 4.0 + err_text_w + 12.0 + warn_icon_sz + 4.0 + warn_text_w;

            if mouse_x >= pen_x && mouse_x <= pen_x + diag_w {
                true
            } else {
                // Check right-hand items (Language & Encoding)
                let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                let active_tab_idx = state.active_tab_idx.min(state.tabs.len().saturating_sub(1));
                let raw_ext = tab_paths.get(active_tab_idx).and_then(|p| p.as_ref())
                    .and_then(|p| std::path::Path::new(p).extension())
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");
                let mut extension = raw_ext.to_string();
                if let Some(path) = tab_paths.get(active_tab_idx).and_then(|p| p.as_ref()) {
                    if let Some(forced_ext) = ui.forced_languages.get(path) {
                        extension = forced_ext.clone();
                    }
                }

                let language = ui.languages.get(&extension)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        if extension.is_empty() {
                            "Plain Text".to_string()
                        } else {
                            let mut chars = extension.chars();
                            match chars.next() {
                                None => "Plain Text".to_string(),
                                Some(first) => {
                                    let mut s = first.to_uppercase().to_string();
                                    s.push_str(&chars.as_str().to_lowercase());
                                    s
                                }
                            }
                        }
                    });

                let encoding = tab_paths.get(active_tab_idx).and_then(|p| p.as_ref())
                    .and_then(|path| ui.forced_encodings.get(path))
                    .map(|s| s.as_str())
                    .unwrap_or("UTF-8");

                let cursor = &state.tabs[active_tab_idx].cursor;
                let cursor_str = format!("Ln {}, Col {}", cursor.line + 1, cursor.col + 1);

                let mut cur_right_x = term_btn_x - 10.0;
                
                // First component: Cursor position
                let cursor_w = cursor_str.chars().count() as f32 * ui.ui_char_width;
                cur_right_x -= cursor_w + 16.0;

                // Second component: Language
                let lang_w = language.chars().count() as f32 * ui.ui_char_width;
                let lang_left = cur_right_x - lang_w - 16.0;
                let lang_right = cur_right_x;
                cur_right_x -= lang_w + 16.0;

                // Third component: Encoding
                let enc_w = encoding.chars().count() as f32 * ui.ui_char_width;
                let enc_left = cur_right_x - enc_w - 16.0;
                let enc_right = cur_right_x;

                if mouse_x >= lang_left && mouse_x < lang_right {
                    true
                } else if mouse_x >= enc_left && mouse_x < enc_right {
                    true
                } else {
                    false
                }
            }
        }
    } else {
        false
    };

    if is_on_statusbar_item {
        window.set_cursor_icon(winit::window::CursorIcon::Pointer);
        return;
    }

    if state.tabs.is_empty() {
        window.set_cursor_icon(winit::window::CursorIcon::Default);
        return;
    }

    let sidebar_original = ui.sidebar_width;
    let mut sidebar_width = ui.sidebar_width;
    let mut w_width = size.width as f32;

    let hovered_pane_idx = if state.inactive_panes.is_empty() {
        0
    } else {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        if mouse_x < sidebar_original + pane_width { 0 } else { 1 }
    };

    if !state.inactive_panes.is_empty() {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        if hovered_pane_idx == 0 {
            w_width = sidebar_original + pane_width;
        } else {
            sidebar_width = sidebar_original + pane_width;
        }
    }

    let (hovered_tabs, hovered_active_tab_idx) = if hovered_pane_idx == state.active_pane_idx {
        (&state.tabs, state.active_tab_idx)
    } else {
        (&state.inactive_panes[0].tabs, state.inactive_panes[0].active_tab_idx)
    };

    if hovered_tabs.is_empty() {
        window.set_cursor_icon(winit::window::CursorIcon::Default);
        return;
    }

    let active_tab = &hovered_tabs[hovered_active_tab_idx.min(hovered_tabs.len() - 1)];
    let buffer = &active_tab.buffer;
    let is_diagnostics = active_tab.path.as_deref().map_or(false, |p| p.starts_with("diagnostics://"));
    let max_line_digits = buffer.len().to_string().len().max(3);
    let gutter_width = if is_diagnostics { 0.0 } else { (max_line_digits as f32 + 2.0) * ui.buffer_char_width };
    let activity_bar_width = 0.0;
    let text_area_x = activity_bar_width + sidebar_width + gutter_width;
    
    let scrollbar_width = ui.scrollbar_width();
    let minimap_width = if is_diagnostics { 0.0 } else { ui.minimap_width() };
    let sb_x = w_width - scrollbar_width;
    let minimap_x = sb_x - minimap_width;

    let on_sidebar_border = sidebar_original > 0.0 && (mouse_x - sidebar_original).abs() <= 4.0;
    
    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let on_dock_border = ui.show_dock && (mouse_y - dock_start_y).abs() <= 4.0;
    
    let mut pane_top = main_y;
    let mut pane_bottom = dock_start_y;
    if !state.inactive_panes.is_empty() && state.is_split_horizontal {
        let editor_area_height = dock_start_y - main_y;
        let pane_height = (editor_area_height / 2.0).round();
        if hovered_pane_idx == 0 {
            pane_bottom = main_y + pane_height;
        } else {
            pane_top = main_y + pane_height;
        }
    }

    if on_sidebar_border {
        window.set_cursor_icon(winit::window::CursorIcon::ColResize);
    } else if on_dock_border {
        window.set_cursor_icon(winit::window::CursorIcon::RowResize);
    } else {
        let is_in_editor = ui.active_modal.is_none()
            && ui.active_menu.is_none()
            && mouse_x >= text_area_x
            && mouse_x < minimap_x
            && mouse_y >= pane_top + ui.tabbar_height + ui.breadcrumb_height
            && mouse_y < pane_bottom - 14.0;
            
        if is_in_editor {
            window.set_cursor_icon(winit::window::CursorIcon::Text);
        } else {
            let mut is_pointer = false;
            
            // 1. Sidebar items (only actual node items, not empty space below)
            let sidebar_item_count = 1 + ui.visible_nodes.len();
            let sidebar_height_limit = ui.titlebar_height + (sidebar_item_count as f32 - ui.sidebar_scroll as f32) * ui.ui_line_height;
            if sidebar_original > 0.0 && mouse_x < sidebar_original && mouse_y >= ui.titlebar_height && mouse_y < sidebar_height_limit.min(dock_start_y) {
                is_pointer = true;
            }
            // 2. Tabbar (only over actual tabs, not empty space to the right)
            else if mouse_y >= pane_top && mouse_y < pane_top + ui.tabbar_height {
                // Calculate which pane and active tab widths
                let mut in_tab_area = false;
                if state.inactive_panes.is_empty() {
                    // Single pane
                    let start_x = ui.sidebar_width;
                    let mut total_tabs_width = 0.0f32;
                    let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
                    let close_reserved = 8.0f32 + tab_close_icon_sz;
                    for t in &state.tabs {
                        let file_name = t.path.as_ref()
                            .and_then(|p| std::path::Path::new(p).file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "untitled.txt".to_string());
                        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                        let dot_reserved = 18.0f32;
                        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                        total_tabs_width += tab_w;
                    }
                    if mouse_x >= start_x && mouse_x < (start_x + total_tabs_width - ui.tab_scroll_x).min(size.width as f32) {
                        in_tab_area = true;
                    }
                } else {
                    // Split panes (Left/Right or Top/Bottom)
                    let (start_x_0, end_x_0, start_x_1, end_x_1) = if state.is_split_horizontal {
                        (sidebar_original, size.width as f32, sidebar_original, size.width as f32)
                    } else {
                        let editor_area_width = size.width as f32 - sidebar_original;
                        let pane_width = editor_area_width / 2.0;
                        (sidebar_original, sidebar_original + pane_width, sidebar_original + pane_width, size.width as f32)
                    };

                    let (tabs_0, scroll_x_0) = if state.active_pane_idx == 0 {
                        (&state.tabs, ui.tab_scroll_x)
                    } else {
                        (&state.inactive_panes[0].tabs, 0.0)
                    };

                    let (tabs_1, scroll_x_1) = if state.active_pane_idx == 1 {
                        (&state.tabs, ui.tab_scroll_x)
                    } else {
                        (&state.inactive_panes[0].tabs, 0.0)
                    };

                    if hovered_pane_idx == 0 {
                        let mut total_w_0 = 0.0f32;
                        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
                        let close_reserved = 8.0f32 + tab_close_icon_sz;
                        for t in tabs_0 {
                            let file_name = t.path.as_ref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "untitled.txt".to_string());
                            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                            let dot_reserved = 18.0f32;
                            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                            total_w_0 += tab_w;
                        }
                        if mouse_x >= start_x_0 && mouse_x < (start_x_0 + total_w_0 - scroll_x_0).min(end_x_0) {
                            in_tab_area = true;
                        }
                    } else {
                        let mut total_w_1 = 0.0f32;
                        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
                        let close_reserved = 8.0f32 + tab_close_icon_sz;
                        for t in tabs_1 {
                            let file_name = t.path.as_ref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "untitled.txt".to_string());
                            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                            let dot_reserved = 18.0f32;
                            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                            total_w_1 += tab_w;
                        }
                        if mouse_x >= start_x_1 && mouse_x < (start_x_1 + total_w_1 - scroll_x_1).min(end_x_1) {
                            in_tab_area = true;
                        }
                    }
                }
                if in_tab_area {
                    is_pointer = true;
                }
            }
            // 3. Titlebar Menu (only menu labels, not drag area)
            else if mouse_y < ui.titlebar_height {
                let menu_items = ["Garage", "File", "Edit", "Selection", "View"];
                let mut menu_width = 0.0f32;
                for (i, label) in menu_items.iter().enumerate() {
                    let label_len = label.chars().count() as f32;
                    let text_w = label_len * ui.ui_char_width;
                    let (left_pad, right_pad) = if i == 0 { (14.0, 10.0) } else { (10.0, 10.0) };
                    menu_width += text_w + left_pad + right_pad;
                }
                let max_drag_x = if ui.is_tiling_wm() {
                    size.width as f32
                } else {
                    size.width as f32 - 135.0
                };
                if mouse_x < menu_width || mouse_x >= max_drag_x {
                    is_pointer = true;
                }
            }
            // 4. Scrollbar thumb/track only (excluding minimap itself)
            else if mouse_x >= sb_x && mouse_x < w_width && mouse_y >= ui.titlebar_height + ui.tabbar_height && mouse_y < dock_start_y {
                is_pointer = true;
            }
            // 5. Active modal interactive areas
            else if let Some(modal) = ui.active_modal {
                let modal_w = match modal {
                    crate::ui::ModalType::Settings => (45.0 * ui.ui_char_width).max(500.0).round(),
                    crate::ui::ModalType::About => 520.0,
                    crate::ui::ModalType::CommandPalette => (50.0 * ui.ui_char_width).max(500.0).round(),
                    crate::ui::ModalType::UnsavedChanges => 520.0,
                    crate::ui::ModalType::SidebarInput => 400.0,
                    crate::ui::ModalType::GlobalSearch => 650.0,
                };
                let modal_h = match modal {
                    crate::ui::ModalType::Settings => {
                        let row_height = (ui.ui_line_height * 2.2).round();
                        (row_height * 8.2).max(430.0).round()
                    }
                    crate::ui::ModalType::About => 190.0,
                    crate::ui::ModalType::CommandPalette => {
                        let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
                        let filtered_len = ui.get_filtered_commands().len();
                        let visible_items = filtered_len.min(10);
                        let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
                        (header_h + visible_items as f32 * item_height).round()
                    }
                    crate::ui::ModalType::UnsavedChanges => 200.0,
                    crate::ui::ModalType::SidebarInput => 150.0,
                    crate::ui::ModalType::GlobalSearch => {
                        let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
                        let count = ui.global_search_results.len().min(10).max(1);
                        let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
                        (header_h + count as f32 * item_height).round()
                    }
                };
                let modal_x = ((size.width as f32 - modal_w) / 2.0).round();
                let modal_y = ((size.height as f32 - modal_h) / 2.0).round();
                
                if mouse_x >= modal_x && mouse_x <= modal_x + modal_w && mouse_y >= modal_y && mouse_y <= modal_y + modal_h {
                    if modal == crate::ui::ModalType::CommandPalette || modal == crate::ui::ModalType::GlobalSearch {
                        let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
                        if mouse_y >= modal_y + header_h {
                            is_pointer = true;
                        }
                    } else {
                        is_pointer = true;
                    }
                }
            }
            
            if is_pointer {
                window.set_cursor_icon(winit::window::CursorIcon::Pointer);
            } else {
                window.set_cursor_icon(winit::window::CursorIcon::Default);
            }
        }
    }
}

pub fn handle_cursor_moved(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    position_x: f32,
    position_y: f32,
) {
    if !state.tabs.is_empty() {
        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
    }
    for p in &mut state.inactive_panes {
        if !p.tabs.is_empty() {
            p.active_tab_idx = p.active_tab_idx.min(p.tabs.len() - 1);
        }
    }
    state.mouse_x = position_x;
    state.mouse_y = position_y;
    update_cursor_icon(window, ui, state);
 
    let size = window.inner_size();
    ui.hover_pos = None;
    ui.hover_start = None;
    ui.hovered_diagnostic = None;
    ui.mouse_in_popup = false;
    ui.hovered_copy_button = false;
    
    let sidebar_original = ui.sidebar_width;
    let mut sidebar_width = ui.sidebar_width;
    let mut w_width = size.width as f32;
    
    if !state.inactive_panes.is_empty() && !state.is_split_horizontal {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        
        if state.active_pane_idx == 0 {
            w_width = sidebar_original + pane_width;
        } else {
            sidebar_width = sidebar_original + pane_width;
        }
    }

    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let editor_bottom_limit = if ui.show_dock {
        dock_start_y
    } else {
        size.height as f32 - ui.status_height
    };

    let mut pane_top = main_y;
    let mut pane_bottom = editor_bottom_limit;

    if !state.inactive_panes.is_empty() && state.is_split_horizontal {
        let editor_area_height = editor_bottom_limit - main_y;
        let pane_height = (editor_area_height / 2.0).round();
        if state.active_pane_idx == 0 {
            pane_bottom = main_y + pane_height;
        } else {
            pane_top = main_y + pane_height;
        }
    }
 
    if let Some(dragged_idx) = state.dragged_tab_idx {
        if state.is_actually_dragging_tab() {
            let is_inside_tabbar = state.mouse_y >= pane_top && state.mouse_y < pane_top + ui.tabbar_height;
            if is_inside_tabbar {
                let tabbar_start_x = sidebar_width;
                let mut tab_widths = Vec::new();
                let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
                let close_reserved = 8.0f32 + tab_close_icon_sz;
                let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
                for idx in 0..tab_paths.len() {
                    let path_opt = &tab_paths[idx];
                    let file_name = path_opt.as_ref()
                        .and_then(|p| std::path::Path::new(p).file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "untitled.txt".to_string());
                    let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                    let dot_reserved = 18.0f32;
                    let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                    tab_widths.push(tab_w);
                }

                let mut hovered_idx = None;
                let mut current_tab_x = tabbar_start_x;
                for idx in 0..state.tabs.len() {
                    let tab_w = tab_widths[idx];
                    let draw_x = current_tab_x - ui.tab_scroll_x;
                    let clip_left = draw_x.max(tabbar_start_x);
                    let clip_right = (draw_x + tab_w).min(w_width);
                    
                    if clip_left < clip_right && state.mouse_x >= clip_left && state.mouse_x < clip_right {
                        hovered_idx = Some(idx);
                        break;
                    }
                    current_tab_x += tab_w;
                }

                if let Some(h_idx) = hovered_idx {
                    if h_idx != dragged_idx {
                        let tab = state.tabs.remove(dragged_idx);
                        state.tabs.insert(h_idx, tab);
                        state.dragged_tab_idx = Some(h_idx);
                        state.active_tab_idx = h_idx;
                        window.request_redraw();
                    }
                }
            }
        }
    } else if state.is_dragging_sidebar {
        let new_width = if state.mouse_x < 30.0 { 0.0 } else { state.mouse_x.clamp(50.0, 600.0) };
        ui.sidebar_width = new_width;
        ui.target_sidebar_width = new_width;
    } else if ui.tab_scroll_is_dragging {
        let tabbar_start_x = sidebar_width;
        let visible_width = w_width - tabbar_start_x;
        
        let mut total_tabs_width = 0.0f32;
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = path_opt.as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string());
            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
            let dot_reserved = 18.0f32;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
            total_tabs_width += tab_w;
        }
 
        let ratio = visible_width / total_tabs_width;
        let thumb_w = (visible_width * ratio).clamp(20.0_f32.min(visible_width), visible_width);
        let max_scroll_x = (total_tabs_width - visible_width).max(0.0);
 
        if max_scroll_x > 0.0 {
            let target_thumb_x = state.mouse_x - state.scroll_drag_offset_x;
            let scroll_range = visible_width - thumb_w;
            let scroll_ratio = if scroll_range > 0.0 {
                ((target_thumb_x - tabbar_start_x) / scroll_range).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.tab_scroll_x = scroll_ratio * max_scroll_x;
        } else {
            ui.tab_scroll_x = 0.0;
        }
    } else if state.is_dragging_dock_border {
        let main_y = ui.titlebar_height;
        let max_y = size.height as f32 - ui.status_height - 50.0;
        let min_y = main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0;
        let target_y = state.mouse_y.clamp(min_y, max_y);
        let new_height = size.height as f32 - ui.status_height - target_y;
        ui.dock_height = new_height;
    } else {
        ui.sidebar_width = sidebar_width;
        if state.is_dragging_scroll {
            let active_path = state.tabs[state.active_tab_idx].path.as_deref().unwrap_or("");
            let is_diagnostics = active_path.starts_with("diagnostics://");
     
            let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
            let status_y = pane_bottom.round();
     
            let show_horizontal_scrollbar = if is_diagnostics {
                false
            } else {
                let max_line_len = ui.get_max_line_len(&state.tabs[state.active_tab_idx].buffer, Some(active_path), state.tabs[state.active_tab_idx].cursor.line);
                let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
                let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                let text_area_x = ui.sidebar_width + gutter_width;
                let scrollbar_width = ui.scrollbar_width();
                let minimap_width = ui.minimap_width();
                let sb_x = w_width - scrollbar_width;
                let minimap_x = sb_x - minimap_width;
                let text_viewport_w = (minimap_x - text_area_x).max(10.0);
                let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
                max_line_len > visible_cols
            };
            let hs_height = if show_horizontal_scrollbar { 14.0 } else { 0.0 };
            let editor_height = status_y - editor_top - hs_height;
            let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
     
            let virtual_len = if is_diagnostics {
                let mut count = 0;
                for (file_path, diags) in &ui.lsp_diagnostics_details {
                    if diags.is_empty() {
                        continue;
                    }
                    let file_lines_len = ui.diagnostics_file_cache.get(file_path).map(|l| l.len()).unwrap_or(0);
                    for diag in diags {
                        let start_line = diag.line.saturating_sub(3);
                        let end_line = if file_lines_len > 0 {
                            (diag.line + 3).min(file_lines_len - 1)
                        } else {
                            diag.line + 3
                        };
                        let num_code_lines = end_line - start_line + 1;
                        count += 1 + num_code_lines + 1; // Header + Code lines + Banner
                    }
                }
                count.max(1)
            } else {
                state.tabs[state.active_tab_idx].buffer.len()
            };
     
            let ratio = visible_lines as f32 / virtual_len as f32;
            let thumb_h = (editor_height * ratio).clamp(20.0_f32.min(editor_height), editor_height);
            let max_scroll = (virtual_len as isize - visible_lines as isize).max(0) as f32;
            let relative_y = state.mouse_y - editor_top - state.scroll_drag_offset_y;
            let scroll_range = editor_height - thumb_h;
            let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
            ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
        } else if state.is_dragging_horizontal_scroll {
            let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
            let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
            let text_area_x = ui.sidebar_width + gutter_width;
            let scrollbar_width = ui.scrollbar_width();
            let minimap_width = ui.minimap_width();
            let sb_x = w_width - scrollbar_width;
            let minimap_x = sb_x - minimap_width;
            let text_viewport_w = (minimap_x - text_area_x).max(10.0);
     
            let max_line_len = ui.get_max_line_len(&state.tabs[state.active_tab_idx].buffer, state.tabs[state.active_tab_idx].path.as_deref(), state.tabs[state.active_tab_idx].cursor.line);
            let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
            let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
            let thumb_w = (text_viewport_w * ratio_x).clamp(20.0_f32.min(text_viewport_w), text_viewport_w);
            let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
            let relative_x = state.mouse_x - text_area_x - state.scroll_drag_offset_x;
            let scroll_range = text_viewport_w - thumb_w;
            let scroll_ratio = if scroll_range > 0.0 { (relative_x / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
            ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
        } else if state.is_dragging_minimap {
            let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
            let status_y = pane_bottom.round();
            let total_editor_height = status_y - editor_top;
            let editor_height = total_editor_height - 14.0;
            let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
            let max_scroll = (state.tabs[state.active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0) as f32;
            let relative_y = state.mouse_y - editor_top;
            
            let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
            let minimap_total_h = state.tabs[state.active_tab_idx].buffer.len() as f32 * minimap_line_height;
            
            let active_tab = &mut state.tabs[state.active_tab_idx];
            let clicked_line = if minimap_total_h > total_editor_height {
                let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
                (scroll_ratio * (active_tab.buffer.len() - 1) as f32).round() as usize
            } else {
                (relative_y / minimap_line_height).floor() as usize
            };
            let clicked_line = clicked_line.min(active_tab.buffer.len() - 1);
            
            active_tab.cursor.line = clicked_line;
            let line_chars = active_tab.buffer.lines()[clicked_line].chars().count();
            active_tab.cursor.col = active_tab.cursor.col.min(line_chars);
            active_tab.cursor.intended_col = active_tab.cursor.col;
            
            ui.scroll_y = clicked_line.saturating_sub(visible_lines / 2).min(max_scroll as usize);
        } else if state.is_dragging {
            let is_diagnostics = state.tabs[state.active_tab_idx].path.as_deref().map_or(false, |p| p.starts_with("diagnostics://"));
            let max_line_digits = if is_diagnostics { 3 } else { state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3) };
            let gutter_width = if is_diagnostics { 0.0 } else { (max_line_digits as f32 + 2.0) * ui.buffer_char_width };
            let text_area_x = ui.sidebar_width + gutter_width;
            let scrollbar_width = ui.scrollbar_width();
            let minimap_width = if is_diagnostics { 0.0 } else { ui.minimap_width() };
            let sb_x = w_width - scrollbar_width;
            let minimap_x = sb_x - minimap_width;
      
            let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
            let raw_line_idx = if state.mouse_y >= editor_top {
                ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y
            } else {
                ui.scroll_y
            };
     
            let line_idx = if is_diagnostics {
                let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                if visual_lines.is_empty() {
                    0
                } else {
                    raw_line_idx.min(visual_lines.len() - 1)
                }
            } else {
                raw_line_idx.min(state.tabs[state.active_tab_idx].buffer.len().saturating_sub(1))
            };
      
            let mouse_x_clamped = state.mouse_x.min(minimap_x);
            let col_idx = if mouse_x_clamped > text_area_x {
                ((mouse_x_clamped - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x
            } else {
                0
            };
     
            let line_chars = if is_diagnostics {
                let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                visual_lines.get(line_idx).map_or(0, |vl| match vl {
                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
                })
            } else {
                state.tabs[state.active_tab_idx].buffer.lines()[line_idx].chars().count()
            };
            let col_idx = col_idx.min(line_chars);
      
            state.tabs[state.active_tab_idx].cursor.line = line_idx;
            state.tabs[state.active_tab_idx].cursor.col = col_idx;
            state.tabs[state.active_tab_idx].cursor.intended_col = col_idx;
     
            ui.scroll_to_cursor(&state.tabs[state.active_tab_idx].cursor, state.tabs[state.active_tab_idx].buffer.len(), w_width, size.height as f32);
        }
        ui.sidebar_width = sidebar_original;
    } 
    let any_dragging = state.is_dragging_sidebar
        || state.is_dragging_dock_border
        || state.is_dragging_scroll
        || state.is_dragging_horizontal_scroll
        || state.is_dragging_minimap
        || state.is_dragging
        || ui.tab_scroll_is_dragging
        || state.dragged_tab_idx.is_some();

    if any_dragging {
        window.request_redraw();
    }

    update_cursor_icon(window, ui, state);
}

struct SidebarGuard {
    ptr: *mut f32,
    original_value: f32,
}

impl Drop for SidebarGuard {
    fn drop(&mut self) {
        unsafe {
            *self.ptr = self.original_value;
        }
    }
}

pub fn handle_mouse_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    input_state: ElementState,
    button: MouseButton,
) {
    if !state.tabs.is_empty() {
        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
    }
    for p in &mut state.inactive_panes {
        if !p.tabs.is_empty() {
            p.active_tab_idx = p.active_tab_idx.min(p.tabs.len() - 1);
        }
    }
    let size = window.inner_size();
    let sidebar_original = ui.sidebar_width;
    let mut sidebar_width = ui.sidebar_width;
    let mut w_width = size.width as f32;

    if input_state == ElementState::Pressed {
        if !state.inactive_panes.is_empty() {
            let main_y = ui.titlebar_height;
            let mut dock_start_y = size.height as f32 - ui.status_height;
            if ui.show_dock {
                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
            }
            let editor_bottom_limit = if ui.show_dock {
                dock_start_y
            } else {
                size.height as f32 - ui.status_height
            };
            
            // Switch focus only if click is inside the editor area and outside sidebar
            if state.mouse_x >= sidebar_original && state.mouse_y >= main_y && state.mouse_y < editor_bottom_limit {
                let clicked_pane_idx = if state.is_split_horizontal {
                    let editor_area_height = editor_bottom_limit - main_y;
                    let pane_height = (editor_area_height / 2.0).round();
                    if state.mouse_y < main_y + pane_height { 0 } else { 1 }
                } else {
                    let editor_area_width = size.width as f32 - sidebar_original;
                    let pane_width = editor_area_width / 2.0;
                    let divider_x = sidebar_original + pane_width;
                    if state.mouse_x < divider_x { 0 } else { 1 }
                };
                if clicked_pane_idx != state.active_pane_idx {
                    state.switch_pane(clicked_pane_idx);
                    if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
                        ui.scroll_x = active_tab.scroll_x;
                        ui.scroll_y = active_tab.scroll_y;
                    }
                    // Recalculate w_width and sidebar_width since active_pane_idx changed!
                    if state.is_split_horizontal {
                        w_width = size.width as f32;
                        sidebar_width = sidebar_original;
                    } else {
                        let editor_area_width = size.width as f32 - sidebar_original;
                        let pane_width = editor_area_width / 2.0;
                        if state.active_pane_idx == 0 {
                            w_width = sidebar_original + pane_width;
                            sidebar_width = sidebar_original;
                        } else {
                            sidebar_width = sidebar_original + pane_width;
                            w_width = size.width as f32;
                        }
                    }
                    ui.sidebar_width = sidebar_width;
                }
            }
        }
    }

    if !state.inactive_panes.is_empty() && !state.is_split_horizontal {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        if state.active_pane_idx == 0 {
            w_width = sidebar_original + pane_width;
        } else {
            sidebar_width = sidebar_original + pane_width;
        }
    }

    ui.sidebar_width = sidebar_width;
    let _sidebar_guard = SidebarGuard {
        ptr: &mut ui.sidebar_width as *mut f32,
        original_value: sidebar_original,
    };

    if button == MouseButton::Right && input_state == ElementState::Pressed {
        ui.active_menu = None;
        ui.sidebar_context_menu = None;
        let size = window.inner_size();
        let main_y = ui.titlebar_height;
        if ui.sidebar_width > 0.0 && state.mouse_x >= 0.0 && state.mouse_x < ui.sidebar_width && state.mouse_y > main_y && state.mouse_y < size.height as f32 - ui.status_height {
            let tree_y = state.mouse_y - main_y;
            let row_idx = (tree_y / ui.ui_line_height).floor() as usize;
            let r = row_idx + ui.sidebar_scroll;
            
            let mut target_path = std::path::PathBuf::from(".");
            let mut is_dir = true;
            if r >= 1 {
                let node_idx = r - 1;
                if node_idx < ui.visible_nodes.len() {
                    target_path = ui.visible_nodes[node_idx].path.clone();
                    is_dir = ui.visible_nodes[node_idx].is_dir;
                }
            }
            ui.sidebar_context_menu = Some((state.mouse_x, state.mouse_y, target_path, is_dir));
            window.request_redraw();
            return;
        }
    }

    if button == MouseButton::Left {
        let size = window.inner_size();
        if input_state == ElementState::Pressed {
            // Check sidebar context menu click
            if let Some((menu_x, menu_y, target_path, _is_dir)) = ui.sidebar_context_menu.clone() {
                ui.sidebar_context_menu = None;
                let menu_w = 120.0f32;
                let item_height = ui.ui_line_height;
                let menu_h = 4.0 * item_height;
                
                if state.mouse_x >= menu_x && state.mouse_x < menu_x + menu_w && state.mouse_y >= menu_y && state.mouse_y < menu_y + menu_h {
                    let idx = ((state.mouse_y - menu_y) / item_height).floor() as usize;
                    match idx {
                        0 => { // New File
                            ui.active_modal = Some(crate::ui::ModalType::SidebarInput);
                            ui.sidebar_input_type = "new_file".to_string();
                            ui.sidebar_input_target = target_path;
                            ui.sidebar_input_value.clear();
                            window.request_redraw();
                            return;
                        }
                        1 => { // New Folder
                            ui.active_modal = Some(crate::ui::ModalType::SidebarInput);
                            ui.sidebar_input_type = "new_folder".to_string();
                            ui.sidebar_input_target = target_path;
                            ui.sidebar_input_value.clear();
                            window.request_redraw();
                            return;
                        }
                        2 => { // Rename
                            ui.active_modal = Some(crate::ui::ModalType::SidebarInput);
                            ui.sidebar_input_type = "rename".to_string();
                            ui.sidebar_input_target = target_path.clone();
                            ui.sidebar_input_value = target_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            window.request_redraw();
                            return;
                        }
                        3 => { // Delete
                            if target_path.is_dir() {
                                let _ = std::fs::remove_dir_all(&target_path);
                            } else {
                                let _ = std::fs::remove_file(&target_path);
                            }
                            ui.rebuild_tree();
                            window.request_redraw();
                            return;
                        }
                        _ => {}
                    }
                }
                window.request_redraw();
                return;
            }

            // Check SidebarInput modal click
            if ui.active_modal == Some(crate::ui::ModalType::SidebarInput) {
                let modal_w = 400.0f32;
                let modal_h = 150.0f32;
                let modal_x = ((size.width as f32 - modal_w) / 2.0).round();
                let modal_y = ((size.height as f32 - modal_h) / 2.0).round();
                
                let title_y = modal_y + 20.0;
                let input_y = title_y + ui.ui_line_height + 15.0;
                let input_h = ui.ui_line_height + 8.0;
                
                let btn_w = 80.0f32;
                let btn_h = 24.0f32;
                let cancel_x = modal_x + modal_w - 20.0 - btn_w * 2.0 - 10.0;
                let confirm_x = modal_x + modal_w - 20.0 - btn_w;
                let btn_y = input_y + input_h + 15.0;
                
                if state.mouse_x >= cancel_x && state.mouse_x <= cancel_x + btn_w && state.mouse_y >= btn_y && state.mouse_y <= btn_y + btn_h {
                    ui.active_modal = None;
                    window.request_redraw();
                    return;
                }
                
                if state.mouse_x >= confirm_x && state.mouse_x <= confirm_x + btn_w && state.mouse_y >= btn_y && state.mouse_y <= btn_y + btn_h {
                    let target = &ui.sidebar_input_target;
                    let val = &ui.sidebar_input_value;
                    if !val.is_empty() {
                        match ui.sidebar_input_type.as_str() {
                            "new_file" => {
                                let parent = if target.is_dir() { target.clone() } else { target.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from(".")) };
                                let new_path = parent.join(val);
                                let _ = std::fs::File::create(new_path);
                            }
                            "new_folder" => {
                                let parent = if target.is_dir() { target.clone() } else { target.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from(".")) };
                                let new_path = parent.join(val);
                                let _ = std::fs::create_dir_all(new_path);
                            }
                            "rename" => {
                                if let Some(parent) = target.parent() {
                                    let new_path = parent.join(val);
                                    let _ = std::fs::rename(target, new_path);
                                }
                            }
                            _ => {}
                        }
                    }
                    ui.active_modal = None;
                    ui.rebuild_tree();
                    window.request_redraw();
                    return;
                }
                
                let clicked_outside = state.mouse_x < modal_x || state.mouse_x > modal_x + modal_w || state.mouse_y < modal_y || state.mouse_y > modal_y + modal_h;
                if clicked_outside {
                    ui.active_modal = None;
                    window.request_redraw();
                }
                return;
            }

            // Check Search Panel click
            if ui.show_search_panel {
                let panel_w = 360.0f32;
                let panel_h = 80.0f32;
                let sb_width = ui.scrollbar_width() + ui.minimap_width();
                let panel_x = (size.width as f32 - panel_w - sb_width - 15.0).max(ui.sidebar_width + 10.0);
                let panel_y = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height + 10.0;
                
                if state.mouse_x >= panel_x && state.mouse_x < panel_x + panel_w && state.mouse_y >= panel_y && state.mouse_y < panel_y + panel_h {
                    let label_w = 60.0f32;
                    let input_w = 180.0f32;
                    let input_h = ui.ui_line_height + 4.0;
                    let r1_y = panel_y + 10.0;
                    let in1_x = panel_x + 10.0 + label_w;
                    
                    // Click on Find input
                    if state.mouse_x >= in1_x && state.mouse_x < in1_x + input_w && state.mouse_y >= r1_y && state.mouse_y < r1_y + input_h {
                        ui.search_focus_replace = false;
                        window.request_redraw();
                        return;
                    }
                    
                    // Click on Close button
                    let close_btn_x = panel_x + panel_w - 25.0;
                    let close_btn_y = panel_y + 8.0;
                    if state.mouse_x >= close_btn_x && state.mouse_x < close_btn_x + 18.0 && state.mouse_y >= close_btn_y && state.mouse_y < close_btn_y + 18.0 {
                        ui.show_search_panel = false;
                        window.request_redraw();
                        return;
                    }
                    
                    // Click on Replace input
                    let r2_y = r1_y + input_h + 8.0;
                    let in2_x = panel_x + 10.0 + label_w;
                    if state.mouse_x >= in2_x && state.mouse_x < in2_x + input_w && state.mouse_y >= r2_y && state.mouse_y < r2_y + input_h {
                        ui.search_focus_replace = true;
                        window.request_redraw();
                        return;
                    }
                    
                    // Click on Prev button (◀)
                    let btn_w = 24.0f32;
                    let btn_h = input_h;
                    let prev_x = in2_x + input_w + 10.0;
                    if state.mouse_x >= prev_x && state.mouse_x < prev_x + btn_w && state.mouse_y >= r2_y && state.mouse_y < r2_y + btn_h {
                        if !ui.search_matches.is_empty() {
                            if ui.active_search_match_idx == 0 {
                                ui.active_search_match_idx = ui.search_matches.len() - 1;
                            } else {
                                ui.active_search_match_idx -= 1;
                            }
                            // Scroll to active match
                            if state.active_tab_idx < state.tabs.len() {
                                let (m_line, m_col) = ui.search_matches[ui.active_search_match_idx];
                                state.tabs[state.active_tab_idx].cursor.line = m_line;
                                state.tabs[state.active_tab_idx].cursor.col = m_col;
                                state.tabs[state.active_tab_idx].cursor.clear_selection();
                            }
                        }
                        window.request_redraw();
                        return;
                    }
                    
                    // Click on Next button (▶)
                    let next_x = prev_x + btn_w + 4.0;
                    if state.mouse_x >= next_x && state.mouse_x < next_x + btn_w && state.mouse_y >= r2_y && state.mouse_y < r2_y + btn_h {
                        if !ui.search_matches.is_empty() {
                            if ui.active_search_match_idx >= ui.search_matches.len() - 1 {
                                ui.active_search_match_idx = 0;
                            } else {
                                ui.active_search_match_idx += 1;
                            }
                            // Scroll to active match
                            if state.active_tab_idx < state.tabs.len() {
                                let (m_line, m_col) = ui.search_matches[ui.active_search_match_idx];
                                state.tabs[state.active_tab_idx].cursor.line = m_line;
                                state.tabs[state.active_tab_idx].cursor.col = m_col;
                                state.tabs[state.active_tab_idx].cursor.clear_selection();
                            }
                        }
                        window.request_redraw();
                        return;
                    }
                    
                    // Click on Replace button
                    let rep_x = next_x + btn_w + 8.0;
                    let rep_w = 60.0f32;
                    if state.mouse_x >= rep_x && state.mouse_x < rep_x + rep_w && state.mouse_y >= r2_y && state.mouse_y < r2_y + btn_h {
                        if !ui.search_matches.is_empty() && state.active_tab_idx < state.tabs.len() {
                            let (m_line, m_col) = ui.search_matches[ui.active_search_match_idx];
                            let active_tab = &mut state.tabs[state.active_tab_idx];
                            active_tab.buffer.commit_transaction();
                            active_tab.buffer.start_transaction();
                            
                            let q_len = ui.search_query.chars().count();
                            active_tab.buffer.delete(m_line, m_col, m_line, m_col + q_len);
                            active_tab.buffer.insert(m_line, m_col, &ui.replace_query);
                            active_tab.buffer.commit_transaction();
                            
                            active_tab.cursor.line = m_line;
                            active_tab.cursor.col = m_col + ui.replace_query.chars().count();
                            active_tab.cursor.clear_selection();
                            
                            ui.perform_search(state);
                        }
                        window.request_redraw();
                        return;
                    }
                    
                    return;
                }
            }

            // Check if click is on tab scrollbar
            let tabbar_start_x = ui.sidebar_width;
            let visible_width = w_width - tabbar_start_x;
            
            // Calculate total width of all tabs
            let mut total_tabs_width = 0.0f32;
            let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
            let close_reserved = 8.0f32 + tab_close_icon_sz;
            let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
            for idx in 0..tab_paths.len() {
                let path_opt = &tab_paths[idx];
                let file_name = path_opt.as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "untitled.txt".to_string());
                let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                let dot_reserved = 18.0f32;
                let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                total_tabs_width += tab_w;
            }

            if total_tabs_width > visible_width {
                let main_y = ui.titlebar_height;
                let is_on_scrollbar = state.mouse_y >= main_y + ui.tabbar_height - 6.0
                    && state.mouse_y < main_y + ui.tabbar_height
                    && state.mouse_x >= tabbar_start_x
                    && state.mouse_x < w_width;

                if is_on_scrollbar {
                    let ratio = visible_width / total_tabs_width;
                    let thumb_w = (visible_width * ratio).clamp(20.0_f32.min(visible_width), visible_width);
                    let max_scroll_x = total_tabs_width - visible_width;
                    let scroll_ratio_x = if max_scroll_x > 0.0 { ui.tab_scroll_x / max_scroll_x } else { 0.0 };
                    let thumb_x = tabbar_start_x + scroll_ratio_x * (visible_width - thumb_w);

                    ui.tab_scroll_is_dragging = true;
                    let is_on_thumb = state.mouse_x >= thumb_x && state.mouse_x < thumb_x + thumb_w;
                    if is_on_thumb {
                        state.scroll_drag_offset_x = state.mouse_x - thumb_x;
                    } else {
                        state.scroll_drag_offset_x = thumb_w / 2.0;
                        let target_thumb_x = state.mouse_x - state.scroll_drag_offset_x;
                        let target_ratio = if visible_width - thumb_w > 0.0 {
                            (target_thumb_x - tabbar_start_x) / (visible_width - thumb_w)
                        } else {
                            0.0
                        };
                        ui.tab_scroll_x = (target_ratio * max_scroll_x).clamp(0.0, max_scroll_x);
                    }
                    window.request_redraw();
                    return;
                }
            }

            // Check if click is on custom titlebar drag zone (CSD)
            let menu_items = ["Garage", "File", "Edit", "Selection", "View"];
            let mut menu_width = 0.0f32;
            for (i, label) in menu_items.iter().enumerate() {
                let label_len = label.chars().count() as f32;
                let text_w = label_len * ui.ui_char_width;
                let (left_pad, right_pad) = if i == 0 { (14.0, 10.0) } else { (10.0, 10.0) };
                menu_width += text_w + left_pad + right_pad;
            }
            
            let max_drag_x = if ui.is_tiling_wm() {
                size.width as f32
            } else {
                size.width as f32 - 135.0
            };
            let is_titlebar_drag_zone = state.mouse_y < ui.titlebar_height
                && state.mouse_x >= menu_width
                && state.mouse_x < max_drag_x;
 
            if is_titlebar_drag_zone {
                let now = Instant::now();
                let is_double_click = if let Some(last) = state.last_click_time {
                    now.duration_since(last) < std::time::Duration::from_millis(300)
                } else {
                    false
                };
                state.last_click_time = Some(now);
 
                if is_double_click {
                    let is_max = window.is_maximized();
                    window.set_maximized(!is_max);
                } else {
                    let _ = window.drag_window();
                }
                return;
            }
 
            // Calculate dock border position
            let main_y = ui.titlebar_height;
            let mut dock_start_y = size.height as f32 - ui.status_height;
            if ui.show_dock {
                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
            }
            
            // Check if focus changes
            if ui.show_dock && state.mouse_x >= ui.sidebar_width && state.mouse_y >= dock_start_y {
                state.terminal_focus = true;
            } else {
                state.terminal_focus = false;
            }
 
            // Check if click is on dock resize border
            let on_dock_border = ui.show_dock && (state.mouse_y - dock_start_y).abs() <= 4.0;
 
            if on_dock_border {
                state.is_dragging_dock_border = true;
            } else if ui.active_modal.is_some() {
                let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                let tab_modified: Vec<bool> = state.tabs.iter().map(|t| t.buffer.is_modified).collect();
                let action_res = {
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    ui.handle_click(
                        state.mouse_x,
                        state.mouse_y,
                        size.width as f32,
                        w_width,
                        size.height as f32,
                        &mut active_tab.buffer,
                        &mut active_tab.cursor,
                        &tab_paths,
                        &tab_modified,
                        &state.dock_terminals,
                        state.active_tab_idx,
                    )
                };
                handle_action(ui, state, action_res, window, elwt, gpu, atlas, font_bytes);
            } else {
                // Check if click is on sidebar resize border
                let on_sidebar_border = ui.sidebar_width > 0.0 && (state.mouse_x - ui.sidebar_width).abs() <= 4.0;
                if on_sidebar_border {
                    state.is_dragging_sidebar = true;
                } else {
                    let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                    let tab_modified: Vec<bool> = state.tabs.iter().map(|t| t.buffer.is_modified).collect();
                    let action_res = {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        let orig_titlebar_height = ui.titlebar_height;
                        if !state.inactive_panes.is_empty() && state.is_split_horizontal && state.mouse_x >= sidebar_original {
                            let main_y = ui.titlebar_height;
                            let mut dock_start_y = size.height as f32 - ui.status_height;
                            if ui.show_dock {
                                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
                            }
                            let editor_bottom_limit = if ui.show_dock {
                                dock_start_y
                            } else {
                                size.height as f32 - ui.status_height
                            };
                            let editor_area_height = editor_bottom_limit - main_y;
                            let pane_height = (editor_area_height / 2.0).round();
                            if state.active_pane_idx == 1 {
                                ui.titlebar_height = main_y + pane_height;
                            }
                        }
                        
                        let res = ui.handle_click(
                            state.mouse_x,
                            state.mouse_y,
                            size.width as f32,
                            w_width,
                            size.height as f32,
                            &mut active_tab.buffer,
                            &mut active_tab.cursor,
                            &tab_paths,
                            &tab_modified,
                            &state.dock_terminals,
                            state.active_tab_idx,
                        );
                        ui.titlebar_height = orig_titlebar_height;
                        res
                    };
 
                    match action_res {
                        UiAction::None => {
                            let active_tab_idx = state.active_tab_idx;
                            let main_y = ui.titlebar_height;
                            let mut dock_start_y = size.height as f32 - ui.status_height;
                            if ui.show_dock {
                                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
                            }
                            let editor_bottom_limit = if ui.show_dock {
                                dock_start_y
                            } else {
                                size.height as f32 - ui.status_height
                            };

                            let mut pane_top = main_y;
                            let mut pane_bottom = editor_bottom_limit;

                            if !state.inactive_panes.is_empty() && state.is_split_horizontal {
                                let editor_area_height = editor_bottom_limit - main_y;
                                let pane_height = (editor_area_height / 2.0).round();
                                if state.active_pane_idx == 0 {
                                    pane_bottom = main_y + pane_height;
                                } else {
                                    pane_top = main_y + pane_height;
                                }
                            }

                            let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
                            let editor_bottom_limit = pane_bottom;
                            let is_diagnostics = state.tabs[active_tab_idx].path.as_deref().map_or(false, |p| p.starts_with("diagnostics://"));
                            let active_tab_len = state.tabs[active_tab_idx].buffer.len();
                            let max_line_digits = active_tab_len.to_string().len().max(3);
                            let gutter_width = if is_diagnostics { 0.0 } else { (max_line_digits as f32 + 2.0) * ui.buffer_char_width };
                            let text_area_x = ui.sidebar_width + gutter_width;
                            let scrollbar_width = ui.scrollbar_width();
                            let minimap_width = if is_diagnostics { 0.0 } else { ui.minimap_width() };
                            let sb_x = w_width - scrollbar_width;
                            let minimap_x = sb_x - minimap_width;
                            let text_viewport_w = (minimap_x - text_area_x).max(10.0);
 
                            let show_horizontal_scrollbar = if is_diagnostics {
                                false
                            } else {
                                let max_line_len = ui.get_max_line_len(&state.tabs[active_tab_idx].buffer, state.tabs[active_tab_idx].path.as_deref(), state.tabs[active_tab_idx].cursor.line);
                                let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
                                max_line_len > visible_cols
                            };
                            let hs_height = if show_horizontal_scrollbar { 14.0 } else { 0.0 };
                            let editor_height = editor_bottom_limit - editor_top - hs_height;
 
                            let virtual_len = if is_diagnostics {
                                let mut count = 0;
                                for (file_path, diags) in &ui.lsp_diagnostics_details {
                                    if diags.is_empty() {
                                        continue;
                                    }
                                    let file_lines_len = ui.diagnostics_file_cache.get(file_path).map(|l| l.len()).unwrap_or(0);
                                    for diag in diags {
                                        let start_line = diag.line.saturating_sub(3);
                                        let end_line = if file_lines_len > 0 {
                                            (diag.line + 3).min(file_lines_len - 1)
                                        } else {
                                            diag.line + 3
                                        };
                                        let num_code_lines = end_line - start_line + 1;
                                        count += 1 + num_code_lines + 1; // Header + Code lines + Banner
                                    }
                                }
                                count.max(1)
                            } else {
                                active_tab_len
                            };
                            
                            // 1. Check if click is on minimap
                            if !is_diagnostics && state.mouse_x >= minimap_x && state.mouse_x < sb_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit {
                                state.is_dragging_minimap = true;
                                let total_editor_height = editor_bottom_limit - editor_top;
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                let max_scroll = (active_tab_len as isize - visible_lines as isize).max(0) as f32;
                                let relative_y = state.mouse_y - editor_top;
                                
                                let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
                                let minimap_total_h = active_tab_len as f32 * minimap_line_height;
                                
                                let clicked_line = if minimap_total_h > total_editor_height {
                                    let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
                                    (scroll_ratio * (active_tab_len - 1) as f32).round() as usize
                                } else {
                                    (relative_y / minimap_line_height).floor() as usize
                                };
                                let clicked_line = clicked_line.min(active_tab_len - 1);
                                
                                let active_tab = &mut state.tabs[active_tab_idx];
                                active_tab.cursor.line = clicked_line;
                                let line_chars = active_tab.buffer.lines()[clicked_line].chars().count();
                                active_tab.cursor.col = active_tab.cursor.col.min(line_chars);
                                active_tab.cursor.intended_col = active_tab.cursor.col;
                                
                                ui.scroll_y = clicked_line.saturating_sub(visible_lines / 2).min(max_scroll as usize);
                                window.request_redraw();
                            }
                            // 2. Check if click is on scrollbar
                            else if state.mouse_x >= sb_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit && {
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                virtual_len > visible_lines
                            } {
                                state.is_dragging_scroll = true;
                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                let ratio = visible_lines as f32 / virtual_len as f32;
                                let thumb_h = (editor_height * ratio).clamp(20.0_f32.min(editor_height), editor_height);
                                let max_scroll = (virtual_len as isize - visible_lines as isize).max(0) as f32;
                                
                                let scroll_ratio = if max_scroll > 0.0 { ui.scroll_y as f32 / max_scroll } else { 0.0 };
                                let thumb_y = editor_top + scroll_ratio * (editor_height - thumb_h);
                                
                                if state.mouse_y >= thumb_y && state.mouse_y < thumb_y + thumb_h {
                                    state.scroll_drag_offset_y = state.mouse_y - thumb_y;
                                } else {
                                    state.scroll_drag_offset_y = thumb_h / 2.0;
                                    let relative_y = state.mouse_y - editor_top - state.scroll_drag_offset_y;
                                    let scroll_range = editor_height - thumb_h;
                                    let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                                    ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                                }
                            }
                            // 3. Check if click is on horizontal scrollbar
                            else if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= editor_bottom_limit - 14.0 && state.mouse_y < editor_bottom_limit {
                                state.is_dragging_horizontal_scroll = true;
                                let active_tab = &mut state.tabs[active_tab_idx];
                                let max_line_len = ui.get_max_line_len(&active_tab.buffer, active_tab.path.as_deref(), active_tab.cursor.line);
                                let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
                                let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
                                let thumb_w = (text_viewport_w * ratio_x).clamp(20.0_f32.min(text_viewport_w), text_viewport_w);
                                let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
                                
                                let scroll_ratio_x = if max_scroll_x > 0.0 { ui.scroll_x as f32 / max_scroll_x } else { 0.0 };
                                let thumb_x = text_area_x + scroll_ratio_x * (text_viewport_w - thumb_w);
                                
                                if state.mouse_x >= thumb_x && state.mouse_x < thumb_x + thumb_w {
                                    state.scroll_drag_offset_x = state.mouse_x - thumb_x;
                                } else {
                                    state.scroll_drag_offset_x = thumb_w / 2.0;
                                    let relative_x = state.mouse_x - text_area_x - state.scroll_drag_offset_x;
                                    let scroll_range = text_viewport_w - thumb_w;
                                    let scroll_ratio = if scroll_range > 0.0 { (relative_x / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                                    ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
                                }
                            } else {
                                 // Click inside editor area
                                 if state.mouse_x >= text_area_x && state.mouse_x < minimap_x && state.mouse_y >= editor_top && state.mouse_y < editor_bottom_limit - 14.0 {
  
                                       // 2. Check if virtual diagnostics tab item was clicked
                                       let mut clicked_info = None;
                                       if state.tabs[active_tab_idx].path.as_deref() == Some("diagnostics://project") {
                                           let clicked_target = ui.diagnostics_click_targets.iter().find(|t| {
                                               state.mouse_x >= t.0 && state.mouse_x <= t.2 && state.mouse_y >= t.1 && state.mouse_y <= t.3
                                           }).cloned();
                                           if let Some((target_path, target_line, target_col, target_type)) = clicked_target.map(|t| (t.4, t.5, t.6, t.7)) {
                                               if target_type == "header" {
                                                   // Check if clicked the toggle arrow / left portion (e.g., mouse_x < text_area_x + 50.0)
                                                   if state.mouse_x < text_area_x + 50.0 {
                                                         if ui.collapsed_diagnostics.contains(&target_path) {
                                                             ui.collapsed_diagnostics.remove(&target_path);
                                                         } else {
                                                             ui.collapsed_diagnostics.insert(target_path);
                                                         }
                                                         ui.diagnostics_changed = true;
                                                         window.request_redraw();
                                                         return;
                                                   } else {
                                                       clicked_info = Some((target_path, target_line, target_col));
                                                   }
                                               } else if target_type == "code" {
                                                   // Place virtual cursor in diagnostics view
                                                   let clicked_line = ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y;
                                                   let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                                                   if !visual_lines.is_empty() {
                                                       let clicked_line = clicked_line.min(visual_lines.len() - 1);
                                                       let code_start_x = text_area_x + 48.0; // gutter_w is 48.0
                                                       let col_idx = ((state.mouse_x - code_start_x) / ui.buffer_char_width).round() as isize + ui.scroll_x as isize;
                                                       let col_idx = col_idx.max(0) as usize;
                                                       
                                                       let active_tab = &mut state.tabs[active_tab_idx];
                                                       if let Some(crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. }) = visual_lines.get(clicked_line) {
                                                           let line_chars = line_content.chars().count();
                                                           let col_idx = col_idx.min(line_chars);
                                                           active_tab.cursor.line = clicked_line;
                                                           active_tab.cursor.col = col_idx;
                                                           active_tab.cursor.intended_col = col_idx;
                                                           active_tab.cursor.selection_anchor = Some((clicked_line, col_idx));
                                                           state.is_dragging = true;
                                                       }
                                                   }
                                                   window.request_redraw();
                                                   return;
                                               }
                                           }
                                       }
                                      if let Some((target_path, target_line, target_col)) = clicked_info {
                                          // Open the file
                                          let open_action = crate::ui::UiAction::OpenFile(std::path::PathBuf::from(target_path));
                                          crate::app::handler::handle_action(
                                              ui,
                                              state,
                                              open_action,
                                              window,
                                              elwt,
                                              gpu,
                                              atlas,
                                              font_bytes,
                                          );
                                          
                                          // Set cursor on new active tab
                                          let new_active_tab = &mut state.tabs[state.active_tab_idx];
                                          new_active_tab.cursor.line = target_line;
                                          new_active_tab.cursor.col = target_col;
                                          new_active_tab.cursor.intended_col = target_col;
                                          new_active_tab.cursor.selection_anchor = Some((target_line, target_col));
                                          ui.scroll_to_cursor(&new_active_tab.cursor, new_active_tab.buffer.len(), size.width as f32, size.height as f32);
                                          window.request_redraw();
                                          return;
                                      }
 
                                     // Normal click
                                     let active_tab = &mut state.tabs[active_tab_idx];
                                     active_tab.buffer.commit_transaction();
                                     state.is_dragging = true;
 
                                     let line_idx = ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y;
                                     let line_idx = line_idx.min(active_tab.buffer.len() - 1);
 
                                     let col_idx = ((state.mouse_x - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x;
                                     let line_chars = active_tab.buffer.lines()[line_idx].chars().count();
                                     let col_idx = col_idx.min(line_chars);
 
                                     let extend_selection = state.modifiers.shift_key();
                                     if extend_selection {
                                         if active_tab.cursor.selection_anchor.is_none() {
                                             active_tab.cursor.selection_anchor = Some((active_tab.cursor.line, active_tab.cursor.col));
                                         }
                                     } else {
                                         active_tab.cursor.selection_anchor = Some((line_idx, col_idx));
                                     }
 
                                     active_tab.cursor.line = line_idx;
                                     active_tab.cursor.col = col_idx;
                                     active_tab.cursor.intended_col = col_idx;
 
                                     ui.scroll_to_cursor(&active_tab.cursor, active_tab.buffer.len(), size.width as f32, size.height as f32);
                                 }
                             }
                         }
                         UiAction::SelectTab(idx) => {
                               state.dragged_tab_idx = Some(idx);
                               state.drag_start_pos = Some((state.mouse_x, state.mouse_y));
                               handle_action(ui, state, UiAction::SelectTab(idx), window, elwt, gpu, atlas, font_bytes);
                           }
                           act => {
                               handle_action(ui, state, act, window, elwt, gpu, atlas, font_bytes);
                           }
                       }
                   }
               }
           } else {
               // Button Released
               ui.tab_scroll_is_dragging = false;
               let was_dragging_sidebar = state.is_dragging_sidebar;
               state.is_dragging = false;
               state.is_dragging_scroll = false;
               state.is_dragging_horizontal_scroll = false;
               state.is_dragging_minimap = false;
               state.is_dragging_sidebar = false;
               state.is_dragging_dock_border = false;
               if was_dragging_sidebar {
                   ui.config.sidebar_width = ui.sidebar_width;
                   ui.config.save_in_background();
               }
               
               if let Some(dragged_idx) = state.dragged_tab_idx.take() {
                   let drag_start = state.drag_start_pos.take();
                   let mut was_dragged = false;
                   if let Some((sx, sy)) = drag_start {
                       let dx = state.mouse_x - sx;
                       let dy = state.mouse_y - sy;
                       if (dx * dx + dy * dy).sqrt() >= 8.0 {
                           was_dragged = true;
                       }
                   }
 
                    if was_dragged {
                        let main_y = ui.titlebar_height;
                        let sidebar_original = ui.config.sidebar_width;
                        let mut dock_start_y = size.height as f32 - ui.status_height;
                        if ui.show_dock {
                            dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
                        }
                        let editor_bottom_limit = if ui.show_dock {
                            dock_start_y
                        } else {
                            size.height as f32 - ui.status_height
                        };

                        let is_outside = state.mouse_x < 0.0 || state.mouse_x >= size.width as f32 || state.mouse_y < 0.0 || state.mouse_y >= size.height as f32;
                        if is_outside {
                            let mut removed = false;
                            if let Some(ref path_str) = state.tabs[dragged_idx].path {
                                if !path_str.starts_with("diagnostics://") {
                                    if state.tabs[dragged_idx].buffer.is_modified {
                                        let _ = state.tabs[dragged_idx].buffer.save_file(path_str);
                                    }
                                    let inner_pos = window.inner_position().unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
                                    let global_x = inner_pos.x + state.mouse_x as i32;
                                    let global_y = inner_pos.y + state.mouse_y as i32;
                                    
                                    if !crate::app::ipc::try_drop_to_other_window(global_x, global_y, path_str) {
                                        if let Ok(exe_path) = std::env::current_exe() {
                                            let _ = std::process::Command::new(exe_path)
                                                .arg(path_str)
                                                .spawn();
                                        }
                                    }
                                    state.tabs.remove(dragged_idx);
                                    removed = true;
                                }
                            }
                            if removed {
                                if state.tabs.is_empty() {
                                    if !state.inactive_panes.is_empty() {
                                        let target_pane = state.inactive_panes.remove(0);
                                        state.tabs = target_pane.tabs;
                                        state.active_tab_idx = target_pane.active_tab_idx.min(state.tabs.len().saturating_sub(1));
                                        state.active_pane_idx = 0;
                                        state.is_split_horizontal = false;
                                    } else {
                                        state.tabs.push(crate::app::state::Tab {
                                            path: None,
                                            buffer: crate::editor::buffer::Buffer::new(),
                                            cursor: crate::editor::cursor::Cursor::new(),
                                            scroll_x: 0,
                                            scroll_y: 0,
                                        });
                                        state.active_tab_idx = 0;
                                    }
                                } else {
                                    state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
                                }
                                if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
                                    ui.scroll_x = active_tab.scroll_x;
                                    ui.scroll_y = active_tab.scroll_y;
                                }
                            }
                            window.request_redraw();
                            update_cursor_icon(window, ui, state);
                            return;
                        }

                        let editor_area_width = size.width as f32 - sidebar_original;
                        let pane_width = editor_area_width / 2.0;
                        
                        let hovered_pane_idx = if state.inactive_panes.is_empty() {
                            0
                        } else {
                            if state.is_split_horizontal {
                                let editor_area_height = editor_bottom_limit - main_y;
                                let pane_height = (editor_area_height / 2.0).round();
                                if state.mouse_y < main_y + pane_height { 0 } else { 1 }
                            } else {
                                if state.mouse_x < sidebar_original + pane_width { 0 } else { 1 }
                            }
                        };
                        
                        let is_in_tabbar = if !state.inactive_panes.is_empty() && state.is_split_horizontal {
                            let editor_area_height = editor_bottom_limit - main_y;
                            let pane_height = (editor_area_height / 2.0).round();
                            let pane_top = if hovered_pane_idx == 0 { main_y } else { main_y + pane_height };
                            state.mouse_y >= pane_top && state.mouse_y < pane_top + ui.tabbar_height
                        } else {
                            state.mouse_y >= main_y && state.mouse_y < main_y + ui.tabbar_height
                        };
                        
                        if is_in_tabbar {
                            if hovered_pane_idx != state.active_pane_idx {
                                // Move tab to the other pane!
                                let tab_to_move = state.tabs.remove(dragged_idx);
                                state.inactive_panes[0].tabs.push(tab_to_move);
                                state.inactive_panes[0].active_tab_idx = state.inactive_panes[0].tabs.len() - 1;
                                
                                // If active pane became empty, collapse the split
                                if state.tabs.is_empty() {
                                    let target_pane = state.inactive_panes.remove(0);
                                    state.tabs = target_pane.tabs;
                                    state.active_tab_idx = target_pane.active_tab_idx.min(state.tabs.len().saturating_sub(1));
                                    state.active_pane_idx = 0;
                                    state.is_split_horizontal = false;
                                } else {
                                    state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
                                    let target_pane = 1 - state.active_pane_idx;
                                    state.switch_pane(target_pane);
                                }
                                if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
                                    ui.scroll_x = active_tab.scroll_x;
                                    ui.scroll_y = active_tab.scroll_y;
                                }
                            }
                        } else {
                            if state.inactive_panes.is_empty() {
                                // Split editor
                                let tab_to_move = if state.tabs.len() > 1 {
                                    state.tabs.remove(dragged_idx)
                                } else {
                                    state.tabs[dragged_idx].clone()
                                };
                                if !state.tabs.is_empty() {
                                    state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
                                }
                                
                                let editor_area_width = size.width as f32 - sidebar_original;
                                let editor_area_height = editor_bottom_limit - (main_y + ui.tabbar_height);
                                
                                if state.mouse_y < main_y + ui.tabbar_height + editor_area_height * 0.25 {
                                    // Top Split
                                    state.is_split_horizontal = true;
                                    let existing_pane = crate::app::state::Pane {
                                        tabs: std::mem::take(&mut state.tabs),
                                        active_tab_idx: state.active_tab_idx,
                                    };
                                    state.inactive_panes.push(existing_pane);
                                    state.tabs = vec![tab_to_move];
                                    state.active_tab_idx = 0;
                                    state.active_pane_idx = 0;
                                } else if state.mouse_y >= main_y + ui.tabbar_height + editor_area_height * 0.75 {
                                    // Bottom Split
                                    state.is_split_horizontal = true;
                                    state.inactive_panes.push(crate::app::state::Pane {
                                        tabs: vec![tab_to_move],
                                        active_tab_idx: 0,
                                    });
                                    state.switch_pane(1);
                                } else if state.mouse_x < sidebar_original + editor_area_width * 0.5 {
                                    // Left Split
                                    state.is_split_horizontal = false;
                                    let existing_pane = crate::app::state::Pane {
                                        tabs: std::mem::take(&mut state.tabs),
                                        active_tab_idx: state.active_tab_idx,
                                    };
                                    state.inactive_panes.push(existing_pane);
                                    state.tabs = vec![tab_to_move];
                                    state.active_tab_idx = 0;
                                    state.active_pane_idx = 0;
                                } else {
                                    // Right Split
                                    state.is_split_horizontal = false;
                                    state.inactive_panes.push(crate::app::state::Pane {
                                        tabs: vec![tab_to_move],
                                        active_tab_idx: 0,
                                    });
                                    state.switch_pane(1);
                                }
                                
                                if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
                                    ui.scroll_x = active_tab.scroll_x;
                                    ui.scroll_y = active_tab.scroll_y;
                                }
                            } else {
                                if hovered_pane_idx != state.active_pane_idx {
                                    // Move to the other pane
                                    let tab_to_move = state.tabs.remove(dragged_idx);
                                    state.inactive_panes[0].tabs.push(tab_to_move);
                                    state.inactive_panes[0].active_tab_idx = state.inactive_panes[0].tabs.len() - 1;
                                    
                                    // If active pane became empty, collapse the split
                                    if state.tabs.is_empty() {
                                        let target_pane = state.inactive_panes.remove(0);
                                        state.tabs = target_pane.tabs;
                                        state.active_tab_idx = target_pane.active_tab_idx.min(state.tabs.len().saturating_sub(1));
                                        state.active_pane_idx = 0;
                                        state.is_split_horizontal = false;
                                    } else {
                                        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
                                        let target_pane = 1 - state.active_pane_idx;
                                        state.switch_pane(target_pane);
                                    }
                                    if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
                                        ui.scroll_x = active_tab.scroll_x;
                                        ui.scroll_y = active_tab.scroll_y;
                                    }
                                }
                            }
                        }    
                    } else {
                        state.drag_start_pos = None;
                    }
                }

               if let Some((s_l, s_c, e_l, e_c)) = state.tabs[state.active_tab_idx].cursor.selection_range() {
                  if s_l == e_l && s_c == e_c {
                      state.tabs[state.active_tab_idx].cursor.clear_selection();
                  }
              }
          }
          update_cursor_icon(window, ui, state);
         window.request_redraw();
     }
 }

pub fn handle_mouse_wheel(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    delta: MouseScrollDelta,
) {
    if ui.active_modal == Some(crate::ui::ModalType::CommandPalette) {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize,
            MouseScrollDelta::PixelDelta(pos) => (pos.y / 15.0) as isize * -1,
        };
        let filtered_len = ui.get_filtered_commands().len();
        let max_visible_items = 10;
        let max_scroll = (filtered_len as isize - max_visible_items as isize).max(0);
        let new_scroll = ui.command_palette_scroll as isize + scroll_lines;
        ui.command_palette_scroll = new_scroll.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }
    if ui.active_modal.is_some() {
        return;
    }
 
    let size = window.inner_size();
    let sidebar_original = ui.sidebar_width;
    let mut sidebar_width = ui.sidebar_width;
    let mut w_width = size.width as f32;

    if !state.inactive_panes.is_empty() {
        let mouse_pane_idx = if state.is_split_horizontal {
            let main_y = ui.titlebar_height;
            let mut dock_start_y = size.height as f32 - ui.status_height;
            if ui.show_dock {
                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
            }
            let editor_bottom_limit = if ui.show_dock {
                dock_start_y
            } else {
                size.height as f32 - ui.status_height
            };
            let editor_area_height = editor_bottom_limit - main_y;
            let pane_height = (editor_area_height / 2.0).round();
            if state.mouse_y < main_y + pane_height { 0 } else { 1 }
        } else {
            let editor_area_width = size.width as f32 - sidebar_original;
            let pane_width = editor_area_width / 2.0;
            if state.mouse_x < sidebar_original + pane_width { 0 } else { 1 }
        };
        if mouse_pane_idx != state.active_pane_idx {
            state.switch_pane(mouse_pane_idx);
        }

        if !state.is_split_horizontal {
            let editor_area_width = size.width as f32 - sidebar_original;
            let pane_width = editor_area_width / 2.0;
            if state.active_pane_idx == 0 {
                w_width = sidebar_original + pane_width;
            } else {
                sidebar_width = sidebar_original + pane_width;
            }
        }
    }

    ui.sidebar_width = sidebar_width;
    let _sidebar_guard = SidebarGuard {
        ptr: &mut ui.sidebar_width as *mut f32,
        original_value: sidebar_original,
    };

    // Handle Tab Bar Scroll
    let tabbar_start_x = ui.sidebar_width;
    if state.mouse_y >= ui.titlebar_height 
        && state.mouse_y < ui.titlebar_height + ui.tabbar_height
        && state.mouse_x >= tabbar_start_x
        && state.mouse_x < w_width 
    {
        let mut total_tabs_width = 0.0f32;
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_paths = state.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>();
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = path_opt.as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.txt".to_string());
            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
            let dot_reserved = 18.0f32;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
            total_tabs_width += tab_w;
        }

        let visible_width = w_width - tabbar_start_x;
        let max_scroll_x = (total_tabs_width - visible_width).max(0.0);

        let scroll_amount = match delta {
            MouseScrollDelta::LineDelta(dx, dy) => {
                let val = if dx.abs() > dy.abs() { dx } else { -dy };
                val * 24.0
            }
            MouseScrollDelta::PixelDelta(pos) => {
                let val = if pos.x.abs() > pos.y.abs() { pos.x } else { -pos.y };
                val as f32
            }
        };

        ui.tab_scroll_x = (ui.tab_scroll_x + scroll_amount).clamp(0.0, max_scroll_x);
        window.request_redraw();
        return;
    }

    // Handle Terminal Dock Scroll
    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let is_mouse_over_terminal = ui.show_dock 
        && state.mouse_y >= dock_start_y + 28.0 
        && state.mouse_y < size.height as f32 - ui.status_height
        && state.mouse_x >= ui.sidebar_width;

    if is_mouse_over_terminal && !state.dock_terminals.is_empty() {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => dy as isize * 3,
            MouseScrollDelta::PixelDelta(pos) => ((pos.y / 15.0) * 3.0) as isize,
        };
        let active_term = &mut state.dock_terminals[state.active_terminal_idx];
        let max_scroll = active_term.grid.scrollback.len() as isize;
        let new_offset = active_term.grid.scroll_offset as isize + scroll_lines;
        active_term.grid.scroll_offset = new_offset.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }

    // Handle Sidebar Scroll
    let sidebar_top = ui.titlebar_height;
    let sidebar_bottom = size.height as f32 - ui.status_height;
    if ui.sidebar_width > 0.0 && state.mouse_x >= 0.0 && state.mouse_x < ui.sidebar_width && state.mouse_y >= sidebar_top && state.mouse_y < sidebar_bottom {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
            MouseScrollDelta::PixelDelta(pos) => ((pos.y / (ui.ui_line_height as f64)) * 3.0) as isize * -1,
        };
        let total_rows = 1 + ui.visible_nodes.len();
        let main_height = sidebar_bottom - sidebar_top;
        let visible_rows = (main_height / ui.ui_line_height).floor() as usize;
        let max_scroll = (total_rows as isize - visible_rows as isize).max(0);
        let new_scroll = ui.sidebar_scroll as isize + scroll_lines;
        ui.sidebar_scroll = new_scroll.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }
 
    let is_shift = state.modifiers.shift_key();
    if is_shift {
        let scroll_cols = match delta {
            MouseScrollDelta::LineDelta(dx, _) if dx != 0.0 => -dx as isize * 3,
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
            MouseScrollDelta::PixelDelta(pos) => {
                let val = if pos.x.abs() > pos.y.abs() { pos.x } else { pos.y };
                ((val / (ui.buffer_char_width as f64)) * 3.0) as isize * -1
            }
        };
        let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = ui.minimap_width();
        let sb_x = w_width - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
        let text_viewport_w = (minimap_x - text_area_x).max(10.0);
        let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
 
        let max_line_len = state.tabs[state.active_tab_idx].buffer.lines().iter().map(|l: &String| l.chars().count()).max().unwrap_or(0);
        let max_scroll = (max_line_len as isize - visible_cols as isize).max(0);
        let new_scroll = ui.scroll_x as isize + scroll_cols;
        ui.scroll_x = new_scroll.clamp(0, max_scroll) as usize;
        state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
        window.request_redraw();
        return;
    }
 
    let scroll_lines = match delta {
        MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
        MouseScrollDelta::PixelDelta(pos) => ((pos.y / (ui.buffer_line_height as f64)) * 3.0) as isize * -1,
    };
 
    let active_path = state.tabs[state.active_tab_idx].path.as_deref().unwrap_or("");
    let is_diagnostics = active_path.starts_with("diagnostics://");

    let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
    let status_y = (window.inner_size().height as f32 - ui.status_height).round();

    let show_horizontal_scrollbar = if is_diagnostics {
        false
    } else {
        let max_line_len = ui.get_max_line_len(&state.tabs[state.active_tab_idx].buffer, Some(active_path), state.tabs[state.active_tab_idx].cursor.line);
        let max_line_digits = state.tabs[state.active_tab_idx].buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        let scrollbar_width = ui.scrollbar_width();
        let minimap_width = ui.minimap_width();
        let sb_x = w_width - scrollbar_width;
        let minimap_x = sb_x - minimap_width;
        let text_viewport_w = (minimap_x - text_area_x).max(10.0);
        let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
        max_line_len > visible_cols
    };
    let hs_height = if show_horizontal_scrollbar { 14.0 } else { 0.0 };
    let editor_height = status_y - editor_top - hs_height;
    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;

    let virtual_len = if is_diagnostics {
        let mut count = 0;
        for (file_path, diags) in &ui.lsp_diagnostics_details {
            if diags.is_empty() {
                continue;
            }
            let file_lines_len = ui.diagnostics_file_cache.get(file_path).map(|l| l.len()).unwrap_or(0);
            for diag in diags {
                let start_line = diag.line.saturating_sub(3);
                let end_line = if file_lines_len > 0 {
                    (diag.line + 3).min(file_lines_len - 1)
                } else {
                    diag.line + 3
                };
                let num_code_lines = end_line - start_line + 1;
                count += 1 + num_code_lines + 1; // Header + Code lines + Banner
            }
        }
        count.max(1)
    } else {
        state.tabs[state.active_tab_idx].buffer.len()
    };

    let max_scroll = (virtual_len as isize - visible_lines as isize).max(0);
 
    let new_scroll = ui.scroll_y as isize + scroll_lines;
    ui.scroll_y = new_scroll.clamp(0, max_scroll) as usize;
 
    window.request_redraw();
}
