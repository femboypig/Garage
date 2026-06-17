use std::sync::Arc;
use std::io::Write;
use winit::window::Window;
use winit::keyboard::{Key, PhysicalKey, NamedKey};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::machkit::{UiState, UiAction};
use crate::renderer::atlas::FontAtlas;
use crate::app::state::AppState;
use crate::app::handler::handle_action;
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::app::input::mouse::update_cursor_icon;

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
    if state.terminal_focus && !state.dock_terminals.is_empty() {
        if let Some(action) = crate::editor::keymap::map_key(
            &ui.keymap,
            &logical_key,
            physical_key,
            ctrl,
            shift,
            alt,
            &["Workspace"],
        ) {
            match action {
                crate::editor::actions::Action::ZoomIn => {
                    let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
                    ui.update_buffer_font_size(&atlas.font, new_size);
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::ZoomOut => {
                    let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
                    ui.update_buffer_font_size(&atlas.font, new_size);
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::CommandPalette => {
                    ui.active_modal = Some(crate::machkit::ModalType::CommandPalette);
                    ui.command_palette_query.clear();
                    ui.command_palette_selected = 0;
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::GlobalSearch => {
                    handle_action(ui, state, UiAction::OpenFile(std::path::PathBuf::from("search://project")), window, elwt, gpu, atlas, font_bytes);
                    window.request_redraw();
                    return;
                }
                _ => {}
            }
        }
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
    if ui.show_search_panel && ui.search_focused {
        // First, check if there's an action mapped
        if let Some(action) = crate::editor::keymap::map_key(&ui.keymap, &logical_key, physical_key, ctrl, shift, alt, &["Editor", "Workspace"]) {
            match action {
                crate::editor::actions::Action::Find => {
                    // Seed search panel with selection if any
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
                    return;
                }
                crate::editor::actions::Action::Split => {
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
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::Escape => {
                    ui.show_search_panel = false;
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::ZoomIn => {
                    let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
                    ui.update_buffer_font_size(&atlas.font, new_size);
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::ZoomOut => {
                    let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
                    ui.update_buffer_font_size(&atlas.font, new_size);
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::CommandPalette => {
                    ui.active_modal = Some(crate::machkit::ModalType::CommandPalette);
                    ui.command_palette_query.clear();
                    ui.command_palette_selected = 0;
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::GlobalSearch => {
                    handle_action(ui, state, UiAction::OpenFile(std::path::PathBuf::from("search://project")), window, elwt, gpu, atlas, font_bytes);
                    window.request_redraw();
                    return;
                }
                crate::editor::actions::Action::SaveFile => {
                    handle_action(ui, state, UiAction::SaveFile, window, elwt, gpu, atlas, font_bytes);
                    window.request_redraw();
                    return;
                }
                _ => {}
            }
        }

        // Otherwise handle input editing
        match &logical_key {
            Key::Named(NamedKey::Escape) => {
                ui.show_search_panel = false;
                window.request_redraw();
                return;
            }
            Key::Named(NamedKey::Tab) => {
                if ui.show_replace {
                    ui.search_focus_replace = !ui.search_focus_replace;
                } else {
                    ui.search_focus_replace = false;
                }
                window.request_redraw();
                return;
            }
            Key::Named(NamedKey::Enter) => {
                ui.perform_search(state);
                if !ui.search_matches.is_empty() {
                    if shift {
                        if ui.active_search_match_idx == 0 {
                            ui.active_search_match_idx = ui.search_matches.len() - 1;
                        } else {
                            ui.active_search_match_idx -= 1;
                        }
                    } else {
                        if ui.active_search_match_idx >= ui.search_matches.len() - 1 {
                            ui.active_search_match_idx = 0;
                        } else {
                            ui.active_search_match_idx += 1;
                        }
                    }
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
            Key::Named(NamedKey::Backspace) => {
                if ui.search_focus_replace {
                    ui.replace_query.pop();
                } else {
                    ui.search_query.pop();
                }
                window.request_redraw();
                return;
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
                    return;
                }
            }
            _ => {}
        }
    }

    // Handle typing inside the SidebarInput modal
    if ui.active_modal == Some(crate::machkit::ModalType::SidebarInput) {
        match &logical_key {
            Key::Named(NamedKey::Escape) => {
                ui.active_modal = None;
                window.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
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
        return;
    }

    // 2. Delegate to command palette modal handler if command palette is active
    if handle_command_palette_input(ui, state, window, elwt, gpu, atlas, font_bytes, &logical_key) {
        return;
    }

    // 2b. Delegate to global search modal handler if global search is active
    if handle_global_search_input(ui, state, window, elwt, gpu, atlas, font_bytes, &logical_key) {
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

        let modifier_code = 1 
            + if shift { 1 } else { 0 } 
            + if alt { 2 } else { 0 } 
            + if ctrl { 4 } else { 0 };

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
                    // Alt + key sends ESC followed by the key character (meta key)
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
                    ui.command_palette_selected = (ui.command_palette_selected + items_count - 1) % items_count;
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
                         ui.execute_command(cmd, &mut active_tab.buffer, &mut active_tab.cursor, active_path)
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
                    ui.global_search_selected = (ui.global_search_selected + items_count - 1) % items_count;
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
                        let (path, line_idx, _) = &ui.global_search_results[ui.global_search_selected];
                        ui.active_modal = None;
                        handle_action(ui, state, UiAction::OpenFileAt(path.clone(), *line_idx), window, elwt, gpu, atlas, font_bytes);
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
    if state.tabs[state.active_tab_idx].path.as_deref() == Some("diagnostics://project") {
        if let Some(action) = crate::editor::keymap::map_key(&ui.keymap, logical_key, physical_key, ctrl, shift, alt, &["Editor", "Workspace"]) {
            let is_navigation_action = match &action {
                crate::editor::actions::Action::MoveUp { .. } |
                crate::editor::actions::Action::MoveDown { .. } |
                crate::editor::actions::Action::MoveLeft { .. } |
                crate::editor::actions::Action::MoveRight { .. } |
                crate::editor::actions::Action::MoveToLineStart { .. } |
                crate::editor::actions::Action::MoveToLineEnd { .. } => true,
                _ => false,
            };

            let is_global_action = match &action {
                crate::editor::actions::Action::ZoomIn |
                crate::editor::actions::Action::ZoomOut |
                crate::editor::actions::Action::CommandPalette |
                crate::editor::actions::Action::GlobalSearch |
                crate::editor::actions::Action::Escape => true,
                _ => false,
            };

            let is_document_action = !is_navigation_action && !is_global_action;

            if is_navigation_action {
                match &action {
                    crate::editor::actions::Action::MoveUp { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        if active_tab.cursor.line > 0 {
                            active_tab.cursor.line -= 1;
                        }
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
                        });
                        active_tab.cursor.col = active_tab.cursor.col.min(line_len);
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    }
                    crate::editor::actions::Action::MoveDown { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        if !visual_lines.is_empty() && active_tab.cursor.line < visual_lines.len() - 1 {
                            active_tab.cursor.line += 1;
                        }
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
                        });
                        active_tab.cursor.col = active_tab.cursor.col.min(line_len);
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    }
                    crate::editor::actions::Action::MoveLeft { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        if active_tab.cursor.col > 0 {
                            active_tab.cursor.col -= 1;
                        } else if active_tab.cursor.line > 0 {
                            active_tab.cursor.line -= 1;
                            let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
                            let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                                crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                                _ => 0,
                            });
                            active_tab.cursor.col = line_len;
                        }
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    }
                    crate::editor::actions::Action::MoveRight { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
                        });
                        if active_tab.cursor.col < line_len {
                            active_tab.cursor.col += 1;
                        } else if active_tab.cursor.line < visual_lines.len().saturating_sub(1) {
                            active_tab.cursor.line += 1;
                            active_tab.cursor.col = 0;
                        }
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    }
                    crate::editor::actions::Action::MoveToLineStart { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        active_tab.cursor.col = 0;
                        active_tab.cursor.intended_col = 0;
                    }
                    crate::editor::actions::Action::MoveToLineEnd { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
                        });
                        active_tab.cursor.col = line_len;
                        active_tab.cursor.intended_col = line_len;
                    }
                    _ => {}
                }
                window.request_redraw();
                return true;
            }

            if is_document_action {
                if matches!(action, crate::editor::actions::Action::SaveFile) {
                    for tab in &mut state.tabs {
                        if let Some(ref p) = tab.path {
                            if !p.starts_with("diagnostics://") && tab.buffer.is_modified {
                                let _ = tab.buffer.save_file(p);
                                tab.buffer.mark_saved();
                                ui.external_change_warnings.remove(p);
                            }
                        }
                    }
                    window.request_redraw();
                    return true;
                }

                let is_modifying_action = match &action {
                    crate::editor::actions::Action::InsertChar(_) |
                    crate::editor::actions::Action::InsertNewLine |
                    crate::editor::actions::Action::InsertTab |
                    crate::editor::actions::Action::DeleteLeft |
                    crate::editor::actions::Action::DeleteRight |
                    crate::editor::actions::Action::Undo |
                    crate::editor::actions::Action::Redo |
                    crate::editor::actions::Action::Paste |
                    crate::editor::actions::Action::Cut => true,
                    _ => false,
                };

                if is_modifying_action {
                    let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);
                    let current_visual_line = {
                        let active_tab = &state.tabs[state.active_tab_idx];
                        visual_lines.get(active_tab.cursor.line).cloned()
                    };

                    let path_opt = current_visual_line.as_ref().map(|vl| match vl {
                        crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.clone(),
                        crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { path, .. } => path.clone(),
                        crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { path, .. } => path.clone(),
                    });

                    if let Some(path) = path_opt {
                        let target_tab_idx = if let Some(idx) = state.tabs.iter().position(|t| t.path.as_deref() == Some(&path)) {
                            idx
                        } else {
                            let mut new_buf = Buffer::new();
                            if let Err(e) = new_buf.load_file(&path) {
                                log::warn!("Failed to load file '{}' in diagnostics view: {}", path, e);
                            }
                            state.tabs.push(crate::app::Tab {
                                path: Some(path.clone()),
                                buffer: new_buf,
                                cursor: Cursor::new(),
                                secondary_cursors: Vec::new(),
                                scroll_x: 0,
                                scroll_y: 0,
                            });
                            state.tabs.len() - 1
                        };

                        let (target_line, target_col) = match current_visual_line.as_ref().unwrap() {
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_idx, line_content, .. } => {
                                let line_idx = *line_idx;
                                let target_line = line_idx.min(state.tabs[target_tab_idx].buffer.len().saturating_sub(1));
                                let line_len = state.tabs[target_tab_idx].buffer.lines().get(target_line).map_or(0, |l| l.chars().count());
                                let target_col = {
                                    let active_tab = &state.tabs[state.active_tab_idx];
                                    active_tab.cursor.col.min(line_content.chars().count()).min(line_len)
                                };
                                (target_line, target_col)
                            }
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { line, col, .. } => {
                                (*line, *col)
                            }
                            crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => {
                                (diag.line, diag.col)
                            }
                        };

                        state.tabs[target_tab_idx].cursor.line = target_line;
                        state.tabs[target_tab_idx].cursor.col = target_col;
                        state.tabs[target_tab_idx].cursor.intended_col = target_col;
                        state.tabs[target_tab_idx].cursor.selection_anchor = None;

                        let original_active_tab_idx = state.active_tab_idx;
                        state.active_tab_idx = target_tab_idx;

                        handle_keyboard_input(ui, state, window, elwt, gpu, atlas, font_bytes, logical_key.clone(), physical_key);

                        let new_line = state.tabs[target_tab_idx].cursor.line;
                        let new_col = state.tabs[target_tab_idx].cursor.col;
                        let target_lines = state.tabs[target_tab_idx].buffer.lines().to_vec();
                        
                        let abs_path = crate::editor::get_absolute_path(&path);
                        ui.diagnostics_file_cache.insert(abs_path, target_lines);

                        state.active_tab_idx = original_active_tab_idx;

                        let visual_lines_new = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui);

                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        if let crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_idx: orig_line_idx, .. } = current_visual_line.as_ref().unwrap() {
                            if new_line == *orig_line_idx {
                                active_tab.cursor.col = new_col;
                                active_tab.cursor.intended_col = new_col;
                            } else if let Some(new_v_idx) = visual_lines_new.iter().position(|vl| {
                                match vl {
                                    crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { path: p, line_idx: li, .. } => {
                                        p == &path && *li == new_line
                                    }
                                    _ => false
                                }
                            }) {
                                active_tab.cursor.line = new_v_idx;
                                active_tab.cursor.col = new_col;
                                active_tab.cursor.intended_col = new_col;
                            }
                        }
                    }
                    window.request_redraw();
                    return true;
                }
            }
        }
    }
    false
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
    if let Some(action) = crate::editor::keymap::map_key(&ui.keymap, logical_key, physical_key, ctrl, shift, alt, &["Editor", "Workspace"]) {
        match action {
            crate::editor::actions::Action::Find => {
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
            crate::editor::actions::Action::Split => {
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
            crate::editor::actions::Action::ZoomIn => {
                let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
                ui.update_buffer_font_size(&atlas.font, new_size);
            }
            crate::editor::actions::Action::ZoomOut => {
                let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
                ui.update_buffer_font_size(&atlas.font, new_size);
            }
            crate::editor::actions::Action::CommandPalette => {
                ui.active_modal = Some(crate::machkit::ModalType::CommandPalette);
                ui.command_palette_query.clear();
                ui.command_palette_selected = 0;
            }
            crate::editor::actions::Action::GlobalSearch => {
                handle_action(ui, state, UiAction::OpenFile(std::path::PathBuf::from("search://project")), window, elwt, gpu, atlas, font_bytes);
            }
            crate::editor::actions::Action::SaveFile => {
                handle_action(ui, state, UiAction::SaveFile, window, elwt, gpu, atlas, font_bytes);
            }
            crate::editor::actions::Action::Escape => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.clear_selection();
                active_tab.secondary_cursors.clear();
            }
            crate::editor::actions::Action::MoveLineUp => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let cursor_line = active_tab.cursor.line;
                if cursor_line > 0 {
                    active_tab.buffer.commit_transaction();
                    active_tab.buffer.start_transaction();
                    
                    let line_text = active_tab.buffer.lines()[cursor_line].clone();
                    if cursor_line == active_tab.buffer.len() - 1 {
                        let prev_len = active_tab.buffer.lines()[cursor_line - 1].chars().count();
                        active_tab.buffer.delete(cursor_line - 1, prev_len, cursor_line, line_text.chars().count());
                        active_tab.buffer.insert(cursor_line - 1, 0, &format!("{}\n", line_text));
                    } else {
                        active_tab.buffer.delete(cursor_line, 0, cursor_line + 1, 0);
                        active_tab.buffer.insert(cursor_line - 1, 0, &format!("{}\n", line_text));
                    }
                    active_tab.buffer.commit_transaction();
                    
                    active_tab.cursor.line -= 1;
                    active_tab.cursor.col = active_tab.cursor.col.min(active_tab.buffer.lines()[active_tab.cursor.line].chars().count());
                    active_tab.cursor.intended_col = active_tab.cursor.col;
                    active_tab.cursor.clear_selection();
                }
            }
            crate::editor::actions::Action::MoveLineDown => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let cursor_line = active_tab.cursor.line;
                if cursor_line < active_tab.buffer.len() - 1 {
                    active_tab.buffer.commit_transaction();
                    active_tab.buffer.start_transaction();
                    
                    let target_line = cursor_line + 1;
                    let line_text = active_tab.buffer.lines()[target_line].clone();
                    if target_line == active_tab.buffer.len() - 1 {
                        let prev_len = active_tab.buffer.lines()[target_line - 1].chars().count();
                        active_tab.buffer.delete(target_line - 1, prev_len, target_line, line_text.chars().count());
                        active_tab.buffer.insert(target_line - 1, 0, &format!("{}\n", line_text));
                    } else {
                        active_tab.buffer.delete(target_line, 0, target_line + 1, 0);
                        active_tab.buffer.insert(target_line - 1, 0, &format!("{}\n", line_text));
                    }
                    active_tab.buffer.commit_transaction();
                    
                    active_tab.cursor.line += 1;
                    active_tab.cursor.col = active_tab.cursor.col.min(active_tab.buffer.lines()[active_tab.cursor.line].chars().count());
                    active_tab.cursor.intended_col = active_tab.cursor.col;
                    active_tab.cursor.clear_selection();
                }
            }
            crate::editor::actions::Action::DuplicateLine => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let cursor_line = active_tab.cursor.line;
                active_tab.buffer.commit_transaction();
                active_tab.buffer.start_transaction();
                
                let line_text = active_tab.buffer.lines()[cursor_line].clone();
                if cursor_line == active_tab.buffer.len() - 1 {
                    active_tab.buffer.insert(cursor_line, line_text.chars().count(), &format!("\n{}", line_text));
                } else {
                    active_tab.buffer.insert(cursor_line + 1, 0, &format!("{}\n", line_text));
                }
                active_tab.buffer.commit_transaction();
                
                active_tab.cursor.line += 1;
                active_tab.cursor.col = active_tab.cursor.col.min(active_tab.buffer.lines()[active_tab.cursor.line].chars().count());
                active_tab.cursor.intended_col = active_tab.cursor.col;
                active_tab.cursor.clear_selection();
            }
            crate::editor::actions::Action::DeleteLine => {
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
                    active_tab.buffer.delete(prev_line, prev_col, cursor_line, active_tab.buffer.lines()[cursor_line].chars().count());
                }
                active_tab.buffer.commit_transaction();
                
                active_tab.cursor.line = cursor_line.min(active_tab.buffer.len() - 1);
                active_tab.cursor.col = active_tab.cursor.col.min(active_tab.buffer.lines()[active_tab.cursor.line].chars().count());
                active_tab.cursor.intended_col = active_tab.cursor.col;
                active_tab.cursor.clear_selection();
            }
            crate::editor::actions::Action::MoveLeft { select, word } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                for cursor in &mut cursors {
                    if word {
                        cursor.move_word_left(&active_tab.buffer, select);
                    } else {
                        cursor.move_left(&active_tab.buffer, select);
                    }
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
            }
            crate::editor::actions::Action::MoveRight { select, word } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                for cursor in &mut cursors {
                    if word {
                        cursor.move_word_right(&active_tab.buffer, select);
                    } else {
                        cursor.move_right(&active_tab.buffer, select);
                    }
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
            }
            crate::editor::actions::Action::MoveUp { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                for cursor in &mut cursors {
                    cursor.move_up(&active_tab.buffer, select);
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
            }
            crate::editor::actions::Action::MoveDown { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                for cursor in &mut cursors {
                    cursor.move_down(&active_tab.buffer, select);
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
            }
            crate::editor::actions::Action::MoveToLineStart { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                for cursor in &mut cursors {
                    cursor.move_to_line_start(select);
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
            }
            crate::editor::actions::Action::MoveToLineEnd { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                for cursor in &mut cursors {
                    cursor.move_to_line_end(&active_tab.buffer, select);
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
            }
            crate::editor::actions::Action::SelectAll => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.selection_anchor = Some((0, 0));
                active_tab.cursor.line = active_tab.buffer.len() - 1;
                active_tab.cursor.col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                active_tab.cursor.intended_col = active_tab.cursor.col;
                active_tab.secondary_cursors.clear();
            }
            crate::editor::actions::Action::Copy => {
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
                    let text = selections.join("\n");
                    state.copy_to_clipboard(text);
                }
            }
            crate::editor::actions::Action::Cut => {
                let selections = {
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
                    selections
                };
                
                if !selections.is_empty() {
                    let text = selections.join("\n");
                    state.copy_to_clipboard(text);
                    
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    active_tab.buffer.commit_transaction();
                    active_tab.buffer.start_transaction();
                    
                    let mut cursors = vec![active_tab.cursor];
                    cursors.extend(active_tab.secondary_cursors.drain(..));
                    
                    let mut sorted_cursors = cursors.clone();
                    sorted_cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));
                    for cursor in &mut sorted_cursors {
                        if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                            cursor.line = s_l;
                            cursor.col = s_c;
                            cursor.clear_selection();
                        }
                    }
                    
                    sorted_cursors.sort_by_key(|c| (c.line, c.col));
                    sorted_cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                    active_tab.cursor = sorted_cursors[0];
                    active_tab.secondary_cursors = sorted_cursors[1..].to_vec();
                    
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::Paste => {
                let clipboard_text = state.paste_from_clipboard();
                if !clipboard_text.is_empty() {
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    active_tab.buffer.commit_transaction();
                    active_tab.buffer.start_transaction();
                    
                    let mut cursors = vec![active_tab.cursor];
                    cursors.extend(active_tab.secondary_cursors.drain(..));
                    cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));
                    
                    for cursor in &mut cursors {
                        if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                            cursor.line = s_l;
                            cursor.col = s_c;
                            cursor.clear_selection();
                        }
                        active_tab.buffer.insert(cursor.line, cursor.col, &clipboard_text);
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
            crate::editor::actions::Action::Undo => {
                handle_action(ui, state, UiAction::Undo, window, elwt, gpu, atlas, font_bytes);
                state.tabs[state.active_tab_idx].cursor.intended_col = state.tabs[state.active_tab_idx].cursor.col;
            }
            crate::editor::actions::Action::Redo => {
                handle_action(ui, state, UiAction::Redo, window, elwt, gpu, atlas, font_bytes);
                state.tabs[state.active_tab_idx].cursor.intended_col = state.tabs[state.active_tab_idx].cursor.col;
            }
            crate::editor::actions::Action::DeleteLeft => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.buffer.start_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));
                
                for cursor in &mut cursors {
                    if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                        active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                        cursor.line = s_l;
                        cursor.col = s_c;
                        cursor.intended_col = s_c;
                        cursor.clear_selection();
                    } else if cursor.col > 0 || cursor.line > 0 {
                        let is_paired = if cursor.col > 0 {
                            let line_chars: Vec<char> = active_tab.buffer.lines()[cursor.line].chars().collect();
                            if cursor.col < line_chars.len() {
                                let left_char = line_chars[cursor.col - 1];
                                let right_char = line_chars[cursor.col];
                                match (left_char, right_char) {
                                    ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"') | ('\'', '\'') => true,
                                    _ => false,
                                }
                            } else { false }
                        } else { false };
                        
                        if is_paired {
                            active_tab.buffer.delete(cursor.line, cursor.col - 1, cursor.line, cursor.col + 1);
                            cursor.col -= 1;
                            cursor.intended_col = cursor.col;
                        } else {
                            let mut prev_cursor = *cursor;
                            prev_cursor.move_left(&active_tab.buffer, false);
                            active_tab.buffer.delete(prev_cursor.line, prev_cursor.col, cursor.line, cursor.col);
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
            crate::editor::actions::Action::DeleteRight => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.buffer.start_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
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
                            active_tab.buffer.delete(cursor.line, cursor.col, next_cursor.line, next_cursor.col);
                        }
                    }
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
                
                active_tab.buffer.commit_transaction();
            }
            crate::editor::actions::Action::InsertNewLine => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.buffer.start_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
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
            crate::editor::actions::Action::InsertTab => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.buffer.start_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
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
            crate::editor::actions::Action::InsertChar(s) => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let had_selection = active_tab.cursor.selection_range().is_some() || active_tab.secondary_cursors.iter().any(|c| c.selection_range().is_some());
                
                active_tab.buffer.commit_transaction();
                active_tab.buffer.start_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(active_tab.secondary_cursors.drain(..));
                cursors.sort_by(|a, b| (b.line, b.col).cmp(&(a.line, a.col)));
                
                for cursor in &mut cursors {
                    let step_over = if s.chars().count() == 1 && cursor.selection_range().is_none() {
                        let c = s.chars().next().unwrap();
                        if c == ')' || c == ']' || c == '}' || c == '"' || c == '\'' {
                            let line_chars: Vec<char> = active_tab.buffer.lines()[cursor.line].chars().collect();
                            if cursor.col < line_chars.len() && line_chars[cursor.col] == c {
                                true
                            } else { false }
                        } else { false }
                    } else { false };
                    
                    if step_over {
                        cursor.col += 1;
                        cursor.intended_col = cursor.col;
                    } else {
                        let wrapped = if s.chars().count() == 1 {
                            let c = s.chars().next().unwrap();
                            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                let matching_close = match c {
                                    '(' => Some(')'),
                                    '[' => Some(']'),
                                    '{' => Some('}'),
                                    '"' => Some('"'),
                                    '\'' => Some('\''),
                                    _ => None,
                                };
                                
                                if let Some(close_char) = matching_close {
                                    active_tab.buffer.insert(s_l, s_c, &c.to_string());
                                    let adjusted_e_c = if s_l == e_l { e_c + 1 } else { e_c };
                                    active_tab.buffer.insert(e_l, adjusted_e_c, &close_char.to_string());
                                    
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
                                } else { false }
                            } else { false }
                        } else { false };
                        
                        if !wrapped {
                            let paired = if s.chars().count() == 1 && cursor.selection_range().is_none() {
                                let c = s.chars().next().unwrap();
                                let matching_close = match c {
                                    '(' => Some(')'),
                                    '[' => Some(']'),
                                    '{' => Some('}'),
                                    '"' => Some('"'),
                                    '\'' => Some('\''),
                                    _ => None,
                                };
                                
                                if let Some(close_char) = matching_close {
                                    let should_pair = match c {
                                        '(' | '[' | '{' => true,
                                        '"' | '\'' => {
                                            let line_chars: Vec<char> = active_tab.buffer.lines()[cursor.line].chars().collect();
                                            let preceded_by_alpha = if cursor.col > 0 {
                                                line_chars[cursor.col - 1].is_alphanumeric()
                                            } else {
                                                false
                                            };
                                            let followed_by_alpha = if cursor.col < line_chars.len() {
                                                line_chars[cursor.col].is_alphanumeric()
                                            } else {
                                                false
                                            };
                                            !preceded_by_alpha && !followed_by_alpha
                                        }
                                        _ => false,
                                    };
                                    
                                    if should_pair {
                                        let pair_str = format!("{}{}", c, close_char);
                                        active_tab.buffer.insert(cursor.line, cursor.col, &pair_str);
                                        cursor.col += 1;
                                        cursor.intended_col = cursor.col;
                                        true
                                    } else { false }
                                } else { false }
                            } else { false };
                            
                            if !paired {
                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                                    cursor.line = s_l;
                                    cursor.col = s_c;
                                    cursor.clear_selection();
                                }
                                active_tab.buffer.insert(cursor.line, cursor.col, &s);
                                cursor.col += s.chars().count();
                                cursor.intended_col = cursor.col;
                            }
                        }
                    }
                }
                
                cursors.sort_by_key(|c| (c.line, c.col));
                cursors.dedup_by(|a, b| a.line == b.line && a.col == b.col);
                active_tab.cursor = cursors[0];
                active_tab.secondary_cursors = cursors[1..].to_vec();
                
                let is_boundary = s.chars().any(|c| c.is_whitespace() || c.is_ascii_punctuation()) || had_selection;
                if is_boundary {
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::AddCursorUp => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(&active_tab.secondary_cursors);
                
                if let Some(min_cursor) = cursors.iter().min_by_key(|c| c.line) {
                    if min_cursor.line > 0 {
                        let target_line = min_cursor.line - 1;
                        let line_len = active_tab.buffer.lines()[target_line].chars().count();
                        let target_col = min_cursor.intended_col.min(line_len);
                        
                        let new_cursor = Cursor {
                            line: target_line,
                            col: target_col,
                            intended_col: min_cursor.intended_col,
                            selection_anchor: None,
                        };
                        
                        if !cursors.iter().any(|c| c.line == target_line && c.col == target_col) {
                            active_tab.secondary_cursors.push(new_cursor);
                        }
                    }
                }
            }
            crate::editor::actions::Action::AddCursorDown => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                
                let mut cursors = vec![active_tab.cursor];
                cursors.extend(&active_tab.secondary_cursors);
                
                if let Some(max_cursor) = cursors.iter().max_by_key(|c| c.line) {
                    if max_cursor.line < active_tab.buffer.len() - 1 {
                        let target_line = max_cursor.line + 1;
                        let line_len = active_tab.buffer.lines()[target_line].chars().count();
                        let target_col = max_cursor.intended_col.min(line_len);
                        
                        let new_cursor = Cursor {
                            line: target_line,
                            col: target_col,
                            intended_col: max_cursor.intended_col,
                            selection_anchor: None,
                        };
                        
                        if !cursors.iter().any(|c| c.line == target_line && c.col == target_col) {
                            active_tab.secondary_cursors.push(new_cursor);
                        }
                    }
                }
            }
        }
    }
    
    if let (Some(start_path), Some(old_rev)) = (active_path_start, old_revision) {
        if state.active_tab_idx < state.tabs.len() {
            let active_tab = &state.tabs[state.active_tab_idx];
            if active_tab.path.as_ref() == Some(&start_path) {
                if old_rev != active_tab.buffer.revision {
                    let abs_path = crate::editor::get_absolute_path(&start_path);
                    ui.diagnostics_file_cache.insert(abs_path.clone(), active_tab.buffer.lines().to_vec());
                    ui.synced_revisions.insert(abs_path, active_tab.buffer.revision);
                    ui.diagnostics_changed = true;
                }
            }
        }
    }
    
    let active_tab = &state.tabs[state.active_tab_idx];
    ui.scroll_to_cursor(&active_tab.cursor, active_tab.buffer.len(), window.inner_size().width as f32, window.inner_size().height as f32);
    update_cursor_icon(window, ui, state);
    window.request_redraw();
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
    if let Some(lines) = ui.project_search_file_cache.get_mut(&path) {
        if line_idx < lines.len() {
            lines[line_idx] = new_line_content.clone();
        }
    }

    // 2. Update all open tabs/buffers matching this path in active and inactive panes
    let abs_target_path = crate::editor::get_absolute_path(&path.to_string_lossy());
    
    // Check active tabs
    for tab in &mut state.tabs {
        if let Some(ref tab_path) = tab.path {
            if crate::editor::get_absolute_path(tab_path) == abs_target_path {
                if line_idx < tab.buffer.len() {
                    let old_line_len = tab.buffer.lines()[line_idx].chars().count();
                    tab.buffer.commit_transaction();
                    tab.buffer.start_transaction();
                    tab.buffer.delete(line_idx, 0, line_idx, old_line_len);
                    tab.buffer.insert(line_idx, 0, &new_line_content);
                    tab.buffer.commit_transaction();
                }
            }
        }
    }
    // Check inactive pane tabs
    for pane in &mut state.inactive_panes {
        for tab in &mut pane.tabs {
            if let Some(ref tab_path) = tab.path {
                if crate::editor::get_absolute_path(tab_path) == abs_target_path {
                    if line_idx < tab.buffer.len() {
                        let old_line_len = tab.buffer.lines()[line_idx].chars().count();
                        tab.buffer.commit_transaction();
                        tab.buffer.start_transaction();
                        tab.buffer.delete(line_idx, 0, line_idx, old_line_len);
                        tab.buffer.insert(line_idx, 0, &new_line_content);
                        tab.buffer.commit_transaction();
                    }
                }
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
        } else {
            if let Ok(content) = std::fs::read_to_string(&path_clone) {
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                if line_idx < lines.len() {
                    lines[line_idx] = new_line_content;
                    let joined = lines.join("\n");
                    let _ = std::fs::write(&path_clone, joined);
                }
            }
        }
    });
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

    if let Some(action) = crate::editor::keymap::map_key(&ui.keymap, logical_key, physical_key, ctrl, shift, alt, &["Editor", "Workspace"]) {
        let is_global_action = match &action {
            crate::editor::actions::Action::ZoomIn |
            crate::editor::actions::Action::ZoomOut |
            crate::editor::actions::Action::CommandPalette |
            crate::editor::actions::Action::GlobalSearch |
            crate::editor::actions::Action::Escape => true,
            _ => false,
        };
        if is_global_action {
            return false;
        }
    }

    if ui.global_search_focused {
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
                    ui.global_search_selected = (ui.global_search_selected + items_count - 1) % items_count;
                }
                window.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                if alt {
                    let results_len = ui.global_search_results.len();
                    if ui.global_search_selected < results_len {
                        let (path, line_idx, _) = &ui.global_search_results[ui.global_search_selected];
                        handle_action(ui, state, UiAction::OpenFileAt(path.clone(), *line_idx), window, elwt, gpu, atlas, font_bytes);
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
                if !ctrl && !alt {
                    if ui.global_search_focus_replace {
                        ui.global_replace_query.pop();
                    } else {
                        ui.global_search_query.pop();
                        ui.global_search_selected = 0;
                    }
                    window.request_redraw();
                }
            }
            Key::Character(text) => {
                if !ctrl && !alt {
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
            }
            _ => {}
        }
    } else {
        let results_len = ui.global_search_results.len();
        if ui.global_search_selected < results_len {
            let (path, line_idx, current_content) = ui.global_search_results[ui.global_search_selected].clone();
            
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
                    let char_count = current_content.chars().count();
                    ui.global_search_col = char_count;
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
                        ui.global_search_selected = (ui.global_search_selected + items_count - 1) % items_count;
                        let new_content = &ui.global_search_results[ui.global_search_selected].2;
                        ui.global_search_col = ui.global_search_col.min(new_content.chars().count());
                    }
                    window.request_redraw();
                }
                Key::Named(NamedKey::Enter) => {
                    handle_action(ui, state, UiAction::OpenFileAt(path.clone(), line_idx), window, elwt, gpu, atlas, font_bytes);
                    window.request_redraw();
                }
                Key::Named(NamedKey::Backspace) => {
                    if !ctrl && !alt {
                        if ui.global_search_col > 0 {
                            let new_content = remove_char_at(&current_content, ui.global_search_col - 1);
                            ui.global_search_col -= 1;
                            sync_file_changes(ui, state, path, line_idx, new_content);
                            window.request_redraw();
                        }
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
                                temp_content = insert_char_at(&temp_content, ui.global_search_col + inserted_count, c);
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
    }
    true
}
