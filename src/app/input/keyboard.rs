use std::io::Write;
use std::sync::Arc;
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::window::Window;

use crate::app::handler::handle_action;
use crate::app::input::mouse::update_cursor_icon;
use crate::app::state::AppState;
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::{UiAction, UiState};
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::GpuContext;

pub fn handle_keyboard_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: Key,
    physical_key: PhysicalKey,
) {
    if !state.tabs.is_empty() {
        state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
    }
    for p in &mut state.inactive_panes {
        if !p.tabs.is_empty() {
            p.active_tab_idx = p.active_tab_idx.min(p.tabs.len() - 1);
        }
    }
    let ctrl = state.modifiers.control_key();
    let shift = state.modifiers.shift_key();
    let alt = state.modifiers.alt_key();

    // If terminal is focused, check if key maps to a workspace action first
    if state.terminal_focus
        && !state.dock_terminals.is_empty()
        && let Some(action) = crate::editor::keymap::map_key(
            &ui.keymap,
            &logical_key,
            physical_key,
            ctrl,
            shift,
            alt,
            &["Workspace"],
        )
        && handle_workspace_action_for_terminal(
            ui, state, action, window, elwt, gpu, atlas, font_bytes,
        )
    {
        return;
    }

    // 1. Delegate to terminal input handler if terminal is focused
    if handle_terminal_input(state, window, &logical_key) {
        return;
    }

    let (active_path_start, old_revision) = {
        if state.active_tab_idx < state.tabs.len() {
            let active_tab = &state.tabs[state.active_tab_idx];
            if let Some(ref path) = active_tab.path {
                if !path.starts_with("diagnostics://") {
                    (Some(path.clone()), Some(active_tab.buffer.revision))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    };

    // Handle typing inside the Search Panel
    if ui.show_search_panel
        && ui.search_focused
        && handle_search_panel_input(
            ui,
            state,
            window,
            elwt,
            gpu,
            atlas,
            font_bytes,
            &logical_key,
            physical_key,
            ctrl,
            shift,
            alt,
        )
    {
        return;
    }

    // Handle typing inside the SidebarInput modal
    if ui.active_modal == Some(crate::machkit::ModalType::SidebarInput) {
        handle_sidebar_input(ui, window, &logical_key);
        return;
    }

    // 2. Delegate to command palette modal handler if command palette is active
    if handle_command_palette_input(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
    ) {
        return;
    }

    // 2b. Delegate to global search modal handler if global search is active
    if handle_global_search_input(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
    ) {
        return;
    }

    // 3. Delegate to modal handler (Escape key behavior for other modals)
    if ui.active_modal.is_some() {
        if let Key::Named(NamedKey::Escape) = &logical_key {
            ui.active_modal = None;
            window.request_redraw();
        }
        return;
    }

    // 4. Delegate to virtual diagnostics view handler if active
    if handle_diagnostics_keyboard(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
    ) {
        return;
    }

    // 4b. Delegate to project search keyboard handler if active
    if handle_project_search_keyboard(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
    ) {
        return;
    }

    // 5. Delegate to core editor handler
    handle_editor_keyboard(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
        active_path_start,
        old_revision,
    );
}

/// Handles workspace-level actions when the terminal has focus.
/// Returns true if the action was consumed.
fn handle_workspace_action_for_terminal(
    ui: &mut UiState,
    state: &mut AppState,
    action: crate::editor::actions::Action,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
) -> bool {
    match action {
        crate::editor::actions::Action::ZoomIn => {
            let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::ZoomOut => {
            let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::CommandPalette => {
            ui.active_modal = Some(crate::machkit::ModalType::CommandPalette);
            ui.command_palette_query.clear();
            ui.command_palette_selected = 0;
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::GlobalSearch => {
            handle_action(
                ui,
                state,
                UiAction::OpenFile(std::path::PathBuf::from("search://project")),
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
            window.request_redraw();
            true
        }
        _ => false,
    }
}

/// Handles keyboard input while the inline search panel is active.
/// Returns true if the key was consumed.
fn handle_search_panel_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    // Check if there's an action mapped
    if let Some(action) = crate::editor::keymap::map_key(
        &ui.keymap,
        logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
        &["Editor", "Workspace"],
    ) && handle_search_panel_action(ui, state, action, window, elwt, gpu, atlas, font_bytes)
    {
        return true;
    }

    // Otherwise handle raw input editing
    match logical_key {
        Key::Named(NamedKey::Escape) => {
            ui.show_search_panel = false;
            window.request_redraw();
            true
        }
        Key::Named(NamedKey::Tab) => {
            if ui.show_replace {
                ui.search_focus_replace = !ui.search_focus_replace;
            } else {
                ui.search_focus_replace = false;
            }
            window.request_redraw();
            true
        }
        Key::Named(NamedKey::Enter) => {
            handle_search_panel_enter(ui, state, shift, window);
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if ui.search_focus_replace {
                ui.replace_query.pop();
            } else {
                ui.search_query.pop();
            }
            window.request_redraw();
            true
        }
        Key::Character(text) => {
            if !ctrl && !alt {
                for c in text.chars() {
                    if !c.is_control() {
                        if ui.search_focus_replace {
                            ui.replace_query.push(c);
                        } else {
                            ui.search_query.push(c);
                        }
                    }
                }
                window.request_redraw();
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Handles an action key while the search panel is focused.
/// Returns true if consumed.
fn handle_search_panel_action(
    ui: &mut UiState,
    state: &mut AppState,
    action: crate::editor::actions::Action,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
) -> bool {
    match action {
        crate::editor::actions::Action::Find => {
            if state.active_tab_idx < state.tabs.len() {
                let active_tab = &state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    let selected_text = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
                    if !selected_text.contains('\n') && !selected_text.is_empty() {
                        ui.search_query = selected_text;
                    }
                }
            }
            ui.search_focused = true;
            ui.search_focus_replace = false;
            ui.perform_search(state);
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::Split => {
            handle_split_action(state);
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::Escape => {
            ui.show_search_panel = false;
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::ZoomIn => {
            let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::ZoomOut => {
            let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::CommandPalette => {
            ui.active_modal = Some(crate::machkit::ModalType::CommandPalette);
            ui.command_palette_query.clear();
            ui.command_palette_selected = 0;
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::GlobalSearch => {
            handle_action(
                ui,
                state,
                UiAction::OpenFile(std::path::PathBuf::from("search://project")),
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
            window.request_redraw();
            true
        }
        crate::editor::actions::Action::SaveFile => {
            handle_action(
                ui,
                state,
                UiAction::SaveFile,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
            window.request_redraw();
            true
        }
        _ => false,
    }
}

/// Navigate to next/previous search match when Enter is pressed in the search panel.
fn handle_search_panel_enter(
    ui: &mut UiState,
    state: &mut AppState,
    shift: bool,
    window: &mut Arc<Window>,
) {
    ui.perform_search(state);
    if !ui.search_matches.is_empty() {
        if shift {
            if ui.active_search_match_idx == 0 {
                ui.active_search_match_idx = ui.search_matches.len() - 1;
            } else {
                ui.active_search_match_idx -= 1;
            }
        } else if ui.active_search_match_idx >= ui.search_matches.len() - 1 {
            ui.active_search_match_idx = 0;
        } else {
            ui.active_search_match_idx += 1;
        }
        if state.active_tab_idx < state.tabs.len() {
            let (m_line, m_col) = ui.search_matches[ui.active_search_match_idx];
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
    window.request_redraw();
}

/// Handles keyboard input while the sidebar (file tree) input modal is active.
fn handle_sidebar_input(ui: &mut UiState, window: &mut Arc<Window>, logical_key: &Key) {
    match logical_key {
        Key::Named(NamedKey::Escape) => {
            ui.active_modal = None;
            window.request_redraw();
        }
        Key::Named(NamedKey::Enter) => {
            handle_sidebar_input_confirm(ui);
            window.request_redraw();
        }
        Key::Named(NamedKey::Backspace) => {
            ui.sidebar_input_value.pop();
            window.request_redraw();
        }
        Key::Character(text) => {
            for c in text.chars() {
                if !c.is_control() {
                    ui.sidebar_input_value.push(c);
                }
            }
            window.request_redraw();
        }
        _ => {}
    }
}

/// Confirms sidebar file/folder creation or rename operation.
fn handle_sidebar_input_confirm(ui: &mut UiState) {
    let target = &ui.sidebar_input_target;
    let val = &ui.sidebar_input_value;
    if !val.is_empty() {
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
                let new_path = parent.join(val);
                let _ = std::fs::File::create(new_path);
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
}

/// Handles pane split action, shared across multiple contexts.
fn handle_split_action(state: &mut AppState) {
    if state.inactive_panes.is_empty() {
        if state.active_tab_idx < state.tabs.len() {
            let active_tab = state.tabs[state.active_tab_idx].clone();
            let new_pane = crate::app::state::Pane {
                tabs: vec![active_tab],
                active_tab_idx: 0,
                tab_scroll_x: 0.0,
            };
            state.inactive_panes.push(new_pane);
        } else {
            let initial_tab = crate::app::Tab {
                path: None,
                buffer: Buffer::new(),
                cursor: Cursor::new(),
                secondary_cursors: Vec::new(),
                scroll_x: 0,
                scroll_y: 0,
            };
            let new_pane = crate::app::state::Pane {
                tabs: vec![initial_tab],
                active_tab_idx: 0,
                tab_scroll_x: 0.0,
            };
            state.inactive_panes.push(new_pane);
        }
        state.switch_pane(1);
    } else {
        let target_pane = if state.active_pane_idx == 0 { 1 } else { 0 };
        state.switch_pane(target_pane);
    }
}

pub fn handle_terminal_input(
    state: &mut AppState,
    window: &Arc<Window>,
    logical_key: &Key,
) -> bool {
    if state.terminal_focus && !state.dock_terminals.is_empty() {
        let active_term = &mut state.dock_terminals[state.active_terminal_idx];

        let ctrl = state.modifiers.control_key();
        let shift = state.modifiers.shift_key();
        let alt = state.modifiers.alt_key();

        let modifier_code =
            1 + if shift { 1 } else { 0 } + if alt { 2 } else { 0 } + if ctrl { 4 } else { 0 };

        let bytes_to_write: Option<Vec<u8>> = match logical_key {
            Key::Character(text) => {
                if ctrl && text.len() == 1 {
                    let c = text.chars().next().unwrap();
                    if c.is_ascii_alphabetic() {
                        let code = c.to_ascii_uppercase() as u8 - b'A' + 1;
                        Some(vec![code])
                    } else {
                        Some(text.as_bytes().to_vec())
                    }
                } else if alt && text.len() == 1 {
                    let mut bytes = vec![27];
                    bytes.extend_from_slice(text.as_bytes());
                    Some(bytes)
                } else {
                    Some(text.as_bytes().to_vec())
                }
            }
            Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
            Key::Named(NamedKey::Space) => {
                if ctrl {
                    Some(vec![0])
                } else {
                    Some(vec![b' '])
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if alt {
                    Some(vec![27, 127])
                } else if ctrl {
                    Some(vec![8])
                } else {
                    Some(vec![127])
                }
            }
            Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
            Key::Named(NamedKey::Escape) => Some(vec![27]),
            Key::Named(NamedKey::ArrowUp) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[1;{}A", modifier_code).into_bytes())
                } else if active_term.grid.decckm {
                    Some(b"\x1bOA".to_vec())
                } else {
                    Some(b"\x1b[A".to_vec())
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[1;{}B", modifier_code).into_bytes())
                } else if active_term.grid.decckm {
                    Some(b"\x1bOB".to_vec())
                } else {
                    Some(b"\x1b[B".to_vec())
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[1;{}C", modifier_code).into_bytes())
                } else if active_term.grid.decckm {
                    Some(b"\x1bOC".to_vec())
                } else {
                    Some(b"\x1b[C".to_vec())
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[1;{}D", modifier_code).into_bytes())
                } else if active_term.grid.decckm {
                    Some(b"\x1bOD".to_vec())
                } else {
                    Some(b"\x1b[D".to_vec())
                }
            }
            Key::Named(NamedKey::Home) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[1;{}H", modifier_code).into_bytes())
                } else {
                    Some(b"\x1b[H".to_vec())
                }
            }
            Key::Named(NamedKey::End) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[1;{}F", modifier_code).into_bytes())
                } else {
                    Some(b"\x1b[F".to_vec())
                }
            }
            Key::Named(NamedKey::Delete) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[3;{}~", modifier_code).into_bytes())
                } else {
                    Some(b"\x1b[3~".to_vec())
                }
            }
            Key::Named(NamedKey::Insert) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[2;{}~", modifier_code).into_bytes())
                } else {
                    Some(b"\x1b[2~".to_vec())
                }
            }
            Key::Named(NamedKey::PageUp) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[5;{}~", modifier_code).into_bytes())
                } else {
                    Some(b"\x1b[5~".to_vec())
                }
            }
            Key::Named(NamedKey::PageDown) => {
                if modifier_code > 1 {
                    Some(format!("\x1b[6;{}~", modifier_code).into_bytes())
                } else {
                    Some(b"\x1b[6~".to_vec())
                }
            }
            _ => None,
        };

        if let Some(bytes) = bytes_to_write {
            let _ = active_term.pty_writer.write_all(&bytes);
            let _ = active_term.pty_writer.flush();
            active_term.grid.scroll_offset = 0;
        }
        window.request_redraw();
        true
    } else {
        false
    }
}

pub fn handle_command_palette_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
) -> bool {
    if let Some(crate::machkit::ModalType::CommandPalette) = ui.active_modal {
        match logical_key {
            Key::Named(NamedKey::Escape) => {
                ui.active_modal = None;
                ui.command_palette_mode = crate::machkit::CommandPaletteMode::Commands;
                window.request_redraw();
            }
            Key::Named(NamedKey::ArrowDown) => {
                let items_count = ui.get_filtered_commands().len();
                if items_count > 0 {
                    ui.command_palette_selected = (ui.command_palette_selected + 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::ArrowUp) => {
                let items_count = ui.get_filtered_commands().len();
                if items_count > 0 {
                    ui.command_palette_selected =
                        (ui.command_palette_selected + items_count - 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                let filtered = ui.get_filtered_commands();
                if ui.command_palette_selected < filtered.len() {
                    let cmd = filtered[ui.command_palette_selected];
                    ui.active_modal = None;

                    let action_res = {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        let active_path = active_tab.path.as_deref();
                        ui.execute_command(
                            cmd,
                            &mut active_tab.buffer,
                            &mut active_tab.cursor,
                            active_path,
                        )
                    };
                    handle_action(ui, state, action_res, window, elwt, gpu, atlas, font_bytes);
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Backspace) => {
                ui.command_palette_query.pop();
                ui.command_palette_selected = 0;
                window.request_redraw();
            }
            Key::Character(text) => {
                for c in text.chars() {
                    if !c.is_control() {
                        ui.command_palette_query.push(c);
                    }
                }
                ui.command_palette_selected = 0;
                window.request_redraw();
            }
            _ => {}
        }
        true
    } else {
        false
    }
}

pub fn handle_global_search_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
) -> bool {
    if let Some(crate::machkit::ModalType::GlobalSearch) = ui.active_modal {
        match logical_key {
            Key::Named(NamedKey::Escape) => {
                ui.active_modal = None;
                window.request_redraw();
            }
            Key::Named(NamedKey::ArrowDown) => {
                let items_count = ui.global_search_results.len();
                if items_count > 0 {
                    ui.global_search_selected = (ui.global_search_selected + 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::ArrowUp) => {
                let items_count = ui.global_search_results.len();
                if items_count > 0 {
                    ui.global_search_selected =
                        (ui.global_search_selected + items_count - 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                if ui.global_search_query != ui.last_searched_global_query {
                    ui.last_searched_global_query = ui.global_search_query.clone();
                    let q = ui.global_search_query.clone();
                    ui.run_global_search(q);
                } else {
                    let results_len = ui.global_search_results.len();
                    if ui.global_search_selected < results_len {
                        let (path, line_idx, _) =
                            &ui.global_search_results[ui.global_search_selected];
                        ui.active_modal = None;
                        handle_action(
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
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Backspace) => {
                ui.global_search_query.pop();
                ui.global_search_selected = 0;
                window.request_redraw();
            }
            Key::Character(text) => {
                for c in text.chars() {
                    if !c.is_control() {
                        ui.global_search_query.push(c);
                    }
                }
                ui.global_search_selected = 0;
                window.request_redraw();
            }
            _ => {}
        }
        true
    } else {
        false
    }
}

pub fn handle_diagnostics_keyboard(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    if state.tabs[state.active_tab_idx].path.as_deref() == Some("diagnostics://project")
        && let Some(action) = crate::editor::keymap::map_key(
            &ui.keymap,
            logical_key,
            physical_key,
            ctrl,
            shift,
            alt,
            &["Editor", "Workspace"],
        )
    {
        let is_navigation_action = is_cursor_navigation_action(&action);
        let is_global_action = is_workspace_global_action(&action);
        let is_document_action = !is_navigation_action && !is_global_action;

        if is_navigation_action {
            handle_diagnostics_navigation(ui, state, &action);
            window.request_redraw();
            return true;
        }

        if is_document_action
            && handle_diagnostics_document_action(
                ui,
                state,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
                logical_key,
                physical_key,
                &action,
            )
        {
            return true;
        }
    }
    false
}

/// Returns true if the action is a cursor movement action.
fn is_cursor_navigation_action(action: &crate::editor::actions::Action) -> bool {
    matches!(
        action,
        crate::editor::actions::Action::MoveUp { .. }
            | crate::editor::actions::Action::MoveDown { .. }
            | crate::editor::actions::Action::MoveLeft { .. }
            | crate::editor::actions::Action::MoveRight { .. }
            | crate::editor::actions::Action::MoveToLineStart { .. }
            | crate::editor::actions::Action::MoveToLineEnd { .. }
    )
}

/// Returns true if the action is a global workspace action.
fn is_workspace_global_action(action: &crate::editor::actions::Action) -> bool {
    matches!(
        action,
        crate::editor::actions::Action::ZoomIn
            | crate::editor::actions::Action::ZoomOut
            | crate::editor::actions::Action::CommandPalette
            | crate::editor::actions::Action::GlobalSearch
            | crate::editor::actions::Action::Escape
    )
}

/// Gets the line length in the diagnostics virtual view for a given line index.
fn diagnostics_visual_line_len(ui: &mut UiState, line_idx: usize) -> usize {
    let visual_lines =
        crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
    visual_lines.get(line_idx).map_or(0, |vl| {
        use crate::machkit::components::editor::text_area::VisualDiagnosticLine;
        match vl {
            VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
            VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
            VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
        }
    })
}

/// Handles cursor navigation within the diagnostics virtual view.
fn handle_diagnostics_navigation(
    ui: &mut UiState,
    state: &mut AppState,
    action: &crate::editor::actions::Action,
) {
    use crate::machkit::components::editor::text_area::get_visual_diagnostic_lines;
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.cursor.clear_selection();

    match action {
        crate::editor::actions::Action::MoveUp { .. } => {
            if active_tab.cursor.line > 0 {
                active_tab.cursor.line -= 1;
            }
            let line_len = diagnostics_visual_line_len(ui, active_tab.cursor.line);
            active_tab.cursor.col = active_tab.cursor.col.min(line_len);
            active_tab.cursor.intended_col = active_tab.cursor.col;
        }
        crate::editor::actions::Action::MoveDown { .. } => {
            let visual_lines = get_visual_diagnostic_lines(ui);
            if !visual_lines.is_empty() && active_tab.cursor.line < visual_lines.len() - 1 {
                active_tab.cursor.line += 1;
            }
            let line_len = diagnostics_visual_line_len(ui, active_tab.cursor.line);
            active_tab.cursor.col = active_tab.cursor.col.min(line_len);
            active_tab.cursor.intended_col = active_tab.cursor.col;
        }
        crate::editor::actions::Action::MoveLeft { .. } => {
            if active_tab.cursor.col > 0 {
                active_tab.cursor.col -= 1;
            } else if active_tab.cursor.line > 0 {
                active_tab.cursor.line -= 1;
                let line_len = diagnostics_visual_line_len(ui, active_tab.cursor.line);
                active_tab.cursor.col = line_len;
            }
            active_tab.cursor.intended_col = active_tab.cursor.col;
        }
        crate::editor::actions::Action::MoveRight { .. } => {
            let line_len = diagnostics_visual_line_len(ui, active_tab.cursor.line);
            let visual_lines = get_visual_diagnostic_lines(ui);
            if active_tab.cursor.col < line_len {
                active_tab.cursor.col += 1;
            } else if active_tab.cursor.line < visual_lines.len().saturating_sub(1) {
                active_tab.cursor.line += 1;
                active_tab.cursor.col = 0;
            }
            active_tab.cursor.intended_col = active_tab.cursor.col;
        }
        crate::editor::actions::Action::MoveToLineStart { .. } => {
            active_tab.cursor.col = 0;
            active_tab.cursor.intended_col = 0;
        }
        crate::editor::actions::Action::MoveToLineEnd { .. } => {
            let line_len = diagnostics_visual_line_len(ui, active_tab.cursor.line);
            active_tab.cursor.col = line_len;
            active_tab.cursor.intended_col = line_len;
        }
        _ => {}
    }
}

/// Handles document actions (e.g., edits) while the diagnostics view is active.
/// Returns true if the action was consumed.
fn handle_diagnostics_document_action(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    physical_key: PhysicalKey,
    action: &crate::editor::actions::Action,
) -> bool {
    if matches!(action, crate::editor::actions::Action::SaveFile) {
        for tab in &mut state.tabs {
            if let Some(ref p) = tab.path
                && !p.starts_with("diagnostics://")
                && tab.buffer.is_modified
            {
                let _ = tab.buffer.save_file(p);
                tab.buffer.mark_saved();
                ui.external_change_warnings.remove(p);
            }
        }
        window.request_redraw();
        return true;
    }

    let is_modifying_action = matches!(
        action,
        crate::editor::actions::Action::InsertChar(_)
            | crate::editor::actions::Action::InsertNewLine
            | crate::editor::actions::Action::InsertTab
            | crate::editor::actions::Action::DeleteLeft
            | crate::editor::actions::Action::DeleteRight
            | crate::editor::actions::Action::Undo
            | crate::editor::actions::Action::Redo
            | crate::editor::actions::Action::Paste
            | crate::editor::actions::Action::Cut
    );

    if is_modifying_action {
        handle_diagnostics_edit(
            ui,
            state,
            window,
            elwt,
            gpu,
            atlas,
            font_bytes,
            logical_key,
            physical_key,
        );
        return true;
    }

    false
}

/// Dispatches an edit action to the corresponding real file while in the diagnostics view.
fn handle_diagnostics_edit(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    physical_key: PhysicalKey,
) {
    use crate::machkit::components::editor::text_area::{
        VisualDiagnosticLine, get_visual_diagnostic_lines,
    };

    let visual_lines = get_visual_diagnostic_lines(ui);
    let current_visual_line = {
        let active_tab = &state.tabs[state.active_tab_idx];
        visual_lines.get(active_tab.cursor.line).cloned()
    };

    let path_opt = current_visual_line.as_ref().map(|vl| match vl {
        VisualDiagnosticLine::Header { path, .. } => path.clone(),
        VisualDiagnosticLine::Code { path, .. } => path.clone(),
        VisualDiagnosticLine::Banner { path, .. } => path.clone(),
    });

    if let Some(path) = path_opt {
        let target_tab_idx = find_or_open_tab(state, &path);

        let (target_line, target_col) =
            compute_target_position(state, &current_visual_line, target_tab_idx);
        state.tabs[target_tab_idx].cursor.line = target_line;
        state.tabs[target_tab_idx].cursor.col = target_col;
        state.tabs[target_tab_idx].cursor.intended_col = target_col;
        state.tabs[target_tab_idx].cursor.selection_anchor = None;

        let original_active_tab_idx = state.active_tab_idx;
        state.active_tab_idx = target_tab_idx;

        handle_keyboard_input(
            ui,
            state,
            window,
            elwt,
            gpu,
            atlas,
            font_bytes,
            logical_key.clone(),
            physical_key,
        );

        let new_line = state.tabs[target_tab_idx].cursor.line;
        let new_col = state.tabs[target_tab_idx].cursor.col;
        let target_lines = state.tabs[target_tab_idx].buffer.lines().to_vec();

        let abs_path = crate::editor::get_absolute_path(&path);
        ui.diagnostics_file_cache.insert(abs_path, target_lines);

        state.active_tab_idx = original_active_tab_idx;

        let visual_lines_new = get_visual_diagnostic_lines(ui);
        let active_tab = &mut state.tabs[state.active_tab_idx];
        if let Some(VisualDiagnosticLine::Code {
            line_idx: orig_line_idx,
            ..
        }) = current_visual_line.as_ref()
        {
            if new_line == *orig_line_idx {
                active_tab.cursor.col = new_col;
                active_tab.cursor.intended_col = new_col;
            } else if let Some(new_v_idx) = visual_lines_new.iter().position(|vl| {
                matches!(vl, VisualDiagnosticLine::Code { path: p, line_idx: li, .. } if p == &path && *li == new_line)
            }) {
                active_tab.cursor.line = new_v_idx;
                active_tab.cursor.col = new_col;
                active_tab.cursor.intended_col = new_col;
            }
        }
    }
    window.request_redraw();
}

/// Finds an open tab for the given path or opens the file in a new tab.
fn find_or_open_tab(state: &mut AppState, path: &str) -> usize {
    if let Some(idx) = state
        .tabs
        .iter()
        .position(|t| t.path.as_deref() == Some(path))
    {
        idx
    } else {
        let mut new_buf = Buffer::new();
        if let Err(e) = new_buf.load_file(path) {
            log::warn!("Failed to load file '{}' in diagnostics view: {}", path, e);
        }
        state.tabs.push(crate::app::Tab {
            path: Some(path.to_string()),
            buffer: new_buf,
            cursor: Cursor::new(),
            secondary_cursors: Vec::new(),
            scroll_x: 0,
            scroll_y: 0,
        });
        state.tabs.len() - 1
    }
}

/// Computes the target (line, col) position in the real file from a diagnostics visual line.
fn compute_target_position(
    state: &AppState,
    current_visual_line: &Option<
        crate::machkit::components::editor::text_area::VisualDiagnosticLine,
    >,
    target_tab_idx: usize,
) -> (usize, usize) {
    use crate::machkit::components::editor::text_area::VisualDiagnosticLine;
    match current_visual_line.as_ref().unwrap() {
        VisualDiagnosticLine::Code {
            line_idx,
            line_content,
            ..
        } => {
            let line_idx = *line_idx;
            let target_line =
                line_idx.min(state.tabs[target_tab_idx].buffer.len().saturating_sub(1));
            let line_len = state.tabs[target_tab_idx]
                .buffer
                .lines()
                .get(target_line)
                .map_or(0, |l| l.chars().count());
            let active_cursor_col = state.tabs[state.active_tab_idx].cursor.col;
            let target_col = active_cursor_col
                .min(line_content.chars().count())
                .min(line_len);
            (target_line, target_col)
        }
        VisualDiagnosticLine::Header { line, col, .. } => (*line, *col),
        VisualDiagnosticLine::Banner { diag, .. } => (diag.line, diag.col),
    }
}

pub fn handle_editor_keyboard(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
    active_path_start: Option<String>,
    old_revision: Option<usize>,
) {
    if let Some(action) = crate::editor::keymap::map_key(
        &ui.keymap,
        logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
        &["Editor", "Workspace"],
    ) {
        handle_editor_action(
            ui, state, window, elwt, gpu, atlas, font_bytes, action, shift,
        );
    }

    if let (Some(start_path), Some(old_rev)) = (active_path_start, old_revision)
        && state.active_tab_idx < state.tabs.len()
    {
        let active_tab = &state.tabs[state.active_tab_idx];
        if active_tab.path.as_ref() == Some(&start_path) && old_rev != active_tab.buffer.revision {
            let abs_path = crate::editor::get_absolute_path(&start_path);
            ui.diagnostics_file_cache
                .insert(abs_path.clone(), active_tab.buffer.lines().to_vec());
            ui.synced_revisions
                .insert(abs_path, active_tab.buffer.revision);
            ui.diagnostics_changed = true;
        }
    }

    let active_tab = &state.tabs[state.active_tab_idx];
    ui.scroll_to_cursor(
        &active_tab.cursor,
        active_tab.buffer.len(),
        window.inner_size().width as f32,
        window.inner_size().height as f32,
    );
    update_cursor_icon(window, ui, state);
    window.request_redraw();
}

/// Dispatches a mapped editor action to the appropriate sub-handler.
fn handle_editor_action(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    action: crate::editor::actions::Action,
    _shift: bool,
) {
    use crate::editor::actions::Action;
    match action {
        // --- Workspace / global actions ---
        Action::ZoomIn => {
            let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
        }
        Action::ZoomOut => {
            let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
        }
        Action::CommandPalette => {
            ui.active_modal = Some(crate::machkit::ModalType::CommandPalette);
            ui.command_palette_query.clear();
            ui.command_palette_selected = 0;
        }
        Action::GlobalSearch => {
            handle_action(
                ui,
                state,
                UiAction::OpenFile(std::path::PathBuf::from("search://project")),
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
        }
        Action::SaveFile => {
            handle_action(
                ui,
                state,
                UiAction::SaveFile,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
        }
        Action::Escape => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            active_tab.buffer.commit_transaction();
            active_tab.cursor.clear_selection();
            active_tab.secondary_cursors.clear();
        }
        Action::Find => {
            handle_editor_find(ui, state);
        }
        Action::Split => {
            handle_split_action(state);
        }

        // --- Cursor navigation ---
        Action::MoveLeft { select, word } => {
            handle_cursor_move_horizontal(state, select, word, false)
        }
        Action::MoveRight { select, word } => {
            handle_cursor_move_horizontal(state, select, word, true)
        }
        Action::MoveUp { select } => handle_cursor_move_vertical(state, select, false),
        Action::MoveDown { select } => handle_cursor_move_vertical(state, select, true),
        Action::MoveToLineStart { select } => handle_cursor_to_line_boundary(state, select, false),
        Action::MoveToLineEnd { select } => handle_cursor_to_line_boundary(state, select, true),
        Action::SelectAll => handle_select_all(state),

        // --- Line operations ---
        Action::MoveLineUp => handle_move_line_up(state),
        Action::MoveLineDown => handle_move_line_down(state),
        Action::DuplicateLine => handle_duplicate_line(state),
        Action::DeleteLine => handle_delete_line(state),

        // --- Clipboard ---
        Action::Copy => handle_copy(state),
        Action::Cut => handle_cut(state),
        Action::Paste => handle_paste(state),
        Action::Undo => {
            handle_action(
                ui,
                state,
                UiAction::Undo,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
            state.tabs[state.active_tab_idx].cursor.intended_col =
                state.tabs[state.active_tab_idx].cursor.col;
        }
        Action::Redo => {
            handle_action(
                ui,
                state,
                UiAction::Redo,
                window,
                elwt,
                gpu,
                atlas,
                font_bytes,
            );
            state.tabs[state.active_tab_idx].cursor.intended_col =
                state.tabs[state.active_tab_idx].cursor.col;
        }

        // --- Deletion ---
        Action::DeleteLeft => handle_delete_left(state),
        Action::DeleteRight => handle_delete_right(state),

        // --- Insertion ---
        Action::InsertNewLine => handle_insert_new_line(state),
        Action::InsertTab => handle_insert_tab(state),
        Action::InsertChar(s) => handle_insert_char(state, &s),

        // --- Multi-cursor ---
        Action::AddCursorUp => handle_add_cursor_up(state),
        Action::AddCursorDown => handle_add_cursor_down(state),
    }
}

fn handle_editor_find(ui: &mut UiState, state: &mut AppState) {
    ui.show_search_panel = true;
    ui.search_focus_replace = false;
    ui.search_focused = true;
    if state.active_tab_idx < state.tabs.len() {
        let active_tab = &state.tabs[state.active_tab_idx];
        if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
            let selected_text = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
            if !selected_text.contains('\n') && !selected_text.is_empty() {
                ui.search_query = selected_text;
            }
        }
    }
    ui.perform_search(state);
}

fn handle_cursor_move_horizontal(state: &mut AppState, select: bool, word: bool, right: bool) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    for cursor in &mut cursors {
        if right {
            if word {
                cursor.move_word_right(&active_tab.buffer, select);
            } else {
                cursor.move_right(&active_tab.buffer, select);
            }
        } else {
            if word {
                cursor.move_word_left(&active_tab.buffer, select);
            } else {
                cursor.move_left(&active_tab.buffer, select);
            }
        }
    }
    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
}

fn handle_cursor_move_vertical(state: &mut AppState, select: bool, down: bool) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    for cursor in &mut cursors {
        if down {
            cursor.move_down(&active_tab.buffer, select);
        } else {
            cursor.move_up(&active_tab.buffer, select);
        }
    }
    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
}

fn handle_cursor_to_line_boundary(state: &mut AppState, select: bool, end: bool) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    for cursor in &mut cursors {
        if end {
            cursor.move_to_line_end(&active_tab.buffer, select);
        } else {
            cursor.move_to_line_start(select);
        }
    }
    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
}

fn handle_select_all(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    active_tab.cursor.selection_anchor = Some((0, 0));
    active_tab.cursor.line = active_tab.buffer.len() - 1;
    active_tab.cursor.col = active_tab.buffer.lines()[active_tab.cursor.line]
        .chars()
        .count();
    active_tab.cursor.intended_col = active_tab.cursor.col;
    active_tab.secondary_cursors.clear();
}

fn handle_move_line_up(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    let cursor_line = active_tab.cursor.line;
    if cursor_line > 0 {
        active_tab.buffer.commit_transaction();
        active_tab.buffer.start_transaction();
        let line_text = active_tab.buffer.lines()[cursor_line].clone();
        if cursor_line == active_tab.buffer.len() - 1 {
            let prev_len = active_tab.buffer.lines()[cursor_line - 1].chars().count();
            active_tab.buffer.delete(
                cursor_line - 1,
                prev_len,
                cursor_line,
                line_text.chars().count(),
            );
            active_tab
                .buffer
                .insert(cursor_line - 1, 0, &format!("{}\n", line_text));
        } else {
            active_tab.buffer.delete(cursor_line, 0, cursor_line + 1, 0);
            active_tab
                .buffer
                .insert(cursor_line - 1, 0, &format!("{}\n", line_text));
        }
        active_tab.buffer.commit_transaction();
        active_tab.cursor.line -= 1;
        active_tab.cursor.col = active_tab.cursor.col.min(
            active_tab.buffer.lines()[active_tab.cursor.line]
                .chars()
                .count(),
        );
        active_tab.cursor.intended_col = active_tab.cursor.col;
        active_tab.cursor.clear_selection();
    }
}

fn handle_move_line_down(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    let cursor_line = active_tab.cursor.line;
    if cursor_line < active_tab.buffer.len() - 1 {
        active_tab.buffer.commit_transaction();
        active_tab.buffer.start_transaction();
        let target_line = cursor_line + 1;
        let line_text = active_tab.buffer.lines()[target_line].clone();
        if target_line == active_tab.buffer.len() - 1 {
            let prev_len = active_tab.buffer.lines()[target_line - 1].chars().count();
            active_tab.buffer.delete(
                target_line - 1,
                prev_len,
                target_line,
                line_text.chars().count(),
            );
            active_tab
                .buffer
                .insert(target_line - 1, 0, &format!("{}\n", line_text));
        } else {
            active_tab.buffer.delete(target_line, 0, target_line + 1, 0);
            active_tab
                .buffer
                .insert(target_line - 1, 0, &format!("{}\n", line_text));
        }
        active_tab.buffer.commit_transaction();
        active_tab.cursor.line += 1;
        active_tab.cursor.col = active_tab.cursor.col.min(
            active_tab.buffer.lines()[active_tab.cursor.line]
                .chars()
                .count(),
        );
        active_tab.cursor.intended_col = active_tab.cursor.col;
        active_tab.cursor.clear_selection();
    }
}

fn handle_duplicate_line(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    let cursor_line = active_tab.cursor.line;
    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();
    let line_text = active_tab.buffer.lines()[cursor_line].clone();
    if cursor_line == active_tab.buffer.len() - 1 {
        active_tab.buffer.insert(
            cursor_line,
            line_text.chars().count(),
            &format!("\n{}", line_text),
        );
    } else {
        active_tab
            .buffer
            .insert(cursor_line + 1, 0, &format!("{}\n", line_text));
    }
    active_tab.buffer.commit_transaction();
    active_tab.cursor.line += 1;
    active_tab.cursor.col = active_tab.cursor.col.min(
        active_tab.buffer.lines()[active_tab.cursor.line]
            .chars()
            .count(),
    );
    active_tab.cursor.intended_col = active_tab.cursor.col;
    active_tab.cursor.clear_selection();
}

fn handle_delete_line(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    let cursor_line = active_tab.cursor.line;
    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();
    if active_tab.buffer.len() == 1 {
        let col_count = active_tab.buffer.lines()[0].chars().count();
        if col_count > 0 {
            active_tab.buffer.delete(0, 0, 0, col_count);
        }
    } else if cursor_line < active_tab.buffer.len() - 1 {
        active_tab.buffer.delete(cursor_line, 0, cursor_line + 1, 0);
    } else {
        let prev_line = cursor_line - 1;
        let prev_col = active_tab.buffer.lines()[prev_line].chars().count();
        active_tab.buffer.delete(
            prev_line,
            prev_col,
            cursor_line,
            active_tab.buffer.lines()[cursor_line].chars().count(),
        );
    }
    active_tab.buffer.commit_transaction();
    active_tab.cursor.line = cursor_line.min(active_tab.buffer.len() - 1);
    active_tab.cursor.col = active_tab.cursor.col.min(
        active_tab.buffer.lines()[active_tab.cursor.line]
            .chars()
            .count(),
    );
    active_tab.cursor.intended_col = active_tab.cursor.col;
    active_tab.cursor.clear_selection();
}

fn handle_copy(state: &mut AppState) {
    let active_tab = &state.tabs[state.active_tab_idx];
    let mut selections = Vec::new();
    if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
        selections.push(active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c));
    }
    for cur in &active_tab.secondary_cursors {
        if let Some((s_l, s_c, e_l, e_c)) = cur.selection_range() {
            selections.push(active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c));
        }
    }
    if !selections.is_empty() {
        state.copy_to_clipboard(selections.join("\n"));
    }
}

fn handle_cut(state: &mut AppState) {
    let selections = {
        let active_tab = &state.tabs[state.active_tab_idx];
        let mut sels = Vec::new();
        if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
            sels.push(active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c));
        }
        for cur in &active_tab.secondary_cursors {
            if let Some((s_l, s_c, e_l, e_c)) = cur.selection_range() {
                sels.push(active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c));
            }
        }
        sels
    };

    if !selections.is_empty() {
        state.copy_to_clipboard(selections.join("\n"));
        let active_tab = &mut state.tabs[state.active_tab_idx];
        active_tab.buffer.commit_transaction();
        active_tab.buffer.start_transaction();

        let mut cursors = vec![active_tab.cursor];
        cursors.append(&mut active_tab.secondary_cursors);
        cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));
        for cursor in &mut cursors {
            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                cursor.line = s_l;
                cursor.col = s_c;
                cursor.clear_selection();
            }
        }
        cursors.sort_by_key(|c| (c.line, c.col));
        cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
        active_tab.cursor = cursors[0];
        active_tab.secondary_cursors = cursors[1..].to_vec();
        active_tab.buffer.commit_transaction();
    }
}

fn handle_paste(state: &mut AppState) {
    let clipboard_text = state.paste_from_clipboard();
    if !clipboard_text.is_empty() {
        let active_tab = &mut state.tabs[state.active_tab_idx];
        active_tab.buffer.commit_transaction();
        active_tab.buffer.start_transaction();

        let mut cursors = vec![active_tab.cursor];
        cursors.append(&mut active_tab.secondary_cursors);
        cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));

        for cursor in &mut cursors {
            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                cursor.line = s_l;
                cursor.col = s_c;
                cursor.clear_selection();
            }
            active_tab
                .buffer
                .insert(cursor.line, cursor.col, &clipboard_text);
            let parts = clipboard_text.split('\n').collect::<Vec<&str>>();
            if parts.len() == 1 {
                cursor.col += clipboard_text.chars().count();
            } else {
                cursor.line += parts.len() - 1;
                cursor.col = parts.last().unwrap().chars().count();
            }
            cursor.intended_col = cursor.col;
        }

        cursors.sort_by_key(|c| (c.line, c.col));
        cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
        active_tab.cursor = cursors[0];
        active_tab.secondary_cursors = cursors[1..].to_vec();
        active_tab.buffer.commit_transaction();
    }
}

fn handle_delete_left(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();

    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));

    for cursor in &mut cursors {
        if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
            cursor.line = s_l;
            cursor.col = s_c;
            cursor.intended_col = s_c;
            cursor.clear_selection();
        } else if cursor.col > 0 || cursor.line > 0 {
            if try_delete_paired_char(active_tab, cursor) {
                // paired bracket/quote deleted
            } else {
                let mut prev_cursor = *cursor;
                prev_cursor.move_left(&active_tab.buffer, false);
                active_tab.buffer.delete(
                    prev_cursor.line,
                    prev_cursor.col,
                    cursor.line,
                    cursor.col,
                );
                *cursor = prev_cursor;
            }
        }
    }

    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
    active_tab.buffer.commit_transaction();
}

/// Tries to delete a paired bracket/quote at cursor. Returns true if a pair was deleted.
fn try_delete_paired_char(active_tab: &mut crate::app::Tab, cursor: &mut Cursor) -> bool {
    if cursor.col == 0 {
        return false;
    }
    let line_chars: Vec<char> = active_tab.buffer.lines()[cursor.line].chars().collect();
    if cursor.col >= line_chars.len() {
        return false;
    }
    let left_char = line_chars[cursor.col - 1];
    let right_char = line_chars[cursor.col];
    let is_paired = matches!(
        (left_char, right_char),
        ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"') | ('\'', '\'')
    );
    if is_paired {
        active_tab
            .buffer
            .delete(cursor.line, cursor.col - 1, cursor.line, cursor.col + 1);
        cursor.col -= 1;
        cursor.intended_col = cursor.col;
        true
    } else {
        false
    }
}

fn handle_delete_right(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();

    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));

    for cursor in &mut cursors {
        if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
            cursor.line = s_l;
            cursor.col = s_c;
            cursor.intended_col = s_c;
            cursor.clear_selection();
        } else {
            let line_len = active_tab.buffer.lines()[cursor.line].chars().count();
            if cursor.col < line_len || cursor.line < active_tab.buffer.len() - 1 {
                let mut next_cursor = *cursor;
                next_cursor.move_right(&active_tab.buffer, false);
                active_tab.buffer.delete(
                    cursor.line,
                    cursor.col,
                    next_cursor.line,
                    next_cursor.col,
                );
            }
        }
    }

    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
    active_tab.buffer.commit_transaction();
}

fn handle_insert_new_line(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();

    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));

    for cursor in &mut cursors {
        if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
            cursor.line = s_l;
            cursor.col = s_c;
            cursor.clear_selection();
        }
        active_tab.buffer.insert(cursor.line, cursor.col, "\n");
        cursor.line += 1;
        cursor.col = 0;
        cursor.intended_col = 0;
    }

    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
    active_tab.buffer.commit_transaction();
}

fn handle_insert_tab(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();

    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));

    for cursor in &mut cursors {
        if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
            cursor.line = s_l;
            cursor.col = s_c;
            cursor.clear_selection();
        }
        active_tab.buffer.insert(cursor.line, cursor.col, "    ");
        cursor.col += 4;
        cursor.intended_col = cursor.col;
    }

    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();
    active_tab.buffer.commit_transaction();
}

fn handle_insert_char(state: &mut AppState, s: &str) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    let had_selection = active_tab.cursor.selection_range().is_some()
        || active_tab
            .secondary_cursors
            .iter()
            .any(|c| c.selection_range().is_some());

    active_tab.buffer.commit_transaction();
    active_tab.buffer.start_transaction();

    let mut cursors = vec![active_tab.cursor];
    cursors.append(&mut active_tab.secondary_cursors);
    cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));

    for cursor in &mut cursors {
        insert_char_at_cursor(active_tab, cursor, s);
    }

    cursors.sort_by_key(|c| (c.line, c.col));
    cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
    active_tab.cursor = cursors[0];
    active_tab.secondary_cursors = cursors[1..].to_vec();

    let is_boundary = s
        .chars()
        .any(|c| c.is_whitespace() || c.is_ascii_punctuation())
        || had_selection;
    if is_boundary {
        active_tab.buffer.commit_transaction();
    }
}

