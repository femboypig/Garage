use std::path::{Component, Path};
use std::sync::Arc;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

use crate::app::handler::handle_action;
use crate::app::state::AppState;
use crate::machkit::{UiAction, UiState};
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::GpuContext;

pub fn update_cursor_icon(window: &Window, ui: &mut UiState, state: &AppState) {
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
            if ui.config.show_git_branch
                && let Some(ref branch) = ui.git_branch
            {
                let icon_sz = (ui.ui_font_size * 0.9).round().max(12.0);
                pen_x += icon_sz + 4.0;
                let branch_len = branch.chars().count() as f32;
                pen_x += branch_len * ui.ui_char_width;
                pen_x += 15.0;
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
                let tab_paths: Vec<Option<String>> =
                    state.tabs.iter().map(|t| t.path.clone()).collect();
                let active_tab_idx = state.active_tab_idx.min(state.tabs.len().saturating_sub(1));
                let raw_ext = tab_paths
                    .get(active_tab_idx)
                    .and_then(|p| p.as_ref())
                    .and_then(|p| std::path::Path::new(p).extension())
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");
                let mut extension = raw_ext.to_string();
                if let Some(path) = tab_paths.get(active_tab_idx).and_then(|p| p.as_ref())
                    && let Some(forced_ext) = ui.forced_languages.get(path)
                {
                    extension = forced_ext.clone();
                }

                let language = ui
                    .languages
                    .get(&extension)
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

                let encoding = tab_paths
                    .get(active_tab_idx)
                    .and_then(|p| p.as_ref())
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
                } else {
                    mouse_x >= enc_left && mouse_x < enc_right
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

    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height)
            .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let editor_bottom_limit = if ui.show_dock {
        dock_start_y
    } else {
        size.height as f32 - ui.status_height
    };

    let hovered_pane_idx = if state.inactive_panes.is_empty() {
        0
    } else if state.is_split_horizontal {
        let editor_area_height = editor_bottom_limit - main_y;
        let pane_height = (editor_area_height / 2.0).round();
        if mouse_y < main_y + pane_height { 0 } else { 1 }
    } else {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        if mouse_x < sidebar_original + pane_width {
            0
        } else {
            1
        }
    };

    if !state.inactive_panes.is_empty() && !state.is_split_horizontal {
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
        (
            &state.inactive_panes[0].tabs,
            state.inactive_panes[0].active_tab_idx,
        )
    };

    if hovered_tabs.is_empty() {
        window.set_cursor_icon(winit::window::CursorIcon::Default);
        return;
    }

    let active_tab = &hovered_tabs[hovered_active_tab_idx.min(hovered_tabs.len() - 1)];
    let buffer = &active_tab.buffer;
    let is_diagnostics = active_tab
        .path
        .as_deref()
        .is_some_and(|p| p.starts_with("diagnostics://") || p == "search://project");
    let max_line_digits = buffer.len().to_string().len().max(3);
    let gutter_width = if is_diagnostics {
        0.0
    } else {
        (max_line_digits as f32 + 2.0) * ui.buffer_char_width
    };
    let activity_bar_width = 0.0;
    let text_area_x = activity_bar_width + sidebar_width + gutter_width;

    let scrollbar_width = ui.scrollbar_width();
    let minimap_width = if is_diagnostics {
        0.0
    } else {
        ui.minimap_width()
    };
    let sb_x = w_width - scrollbar_width;
    let minimap_x = sb_x - minimap_width;

    let on_sidebar_border = sidebar_original > 0.0 && (mouse_x - sidebar_original).abs() <= 4.0;
    let on_dock_border = ui.show_dock && (mouse_y - dock_start_y).abs() <= 4.0;

    let mut pane_top = main_y;
    let mut pane_bottom = editor_bottom_limit;
    if !state.inactive_panes.is_empty() && state.is_split_horizontal {
        let editor_area_height = editor_bottom_limit - main_y;
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
        let active_path = active_tab.path.as_deref().unwrap_or("");
        let is_special_view =
            active_path.starts_with("diagnostics://") || active_path == "search://project";

        let is_in_editor = ui.active_modal.is_none()
            && ui.active_menu.is_none()
            && !is_special_view
            && mouse_x >= text_area_x
            && mouse_x < minimap_x
            && mouse_y >= pane_top + ui.tabbar_height + ui.breadcrumb_height
            && mouse_y < pane_bottom - 14.0;

        if is_in_editor {
            window.set_cursor_icon(winit::window::CursorIcon::Text);
        } else {
            let mut is_pointer = false;

            if is_special_view
                && mouse_x >= text_area_x
                && mouse_x < minimap_x
                && mouse_y >= pane_top + ui.tabbar_height + ui.breadcrumb_height
                && mouse_y < pane_bottom - 14.0
            {
                if active_path == "search://project" {
                    let list_y = pane_top + ui.tabbar_height + ui.breadcrumb_height;
                    let item_height = ui.buffer_line_height;
                    let render_items = crate::machkit::components::editor::project_search::build_search_render_items(ui);
                    let item_idx =
                        ui.scroll_y + ((mouse_y - list_y) / item_height).floor() as usize;
                    if item_idx < render_items.len() {
                        match &render_items[item_idx] {
                            crate::machkit::SearchRenderItem::FileHeader { .. } => {
                                is_pointer = true;
                            }
                            crate::machkit::SearchRenderItem::CodeLine {
                                is_first_in_range,
                                is_last_in_range,
                                ..
                            } => {
                                if (*is_first_in_range || *is_last_in_range)
                                    && mouse_x >= text_area_x
                                    && mouse_x < text_area_x + 22.0
                                {
                                    is_pointer = true;
                                } else {
                                    window.set_cursor_icon(winit::window::CursorIcon::Text);
                                    return;
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    is_pointer = true;
                }
            }

            let is_local = active_path != "search://project";
            let is_search_shown = ui.show_search_panel || !is_local;

            if is_search_shown {
                let bar_x = sidebar_width;
                let bar_w = w_width - sidebar_width;
                let bar_y = pane_top + ui.tabbar_height;
                let bar_h = ui.breadcrumb_height;

                if mouse_x >= bar_x
                    && mouse_x < bar_x + bar_w
                    && mouse_y >= bar_y
                    && mouse_y < bar_y + bar_h
                {
                    let show_replace = if is_local {
                        ui.show_replace
                    } else {
                        ui.global_show_replace
                    };

                    let input_h = 26.0f32;
                    let path_h = if is_local { 20.0f32 } else { 0.0f32 };
                    let remaining_h = bar_h - path_h;
                    let row_h = if show_replace {
                        remaining_h / 2.0
                    } else {
                        remaining_h
                    };
                    let input_y_1 = bar_y + path_h + (row_h - input_h) / 2.0;
                    let input_y_2 = bar_y + path_h + row_h + (row_h - input_h) / 2.0;

                    let toggle_btn_w = 24.0f32;
                    let toggle_btn_x = bar_x + 10.0;
                    let input_start_x = toggle_btn_x + toggle_btn_w + 6.0;

                    let close_btn_w = 24.0f32;
                    let btn_next_w = 24.0f32;
                    let btn_prev_w = 24.0f32;
                    let btn_rep_toggle_w = if is_local { 0.0f32 } else { 24.0f32 };
                    let btn_filter_w = if is_local { 0.0f32 } else { 24.0f32 };
                    let count_w = if is_local { 70.0f32 } else { 75.0f32 };

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

                    let input_find_w = (count_x - 10.0 - input_start_x).max(50.0);

                    // Row 1 hover
                    if mouse_y >= input_y_1 && mouse_y < input_y_1 + input_h {
                        // Options inside find input
                        let opt_btn_w = 22.0f32;
                        let opt_regex_x = input_start_x + input_find_w - 5.0 - opt_btn_w;
                        let opt_word_x = opt_regex_x - 2.0 - opt_btn_w;
                        let opt_case_x = opt_word_x - 2.0 - opt_btn_w;

                        if mouse_x >= opt_case_x && mouse_x < opt_regex_x + opt_btn_w {
                            window.set_cursor_icon(winit::window::CursorIcon::Pointer);
                            return;
                        } else if mouse_x >= input_start_x && mouse_x < input_start_x + input_find_w
                        {
                            window.set_cursor_icon(winit::window::CursorIcon::Text);
                            return;
                        } else if (mouse_x >= toggle_btn_x && mouse_x < toggle_btn_x + toggle_btn_w)
                            || (mouse_x >= filter_x && mouse_x < filter_x + btn_filter_w)
                            || (mouse_x >= rep_toggle_x
                                && mouse_x < rep_toggle_x + btn_rep_toggle_w)
                            || (mouse_x >= prev_x && mouse_x < prev_x + btn_prev_w)
                            || (mouse_x >= next_x && mouse_x < next_x + btn_next_w)
                            || (mouse_x >= close_x && mouse_x < close_x + close_btn_w)
                        {
                            window.set_cursor_icon(winit::window::CursorIcon::Pointer);
                            return;
                        }
                    }

                    // Row 2 hover
                    if show_replace && mouse_y >= input_y_2 && mouse_y < input_y_2 + input_h {
                        if mouse_x >= input_start_x && mouse_x < input_start_x + input_find_w {
                            window.set_cursor_icon(winit::window::CursorIcon::Text);
                            return;
                        } else if (mouse_x >= prev_x && mouse_x < prev_x + btn_prev_w)
                            || (mouse_x >= next_x && mouse_x < next_x + btn_next_w)
                        {
                            window.set_cursor_icon(winit::window::CursorIcon::Pointer);
                            return;
                        }
                    }

                    window.set_cursor_icon(winit::window::CursorIcon::Default);
                    return;
                }
            }

            // 1. Sidebar items (only actual node items, not empty space below)
            let sidebar_item_count = 1 + ui.visible_nodes.len();
            let sidebar_height_limit = ui.titlebar_height
                + (sidebar_item_count as f32 - ui.sidebar_scroll as f32) * ui.ui_line_height;
            if sidebar_original > 0.0
                && mouse_x < sidebar_original
                && mouse_y >= ui.titlebar_height
                && mouse_y < sidebar_height_limit.min(dock_start_y)
            {
                is_pointer = true;
            }
            // 2. Tabbar (only over actual tabs, not empty space to the right)
            else if mouse_y >= pane_top && mouse_y < pane_top + ui.tabbar_height {
                if is_pointer_in_tabbar(ui, state, size, hovered_pane_idx, pane_top, mouse_x) {
                    is_pointer = true;
                }
            }
            // 3. Titlebar Menu (only menu labels and window controls, not empty space or drag area)
            else if mouse_y < ui.titlebar_height {
                let menu_items = ["Garage", "File", "Edit", "Selection", "View"];
                let mut menu_width = 0.0f32;
                for (i, label) in menu_items.iter().enumerate() {
                    let label_len = label.chars().count() as f32;
                    let text_w = label_len * ui.ui_char_width;
                    let (left_pad, right_pad) = if i == 0 { (14.0, 10.0) } else { (10.0, 10.0) };
                    menu_width += text_w + left_pad + right_pad;
                }
                if mouse_x < menu_width {
                    is_pointer = true;
                } else if !ui.is_tiling_wm() {
                    let btn_w = 45.0f32;
                    let min_x = size.width as f32 - btn_w * 3.0;
                    if mouse_x >= min_x {
                        is_pointer = true;
                    }
                }
            }
            // 4. Scrollbar thumb/track only (excluding minimap itself)
            else if mouse_x >= sb_x
                && mouse_x < w_width
                && mouse_y >= ui.titlebar_height + ui.tabbar_height
                && mouse_y < dock_start_y
            {
                is_pointer = true;
            }
            // 5. Active modal interactive areas
            else if let Some(modal) = ui.active_modal {
                if cursor_icon_for_modal(window, ui, size, modal, mouse_x, mouse_y) {
                    return;
                }
                // is_pointer will be set inside if needed — recheck
                is_pointer = check_modal_pointer(ui, size, modal, mouse_x, mouse_y);
            }

            if is_pointer {
                window.set_cursor_icon(winit::window::CursorIcon::Pointer);
            } else {
                window.set_cursor_icon(winit::window::CursorIcon::Default);
            }
        }
    }
}

/// Returns true and sets the cursor if the modal requires a non-default/non-pointer cursor.
fn cursor_icon_for_modal(
    window: &Window,
    ui: &UiState,
    size: winit::dpi::PhysicalSize<u32>,
    modal: crate::machkit::ModalType,
    mx: f32,
    my: f32,
) -> bool {
    let modal_w = compute_modal_w(ui, modal);
    let modal_h = compute_modal_h(ui, modal);
    let modal_x = ((size.width as f32 - modal_w) / 2.0).round();
    let modal_y = ((size.height as f32 - modal_h) / 2.0).round();

    if mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h {
        return false;
    }

    match modal {
        crate::machkit::ModalType::CommandPalette | crate::machkit::ModalType::GlobalSearch => {
            let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
            if my >= modal_y + 15.0 && my < modal_y + 15.0 + ui.ui_line_height + 8.0 {
                window.set_cursor_icon(winit::window::CursorIcon::Text);
                return true;
            }
            // list area: pointer, handled by check_modal_pointer
            let _ = header_h;
            false
        }
        crate::machkit::ModalType::SidebarInput => {
            let input_x = modal_x + 20.0;
            let title_y = modal_y + 20.0;
            let input_y = title_y + ui.ui_line_height + 15.0;
            let input_w = modal_w - 40.0;
            let input_h = ui.ui_line_height + 8.0;
            if mx >= input_x && mx <= input_x + input_w && my >= input_y && my <= input_y + input_h
            {
                window.set_cursor_icon(winit::window::CursorIcon::Text);
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Returns `true` if mouse is over a pointer-cursor area within the active modal.
fn check_modal_pointer(
    ui: &UiState,
    size: winit::dpi::PhysicalSize<u32>,
    modal: crate::machkit::ModalType,
    mx: f32,
    my: f32,
) -> bool {
    let modal_w = compute_modal_w(ui, modal);
    let modal_h = compute_modal_h(ui, modal);
    let modal_x = ((size.width as f32 - modal_w) / 2.0).round();
    let modal_y = ((size.height as f32 - modal_h) / 2.0).round();

    if mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h {
        return false;
    }

    match modal {
        crate::machkit::ModalType::CommandPalette | crate::machkit::ModalType::GlobalSearch => {
            let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
            my >= modal_y + header_h
        }
        crate::machkit::ModalType::Settings => {
            check_settings_modal_pointer(ui, modal_x, modal_y, modal_w, modal_h, mx, my)
        }
        crate::machkit::ModalType::About => {
            let close_btn_w = (12.0 * ui.ui_char_width).max(100.0).round();
            let close_btn_h = (ui.ui_line_height * 1.6).max(30.0).round();
            let close_btn_x = modal_x + ((modal_w - close_btn_w) / 2.0).round();
            let close_btn_y = modal_y + modal_h - close_btn_h - (ui.ui_line_height * 1.0).round();
            mx >= close_btn_x
                && mx <= close_btn_x + close_btn_w
                && my >= close_btn_y
                && my <= close_btn_y + close_btn_h
        }
        crate::machkit::ModalType::UnsavedChanges => {
            let btn_w = 130.0f32;
            let btn_h = 34.0f32;
            let spacing = 15.0f32;
            let total_btn_block_w = 3.0 * btn_w + 2.0 * spacing;
            let start_btn_x = modal_x + ((modal_w - total_btn_block_w) / 2.0).round();
            let btn_y = modal_y + modal_h - btn_h - 20.0;
            let ds_x = start_btn_x + btn_w + spacing;
            let c_x = start_btn_x + 2.0 * (btn_w + spacing);
            (mx >= start_btn_x && mx <= start_btn_x + btn_w && my >= btn_y && my <= btn_y + btn_h)
                || (mx >= ds_x && mx <= ds_x + btn_w && my >= btn_y && my <= btn_y + btn_h)
                || (mx >= c_x && mx <= c_x + btn_w && my >= btn_y && my <= btn_y + btn_h)
        }
        crate::machkit::ModalType::SidebarInput => {
            let btn_w = 80.0f32;
            let btn_h = 24.0f32;
            let cancel_x = modal_x + modal_w - 20.0 - btn_w * 2.0 - 10.0;
            let confirm_x = modal_x + modal_w - 20.0 - btn_w;
            let title_y = modal_y + 20.0;
            let input_y = title_y + ui.ui_line_height + 15.0;
            let input_h = ui.ui_line_height + 8.0;
            let btn_y = input_y + input_h + 15.0;
            (mx >= cancel_x && mx <= cancel_x + btn_w && my >= btn_y && my <= btn_y + btn_h)
                || (mx >= confirm_x
                    && mx <= confirm_x + btn_w
                    && my >= btn_y
                    && my <= btn_y + btn_h)
        }
    }
}

/// Returns the modal width for a given modal type.
fn compute_modal_w(ui: &UiState, modal: crate::machkit::ModalType) -> f32 {
    match modal {
        crate::machkit::ModalType::Settings => (45.0 * ui.ui_char_width).max(500.0).round(),
        crate::machkit::ModalType::About => 520.0,
        crate::machkit::ModalType::CommandPalette => (50.0 * ui.ui_char_width).max(500.0).round(),
        crate::machkit::ModalType::UnsavedChanges => 520.0,
        crate::machkit::ModalType::SidebarInput => 400.0,
        crate::machkit::ModalType::GlobalSearch => 650.0,
    }
}

/// Returns the modal height for a given modal type.
fn compute_modal_h(ui: &UiState, modal: crate::machkit::ModalType) -> f32 {
    match modal {
        crate::machkit::ModalType::Settings => {
            let row_height = (ui.ui_line_height * 2.2).round();
            (row_height * 8.2).max(430.0).round()
        }
        crate::machkit::ModalType::About => 190.0,
        crate::machkit::ModalType::CommandPalette => {
            let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
            let filtered_len = ui.get_filtered_commands().len();
            let visible_items = filtered_len.min(10);
            let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
            (header_h + visible_items as f32 * item_height).round()
        }
        crate::machkit::ModalType::UnsavedChanges => 200.0,
        crate::machkit::ModalType::SidebarInput => 150.0,
        crate::machkit::ModalType::GlobalSearch => {
            let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
            let count = ui.global_search_results.len().min(10).max(1);
            let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
            (header_h + count as f32 * item_height).round()
        }
    }
}

/// Check Settings modal pointer areas.
fn check_settings_modal_pointer(
    ui: &UiState,
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
    modal_h: f32,
    mx: f32,
    my: f32,
) -> bool {
    let row_height = (ui.ui_line_height * 2.2).round();
    let control_x = modal_x + 24.0 * ui.ui_char_width;
    let btn_h = (ui.ui_line_height * 1.3).round().max(24.0);
    let btn_w = (ui.ui_char_width * 3.0).round().max(24.0);
    let backend_btn_w = (ui.ui_char_width * 10.0).round().max(80.0);
    let theme_btn_w = (ui.ui_char_width * 16.0).round().max(140.0);

    let inc_btn_x = control_x + btn_w + ui.ui_char_width;
    let opengl_btn_x = control_x + backend_btn_w + ui.ui_char_width;
    let disabled5_btn_x = control_x + backend_btn_w + ui.ui_char_width;
    let disabled6_btn_x = control_x + backend_btn_w + ui.ui_char_width;

    let close_btn_w = (12.0 * ui.ui_char_width).max(100.0).round();
    let close_btn_h = (ui.ui_line_height * 1.6).max(30.0).round();
    let close_btn_x = modal_x + ((modal_w - close_btn_w) / 2.0).round();
    let close_btn_y = modal_y + modal_h - close_btn_h - (ui.ui_line_height * 1.0).round();

    let rows: &[(f32, f32, f32, f32)] = &[
        (row_height * 1.0, btn_w, control_x, inc_btn_x),
        (row_height * 2.0, btn_w, control_x, inc_btn_x),
    ];
    for (base_y, _bw, ctrl_x, inc_x) in rows {
        let btn_y = modal_y + base_y + ((ui.ui_line_height - btn_h) / 2.0).round();
        if (mx >= *ctrl_x && mx <= ctrl_x + btn_w && my >= btn_y && my <= btn_y + btn_h)
            || (mx >= *inc_x && mx <= inc_x + btn_w && my >= btn_y && my <= btn_y + btn_h)
        {
            return true;
        }
    }

    let btn3_y = modal_y + row_height * 3.0 + ((ui.ui_line_height - btn_h) / 2.0).round();
    let btn4_y = modal_y + row_height * 4.0 + ((ui.ui_line_height - btn_h) / 2.0).round();
    let btn5_y = modal_y + row_height * 5.0 + ((ui.ui_line_height - btn_h) / 2.0).round();
    let btn6_y = modal_y + row_height * 6.0 + ((ui.ui_line_height - btn_h) / 2.0).round();

    let on_row3 = (mx >= control_x
        && mx <= control_x + backend_btn_w
        && my >= btn3_y
        && my <= btn3_y + btn_h)
        || (mx >= opengl_btn_x
            && mx <= opengl_btn_x + backend_btn_w
            && my >= btn3_y
            && my <= btn3_y + btn_h);
    let on_row4 =
        mx >= control_x && mx <= control_x + theme_btn_w && my >= btn4_y && my <= btn4_y + btn_h;
    let on_dropdown = ui.theme_dropdown_open && {
        let dropdown_y = btn4_y + btn_h;
        let item_height = (ui.ui_line_height * 1.5).round().max(24.0);
        let dropdown_h = 2.0 * item_height;
        mx >= control_x
            && mx <= control_x + theme_btn_w
            && my >= dropdown_y
            && my <= dropdown_y + dropdown_h
    };
    let on_row5 = (mx >= control_x
        && mx <= control_x + backend_btn_w
        && my >= btn5_y
        && my <= btn5_y + btn_h)
        || (mx >= disabled5_btn_x
            && mx <= disabled5_btn_x + backend_btn_w
            && my >= btn5_y
            && my <= btn5_y + btn_h);
    let on_row6 = (mx >= control_x
        && mx <= control_x + backend_btn_w
        && my >= btn6_y
        && my <= btn6_y + btn_h)
        || (mx >= disabled6_btn_x
            && mx <= disabled6_btn_x + backend_btn_w
            && my >= btn6_y
            && my <= btn6_y + btn_h);
    let on_close = mx >= close_btn_x
        && mx <= close_btn_x + close_btn_w
        && my >= close_btn_y
        && my <= close_btn_y + close_btn_h;

    on_row3 || on_row4 || on_dropdown || on_row5 || on_row6 || on_close
}

/// Returns true if the mouse is over a pointer-cursor area in the tab bar.
fn is_pointer_in_tabbar(
    ui: &UiState,
    state: &AppState,
    size: winit::dpi::PhysicalSize<u32>,
    hovered_pane_idx: usize,
    pane_top: f32,
    mouse_x: f32,
) -> bool {
    let sidebar_original = ui.sidebar_width;
    if state.inactive_panes.is_empty() {
        let start_x = ui.sidebar_width;
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let total_tabs_width: f32 = state
            .tabs
            .iter()
            .map(|t| {
                let file_name = ui.get_tab_name(t.path.as_deref());
                let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                (12.0 + 18.0 + name_w + close_reserved + 10.0_f32).max(110.0)
            })
            .sum();
        mouse_x >= start_x
            && mouse_x < (start_x + total_tabs_width - state.tab_scroll_x).min(size.width as f32)
    } else {
        let (start_x_0, end_x_0, start_x_1, end_x_1) = if state.is_split_horizontal {
            (
                sidebar_original,
                size.width as f32,
                sidebar_original,
                size.width as f32,
            )
        } else {
            let editor_area_width = size.width as f32 - sidebar_original;
            let pane_width = editor_area_width / 2.0;
            (
                sidebar_original,
                sidebar_original + pane_width,
                sidebar_original + pane_width,
                size.width as f32,
            )
        };

        let scroll_x_0 = state.get_pane_scroll_x(0);
        let scroll_x_1 = state.get_pane_scroll_x(1);
        let tabs_0 = if state.active_pane_idx == 0 {
            &state.tabs
        } else {
            &state.inactive_panes[0].tabs
        };
        let tabs_1 = if state.active_pane_idx == 1 {
            &state.tabs
        } else {
            &state.inactive_panes[0].tabs
        };

        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let _ = pane_top;
        if hovered_pane_idx == 0 {
            let total_w: f32 = tabs_0
                .iter()
                .map(|t| {
                    let file_name = ui.get_tab_name(t.path.as_deref());
                    let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                    (12.0 + 18.0 + name_w + close_reserved + 10.0_f32).max(110.0)
                })
                .sum();
            mouse_x >= start_x_0 && mouse_x < (start_x_0 + total_w - scroll_x_0).min(end_x_0)
        } else {
            let total_w: f32 = tabs_1
                .iter()
                .map(|t| {
                    let file_name = ui.get_tab_name(t.path.as_deref());
                    let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                    (12.0 + 18.0 + name_w + close_reserved + 10.0_f32).max(110.0)
                })
                .sum();
            mouse_x >= start_x_1 && mouse_x < (start_x_1 + total_w - scroll_x_1).min(end_x_1)
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
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height)
            .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
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
            handle_tab_drag_reorder(
                ui,
                state,
                window,
                dragged_idx,
                pane_top,
                sidebar_width,
                w_width,
            );
        }
    } else if state.is_dragging_sidebar {
        let new_width = if state.mouse_x < 30.0 {
            0.0
        } else {
            state.mouse_x.clamp(50.0, 600.0)
        };
        ui.sidebar_width = new_width;
        ui.target_sidebar_width = new_width;
    } else if ui.tab_scroll_is_dragging {
        let tabbar_start_x = sidebar_width;
        let visible_width = w_width - tabbar_start_x;

        let mut total_tabs_width = 0.0f32;
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_paths = state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect::<Vec<_>>();
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = ui.get_tab_name(path_opt.as_deref());
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
            state.tab_scroll_x = scroll_ratio * max_scroll_x;
        } else {
            state.tab_scroll_x = 0.0;
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
            handle_scroll_drag(ui, state, w_width, pane_top, pane_bottom);
        } else if state.is_dragging_horizontal_scroll {
            let max_line_digits = state.tabs[state.active_tab_idx]
                .buffer
                .len()
                .to_string()
                .len()
                .max(3);
            let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
            let text_area_x = ui.sidebar_width + gutter_width;
            let scrollbar_width = ui.scrollbar_width();
            let minimap_width = ui.minimap_width();
            let sb_x = w_width - scrollbar_width;
            let minimap_x = sb_x - minimap_width;
            let text_viewport_w = (minimap_x - text_area_x).max(10.0);

            let max_line_len = ui.get_max_line_len(
                &state.tabs[state.active_tab_idx].buffer,
                state.tabs[state.active_tab_idx].path.as_deref(),
                state.tabs[state.active_tab_idx].cursor.line,
            );
            let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
            let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
            let thumb_w =
                (text_viewport_w * ratio_x).clamp(20.0_f32.min(text_viewport_w), text_viewport_w);
            let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
            let relative_x = state.mouse_x - text_area_x - state.scroll_drag_offset_x;
            let scroll_range = text_viewport_w - thumb_w;
            let scroll_ratio = if scroll_range > 0.0 {
                (relative_x / scroll_range).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
        } else if state.is_dragging_minimap {
            let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
            let status_y = pane_bottom.round();
            let total_editor_height = status_y - editor_top;
            let editor_height = total_editor_height - 14.0;
            let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
            let max_scroll = (state.tabs[state.active_tab_idx].buffer.len() as isize
                - visible_lines as isize)
                .max(0) as f32;
            let relative_y = state.mouse_y - editor_top;

            let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
            let minimap_total_h =
                state.tabs[state.active_tab_idx].buffer.len() as f32 * minimap_line_height;

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

            ui.scroll_y = clicked_line
                .saturating_sub(visible_lines / 2)
                .min(max_scroll as usize);
        } else if state.is_dragging {
            handle_drag_selection(ui, state, pane_top, w_width, size.height as f32);
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

fn handle_tab_drag_reorder(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    dragged_idx: usize,
    pane_top: f32,
    sidebar_width: f32,
    w_width: f32,
) {
    let is_inside_tabbar = state.mouse_y >= pane_top && state.mouse_y < pane_top + ui.tabbar_height;
    if is_inside_tabbar {
        let tabbar_start_x = sidebar_width;
        let mut tab_widths = Vec::new();
        let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
        let close_reserved = 8.0f32 + tab_close_icon_sz;
        let tab_paths = state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect::<Vec<_>>();
        for idx in 0..tab_paths.len() {
            let path_opt = &tab_paths[idx];
            let file_name = ui.get_tab_name(path_opt.as_deref());
            let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
            let dot_reserved = 18.0f32;
            let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
            tab_widths.push(tab_w);
        }

        let mut hovered_idx = None;
        let mut current_tab_x = tabbar_start_x;
        for idx in 0..state.tabs.len() {
            let tab_w = tab_widths[idx];
            let draw_x = current_tab_x - state.tab_scroll_x;
            let clip_left = draw_x.max(tabbar_start_x);
            let clip_right = (draw_x + tab_w).min(w_width);

            if clip_left < clip_right && state.mouse_x >= clip_left && state.mouse_x < clip_right {
                hovered_idx = Some(idx);
                break;
            }
            current_tab_x += tab_w;
        }

        if let Some(h_idx) = hovered_idx
            && h_idx != dragged_idx
        {
            let tab = state.tabs.remove(dragged_idx);
            state.tabs.insert(h_idx, tab);
            state.dragged_tab_idx = Some(h_idx);
            state.active_tab_idx = h_idx;
            window.request_redraw();
        }
    }
}

fn handle_scroll_drag(
    ui: &mut UiState,
    state: &mut AppState,
    w_width: f32,
    pane_top: f32,
    pane_bottom: f32,
) {
    let active_path = state.tabs[state.active_tab_idx]
        .path
        .as_deref()
        .unwrap_or("");
    let is_diagnostics =
        active_path.starts_with("diagnostics://") || active_path == "search://project";

    let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
    let status_y = pane_bottom.round();

    let show_horizontal_scrollbar = if is_diagnostics {
        false
    } else {
        let max_line_len = ui.get_max_line_len(
            &state.tabs[state.active_tab_idx].buffer,
            Some(active_path),
            state.tabs[state.active_tab_idx].cursor.line,
        );
        let max_line_digits = state.tabs[state.active_tab_idx]
            .buffer
            .len()
            .to_string()
            .len()
            .max(3);
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

    let virtual_len = if active_path == "search://project" {
        crate::machkit::components::editor::project_search::build_search_render_items(ui).len()
    } else if active_path.starts_with("diagnostics://") {
        let mut count = 0;
        for (file_path, diags) in &ui.lsp_diagnostics_details {
            if diags.is_empty() {
                continue;
            }
            let file_lines_len = ui
                .diagnostics_file_cache
                .get(file_path)
                .map(|l| l.len())
                .unwrap_or(0);
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
    let scroll_ratio = if scroll_range > 0.0 {
        (relative_y / scroll_range).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
}

fn handle_drag_selection(
    ui: &mut UiState,
    state: &mut AppState,
    pane_top: f32,
    w_width: f32,
    window_height: f32,
) {
    if state.tabs[state.active_tab_idx].path.as_deref() == Some("search://project") {
        let render_items =
            crate::machkit::components::editor::project_search::build_search_render_items(ui);
        if !render_items.is_empty() {
            let list_y = pane_top + ui.tabbar_height + ui.breadcrumb_height;
            let item_height = ui.buffer_line_height;
            let clicked_row = if state.mouse_y >= list_y {
                ((state.mouse_y - list_y) / item_height).floor() as usize + ui.scroll_y
            } else {
                ui.scroll_y
            };
            let clicked_row = clicked_row.min(render_items.len() - 1);

            // Get content/text of this row
            let text_area_x = ui.sidebar_width;
            let snippet_x = text_area_x + 60.0;
            let col_idx = match &render_items[clicked_row] {
                crate::machkit::SearchRenderItem::CodeLine { content, .. } => {
                    let display_content = content.replace('\t', "    ");
                    let char_count = display_content.chars().count();
                    if state.mouse_x >= snippet_x {
                        let raw =
                            ((state.mouse_x - snippet_x) / ui.buffer_char_width).round() as usize;
                        raw.min(char_count)
                    } else {
                        0
                    }
                }
                crate::machkit::SearchRenderItem::FileHeader { path } => {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let parent_dir = path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let parent_dir = parent_dir
                        .strip_prefix("./")
                        .unwrap_or(&parent_dir)
                        .to_string();
                    let header_text = if parent_dir.is_empty() {
                        file_name.to_string()
                    } else {
                        format!("{} {}/", file_name, parent_dir)
                    };
                    let char_count = header_text.chars().count();
                    if state.mouse_x >= text_area_x + 30.0 {
                        let raw = ((state.mouse_x - (text_area_x + 30.0)) / ui.buffer_char_width)
                            .round() as usize;
                        raw.min(char_count)
                    } else {
                        0
                    }
                }
                _ => 0,
            };

            ui.global_search_cursor_row = clicked_row;
            ui.global_search_col = col_idx;

            // Update ui.global_search_selected to the nearest match
            let mut target_res_idx = None;
            for i in (0..=clicked_row).rev() {
                if let crate::machkit::SearchRenderItem::CodeLine {
                    result_idx: Some(res_idx),
                    ..
                } = &render_items[i]
                {
                    target_res_idx = Some(*res_idx);
                    break;
                }
            }
            if target_res_idx.is_none() {
                for i in clicked_row..render_items.len() {
                    if let crate::machkit::SearchRenderItem::CodeLine {
                        result_idx: Some(res_idx),
                        ..
                    } = &render_items[i]
                    {
                        target_res_idx = Some(*res_idx);
                        break;
                    }
                }
            }
            if let Some(res_idx) = target_res_idx {
                ui.global_search_selected = res_idx;
                ui.last_global_search_selected = Some(res_idx);
            }
        }
        return;
    }

    let is_diagnostics = state.tabs[state.active_tab_idx]
        .path
        .as_deref()
        .is_some_and(|p| p.starts_with("diagnostics://") || p == "search://project");
    let max_line_digits = if is_diagnostics {
        3
    } else {
        state.tabs[state.active_tab_idx]
            .buffer
            .len()
            .to_string()
            .len()
            .max(3)
    };
    let gutter_width = if is_diagnostics {
        0.0
    } else {
        (max_line_digits as f32 + 2.0) * ui.buffer_char_width
    };
    let text_area_x = ui.sidebar_width + gutter_width;
    let scrollbar_width = ui.scrollbar_width();
    let minimap_width = if is_diagnostics {
        0.0
    } else {
        ui.minimap_width()
    };
    let sb_x = w_width - scrollbar_width;
    let minimap_x = sb_x - minimap_width;

    let editor_top = pane_top + ui.tabbar_height + ui.breadcrumb_height;
    let raw_line_idx = if state.mouse_y >= editor_top {
        ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y
    } else {
        ui.scroll_y
    };

    let line_idx = if is_diagnostics {
        let visual_lines =
            crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
        if visual_lines.is_empty() {
            0
        } else {
            raw_line_idx.min(visual_lines.len() - 1)
        }
    } else {
        raw_line_idx.min(
            state.tabs[state.active_tab_idx]
                .buffer
                .len()
                .saturating_sub(1),
        )
    };

    let mouse_x_clamped = state.mouse_x.min(minimap_x);
    let col_idx = if mouse_x_clamped > text_area_x {
        ((mouse_x_clamped - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x
    } else {
        0
    };

    let line_chars = if is_diagnostics {
        let visual_lines =
            crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
        visual_lines.get(line_idx).map_or(0, |vl| match vl {
            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code {
                line_content,
                ..
            } => line_content.chars().count(),
            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header {
                path,
                ..
            } => path.chars().count() + 10,
            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner {
                diag,
                ..
            } => diag.message.chars().count() + 10,
        })
    } else {
        state.tabs[state.active_tab_idx].buffer.lines()[line_idx]
            .chars()
            .count()
    };
    let col_idx = col_idx.min(line_chars);

    state.tabs[state.active_tab_idx].cursor.line = line_idx;
    state.tabs[state.active_tab_idx].cursor.col = col_idx;
    state.tabs[state.active_tab_idx].cursor.intended_col = col_idx;

    ui.scroll_to_cursor(
        &state.tabs[state.active_tab_idx].cursor,
        state.tabs[state.active_tab_idx].buffer.len(),
        w_width,
        window_height,
    );
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

    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height)
            .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let editor_bottom_limit = if ui.show_dock {
        dock_start_y
    } else {
        size.height as f32 - ui.status_height
    };

    if input_state == ElementState::Pressed && !state.inactive_panes.is_empty() {
        // Switch focus only if click is inside the editor area and outside sidebar
        if state.mouse_x >= sidebar_original
            && state.mouse_y >= main_y
            && state.mouse_y < editor_bottom_limit
        {
            let clicked_pane_idx = if state.is_split_horizontal {
                let editor_area_height = editor_bottom_limit - main_y;
                let pane_height = (editor_area_height / 2.0).round();
                if state.mouse_y < main_y + pane_height {
                    0
                } else {
                    1
                }
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
        if handle_right_click_sidebar(ui, state, window) {
            return;
        }
    }

    if button == MouseButton::Left {
        let size = window.inner_size();
        if input_state == ElementState::Pressed {
            ui.search_focused = false;
            ui.global_search_focused = false;
            // Check sidebar context menu click
            if handle_sidebar_context_menu_click(ui, state, window) {
                return;
            }

            // Check SidebarInput modal click
            if handle_sidebar_input_modal_click(ui, state, window, size) {
                return;
            }

            // Check Search Panel click
            let active_file_path = state
                .tabs
                .get(state.active_tab_idx)
                .and_then(|t| t.path.as_deref());
            let is_project_search = active_file_path == Some("search://project");
            if ui.show_search_panel || is_project_search {
                if true {
                    // Keep block nesting for brace matching
                    let mut bar_x = sidebar_original;
                    let mut bar_w = size.width as f32 - sidebar_original;
                    let mut bar_y = main_y + ui.tabbar_height;
                    let bar_h = ui.breadcrumb_height;

                    if !state.inactive_panes.is_empty() {
                        if state.is_split_horizontal {
                            let editor_area_height = editor_bottom_limit - main_y;
                            let pane_height = (editor_area_height / 2.0).round();
                            if state.active_pane_idx == 1 {
                                bar_y = main_y + pane_height + ui.tabbar_height;
                            }
                        } else {
                            let editor_area_width = size.width as f32 - sidebar_original;
                            let pane_width = editor_area_width / 2.0;
                            if state.active_pane_idx == 0 {
                                bar_w = pane_width;
                            } else {
                                bar_x = sidebar_original + pane_width;
                                bar_w = pane_width;
                            }
                        }
                    }

                    if state.mouse_x >= bar_x
                        && state.mouse_x < bar_x + bar_w
                        && state.mouse_y >= bar_y
                        && state.mouse_y < bar_y + bar_h
                    {
                        if is_project_search {
                            ui.global_search_focused = true;
                        } else {
                            ui.search_focused = true;
                        }
                        let is_local = !is_project_search;
                        let show_replace = if is_local {
                            ui.show_replace
                        } else {
                            ui.global_show_replace
                        };

                        let count_w = if is_local { 70.0f32 } else { 75.0f32 };
                        let btn_prev_w = 32.0f32;
                        let btn_next_w = 32.0f32;
                        let close_btn_w = 32.0f32;

                        let btn_rep_toggle_w = if is_local { 0.0f32 } else { 32.0f32 };
                        let btn_filter_w = if is_local { 0.0f32 } else { 32.0f32 };

                        let input_h = 32.0f32;
                        let path_h = if is_local { 24.0f32 } else { 0.0f32 };
                        let remaining_h = bar_h - path_h;
                        let row_h = if show_replace {
                            remaining_h / 2.0
                        } else {
                            remaining_h
                        };
                        let input_y_1 = bar_y + path_h + (row_h - input_h) / 2.0;
                        let input_y_2 = bar_y + path_h + row_h + (row_h - input_h) / 2.0;

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

                        let toggle_btn_w = 32.0f32;
                        let toggle_btn_x = bar_x + 10.0;
                        let input_start_x = toggle_btn_x + toggle_btn_w + 6.0;
                        let input_find_w = (count_x - 10.0 - input_start_x).max(50.0);

                        let pane_top = bar_y - ui.tabbar_height;
                        let pane_bottom = pane_top
                            + (if state.inactive_panes.is_empty() {
                                editor_bottom_limit - main_y
                            } else if state.is_split_horizontal {
                                (editor_bottom_limit - main_y) / 2.0
                            } else {
                                editor_bottom_limit - main_y
                            });

                        // Check Row 1 click (Find, Prev, Next, Close, and options inside find input)
                        if state.mouse_y >= input_y_1 && state.mouse_y < input_y_1 + input_h {
                            // Check Toggle Replace / Collapse All button click
                            if state.mouse_x >= toggle_btn_x
                                && state.mouse_x < toggle_btn_x + toggle_btn_w
                            {
                                if is_local {
                                    ui.show_replace = !ui.show_replace;
                                } else {
                                    let mut unique_files = std::collections::HashSet::new();
                                    for (path, _, _) in &ui.global_search_results {
                                        unique_files.insert(path.clone());
                                    }
                                    let unique_files_count = unique_files.len();
                                    let all_collapsed = unique_files_count > 0
                                        && ui.collapsed_search_files.len() >= unique_files_count;
                                    if all_collapsed {
                                        ui.collapsed_search_files.clear();
                                    } else {
                                        for path in unique_files {
                                            ui.collapsed_search_files.insert(path);
                                        }
                                    }
                                    ui.invalidate_search_render_items();
                                }
                                window.request_redraw();
                                return;
                            }

                            if !is_local {
                                // Click on Replace Toggle button
                                if state.mouse_x >= rep_toggle_x
                                    && state.mouse_x < rep_toggle_x + btn_rep_toggle_w
                                {
                                    ui.global_show_replace = !ui.global_show_replace;
                                    window.request_redraw();
                                    return;
                                }
                                // Click on Filter button
                                if state.mouse_x >= filter_x
                                    && state.mouse_x < filter_x + btn_filter_w
                                {
                                    window.request_redraw();
                                    return;
                                }
                            }

                            // Check options inside Find input
                            let opt_btn_w = 26.0f32;
                            let opt_y = input_y_1 + 3.0;
                            let opt_h = input_h - 6.0;
                            let opt_regex_x = input_start_x + input_find_w - 5.0 - opt_btn_w;
                            let opt_word_x = opt_regex_x - 2.0 - opt_btn_w;
                            let opt_case_x = opt_word_x - 2.0 - opt_btn_w;

                            if state.mouse_y >= opt_y && state.mouse_y < opt_y + opt_h {
                                if state.mouse_x >= opt_case_x
                                    && state.mouse_x < opt_case_x + opt_btn_w
                                {
                                    if is_project_search {
                                        ui.global_search_case_sensitive =
                                            !ui.global_search_case_sensitive;
                                        let q = ui.global_search_query.clone();
                                        ui.run_global_search(q);
                                    } else {
                                        ui.search_case_sensitive = !ui.search_case_sensitive;
                                        ui.perform_search(state);
                                    }
                                    window.request_redraw();
                                    return;
                                }
                                if state.mouse_x >= opt_word_x
                                    && state.mouse_x < opt_word_x + opt_btn_w
                                {
                                    if is_project_search {
                                        ui.global_search_whole_word = !ui.global_search_whole_word;
                                        let q = ui.global_search_query.clone();
                                        ui.run_global_search(q);
                                    } else {
                                        ui.search_whole_word = !ui.search_whole_word;
                                        ui.perform_search(state);
                                    }
                                    window.request_redraw();
                                    return;
                                }
                                if state.mouse_x >= opt_regex_x
                                    && state.mouse_x < opt_regex_x + opt_btn_w
                                {
                                    if is_project_search {
                                        ui.global_search_regex = !ui.global_search_regex;
                                        let q = ui.global_search_query.clone();
                                        ui.run_global_search(q);
                                    } else {
                                        ui.search_regex = !ui.search_regex;
                                        ui.perform_search(state);
                                    }
                                    window.request_redraw();
                                    return;
                                }
                            }

                            // Click on Find input (excluding options)
                            let options_w = 3.0 * opt_btn_w + 10.0;
                            if state.mouse_x >= input_start_x
                                && state.mouse_x < input_start_x + input_find_w - options_w
                            {
                                if is_project_search {
                                    ui.global_search_focus_replace = false;
                                } else {
                                    ui.search_focus_replace = false;
                                }
                                window.request_redraw();
                                return;
                            }

                            // Click on Close button
                            if state.mouse_x >= close_x && state.mouse_x < close_x + close_btn_w {
                                if is_project_search {
                                    let idx = state.active_tab_idx;
                                    state.tabs.remove(idx);
                                    if state.tabs.is_empty() {
                                        if !state.inactive_panes.is_empty() {
                                            let target_pane = state.inactive_panes.remove(0);
                                            state.tabs = target_pane.tabs;
                                            state.active_tab_idx = target_pane
                                                .active_tab_idx
                                                .min(state.tabs.len().saturating_sub(1));
                                            state.active_pane_idx = 0;
                                            state.is_split_horizontal = false;
                                        } else {
                                            state.tabs.push(crate::app::state::Tab {
                                                path: None,
                                                buffer: crate::editor::buffer::Buffer::new(),
                                                cursor: crate::editor::cursor::Cursor::new(),
                                                secondary_cursors: Vec::new(),
                                                scroll_x: 0,
                                                scroll_y: 0,
                                            });
                                        }
                                    }
                                    state.active_tab_idx =
                                        state.active_tab_idx.min(state.tabs.len() - 1);
                                    ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
                                    ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
                                    if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                                        ui.selected_file = Some(std::path::PathBuf::from(path));
                                    } else {
                                        ui.selected_file = None;
                                    }
                                } else {
                                    ui.show_search_panel = false;
                                }
                                window.request_redraw();
                                return;
                            }

                            // Click on Prev button
                            if state.mouse_x >= prev_x && state.mouse_x < prev_x + btn_prev_w {
                                if is_project_search {
                                    if !ui.global_search_results.is_empty() {
                                        if ui.global_search_selected == 0 {
                                            ui.global_search_selected =
                                                ui.global_search_results.len() - 1;
                                        } else {
                                            ui.global_search_selected -= 1;
                                        }
                                        if ui.global_search_selected < ui.global_search_scroll {
                                            ui.global_search_scroll = ui.global_search_selected;
                                        } else {
                                            let results_height = (pane_bottom
                                                - pane_top
                                                - ui.tabbar_height
                                                - ui.breadcrumb_height)
                                                .max(0.0);
                                            let row_height = ui.ui_line_height;
                                            let visible_rows =
                                                (results_height / row_height).floor() as usize;
                                            if ui.global_search_selected
                                                >= ui.global_search_scroll + visible_rows
                                            {
                                                ui.global_search_scroll =
                                                    ui.global_search_selected + 1 - visible_rows;
                                            }
                                        }
                                    }
                                } else {
                                    if !ui.search_matches.is_empty() {
                                        if ui.active_search_match_idx == 0 {
                                            ui.active_search_match_idx =
                                                ui.search_matches.len() - 1;
                                        } else {
                                            ui.active_search_match_idx -= 1;
                                        }
                                        if state.active_tab_idx < state.tabs.len() {
                                            let (m_line, m_col) =
                                                ui.search_matches[ui.active_search_match_idx];
                                            let active_tab = &mut state.tabs[state.active_tab_idx];
                                            active_tab.cursor.line = m_line;
                                            active_tab.cursor.col = m_col;
                                            active_tab.cursor.clear_selection();

                                            let size = window.inner_size();
                                            ui.scroll_to_cursor(
                                                &active_tab.cursor,
                                                active_tab.buffer.len(),
                                                size.width as f32,
                                                size.height as f32,
                                            );
                                            active_tab.scroll_y = ui.scroll_y;
                                            active_tab.scroll_x = ui.scroll_x;
                                        }
                                    }
                                }
                                window.request_redraw();
                                return;
                            }

                            // Click on Next button
                            if state.mouse_x >= next_x && state.mouse_x < next_x + btn_next_w {
                                if is_project_search {
                                    if !ui.global_search_results.is_empty() {
                                        if ui.global_search_selected
                                            >= ui.global_search_results.len() - 1
                                        {
                                            ui.global_search_selected = 0;
                                        } else {
                                            ui.global_search_selected += 1;
                                        }
                                        if ui.global_search_selected < ui.global_search_scroll {
                                            ui.global_search_scroll = ui.global_search_selected;
                                        } else {
                                            let results_height = (pane_bottom
                                                - pane_top
                                                - ui.tabbar_height
                                                - ui.breadcrumb_height)
                                                .max(0.0);
                                            let row_height = ui.ui_line_height;
                                            let visible_rows =
                                                (results_height / row_height).floor() as usize;
                                            if ui.global_search_selected
                                                >= ui.global_search_scroll + visible_rows
                                            {
                                                ui.global_search_scroll =
                                                    ui.global_search_selected + 1 - visible_rows;
                                            }
                                        }
                                    }
                                } else {
                                    if !ui.search_matches.is_empty() {
                                        if ui.active_search_match_idx >= ui.search_matches.len() - 1
                                        {
                                            ui.active_search_match_idx = 0;
                                        } else {
                                            ui.active_search_match_idx += 1;
                                        }
                                        if state.active_tab_idx < state.tabs.len() {
                                            let (m_line, m_col) =
                                                ui.search_matches[ui.active_search_match_idx];
                                            let active_tab = &mut state.tabs[state.active_tab_idx];
                                            active_tab.cursor.line = m_line;
                                            active_tab.cursor.col = m_col;
                                            active_tab.cursor.clear_selection();

                                            let size = window.inner_size();
                                            ui.scroll_to_cursor(
                                                &active_tab.cursor,
                                                active_tab.buffer.len(),
                                                size.width as f32,
                                                size.height as f32,
                                            );
                                            active_tab.scroll_y = ui.scroll_y;
                                            active_tab.scroll_x = ui.scroll_x;
                                        }
                                    }
                                }
                                window.request_redraw();
                                return;
                            }
                        }

                        // Check Row 2 click (Replace input, Replace button, Replace All button)
                        if state.mouse_y >= input_y_2 && state.mouse_y < input_y_2 + input_h {
                            // Click on Replace input
                            if state.mouse_x >= input_start_x
                                && state.mouse_x < input_start_x + input_find_w
                            {
                                if is_project_search {
                                    ui.global_search_focus_replace = true;
                                } else {
                                    ui.search_focus_replace = true;
                                }
                                window.request_redraw();
                                return;
                            }

                            // Click on Replace button
                            if state.mouse_x >= prev_x && state.mouse_x < prev_x + btn_prev_w {
                                if is_project_search {
                                    if !ui.global_search_results.is_empty()
                                        && ui.global_search_selected
                                            < ui.global_search_results.len()
                                    {
                                        let (path, line_idx, _) = ui.global_search_results
                                            [ui.global_search_selected]
                                            .clone();

                                        let pattern = if ui.global_search_regex {
                                            ui.global_search_query.clone()
                                        } else {
                                            regex::escape(&ui.global_search_query)
                                        };
                                        let mut builder = regex::RegexBuilder::new(&pattern);
                                        builder.case_insensitive(!ui.global_search_case_sensitive);

                                        if let Ok(re) = builder.build() {
                                            let mut found_in_tab = false;
                                            for tab in &mut state.tabs {
                                                if let Some(ref tab_path) = tab.path
                                                    && crate::editor::get_absolute_path(tab_path)
                                                        == crate::editor::get_absolute_path(
                                                            &path.to_string_lossy(),
                                                        )
                                                {
                                                    if line_idx < tab.buffer.len() {
                                                        tab.buffer.commit_transaction();
                                                        tab.buffer.start_transaction();
                                                        let line_content =
                                                            &tab.buffer.lines()[line_idx];
                                                        let new_line = re
                                                            .replace_all(
                                                                line_content,
                                                                &ui.global_replace_query,
                                                            )
                                                            .to_string();
                                                        if new_line != *line_content {
                                                            tab.buffer.delete(
                                                                line_idx,
                                                                0,
                                                                line_idx,
                                                                line_content.chars().count(),
                                                            );
                                                            tab.buffer
                                                                .insert(line_idx, 0, &new_line);
                                                        }
                                                        tab.buffer.commit_transaction();
                                                    }
                                                    found_in_tab = true;
                                                    break;
                                                }
                                            }
                                            if !found_in_tab {
                                                for pane in &mut state.inactive_panes {
                                                    for tab in &mut pane.tabs {
                                                        if let Some(ref tab_path) = tab.path
                                                            && crate::editor::get_absolute_path(
                                                                tab_path,
                                                            ) == crate::editor::get_absolute_path(
                                                                &path.to_string_lossy(),
                                                            )
                                                        {
                                                            if line_idx < tab.buffer.len() {
                                                                tab.buffer.commit_transaction();
                                                                tab.buffer.start_transaction();
                                                                let line_content =
                                                                    &tab.buffer.lines()[line_idx];
                                                                let new_line = re
                                                                    .replace_all(
                                                                        line_content,
                                                                        &ui.global_replace_query,
                                                                    )
                                                                    .to_string();
                                                                if new_line != *line_content {
                                                                    tab.buffer.delete(
                                                                        line_idx,
                                                                        0,
                                                                        line_idx,
                                                                        line_content
                                                                            .chars()
                                                                            .count(),
                                                                    );
                                                                    tab.buffer.insert(
                                                                        line_idx, 0, &new_line,
                                                                    );
                                                                }
                                                                tab.buffer.commit_transaction();
                                                            }
                                                            found_in_tab = true;
                                                            break;
                                                        }
                                                    }
                                                    if found_in_tab {
                                                        break;
                                                    }
                                                }
                                            }
                                            if !found_in_tab {
                                                let path_clone = path.clone();
                                                let re_clone = re.clone();
                                                let replace_query = ui.global_replace_query.clone();
                                                std::thread::spawn(move || {
                                                    if let Ok(content) =
                                                        std::fs::read_to_string(&path_clone)
                                                    {
                                                        let mut lines: Vec<String> = content
                                                            .lines()
                                                            .map(|s| s.to_string())
                                                            .collect();
                                                        if line_idx < lines.len() {
                                                            let new_line = re_clone
                                                                .replace_all(
                                                                    &lines[line_idx],
                                                                    &replace_query,
                                                                )
                                                                .to_string();
                                                            lines[line_idx] = new_line;
                                                            let new_content = lines.join("\n");
                                                            let _ = std::fs::write(
                                                                &path_clone,
                                                                new_content,
                                                            );
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        let q = ui.global_search_query.clone();
                                        ui.run_global_search(q);
                                    }
                                } else {
                                    if !ui.search_matches.is_empty()
                                        && state.active_tab_idx < state.tabs.len()
                                    {
                                        let (m_line, m_col) =
                                            ui.search_matches[ui.active_search_match_idx];
                                        let active_tab = &mut state.tabs[state.active_tab_idx];
                                        active_tab.buffer.commit_transaction();
                                        active_tab.buffer.start_transaction();

                                        let q_len = ui.search_query.chars().count();
                                        active_tab.buffer.delete(
                                            m_line,
                                            m_col,
                                            m_line,
                                            m_col + q_len,
                                        );
                                        active_tab.buffer.insert(m_line, m_col, &ui.replace_query);
                                        active_tab.buffer.commit_transaction();

                                        active_tab.cursor.line = m_line;
                                        active_tab.cursor.col =
                                            m_col + ui.replace_query.chars().count();
                                        active_tab.cursor.clear_selection();

                                        ui.perform_search(state);
                                    }
                                }
                                window.request_redraw();
                                return;
                            }

                            // Click on Replace All button
                            if state.mouse_x >= next_x && state.mouse_x < next_x + btn_next_w {
                                if is_project_search {
                                    if !ui.global_search_query.is_empty() {
                                        let mut files_to_process = std::collections::HashSet::new();
                                        for (path, _, _) in &ui.global_search_results {
                                            files_to_process.insert(path.clone());
                                        }

                                        let pattern = if ui.global_search_regex {
                                            ui.global_search_query.clone()
                                        } else {
                                            regex::escape(&ui.global_search_query)
                                        };

                                        let mut builder = regex::RegexBuilder::new(&pattern);
                                        builder.case_insensitive(!ui.global_search_case_sensitive);

                                        if let Ok(re) = builder.build() {
                                            for path in files_to_process {
                                                let mut found_in_tab = false;
                                                for tab in &mut state.tabs {
                                                    if let Some(ref tab_path) = tab.path
                                                        && crate::editor::get_absolute_path(
                                                            tab_path,
                                                        ) == crate::editor::get_absolute_path(
                                                            &path.to_string_lossy(),
                                                        )
                                                    {
                                                        tab.buffer.commit_transaction();
                                                        tab.buffer.start_transaction();
                                                        for line_idx in 0..tab.buffer.len() {
                                                            let line_content =
                                                                &tab.buffer.lines()[line_idx];
                                                            let new_line = re
                                                                .replace_all(
                                                                    line_content,
                                                                    &ui.global_replace_query,
                                                                )
                                                                .to_string();
                                                            if new_line != *line_content {
                                                                tab.buffer.delete(
                                                                    line_idx,
                                                                    0,
                                                                    line_idx,
                                                                    line_content.chars().count(),
                                                                );
                                                                tab.buffer
                                                                    .insert(line_idx, 0, &new_line);
                                                            }
                                                        }
                                                        tab.buffer.commit_transaction();
                                                        found_in_tab = true;
                                                        break;
                                                    }
                                                }
                                                if !found_in_tab {
                                                    for pane in &mut state.inactive_panes {
                                                        for tab in &mut pane.tabs {
                                                            if let Some(ref tab_path) = tab.path
                                                                && crate::editor::get_absolute_path(tab_path) == crate::editor::get_absolute_path(&path.to_string_lossy()) {
                                                                    tab.buffer.commit_transaction();
                                                                    tab.buffer.start_transaction();
                                                                    for line_idx in 0..tab.buffer.len() {
                                                                        let line_content = &tab.buffer.lines()[line_idx];
                                                                        let new_line = re.replace_all(line_content, &ui.global_replace_query).to_string();
                                                                        if new_line != *line_content {
                                                                            tab.buffer.delete(line_idx, 0, line_idx, line_content.chars().count());
                                                                            tab.buffer.insert(line_idx, 0, &new_line);
                                                                        }
                                                                    }
                                                                    tab.buffer.commit_transaction();
                                                                    found_in_tab = true;
                                                                    break;
                                                                }
                                                        }
                                                        if found_in_tab {
                                                            break;
                                                        }
                                                    }
                                                }
                                                if !found_in_tab {
                                                    let path_clone = path.clone();
                                                    let re_clone = re.clone();
                                                    let replace_query =
                                                        ui.global_replace_query.clone();
                                                    std::thread::spawn(move || {
                                                        if let Ok(content) =
                                                            std::fs::read_to_string(&path_clone)
                                                        {
                                                            let new_content = re_clone
                                                                .replace_all(
                                                                    &content,
                                                                    &replace_query,
                                                                )
                                                                .to_string();
                                                            if new_content != content {
                                                                let _ = std::fs::write(
                                                                    &path_clone,
                                                                    new_content,
                                                                );
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                        let q = ui.global_search_query.clone();
                                        ui.run_global_search(q);
                                    }
                                } else {
                                    if !ui.search_matches.is_empty()
                                        && state.active_tab_idx < state.tabs.len()
                                    {
                                        let active_tab = &mut state.tabs[state.active_tab_idx];
                                        active_tab.buffer.commit_transaction();
                                        active_tab.buffer.start_transaction();

                                        let q_len = ui.search_query.chars().count();
                                        let mut matches_to_replace = ui.search_matches.clone();
                                        matches_to_replace.sort_by(|a, b| b.cmp(a)); // Descending order

                                        for (m_line, m_col) in matches_to_replace {
                                            active_tab.buffer.delete(
                                                m_line,
                                                m_col,
                                                m_line,
                                                m_col + q_len,
                                            );
                                            active_tab.buffer.insert(
                                                m_line,
                                                m_col,
                                                &ui.replace_query,
                                            );
                                        }

                                        active_tab.buffer.commit_transaction();
                                        active_tab.cursor.clear_selection();
                                        ui.perform_search(state);
                                    }
                                }
                                window.request_redraw();
                                return;
                            }
                            return;
                        }
                    }
                }
                // Check if click is on tab scrollbar
                let tabbar_start_x = ui.sidebar_width;
                let visible_width = w_width - tabbar_start_x;
                let mut total_tabs_width = 0.0f32;
                let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
                let close_reserved = 8.0f32 + tab_close_icon_sz;
                let tab_paths = state
                    .tabs
                    .iter()
                    .map(|t| t.path.clone())
                    .collect::<Vec<_>>();
                for idx in 0..tab_paths.len() {
                    let path_opt = &tab_paths[idx];
                    let file_name = ui.get_tab_name(path_opt.as_deref());
                    let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
                    let dot_reserved = 18.0f32;
                    let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
                    total_tabs_width += tab_w;
                }

                if total_tabs_width > visible_width {
                    let main_y = ui.titlebar_height;
                    let mut pane_top = main_y;
                    if !state.inactive_panes.is_empty() && state.is_split_horizontal {
                        let editor_area_height = editor_bottom_limit - main_y;
                        let pane_height = (editor_area_height / 2.0).round();
                        if state.active_pane_idx == 1 {
                            pane_top = main_y + pane_height;
                        }
                    }

                    let is_on_scrollbar = state.mouse_y >= pane_top + ui.tabbar_height - 6.0
                        && state.mouse_y < pane_top + ui.tabbar_height
                        && state.mouse_x >= tabbar_start_x
                        && state.mouse_x < w_width;

                    if is_on_scrollbar {
                        let ratio = visible_width / total_tabs_width;
                        let thumb_w = (visible_width * ratio)
                            .clamp(20.0_f32.min(visible_width), visible_width);
                        let max_scroll_x = total_tabs_width - visible_width;
                        let scroll_ratio_x = if max_scroll_x > 0.0 {
                            state.tab_scroll_x / max_scroll_x
                        } else {
                            0.0
                        };
                        let thumb_x = tabbar_start_x + scroll_ratio_x * (visible_width - thumb_w);

                        ui.tab_scroll_is_dragging = true;
                        let is_on_thumb =
                            state.mouse_x >= thumb_x && state.mouse_x < thumb_x + thumb_w;
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
                            state.tab_scroll_x =
                                (target_ratio * max_scroll_x).clamp(0.0, max_scroll_x);
                        }
                        window.request_redraw();
                        return;
                    }
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
                dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height)
                    .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
            }

            // Check if focus changes
            state.terminal_focus =
                ui.show_dock && state.mouse_x >= ui.sidebar_width && state.mouse_y >= dock_start_y;

            // Check if click is on dock resize border
            let on_dock_border = ui.show_dock && (state.mouse_y - dock_start_y).abs() <= 4.0;

            if on_dock_border {
                state.is_dragging_dock_border = true;
            } else if ui.active_modal.is_some() {
                let tab_paths: Vec<Option<String>> =
                    state.tabs.iter().map(|t| t.path.clone()).collect();
                let tab_modified: Vec<bool> =
                    state.tabs.iter().map(|t| t.buffer.is_modified).collect();
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
                        state.tab_scroll_x,
                    )
                };
                handle_action(ui, state, action_res, window, elwt, gpu, atlas, font_bytes);
            } else {
                // Check if click is on sidebar resize border
                let on_sidebar_border =
                    ui.sidebar_width > 0.0 && (state.mouse_x - ui.sidebar_width).abs() <= 4.0;
                if on_sidebar_border {
                    state.is_dragging_sidebar = true;
                } else {
                    let tab_paths: Vec<Option<String>> =
                        state.tabs.iter().map(|t| t.path.clone()).collect();
                    let tab_modified: Vec<bool> =
                        state.tabs.iter().map(|t| t.buffer.is_modified).collect();
                    let action_res = {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        let orig_titlebar_height = ui.titlebar_height;
                        if !state.inactive_panes.is_empty()
                            && state.is_split_horizontal
                            && state.mouse_x >= sidebar_original
                        {
                            let main_y = ui.titlebar_height;
                            let mut dock_start_y = size.height as f32 - ui.status_height;
                            if ui.show_dock {
                                dock_start_y = (size.height as f32
                                    - ui.status_height
                                    - ui.dock_height)
                                    .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
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
                            state.tab_scroll_x,
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
                                dock_start_y = (size.height as f32
                                    - ui.status_height
                                    - ui.dock_height)
                                    .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
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
                            let is_diagnostics =
                                state.tabs[active_tab_idx].path.as_deref().is_some_and(|p| {
                                    p.starts_with("diagnostics://") || p == "search://project"
                                });
                            let active_tab_len = state.tabs[active_tab_idx].buffer.len();
                            let max_line_digits = active_tab_len.to_string().len().max(3);
                            let gutter_width = if is_diagnostics {
                                0.0
                            } else {
                                (max_line_digits as f32 + 2.0) * ui.buffer_char_width
                            };
                            let text_area_x = ui.sidebar_width + gutter_width;
                            let scrollbar_width = ui.scrollbar_width();
                            let minimap_width = if is_diagnostics {
                                0.0
                            } else {
                                ui.minimap_width()
                            };
                            let sb_x = w_width - scrollbar_width;
                            let minimap_x = sb_x - minimap_width;
                            let text_viewport_w = (minimap_x - text_area_x).max(10.0);

                            let show_horizontal_scrollbar = if is_diagnostics {
                                false
                            } else {
                                let max_line_len = ui.get_max_line_len(
                                    &state.tabs[active_tab_idx].buffer,
                                    state.tabs[active_tab_idx].path.as_deref(),
                                    state.tabs[active_tab_idx].cursor.line,
                                );
                                let visible_cols =
                                    (text_viewport_w / ui.buffer_char_width).floor() as usize;
                                max_line_len > visible_cols
                            };
                            let hs_height = if show_horizontal_scrollbar { 14.0 } else { 0.0 };
                            let editor_height = editor_bottom_limit - editor_top - hs_height;

                            let active_path =
                                state.tabs[active_tab_idx].path.as_deref().unwrap_or("");
                            let virtual_len = if active_path == "search://project" {
                                crate::machkit::components::editor::project_search::build_search_render_items(ui).len()
                            } else if is_diagnostics {
                                let mut count = 0;
                                for (file_path, diags) in &ui.lsp_diagnostics_details {
                                    if diags.is_empty() {
                                        continue;
                                    }
                                    let file_lines_len = ui
                                        .diagnostics_file_cache
                                        .get(file_path)
                                        .map(|l| l.len())
                                        .unwrap_or(0);
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
                            if !is_diagnostics
                                && handle_minimap_click(
                                    ui,
                                    state,
                                    window,
                                    editor_top,
                                    editor_bottom_limit,
                                    editor_height,
                                    minimap_x,
                                    sb_x,
                                    active_tab_idx,
                                    active_tab_len,
                                )
                            {
                            }
                            // 2. Check if click is on scrollbar
                            else if handle_scrollbar_click(
                                ui,
                                state,
                                editor_top,
                                editor_bottom_limit,
                                editor_height,
                                sb_x,
                                virtual_len,
                            ) {
                            }
                            // 3. Check if click is on horizontal scrollbar
                            else if show_horizontal_scrollbar
                                && handle_horizontal_scrollbar_click(
                                    ui,
                                    state,
                                    editor_bottom_limit,
                                    text_area_x,
                                    text_viewport_w,
                                    minimap_x,
                                    active_tab_idx,
                                )
                            {
                            } else {
                                // Click inside editor area
                                let bottom_limit = if show_horizontal_scrollbar {
                                    editor_bottom_limit - 14.0
                                } else {
                                    editor_bottom_limit
                                };
                                if state.mouse_x >= text_area_x
                                    && state.mouse_x < minimap_x
                                    && state.mouse_y >= editor_top
                                    && state.mouse_y < bottom_limit
                                {
                                    if state.tabs[active_tab_idx].path.as_deref()
                                        == Some("search://project")
                                        && handle_project_search_click(
                                            ui,
                                            state,
                                            window,
                                            elwt,
                                            gpu,
                                            atlas,
                                            font_bytes,
                                            editor_top,
                                            text_area_x,
                                            text_viewport_w,
                                        )
                                    {
                                        return;
                                    }

                                    // 2. Check if virtual diagnostics tab item was clicked
                                    if state.tabs[active_tab_idx].path.as_deref()
                                        == Some("diagnostics://project")
                                        && handle_diagnostics_click(
                                            ui,
                                            state,
                                            window,
                                            elwt,
                                            gpu,
                                            atlas,
                                            font_bytes,
                                            editor_top,
                                            text_area_x,
                                            active_tab_idx,
                                        )
                                    {
                                        return;
                                    }

                                    // Normal click
                                    handle_text_area_cursor_click(
                                        ui,
                                        state,
                                        editor_top,
                                        text_area_x,
                                        active_tab_idx,
                                    );
                                    let active_tab = &state.tabs[active_tab_idx];
                                    ui.scroll_to_cursor(
                                        &active_tab.cursor,
                                        active_tab.buffer.len(),
                                        size.width as f32,
                                        size.height as f32,
                                    );
                                }
                            }
                        }
                        UiAction::SelectTab(idx) => {
                            state.dragged_tab_idx = Some(idx);
                            state.drag_start_pos = Some((state.mouse_x, state.mouse_y));
                            handle_action(
                                ui,
                                state,
                                UiAction::SelectTab(idx),
                                window,
                                elwt,
                                gpu,
                                atlas,
                                font_bytes,
                            );
                        }
                        UiAction::OpenFile(path) => {
                            handle_action(
                                ui,
                                state,
                                UiAction::OpenFile(path),
                                window,
                                elwt,
                                gpu,
                                atlas,
                                font_bytes,
                            );
                            state.dragged_tab_idx = Some(state.active_tab_idx);
                            state.drag_start_pos = Some((state.mouse_x, state.mouse_y));
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
                handle_tab_release(ui, state, window, dragged_idx, size);
            }

            if let Some((s_l, s_c, e_l, e_c)) =
                state.tabs[state.active_tab_idx].cursor.selection_range()
                && s_l == e_l
                && s_c == e_c
            {
                state.tabs[state.active_tab_idx].cursor.clear_selection();
            }
        }
        update_cursor_icon(window, ui, state);
        window.request_redraw();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Private helper functions extracted from handle_mouse_input
// ──────────────────────────────────────────────────────────────────────────────

/// Handles a right-click on the sidebar file tree to open a context menu.
/// Returns true if the click was consumed.
fn handle_right_click_sidebar(ui: &mut UiState, state: &AppState, window: &Arc<Window>) -> bool {
    let size = window.inner_size();
    let main_y = ui.titlebar_height;
    if ui.sidebar_width > 0.0
        && state.mouse_x >= 0.0
        && state.mouse_x < ui.sidebar_width
        && state.mouse_y > main_y
        && state.mouse_y < size.height as f32 - ui.status_height
    {
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
        true
    } else {
        false
    }
}

/// Handles a left-click on an open sidebar context menu.
/// Returns true if the click was consumed (even if nothing actionable was hit,
/// the context menu is always dismissed).
fn handle_sidebar_context_menu_click(
    ui: &mut UiState,
    state: &AppState,
    window: &Arc<Window>,
) -> bool {
    let (menu_x, menu_y, target_path, _is_dir) = match ui.sidebar_context_menu.clone() {
        Some(m) => m,
        None => return false,
    };
    ui.sidebar_context_menu = None;

    let item_height = ui.ui_line_height;
    let menu_w = 120.0f32;
    let menu_h = 4.0 * item_height;

    if state.mouse_x >= menu_x
        && state.mouse_x < menu_x + menu_w
        && state.mouse_y >= menu_y
        && state.mouse_y < menu_y + menu_h
    {
        let idx = ((state.mouse_y - menu_y) / item_height).floor() as usize;
        match idx {
            0 => {
                // New File
                ui.active_modal = Some(crate::machkit::ModalType::SidebarInput);
                ui.sidebar_input_type = "new_file".to_string();
                ui.sidebar_input_target = target_path;
                ui.sidebar_input_value.clear();
            }
            1 => {
                // New Folder
                ui.active_modal = Some(crate::machkit::ModalType::SidebarInput);
                ui.sidebar_input_type = "new_folder".to_string();
                ui.sidebar_input_target = target_path;
                ui.sidebar_input_value.clear();
            }
            2 => {
                // Rename
                ui.active_modal = Some(crate::machkit::ModalType::SidebarInput);
                ui.sidebar_input_type = "rename".to_string();
                ui.sidebar_input_value = target_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                ui.sidebar_input_target = target_path;
            }
            3 => {
                // Delete
                if target_path.is_dir() {
                    let _ = std::fs::remove_dir_all(&target_path);
                } else {
                    let _ = std::fs::remove_file(&target_path);
                }
                ui.rebuild_tree();
            }
            _ => {}
        }
    }
    window.request_redraw();
    true
}

/// Handles a left-click while the SidebarInput modal (file/folder creation) is active.
/// Returns true if the click was consumed.
fn handle_sidebar_input_modal_click(
    ui: &mut UiState,
    state: &AppState,
    window: &Arc<Window>,
    size: winit::dpi::PhysicalSize<u32>,
) -> bool {
    if ui.active_modal != Some(crate::machkit::ModalType::SidebarInput) {
        return false;
    }
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

    // Cancel button
    if state.mouse_x >= cancel_x
        && state.mouse_x <= cancel_x + btn_w
        && state.mouse_y >= btn_y
        && state.mouse_y <= btn_y + btn_h
    {
        ui.active_modal = None;
        window.request_redraw();
        return true;
    }

    // Confirm button
    if state.mouse_x >= confirm_x
        && state.mouse_x <= confirm_x + btn_w
        && state.mouse_y >= btn_y
        && state.mouse_y <= btn_y + btn_h
    {
        let target = ui.sidebar_input_target.clone();
        let val = ui.sidebar_input_value.clone();
        if is_safe_sidebar_name(&val) {
            match ui.sidebar_input_type.as_str() {
                "new_file" => {
                    let parent = if target.is_dir() {
                        target.clone()
                    } else {
                        target
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                    };
                    let _ = std::fs::File::create(parent.join(&val));
                }
                "new_folder" => {
                    let parent = if target.is_dir() {
                        target.clone()
                    } else {
                        target
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                    };
                    let _ = std::fs::create_dir_all(parent.join(&val));
                }
                "rename" => {
                    if let Some(parent) = target.parent() {
                        let _ = std::fs::rename(&target, parent.join(&val));
                    }
                }
                _ => {}
            }
        }
        ui.active_modal = None;
        ui.rebuild_tree();
        window.request_redraw();
        return true;
    }

    // Click outside modal dismisses it
    let clicked_outside = state.mouse_x < modal_x
        || state.mouse_x > modal_x + modal_w
        || state.mouse_y < modal_y
        || state.mouse_y > modal_y + modal_h;
    if clicked_outside {
        ui.active_modal = None;
        window.request_redraw();
    }
    true
}

fn is_safe_sidebar_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// Handles minimap click. Returns true if the click was consumed.
fn handle_minimap_click(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Arc<Window>,
    editor_top: f32,
    editor_bottom_limit: f32,
    editor_height: f32,
    minimap_x: f32,
    sb_x: f32,
    active_tab_idx: usize,
    active_tab_len: usize,
) -> bool {
    if state.mouse_x < minimap_x
        || state.mouse_x >= sb_x
        || state.mouse_y < editor_top
        || state.mouse_y >= editor_bottom_limit
    {
        return false;
    }
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
    ui.scroll_y = clicked_line
        .saturating_sub(visible_lines / 2)
        .min(max_scroll as usize);
    window.request_redraw();
    true
}

/// Handles vertical scrollbar click. Returns true if the click was consumed.
fn handle_scrollbar_click(
    ui: &mut UiState,
    state: &mut AppState,
    editor_top: f32,
    editor_bottom_limit: f32,
    editor_height: f32,
    sb_x: f32,
    virtual_len: usize,
) -> bool {
    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
    if state.mouse_x < sb_x
        || state.mouse_y < editor_top
        || state.mouse_y >= editor_bottom_limit
        || virtual_len <= visible_lines
    {
        return false;
    }
    state.is_dragging_scroll = true;
    let ratio = visible_lines as f32 / virtual_len as f32;
    let thumb_h = (editor_height * ratio).clamp(20.0_f32.min(editor_height), editor_height);
    let max_scroll = (virtual_len as isize - visible_lines as isize).max(0) as f32;
    let scroll_ratio = if max_scroll > 0.0 {
        ui.scroll_y as f32 / max_scroll
    } else {
        0.0
    };
    let thumb_y = editor_top + scroll_ratio * (editor_height - thumb_h);

    if state.mouse_y >= thumb_y && state.mouse_y < thumb_y + thumb_h {
        state.scroll_drag_offset_y = state.mouse_y - thumb_y;
    } else {
        state.scroll_drag_offset_y = thumb_h / 2.0;
        let relative_y = state.mouse_y - editor_top - state.scroll_drag_offset_y;
        let scroll_range = editor_height - thumb_h;
        let scroll_ratio = if scroll_range > 0.0 {
            (relative_y / scroll_range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
    }
    true
}

/// Handles horizontal scrollbar click. Returns true if the click was consumed.
fn handle_horizontal_scrollbar_click(
    ui: &mut UiState,
    state: &mut AppState,
    editor_bottom_limit: f32,
    text_area_x: f32,
    text_viewport_w: f32,
    minimap_x: f32,
    active_tab_idx: usize,
) -> bool {
    if state.mouse_y < editor_bottom_limit - 14.0
        || state.mouse_y >= editor_bottom_limit
        || state.mouse_x < text_area_x
        || state.mouse_x >= minimap_x
    {
        return false;
    }
    state.is_dragging_horizontal_scroll = true;
    let active_tab = &state.tabs[active_tab_idx];
    let max_line_len = ui.get_max_line_len(
        &active_tab.buffer,
        active_tab.path.as_deref(),
        active_tab.cursor.line,
    );
    let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
    let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
    let thumb_w = (text_viewport_w * ratio_x).clamp(20.0_f32.min(text_viewport_w), text_viewport_w);
    let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
    let scroll_ratio_x = if max_scroll_x > 0.0 {
        ui.scroll_x as f32 / max_scroll_x
    } else {
        0.0
    };
    let thumb_x = text_area_x + scroll_ratio_x * (text_viewport_w - thumb_w);

    if state.mouse_x >= thumb_x && state.mouse_x < thumb_x + thumb_w {
        state.scroll_drag_offset_x = state.mouse_x - thumb_x;
    } else {
        state.scroll_drag_offset_x = thumb_w / 2.0;
        let relative_x = state.mouse_x - text_area_x - state.scroll_drag_offset_x;
        let scroll_range = text_viewport_w - thumb_w;
        let scroll_ratio = if scroll_range > 0.0 {
            (relative_x / scroll_range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
    }
    true
}

/// Handles a click inside the project search result list.
/// Returns true if the click was consumed.
fn handle_project_search_click(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    editor_top: f32,
    text_area_x: f32,
    text_viewport_w: f32,
) -> bool {
    let list_y = editor_top;
    let item_height = ui.buffer_line_height;
    if state.mouse_y < list_y {
        ui.global_search_focused = true;
        ui.global_search_selection_anchor = None;
        return true;
    }
    let clicked_idx = ((state.mouse_y - list_y) / item_height).floor() as usize + ui.scroll_y;
    let render_items =
        crate::machkit::components::editor::project_search::build_search_render_items(ui);
    if clicked_idx >= render_items.len() {
        return true;
    }
    match &render_items[clicked_idx] {
        crate::machkit::SearchRenderItem::FileHeader { path } => {
            ui.global_search_focused = true;
            handle_project_search_header_click(
                ui,
                state,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
                path,
                text_area_x,
                text_viewport_w,
            );
        }
        crate::machkit::SearchRenderItem::CodeLine {
            path,
            line_idx,
            content,
            result_idx,
            is_first_in_range,
            is_last_in_range,
            start_line_of_range,
            end_line_of_range,
            ..
        } => {
            ui.global_search_focused = false;
            handle_project_search_code_click(
                ui,
                state,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
                path,
                *line_idx,
                content,
                *result_idx,
                *is_first_in_range,
                *is_last_in_range,
                *start_line_of_range,
                *end_line_of_range,
                text_area_x,
                clicked_idx,
            );
        }
        _ => {}
    }
    true
}

/// Handles click on a file-header row in the project search results.
fn handle_project_search_header_click(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    path: &std::path::PathBuf,
    text_area_x: f32,
    text_viewport_w: f32,
) {
    let btn_text = "Open File Alt-Enter";
    let btn_w = btn_text.chars().count() as f32 * ui.ui_char_width + 16.0;
    let btn_x = text_area_x + text_viewport_w - btn_w - 15.0;

    if state.mouse_x >= btn_x && state.mouse_x < btn_x + btn_w {
        // Open-file button clicked
        if let Some(pos) = ui
            .global_search_results
            .iter()
            .position(|(p, _, _)| p == path)
        {
            let (_, line_idx, _) = &ui.global_search_results[pos];
            crate::app::handler::handle_action(
                ui,
                state,
                UiAction::OpenFileAt(path.clone(), *line_idx),
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
        }
    } else {
        // Toggle collapse
        if ui.collapsed_search_files.contains(path) {
            ui.collapsed_search_files.remove(path);
        } else {
            ui.collapsed_search_files.insert(path.clone());
        }
        ui.invalidate_search_render_items();
    }
    ui.global_search_selection_anchor = None;
    window.request_redraw();
}

/// Handles click on a code-line row in the project search results.
#[allow(clippy::too_many_arguments)]
fn handle_project_search_code_click(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    _elwt: &EventLoopWindowTarget<()>,
    _gpu: &mut Option<GpuContext>,
    _atlas: &mut FontAtlas,
    _font_bytes: &[u8],
    path: &std::path::PathBuf,
    line_idx: usize,
    content: &str,
    result_idx: Option<usize>,
    is_first_in_range: bool,
    is_last_in_range: bool,
    start_line_of_range: usize,
    end_line_of_range: usize,
    text_area_x: f32,
    clicked_row: usize,
) {
    // Gutter expand/contract buttons
    if state.mouse_x >= text_area_x && state.mouse_x < text_area_x + 22.0 {
        if is_first_in_range {
            if let Some(pos) = ui.global_search_results.iter().position(|(p, l, _)| {
                p == path && *l >= start_line_of_range && *l <= end_line_of_range
            }) {
                let match_line = ui.global_search_results[pos].1;
                let entry = ui
                    .global_search_expanded_margins
                    .entry((path.clone(), match_line))
                    .or_insert((2, 2));
                entry.0 += 10;
                ui.invalidate_search_render_items();
            }
        } else if is_last_in_range
            && let Some(pos) = ui.global_search_results.iter().rposition(|(p, l, _)| {
                p == path && *l >= start_line_of_range && *l <= end_line_of_range
            })
        {
            let match_line = ui.global_search_results[pos].1;
            let entry = ui
                .global_search_expanded_margins
                .entry((path.clone(), match_line))
                .or_insert((2, 2));
            entry.1 += 10;
            ui.invalidate_search_render_items();
        }
    } else {
        // Select the clicked match, or find the nearest match in the same file if a context line is clicked
        let mut target_res_idx = result_idx;
        if target_res_idx.is_none() {
            let mut min_diff = usize::MAX;
            for (idx, (p, l, _)) in ui.global_search_results.iter().enumerate() {
                if p == path {
                    let diff = line_idx.abs_diff(*l);
                    if diff < min_diff {
                        min_diff = diff;
                        target_res_idx = Some(idx);
                    }
                }
            }
        }
        if let Some(res_idx) = target_res_idx {
            ui.global_search_selected = res_idx;
            ui.last_global_search_selected = Some(res_idx);
        }
        // Set focus to the results list (so typing/editing works)
        ui.global_search_focused = false;
        // Position the cursor at the clicked column
        let snippet_x = text_area_x + 60.0;
        let col_idx = if state.mouse_x >= snippet_x {
            let raw = ((state.mouse_x - snippet_x) / ui.buffer_char_width).round() as usize;
            let display_content = content.replace('\t', "    ");
            let char_count = display_content.chars().count();
            raw.min(char_count)
        } else {
            0
        };

        let old_row = ui.global_search_cursor_row;
        let old_col = ui.global_search_col;
        ui.global_search_cursor_row = clicked_row;
        ui.global_search_col = col_idx;
        if state.modifiers.shift_key() {
            if ui.global_search_selection_anchor.is_none() {
                ui.global_search_selection_anchor = Some((old_row, old_col));
            }
        } else {
            ui.global_search_selection_anchor = Some((clicked_row, col_idx));
        }
        state.is_dragging = true;
    }
    window.request_redraw();
}

/// Handles a click inside the diagnostics virtual view.
/// Returns `Some((path, line, col))` if the user opened a file, `None` otherwise.
fn handle_diagnostics_click(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    editor_top: f32,
    text_area_x: f32,
    active_tab_idx: usize,
) -> bool {
    let clicked_target = ui
        .diagnostics_click_targets
        .iter()
        .find(|t| {
            state.mouse_x >= t.0
                && state.mouse_x <= t.2
                && state.mouse_y >= t.1
                && state.mouse_y <= t.3
        })
        .cloned();

    let (target_path, target_line, target_col, target_type) =
        match clicked_target.map(|t| (t.4, t.5, t.6, t.7)) {
            Some(t) => t,
            None => return false,
        };

    if target_type == "header" {
        if state.mouse_x < text_area_x + 50.0 {
            // Toggle collapse
            if ui.collapsed_diagnostics.contains(&target_path) {
                ui.collapsed_diagnostics.remove(&target_path);
            } else {
                ui.collapsed_diagnostics.insert(target_path);
            }
            ui.diagnostics_changed = true;
            window.request_redraw();
            return true;
        }
        // Navigate to file
        let open_action =
            crate::machkit::UiAction::OpenFile(std::path::PathBuf::from(&target_path));
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
        let new_active_tab = &mut state.tabs[state.active_tab_idx];
        new_active_tab.cursor.line = target_line;
        new_active_tab.cursor.col = target_col;
        new_active_tab.cursor.intended_col = target_col;
        new_active_tab.cursor.selection_anchor = Some((target_line, target_col));
        let size = window.inner_size();
        ui.scroll_to_cursor(
            &new_active_tab.cursor,
            new_active_tab.buffer.len(),
            size.width as f32,
            size.height as f32,
        );
        window.request_redraw();
        return true;
    }

    if target_type == "code" {
        // Place virtual cursor in the diagnostics view
        let visual_lines =
            crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
        if !visual_lines.is_empty() {
            let clicked_line = (((state.mouse_y - editor_top) / ui.buffer_line_height).floor()
                as usize
                + ui.scroll_y)
                .min(visual_lines.len() - 1);
            let code_start_x = text_area_x + 48.0;
            let col_idx = ((state.mouse_x - code_start_x) / ui.buffer_char_width).round() as isize
                + ui.scroll_x as isize;
            let col_idx = col_idx.max(0) as usize;

            let active_tab = &mut state.tabs[active_tab_idx];
            if let Some(
                crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code {
                    line_content,
                    ..
                },
            ) = visual_lines.get(clicked_line)
            {
                let col_idx = col_idx.min(line_content.chars().count());
                active_tab.cursor.line = clicked_line;
                active_tab.cursor.col = col_idx;
                active_tab.cursor.intended_col = col_idx;
                active_tab.cursor.selection_anchor = Some((clicked_line, col_idx));
                state.is_dragging = true;
            }
        }
        window.request_redraw();
        return true;
    }
    false
}

/// Handles the normal text-area cursor placement click.
fn handle_text_area_cursor_click(
    ui: &mut UiState,
    state: &mut AppState,
    editor_top: f32,
    text_area_x: f32,
    active_tab_idx: usize,
) {
    let active_tab = &mut state.tabs[active_tab_idx];
    active_tab.buffer.commit_transaction();
    state.is_dragging = true;

    let line_idx =
        ((state.mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y;
    let line_idx = line_idx.min(active_tab.buffer.len() - 1);
    let col_idx = {
        let raw =
            ((state.mouse_x - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x;
        let line_chars = active_tab.buffer.lines()[line_idx].chars().count();
        raw.min(line_chars)
    };

    if state.modifiers.alt_key() {
        // Multi-cursor: add or remove cursor at clicked position
        let clicked_pos = (line_idx, col_idx);
        if let Some(idx) = active_tab
            .secondary_cursors
            .iter()
            .position(|c| c.line == clicked_pos.0 && c.col == clicked_pos.1)
        {
            active_tab.secondary_cursors.remove(idx);
        } else {
            let mut new_cur = crate::editor::cursor::Cursor::new();
            new_cur.line = line_idx;
            new_cur.col = col_idx;
            new_cur.intended_col = col_idx;
            new_cur.selection_anchor = Some((line_idx, col_idx));
            active_tab.secondary_cursors.push(new_cur);
        }
    } else {
        active_tab.secondary_cursors.clear();
        if state.modifiers.shift_key() {
            if active_tab.cursor.selection_anchor.is_none() {
                active_tab.cursor.selection_anchor =
                    Some((active_tab.cursor.line, active_tab.cursor.col));
            }
        } else {
            active_tab.cursor.selection_anchor = Some((line_idx, col_idx));
        }
        active_tab.cursor.line = line_idx;
        active_tab.cursor.col = col_idx;
        active_tab.cursor.intended_col = col_idx;
    }
}

/// Handles tab drag release — moves tabs between panes or spawns a new window.
fn handle_tab_release(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    dragged_idx: usize,
    size: winit::dpi::PhysicalSize<u32>,
) {
    let drag_start = state.drag_start_pos.take();
    let was_dragged = drag_start.is_some_and(|(sx, sy)| {
        let dx = state.mouse_x - sx;
        let dy = state.mouse_y - sy;
        (dx * dx + dy * dy).sqrt() >= 8.0
    });

    if !was_dragged {
        state.drag_start_pos = None;
        return;
    }

    let sidebar_original = ui.config.sidebar_width;
    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height)
            .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let editor_bottom_limit = if ui.show_dock {
        dock_start_y
    } else {
        size.height as f32 - ui.status_height
    };

    let is_outside = state.mouse_x < 0.0
        || state.mouse_x >= size.width as f32
        || state.mouse_y < 0.0
        || state.mouse_y >= size.height as f32;
    if is_outside {
        handle_tab_drag_outside_window(ui, state, window, dragged_idx, size, sidebar_original);
        return;
    }

    let editor_area_width = size.width as f32 - sidebar_original;
    let pane_width = editor_area_width / 2.0;
    let hovered_pane_idx = compute_hovered_pane_idx(
        state,
        sidebar_original,
        pane_width,
        main_y,
        editor_bottom_limit,
    );
    let is_in_tabbar = compute_is_in_tabbar(
        state,
        main_y,
        ui.tabbar_height,
        hovered_pane_idx,
        editor_bottom_limit,
    );

    if is_in_tabbar {
        handle_tab_move_to_pane(
            ui,
            state,
            dragged_idx,
            hovered_pane_idx,
            size,
            sidebar_original,
        );
    } else if state.inactive_panes.is_empty() {
        handle_tab_create_split(
            ui,
            state,
            dragged_idx,
            size,
            sidebar_original,
            main_y,
            editor_bottom_limit,
        );
    } else if hovered_pane_idx != state.active_pane_idx {
        handle_tab_move_to_other_pane(ui, state, dragged_idx, size, sidebar_original);
    }

    if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
        ui.scroll_x = active_tab.scroll_x;
        ui.scroll_y = active_tab.scroll_y;
    }
}

/// Handles a tab dragged completely outside the window (spawn new instance or IPC transfer).
fn handle_tab_drag_outside_window(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    dragged_idx: usize,
    size: winit::dpi::PhysicalSize<u32>,
    sidebar_original: f32,
) {
    let mut removed = false;
    if let Some(ref path_str) = state.tabs[dragged_idx].path.clone()
        && !path_str.starts_with("diagnostics://")
    {
        if state.tabs[dragged_idx].buffer.is_modified {
            let _ = state.tabs[dragged_idx].buffer.save_file(path_str);
        }
        let inner_pos = window
            .inner_position()
            .unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
        let global_x = inner_pos.x + state.mouse_x as i32;
        let global_y = inner_pos.y + state.mouse_y as i32;

        if !crate::app::ipc::try_drop_to_other_window(global_x, global_y, path_str)
            && let Ok(exe_path) = std::env::current_exe()
        {
            let _ = std::process::Command::new(exe_path).arg(path_str).spawn();
        }
        state.tabs.remove(dragged_idx);
        removed = true;
    }

    if removed {
        collapse_or_restore_empty_pane(ui, state, sidebar_original, size);
        if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
            ui.scroll_x = active_tab.scroll_x;
            ui.scroll_y = active_tab.scroll_y;
        }
    }
    window.request_redraw();
    update_cursor_icon(window, ui, state);
}

/// Collapses the split if the active pane became empty, otherwise adjusts active_tab_idx.
fn collapse_or_restore_empty_pane(
    ui: &mut UiState,
    state: &mut AppState,
    sidebar_original: f32,
    size: winit::dpi::PhysicalSize<u32>,
) {
    if state.tabs.is_empty() {
        if !state.inactive_panes.is_empty() {
            let target_pane = state.inactive_panes.remove(0);
            state.tabs = target_pane.tabs;
            state.active_tab_idx = target_pane
                .active_tab_idx
                .min(state.tabs.len().saturating_sub(1));
            state.active_pane_idx = 0;
            state.is_split_horizontal = false;
        } else {
            state.tabs.push(crate::app::state::Tab {
                path: None,
                buffer: crate::editor::buffer::Buffer::new(),
                cursor: crate::editor::cursor::Cursor::new(),
                secondary_cursors: Vec::new(),
                scroll_x: 0,
                scroll_y: 0,
            });
            state.active_tab_idx = 0;
        }
    } else {
        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
    }
    let visible_width = size.width as f32 - sidebar_original;
    let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
    state.tab_scroll_x = ui.scroll_to_tab(
        state.active_tab_idx,
        &tab_paths,
        visible_width,
        state.tab_scroll_x,
    );
}

/// Returns the hovered pane index based on mouse position.
fn compute_hovered_pane_idx(
    state: &AppState,
    sidebar_original: f32,
    pane_width: f32,
    main_y: f32,
    editor_bottom_limit: f32,
) -> usize {
    if state.inactive_panes.is_empty() {
        return 0;
    }
    if state.is_split_horizontal {
        let editor_area_height = editor_bottom_limit - main_y;
        let pane_height = (editor_area_height / 2.0).round();
        if state.mouse_y < main_y + pane_height {
            0
        } else {
            1
        }
    } else {
        if state.mouse_x < sidebar_original + pane_width {
            0
        } else {
            1
        }
    }
}

/// Returns true if the mouse is hovering over the tab bar of the hovered pane.
fn compute_is_in_tabbar(
    state: &AppState,
    main_y: f32,
    tabbar_height: f32,
    hovered_pane_idx: usize,
    editor_bottom_limit: f32,
) -> bool {
    if !state.inactive_panes.is_empty() && state.is_split_horizontal {
        let editor_area_height = editor_bottom_limit - main_y;
        let pane_height = (editor_area_height / 2.0).round();
        let pane_top = if hovered_pane_idx == 0 {
            main_y
        } else {
            main_y + pane_height
        };
        state.mouse_y >= pane_top && state.mouse_y < pane_top + tabbar_height
    } else {
        state.mouse_y >= main_y && state.mouse_y < main_y + tabbar_height
    }
}

/// Moves a tab to another pane (reorder/merge when dragging to tab bar).
fn handle_tab_move_to_pane(
    ui: &mut UiState,
    state: &mut AppState,
    dragged_idx: usize,
    hovered_pane_idx: usize,
    size: winit::dpi::PhysicalSize<u32>,
    sidebar_original: f32,
) {
    if hovered_pane_idx == state.active_pane_idx {
        return;
    }
    let tab_to_move = state.tabs.remove(dragged_idx);
    state.inactive_panes[0].tabs.push(tab_to_move);
    state.inactive_panes[0].active_tab_idx = state.inactive_panes[0].tabs.len() - 1;

    if state.tabs.is_empty() {
        let target_pane = state.inactive_panes.remove(0);
        state.tabs = target_pane.tabs;
        state.active_tab_idx = target_pane
            .active_tab_idx
            .min(state.tabs.len().saturating_sub(1));
        state.active_pane_idx = 0;
        state.is_split_horizontal = false;
        let visible_width = size.width as f32 - sidebar_original;
        let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
        state.tab_scroll_x = ui.scroll_to_tab(
            state.active_tab_idx,
            &tab_paths,
            visible_width,
            state.tab_scroll_x,
        );
    } else {
        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
        let target_pane = 1 - state.active_pane_idx;
        state.switch_pane(target_pane);
        let visible_width = if state.is_split_horizontal {
            size.width as f32 - sidebar_original
        } else {
            ((size.width as f32 - sidebar_original) / 2.0).round()
        };
        let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
        state.tab_scroll_x = ui.scroll_to_tab(
            state.active_tab_idx,
            &tab_paths,
            visible_width,
            state.tab_scroll_x,
        );
    }
    if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
        ui.scroll_x = active_tab.scroll_x;
        ui.scroll_y = active_tab.scroll_y;
    }
}

/// Moves a tab to the other existing pane (not via tab bar).
fn handle_tab_move_to_other_pane(
    ui: &mut UiState,
    state: &mut AppState,
    dragged_idx: usize,
    size: winit::dpi::PhysicalSize<u32>,
    sidebar_original: f32,
) {
    let tab_to_move = state.tabs.remove(dragged_idx);
    state.inactive_panes[0].tabs.push(tab_to_move);
    state.inactive_panes[0].active_tab_idx = state.inactive_panes[0].tabs.len() - 1;

    if state.tabs.is_empty() {
        let target_pane = state.inactive_panes.remove(0);
        state.tabs = target_pane.tabs;
        state.active_tab_idx = target_pane
            .active_tab_idx
            .min(state.tabs.len().saturating_sub(1));
        state.active_pane_idx = 0;
        state.is_split_horizontal = false;
        let visible_width = size.width as f32 - sidebar_original;
        let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
        state.tab_scroll_x = ui.scroll_to_tab(
            state.active_tab_idx,
            &tab_paths,
            visible_width,
            state.tab_scroll_x,
        );
    } else {
        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
        let target_pane = 1 - state.active_pane_idx;
        state.switch_pane(target_pane);
        let visible_width = if state.is_split_horizontal {
            size.width as f32 - sidebar_original
        } else {
            ((size.width as f32 - sidebar_original) / 2.0).round()
        };
        let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
        state.tab_scroll_x = ui.scroll_to_tab(
            state.active_tab_idx,
            &tab_paths,
            visible_width,
            state.tab_scroll_x,
        );
    }
    if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
        ui.scroll_x = active_tab.scroll_x;
        ui.scroll_y = active_tab.scroll_y;
    }
}

/// Creates a new split when a tab is dropped into the editor area without an existing split.
#[allow(clippy::too_many_arguments)]
fn handle_tab_create_split(
    ui: &mut UiState,
    state: &mut AppState,
    dragged_idx: usize,
    size: winit::dpi::PhysicalSize<u32>,
    sidebar_original: f32,
    main_y: f32,
    editor_bottom_limit: f32,
) {
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
        // Top split
        state.is_split_horizontal = true;
        let existing_pane = crate::app::state::Pane {
            tabs: std::mem::take(&mut state.tabs),
            active_tab_idx: state.active_tab_idx,
            tab_scroll_x: state.tab_scroll_x,
        };
        state.inactive_panes.push(existing_pane);
        state.tabs = vec![tab_to_move];
        state.active_tab_idx = 0;
        state.active_pane_idx = 0;
        state.tab_scroll_x = 0.0;
    } else if state.mouse_y >= main_y + ui.tabbar_height + editor_area_height * 0.75 {
        // Bottom split
        state.is_split_horizontal = true;
        state.inactive_panes.push(crate::app::state::Pane {
            tabs: vec![tab_to_move],
            active_tab_idx: 0,
            tab_scroll_x: 0.0,
        });
        state.switch_pane(1);
    } else if state.mouse_x < sidebar_original + editor_area_width * 0.5 {
        // Left split
        state.is_split_horizontal = false;
        let visible_width = ((size.width as f32 - sidebar_original) / 2.0).round();
        let mut existing_pane = crate::app::state::Pane {
            tabs: std::mem::take(&mut state.tabs),
            active_tab_idx: state.active_tab_idx,
            tab_scroll_x: state.tab_scroll_x,
        };
        let existing_paths: Vec<Option<String>> =
            existing_pane.tabs.iter().map(|t| t.path.clone()).collect();
        existing_pane.tab_scroll_x = ui.scroll_to_tab(
            existing_pane.active_tab_idx,
            &existing_paths,
            visible_width,
            existing_pane.tab_scroll_x,
        );
        state.inactive_panes.push(existing_pane);
        state.tabs = vec![tab_to_move];
        state.active_tab_idx = 0;
        state.active_pane_idx = 0;
        state.tab_scroll_x = 0.0;
    } else {
        // Right split
        state.is_split_horizontal = false;
        state.inactive_panes.push(crate::app::state::Pane {
            tabs: vec![tab_to_move],
            active_tab_idx: 0,
            tab_scroll_x: 0.0,
        });
        state.switch_pane(1);
        let visible_width = ((size.width as f32 - sidebar_original) / 2.0).round();
        let existing_paths: Vec<Option<String>> = state.inactive_panes[0]
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect();
        state.inactive_panes[0].tab_scroll_x = ui.scroll_to_tab(
            state.inactive_panes[0].active_tab_idx,
            &existing_paths,
            visible_width,
            state.inactive_panes[0].tab_scroll_x,
        );
    }

    if let Some(active_tab) = state.tabs.get(state.active_tab_idx) {
        ui.scroll_x = active_tab.scroll_x;
        ui.scroll_y = active_tab.scroll_y;
    }
}

pub fn handle_mouse_wheel(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    delta: MouseScrollDelta,
) {
    if ui.active_modal == Some(crate::machkit::ModalType::CommandPalette) {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, dy) => -dy as isize,
            MouseScrollDelta::PixelDelta(pos) => -((pos.y / 15.0) as isize),
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
    let w_width = size.width as f32;
    let sidebar_original = ui.sidebar_width;

    let main_y = ui.titlebar_height;
    let mut dock_start_y = size.height as f32 - ui.status_height;
    if ui.show_dock {
        dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height)
            .max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
    }
    let editor_bottom_limit = if ui.show_dock {
        dock_start_y
    } else {
        size.height as f32 - ui.status_height
    };

    let hovered_pane_idx = if state.inactive_panes.is_empty() {
        0
    } else if state.is_split_horizontal {
        let editor_area_height = editor_bottom_limit - main_y;
        let pane_height = (editor_area_height / 2.0).round();
        if state.mouse_y < main_y + pane_height {
            0
        } else {
            1
        }
    } else {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        if state.mouse_x < sidebar_original + pane_width {
            0
        } else {
            1
        }
    };

    let pane_tabbar_start_x = if !state.inactive_panes.is_empty()
        && !state.is_split_horizontal
        && hovered_pane_idx == 1
    {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        sidebar_original + pane_width
    } else {
        sidebar_original
    };

    let pane_tabbar_end_x = if !state.inactive_panes.is_empty()
        && !state.is_split_horizontal
        && hovered_pane_idx == 0
    {
        let editor_area_width = size.width as f32 - sidebar_original;
        let pane_width = editor_area_width / 2.0;
        sidebar_original + pane_width
    } else {
        size.width as f32
    };

    let pane_tabbar_top =
        if !state.inactive_panes.is_empty() && state.is_split_horizontal && hovered_pane_idx == 1 {
            let editor_area_height = editor_bottom_limit - main_y;
            let pane_height = (editor_area_height / 2.0).round();
            main_y + pane_height
        } else {
            main_y
        };
    let pane_tabbar_bottom = pane_tabbar_top + ui.tabbar_height;

    if state.mouse_x >= pane_tabbar_start_x
        && state.mouse_x < pane_tabbar_end_x
        && state.mouse_y >= pane_tabbar_top
        && state.mouse_y < pane_tabbar_bottom
    {
        scroll_tab_bar(
            ui,
            state,
            window,
            delta,
            hovered_pane_idx,
            pane_tabbar_start_x,
            pane_tabbar_end_x,
        );
        return;
    }

    // Handle Terminal Dock Scroll
    let is_mouse_over_terminal = ui.show_dock
        && state.mouse_y >= dock_start_y + 28.0
        && state.mouse_y < size.height as f32 - ui.status_height
        && state.mouse_x >= ui.sidebar_width;

    if is_mouse_over_terminal && !state.dock_terminals.is_empty() {
        scroll_terminal_dock(state, window, delta);
        return;
    }

    // Handle Sidebar Scroll
    let sidebar_top = ui.titlebar_height;
    let sidebar_bottom = size.height as f32 - ui.status_height;
    if ui.sidebar_width > 0.0
        && state.mouse_x >= 0.0
        && state.mouse_x < ui.sidebar_width
        && state.mouse_y >= sidebar_top
        && state.mouse_y < sidebar_bottom
    {
        scroll_sidebar(ui, window, delta, sidebar_bottom - sidebar_top);
        return;
    }

    let is_shift = state.modifiers.shift_key();
    if is_shift {
        scroll_horizontal_buffer(ui, state, window, delta, w_width);
        return;
    }

    scroll_vertical_buffer(ui, state, window, delta, w_width);
}

fn scroll_tab_bar(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    delta: MouseScrollDelta,
    hovered_pane_idx: usize,
    pane_tabbar_start_x: f32,
    pane_tabbar_end_x: f32,
) {
    let (hovered_tabs, current_scroll_x) = if hovered_pane_idx == state.active_pane_idx {
        (&state.tabs, state.tab_scroll_x)
    } else {
        (
            &state.inactive_panes[0].tabs,
            state.inactive_panes[0].tab_scroll_x,
        )
    };

    let mut total_tabs_width = 0.0f32;
    let tab_close_icon_sz = (ui.ui_font_size * 0.8).round().max(10.0);
    let close_reserved = 8.0f32 + tab_close_icon_sz;
    let dot_reserved = 18.0f32;
    for t in hovered_tabs {
        let file_name = ui.get_tab_name(t.path.as_deref());
        let name_w = file_name.chars().count() as f32 * ui.ui_char_width;
        let tab_w = (12.0 + dot_reserved + name_w + close_reserved + 10.0).max(110.0);
        total_tabs_width += tab_w;
    }

    let visible_width = pane_tabbar_end_x - pane_tabbar_start_x;
    let max_scroll_x = (total_tabs_width - visible_width).max(0.0);

    let scroll_amount = match delta {
        MouseScrollDelta::LineDelta(dx, dy) => {
            let val = if dx.abs() > dy.abs() { dx } else { -dy };
            val * 24.0
        }
        MouseScrollDelta::PixelDelta(pos) => {
            let val = if pos.x.abs() > pos.y.abs() {
                pos.x
            } else {
                -pos.y
            };
            val as f32
        }
    };

    let new_scroll_x = (current_scroll_x + scroll_amount).clamp(0.0, max_scroll_x);
    state.set_pane_scroll_x(hovered_pane_idx, new_scroll_x);
    window.request_redraw();
}

fn scroll_terminal_dock(state: &mut AppState, window: &Window, delta: MouseScrollDelta) {
    let scroll_lines = match delta {
        MouseScrollDelta::LineDelta(_, dy) => dy as isize * 3,
        MouseScrollDelta::PixelDelta(pos) => ((pos.y / 15.0) * 3.0) as isize,
    };
    let active_term = &mut state.dock_terminals[state.active_terminal_idx];
    let max_scroll = active_term.grid.scrollback.len() as isize;
    let new_offset = active_term.grid.scroll_offset as isize + scroll_lines;
    active_term.grid.scroll_offset = new_offset.clamp(0, max_scroll) as usize;
    window.request_redraw();
}

fn scroll_sidebar(ui: &mut UiState, window: &Window, delta: MouseScrollDelta, main_height: f32) {
    let scroll_lines = match delta {
        MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
        MouseScrollDelta::PixelDelta(pos) => {
            -(((pos.y / (ui.ui_line_height as f64)) * 3.0) as isize)
        }
    };
    let total_rows = 1 + ui.visible_nodes.len();
    let visible_rows = (main_height / ui.ui_line_height).floor() as usize;
    let max_scroll = (total_rows as isize - visible_rows as isize).max(0);
    let new_scroll = ui.sidebar_scroll as isize + scroll_lines;
    ui.sidebar_scroll = new_scroll.clamp(0, max_scroll) as usize;
    window.request_redraw();
}

fn scroll_horizontal_buffer(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    delta: MouseScrollDelta,
    w_width: f32,
) {
    let scroll_cols = match delta {
        MouseScrollDelta::LineDelta(dx, _) if dx != 0.0 => -dx as isize * 3,
        MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
        MouseScrollDelta::PixelDelta(pos) => {
            let val = if pos.x.abs() > pos.y.abs() {
                pos.x
            } else {
                pos.y
            };
            -(((val / (ui.buffer_char_width as f64)) * 3.0) as isize)
        }
    };
    let max_line_digits = state.tabs[state.active_tab_idx]
        .buffer
        .len()
        .to_string()
        .len()
        .max(3);
    let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
    let text_area_x = ui.sidebar_width + gutter_width;
    let scrollbar_width = ui.scrollbar_width();
    let minimap_width = ui.minimap_width();
    let sb_x = w_width - scrollbar_width;
    let minimap_x = sb_x - minimap_width;
    let text_viewport_w = (minimap_x - text_area_x).max(10.0);
    let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;

    let max_line_len = state.tabs[state.active_tab_idx]
        .buffer
        .lines()
        .iter()
        .map(|l: &String| l.chars().count())
        .max()
        .unwrap_or(0);
    let max_scroll = (max_line_len as isize - visible_cols as isize).max(0);
    let new_scroll = ui.scroll_x as isize + scroll_cols;
    ui.scroll_x = new_scroll.clamp(0, max_scroll) as usize;
    state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
    window.request_redraw();
}

fn scroll_vertical_buffer(
    ui: &mut UiState,
    state: &mut AppState,
    window: &Window,
    delta: MouseScrollDelta,
    w_width: f32,
) {
    let scroll_lines = match delta {
        MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
        MouseScrollDelta::PixelDelta(pos) => {
            -(((pos.y / (ui.buffer_line_height as f64)) * 3.0) as isize)
        }
    };

    let active_path = state.tabs[state.active_tab_idx]
        .path
        .as_deref()
        .unwrap_or("");
    if active_path == "search://project" {
        let render_items =
            crate::machkit::components::editor::project_search::build_search_render_items(ui);
        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
        let status_y = (window.inner_size().height as f32 - ui.status_height).round();
        let editor_height = status_y - editor_top;
        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;

        let max_scroll = (render_items.len() as isize - visible_lines as isize).max(0);
        let new_scroll = ui.scroll_y as isize + scroll_lines;
        ui.scroll_y = new_scroll.clamp(0, max_scroll) as usize;
        window.request_redraw();
        return;
    }
    let is_diagnostics = active_path.starts_with("diagnostics://");

    let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
    let status_y = (window.inner_size().height as f32 - ui.status_height).round();

    let show_horizontal_scrollbar = if is_diagnostics {
        false
    } else {
        let max_line_len = ui.get_max_line_len(
            &state.tabs[state.active_tab_idx].buffer,
            Some(active_path),
            state.tabs[state.active_tab_idx].cursor.line,
        );
        let max_line_digits = state.tabs[state.active_tab_idx]
            .buffer
            .len()
            .to_string()
            .len()
            .max(3);
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
            let file_lines_len = ui
                .diagnostics_file_cache
                .get(file_path)
                .map(|l| l.len())
                .unwrap_or(0);
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