/// Handles inserting a single character string at a cursor position, with bracket/quote pairing.
fn insert_char_at_cursor(active_tab: &mut crate::app::Tab, cursor: &mut Cursor, s: &str) {
    // Check for step-over (closing bracket under cursor)
    if let Some(c) = single_char(s) {
        if try_step_over_closing(active_tab, cursor, c) {
            return;
        }
        if try_wrap_selection(active_tab, cursor, c) {
            return;
        }
        if try_insert_paired(active_tab, cursor, c) {
            return;
        }
    }
    // Default: delete selection and insert
    if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
        active_tab.buffer.delete(s_l, s_c, e_l, e_c);
        cursor.line = s_l;
        cursor.col = s_c;
        cursor.clear_selection();
    }
    active_tab.buffer.insert(cursor.line, cursor.col, s);
    cursor.col += s.chars().count();
    cursor.intended_col = cursor.col;
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_none() {
        Some(c)
    } else {
        None
    }
}

fn matching_close(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

/// If c is a closing bracket and the character under the cursor matches, step over it.
fn try_step_over_closing(active_tab: &mut crate::app::Tab, cursor: &mut Cursor, c: char) -> bool {
    if cursor.selection_range().is_some() {
        return false;
    }
    if !matches!(c, ')' | ']' | '}' | '"' | '\'') {
        return false;
    }
    let line_chars: Vec<char> = active_tab.buffer.lines()[cursor.line].chars().collect();
    if cursor.col < line_chars.len() && line_chars[cursor.col] == c {
        cursor.col += 1;
        cursor.intended_col = cursor.col;
        true
    } else {
        false
    }
}

/// If there is a selection and c is an opening bracket, wrap the selection with bracket pair.
fn try_wrap_selection(active_tab: &mut crate::app::Tab, cursor: &mut Cursor, c: char) -> bool {
    let close = match matching_close(c) {
        Some(cl) => cl,
        None => return false,
    };
    let (s_l, s_c, e_l, e_c) = match cursor.selection_range() {
        Some(r) => r,
        None => return false,
    };
    active_tab.buffer.insert(s_l, s_c, &c.to_string());
    let adjusted_e_c = if s_l == e_l { e_c + 1 } else { e_c };
    active_tab
        .buffer
        .insert(e_l, adjusted_e_c, &close.to_string());
    if cursor.selection_anchor.unwrap().0 == s_l && cursor.selection_anchor.unwrap().1 == s_c {
        cursor.selection_anchor = Some((s_l, s_c + 1));
        cursor.line = e_l;
        cursor.col = adjusted_e_c;
    } else {
        cursor.selection_anchor = Some((e_l, adjusted_e_c));
        cursor.line = s_l;
        cursor.col = s_c + 1;
    }
    cursor.intended_col = cursor.col;
    true
}

/// If c is an opening bracket/quote with no selection, auto-pair it.
fn try_insert_paired(active_tab: &mut crate::app::Tab, cursor: &mut Cursor, c: char) -> bool {
    if cursor.selection_range().is_some() {
        return false;
    }
    let close = match matching_close(c) {
        Some(cl) => cl,
        None => return false,
    };
    let should_pair = match c {
        '(' | '[' | '{' => true,
        '"' | '\'' => {
            let line_chars: Vec<char> = active_tab.buffer.lines()[cursor.line].chars().collect();
            let preceded_by_alpha = cursor.col > 0 && line_chars[cursor.col - 1].is_alphanumeric();
            let followed_by_alpha =
                cursor.col < line_chars.len() && line_chars[cursor.col].is_alphanumeric();
            !preceded_by_alpha && !followed_by_alpha
        }
        _ => false,
    };
    if should_pair {
        let pair_str = format!("{}{}", c, close);
        active_tab.buffer.insert(cursor.line, cursor.col, &pair_str);
        cursor.col += 1;
        cursor.intended_col = cursor.col;
        true
    } else {
        false
    }
}

fn handle_add_cursor_up(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    let cursors: Vec<Cursor> = std::iter::once(active_tab.cursor)
        .chain(active_tab.secondary_cursors.iter().cloned())
        .collect();
    if let Some(min_cursor) = cursors.iter().min_by_key(|c| c.line)
        && min_cursor.line > 0
    {
        let target_line = min_cursor.line - 1;
        let line_len = active_tab.buffer.lines()[target_line].chars().count();
        let target_col = min_cursor.intended_col.min(line_len);
        let new_cursor = Cursor {
            line: target_line,
            col: target_col,
            intended_col: min_cursor.intended_col,
            selection_anchor: None,
        };
        if !cursors
            .iter()
            .any(|c| c.line == target_line && c.col == target_col)
        {
            active_tab.secondary_cursors.push(new_cursor);
        }
    }
}

fn handle_add_cursor_down(state: &mut AppState) {
    let active_tab = &mut state.tabs[state.active_tab_idx];
    active_tab.buffer.commit_transaction();
    let cursors: Vec<Cursor> = std::iter::once(active_tab.cursor)
        .chain(active_tab.secondary_cursors.iter().cloned())
        .collect();
    if let Some(max_cursor) = cursors.iter().max_by_key(|c| c.line)
        && max_cursor.line < active_tab.buffer.len() - 1
    {
        let target_line = max_cursor.line + 1;
        let line_len = active_tab.buffer.lines()[target_line].chars().count();
        let target_col = max_cursor.intended_col.min(line_len);
        let new_cursor = Cursor {
            line: target_line,
            col: target_col,
            intended_col: max_cursor.intended_col,
            selection_anchor: None,
        };
        if !cursors
            .iter()
            .any(|c| c.line == target_line && c.col == target_col)
        {
            active_tab.secondary_cursors.push(new_cursor);
        }
    }
}

fn insert_char_at(s: &str, idx: usize, c: char) -> String {
    let mut result = String::new();
    let mut inserted = false;
    for (i, ch) in s.chars().enumerate() {
        if i == idx {
            result.push(c);
            inserted = true;
        }
        result.push(ch);
    }
    if !inserted {
        result.push(c);
    }
    result
}

fn remove_char_at(s: &str, idx: usize) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i != idx {
            result.push(ch);
        }
    }
    result
}

fn sync_file_changes(
    ui: &mut UiState,
    state: &mut AppState,
    path: std::path::PathBuf,
    line_idx: usize,
    new_line_content: String,
) {
    // 1. Update project_search_file_cache
    if let Some(lines) = ui.project_search_file_cache.get_mut(&path)
        && line_idx < lines.len()
    {
        lines[line_idx] = new_line_content.clone();
    }

    // 2. Update all open tabs/buffers matching this path in active and inactive panes
    let abs_target_path = crate::editor::get_absolute_path(&path.to_string_lossy());

    // Check active tabs
    for tab in &mut state.tabs {
        if let Some(ref tab_path) = tab.path
            && crate::editor::get_absolute_path(tab_path) == abs_target_path
            && line_idx < tab.buffer.len()
        {
            let old_line_len = tab.buffer.lines()[line_idx].chars().count();
            tab.buffer.commit_transaction();
            tab.buffer.start_transaction();
            tab.buffer.delete(line_idx, 0, line_idx, old_line_len);
            tab.buffer.insert(line_idx, 0, &new_line_content);
            tab.buffer.commit_transaction();
        }
    }
    // Check inactive pane tabs
    for pane in &mut state.inactive_panes {
        for tab in &mut pane.tabs {
            if let Some(ref tab_path) = tab.path
                && crate::editor::get_absolute_path(tab_path) == abs_target_path
                && line_idx < tab.buffer.len()
            {
                let old_line_len = tab.buffer.lines()[line_idx].chars().count();
                tab.buffer.commit_transaction();
                tab.buffer.start_transaction();
                tab.buffer.delete(line_idx, 0, line_idx, old_line_len);
                tab.buffer.insert(line_idx, 0, &new_line_content);
                tab.buffer.commit_transaction();
            }
        }
    }

    // 3. Update all instances in ui.global_search_results
    for (res_path, res_line_idx, res_content) in &mut ui.global_search_results {
        if *res_path == path && *res_line_idx == line_idx {
            *res_content = new_line_content.clone();
        }
    }

    // 4. Write back to disk (Asynchronously on a background thread)
    ui.invalidate_search_render_items();
    let path_clone = path.clone();
    let cache_lines = ui.project_search_file_cache.get(&path).cloned();
    std::thread::spawn(move || {
        if let Some(lines) = cache_lines {
            let joined = lines.join("\n");
            let _ = std::fs::write(&path_clone, joined);
        } else if let Ok(content) = std::fs::read_to_string(&path_clone) {
            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if line_idx < lines.len() {
                lines[line_idx] = new_line_content;
                let joined = lines.join("\n");
                let _ = std::fs::write(&path_clone, joined);
            }
        }
    });
}

fn handle_project_search_action(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    _elwt: &EventLoopWindowTarget<()>,
    _gpu: &mut Option<GpuContext>,
    _atlas: &mut FontAtlas,
    _font_bytes: &[u8],
    action: crate::editor::actions::Action,
    _shift_key: bool,
) -> bool {
    use crate::editor::actions::Action;
    let results_len = ui.global_search_results.len();
    if ui.global_search_selected >= results_len {
        return false;
    }
    let (path, line_idx, current_content) =
        ui.global_search_results[ui.global_search_selected].clone();
    let char_count = current_content.chars().count();

    // Helper to update selection anchor
    let update_selection = |ui: &mut UiState, select: bool, prev_col: usize| {
        if select {
            if ui.global_search_selection_anchor.is_none() {
                ui.global_search_selection_anchor = Some(prev_col);
            }
        } else {
            ui.global_search_selection_anchor = None;
        }
    };

    match action {
        Action::MoveLeft { select, word } => {
            let prev_col = ui.global_search_col;
            update_selection(ui, select, prev_col);
            if word {
                let line_chars: Vec<char> = current_content.chars().collect();
                let mut idx = ui.global_search_col.min(char_count);
                while idx > 0 && idx - 1 < line_chars.len() && line_chars[idx - 1].is_whitespace() {
                    idx -= 1;
                }
                if idx > 0 && idx - 1 < line_chars.len() {
                    let start_is_alphanumeric = line_chars[idx - 1].is_alphanumeric();
                    while idx > 0
                        && idx - 1 < line_chars.len()
                        && line_chars[idx - 1].is_alphanumeric() == start_is_alphanumeric
                        && !line_chars[idx - 1].is_whitespace()
                    {
                        idx -= 1;
                    }
                }
                ui.global_search_col = idx;
            } else if ui.global_search_col > 0 {
                ui.global_search_col -= 1;
            }
            window.request_redraw();
            true
        }
        Action::MoveRight { select, word } => {
            let prev_col = ui.global_search_col;
            update_selection(ui, select, prev_col);
            if word {
                let line_chars: Vec<char> = current_content.chars().collect();
                let mut idx = ui.global_search_col;
                while idx < char_count && idx < line_chars.len() && line_chars[idx].is_whitespace()
                {
                    idx += 1;
                }
                if idx < char_count && idx < line_chars.len() {
                    let start_is_alphanumeric = line_chars[idx].is_alphanumeric();
                    while idx < char_count
                        && idx < line_chars.len()
                        && line_chars[idx].is_alphanumeric() == start_is_alphanumeric
                        && !line_chars[idx].is_whitespace()
                    {
                        idx += 1;
                    }
                }
                ui.global_search_col = idx;
            } else {
                ui.global_search_col = (ui.global_search_col + 1).min(char_count);
            }
            window.request_redraw();
            true
        }
        Action::MoveUp { select } => {
            let prev_col = ui.global_search_col;
            update_selection(ui, select, prev_col);
            let items_count = ui.global_search_results.len();
            if items_count > 0 {
                ui.global_search_selected =
                    (ui.global_search_selected + items_count - 1) % items_count;
                let new_content = &ui.global_search_results[ui.global_search_selected].2;
                ui.global_search_col = ui.global_search_col.min(new_content.chars().count());
                if !select {
                    ui.global_search_selection_anchor = None;
                }
            }
            window.request_redraw();
            true
        }
        Action::MoveDown { select } => {
            let prev_col = ui.global_search_col;
            update_selection(ui, select, prev_col);
            let items_count = ui.global_search_results.len();
            if items_count > 0 {
                ui.global_search_selected = (ui.global_search_selected + 1) % items_count;
                let new_content = &ui.global_search_results[ui.global_search_selected].2;
                ui.global_search_col = ui.global_search_col.min(new_content.chars().count());
                if !select {
                    ui.global_search_selection_anchor = None;
                }
            }
            window.request_redraw();
            true
        }
        Action::MoveToLineStart { select } => {
            let prev_col = ui.global_search_col;
            update_selection(ui, select, prev_col);
            ui.global_search_col = 0;
            window.request_redraw();
            true
        }
        Action::MoveToLineEnd { select } => {
            let prev_col = ui.global_search_col;
            update_selection(ui, select, prev_col);
            ui.global_search_col = char_count;
            window.request_redraw();
            true
        }
        Action::SelectAll => {
            ui.global_search_selection_anchor = Some(0);
            ui.global_search_col = char_count;
            window.request_redraw();
            true
        }
        Action::Copy => {
            if let Some(anchor) = ui.global_search_selection_anchor {
                let start = anchor.min(ui.global_search_col).min(char_count);
                let end = anchor.max(ui.global_search_col).min(char_count);
                if start != end {
                    let chars: Vec<char> = current_content.chars().collect();
                    let selected_str: String = chars[start..end].iter().collect();
                    state.copy_to_clipboard(selected_str);
                }
            }
            true
        }
        Action::Cut => {
            if let Some(anchor) = ui.global_search_selection_anchor {
                let start = anchor.min(ui.global_search_col).min(char_count);
                let end = anchor.max(ui.global_search_col).min(char_count);
                if start != end {
                    let chars: Vec<char> = current_content.chars().collect();
                    let selected_str: String = chars[start..end].iter().collect();
                    state.copy_to_clipboard(selected_str);

                    let mut new_chars = chars;
                    new_chars.drain(start..end);
                    let new_content: String = new_chars.into_iter().collect();
                    ui.global_search_col = start;
                    ui.global_search_selection_anchor = None;
                    sync_file_changes(ui, state, path, line_idx, new_content);
                    window.request_redraw();
                }
            }
            true
        }
        Action::Paste => {
            let pasted = state.paste_from_clipboard();
            if !pasted.is_empty() {
                let chars: Vec<char> = current_content.chars().collect();
                let start = if let Some(anchor) = ui.global_search_selection_anchor {
                    anchor.min(ui.global_search_col).min(char_count)
                } else {
                    ui.global_search_col.min(char_count)
                };
                let end = if let Some(anchor) = ui.global_search_selection_anchor {
                    anchor.max(ui.global_search_col).min(char_count)
                } else {
                    ui.global_search_col.min(char_count)
                };

                let mut new_chars = chars;
                new_chars.drain(start..end);

                let paste_chars: Vec<char> = pasted
                    .chars()
                    .filter(|c| *c != '\n' && *c != '\r')
                    .collect();
                let paste_len = paste_chars.len();
                for (offset, c) in paste_chars.into_iter().enumerate() {
                    new_chars.insert(start + offset, c);
                }

                let new_content: String = new_chars.into_iter().collect();
                ui.global_search_col = start + paste_len;
                ui.global_search_selection_anchor = None;
                sync_file_changes(ui, state, path, line_idx, new_content);
                window.request_redraw();
            }
            true
        }
        Action::DeleteLeft => {
            let chars: Vec<char> = current_content.chars().collect();
            if let Some(anchor) = ui.global_search_selection_anchor
                && anchor != ui.global_search_col
            {
                let start = anchor.min(ui.global_search_col).min(char_count);
                let end = anchor.max(ui.global_search_col).min(char_count);
                let mut new_chars = chars;
                new_chars.drain(start..end);
                let new_content: String = new_chars.into_iter().collect();
                ui.global_search_col = start;
                ui.global_search_selection_anchor = None;
                sync_file_changes(ui, state, path, line_idx, new_content);
            } else if ui.global_search_col > 0 {
                let new_content = remove_char_at(&current_content, ui.global_search_col - 1);
                ui.global_search_col -= 1;
                ui.global_search_selection_anchor = None;
                sync_file_changes(ui, state, path, line_idx, new_content);
            }
            window.request_redraw();
            true
        }
        Action::DeleteRight => {
            let chars: Vec<char> = current_content.chars().collect();
            if let Some(anchor) = ui.global_search_selection_anchor
                && anchor != ui.global_search_col
            {
                let start = anchor.min(ui.global_search_col).min(char_count);
                let end = anchor.max(ui.global_search_col).min(char_count);
                let mut new_chars = chars;
                new_chars.drain(start..end);
                let new_content: String = new_chars.into_iter().collect();
                ui.global_search_col = start;
                ui.global_search_selection_anchor = None;
                sync_file_changes(ui, state, path, line_idx, new_content);
            } else if ui.global_search_col < char_count {
                let new_content = remove_char_at(&current_content, ui.global_search_col);
                ui.global_search_selection_anchor = None;
                sync_file_changes(ui, state, path, line_idx, new_content);
            }
            window.request_redraw();
            true
        }
        Action::InsertNewLine => true,
        Action::InsertChar(s) => {
            let chars: Vec<char> = current_content.chars().collect();
            let start = if let Some(anchor) = ui.global_search_selection_anchor {
                anchor.min(ui.global_search_col).min(char_count)
            } else {
                ui.global_search_col.min(char_count)
            };
            let end = if let Some(anchor) = ui.global_search_selection_anchor {
                anchor.max(ui.global_search_col).min(char_count)
            } else {
                ui.global_search_col.min(char_count)
            };

            let mut new_chars = chars;
            new_chars.drain(start..end);

            let insert_chars: Vec<char> = s.chars().filter(|c| !c.is_control()).collect();
            let insert_len = insert_chars.len();
            for (offset, c) in insert_chars.into_iter().enumerate() {
                new_chars.insert(start + offset, c);
            }

            let new_content: String = new_chars.into_iter().collect();
            ui.global_search_col = start + insert_len;
            ui.global_search_selection_anchor = None;
            sync_file_changes(ui, state, path, line_idx, new_content);
            window.request_redraw();
            true
        }
        Action::Escape => {
            ui.global_search_selection_anchor = None;
            window.request_redraw();
            true
        }
        _ => false,
    }
}

pub fn handle_project_search_keyboard(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    if state.tabs.is_empty() || state.active_tab_idx >= state.tabs.len() {
        return false;
    }
    if state.tabs[state.active_tab_idx].path.as_deref() != Some("search://project") {
        return false;
    }

    if let Some(action) = crate::editor::keymap::map_key(
        &ui.keymap,
        logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
        &["Editor", "Workspace"],
    ) {
        if is_workspace_global_action(&action) {
            return false;
        }

        if !ui.global_search_focused
            && handle_project_search_action(
                ui, state, window, elwt, gpu, atlas, font_bytes, action, shift,
            )
        {
            return true;
        }
    }

    if ui.global_search_focused {
        handle_project_search_focused_input(
            ui,
            state,
            window,
            elwt,
            gpu,
            atlas,
            font_bytes,
            logical_key,
            alt,
        );
    } else {
        handle_project_search_result_input(
            ui,
            state,
            window,
            elwt,
            gpu,
            atlas,
            font_bytes,
            logical_key,
            ctrl,
            alt,
        );
    }
    true
}

fn handle_project_search_focused_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    alt: bool,
) {
    match logical_key {
        Key::Named(NamedKey::ArrowDown) => {
            let items_count = ui.global_search_results.len();
            if items_count > 0 {
                ui.global_search_selected = (ui.global_search_selected + 1) % items_count;
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::ArrowUp) => {
            let items_count = ui.global_search_results.len();
            if items_count > 0 {
                ui.global_search_selected =
                    (ui.global_search_selected + items_count - 1) % items_count;
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::Enter) => {
            if alt {
                let results_len = ui.global_search_results.len();
                if ui.global_search_selected < results_len {
                    let (path, line_idx, _) = &ui.global_search_results[ui.global_search_selected];
                    handle_action(
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
                let q = ui.global_search_query.clone();
                ui.run_global_search(q);
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::Tab) => {
            if ui.global_show_replace {
                ui.global_search_focus_replace = !ui.global_search_focus_replace;
            } else {
                ui.global_search_focus_replace = false;
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::Backspace) => {
            if ui.global_search_focus_replace {
                ui.global_replace_query.pop();
            } else {
                ui.global_search_query.pop();
                ui.global_search_selected = 0;
            }
            window.request_redraw();
        }
        Key::Character(text) => {
            for c in text.chars() {
                if !c.is_control() {
                    if ui.global_search_focus_replace {
                        ui.global_replace_query.push(c);
                    } else {
                        ui.global_search_query.push(c);
                    }
                }
            }
            if !ui.global_search_focus_replace {
                ui.global_search_selected = 0;
            }
            window.request_redraw();
        }
        _ => {}
    }
}

fn handle_project_search_result_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
    ctrl: bool,
    alt: bool,
) {
    let results_len = ui.global_search_results.len();
    if ui.global_search_selected >= results_len {
        return;
    }
    let (path, line_idx, current_content) =
        ui.global_search_results[ui.global_search_selected].clone();

    match logical_key {
        Key::Named(NamedKey::ArrowLeft) => {
            ui.global_search_col = ui.global_search_col.saturating_sub(1);
            window.request_redraw();
        }
        Key::Named(NamedKey::ArrowRight) => {
            let char_count = current_content.chars().count();
            ui.global_search_col = (ui.global_search_col + 1).min(char_count);
            window.request_redraw();
        }
        Key::Named(NamedKey::Home) => {
            ui.global_search_col = 0;
            window.request_redraw();
        }
        Key::Named(NamedKey::End) => {
            ui.global_search_col = current_content.chars().count();
            window.request_redraw();
        }
        Key::Named(NamedKey::ArrowDown) => {
            let items_count = ui.global_search_results.len();
            if items_count > 0 {
                ui.global_search_selected = (ui.global_search_selected + 1) % items_count;
                let new_content = &ui.global_search_results[ui.global_search_selected].2;
                ui.global_search_col = ui.global_search_col.min(new_content.chars().count());
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::ArrowUp) => {
            let items_count = ui.global_search_results.len();
            if items_count > 0 {
                ui.global_search_selected =
                    (ui.global_search_selected + items_count - 1) % items_count;
                let new_content = &ui.global_search_results[ui.global_search_selected].2;
                ui.global_search_col = ui.global_search_col.min(new_content.chars().count());
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::Enter) => {
            if alt {
                handle_action(
                    ui,
                    state,
                    UiAction::OpenFileAt(path.clone(), line_idx),
                    window,
                    elwt,
                    gpu,
                    atlas,
                    font_bytes,
                );
            }
            window.request_redraw();
        }
        Key::Named(NamedKey::Backspace) => {
            if !ctrl && !alt && ui.global_search_col > 0 {
                let new_content = remove_char_at(&current_content, ui.global_search_col - 1);
                ui.global_search_col -= 1;
                sync_file_changes(ui, state, path, line_idx, new_content);
                window.request_redraw();
            }
        }
        Key::Named(NamedKey::Delete) => {
            if !ctrl && !alt {
                let char_count = current_content.chars().count();
                if ui.global_search_col < char_count {
                    let new_content = remove_char_at(&current_content, ui.global_search_col);
                    sync_file_changes(ui, state, path, line_idx, new_content);
                    window.request_redraw();
                }
            }
        }
        Key::Character(text) => {
            if !ctrl && !alt {
                let mut temp_content = current_content.clone();
                let mut inserted_count = 0;
                for c in text.chars() {
                    if !c.is_control() {
                        temp_content =
                            insert_char_at(&temp_content, ui.global_search_col + inserted_count, c);
                        inserted_count += 1;
                    }
                }
                if inserted_count > 0 {
                    ui.global_search_col += inserted_count;
                    sync_file_changes(ui, state, path, line_idx, temp_content);
                    window.request_redraw();
                }
            }
        }
        _ => {}
    }
}
