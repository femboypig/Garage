use std::sync::Arc;
use std::io::Write;
use winit::window::Window;
use winit::keyboard::{Key, PhysicalKey, NamedKey};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::ui::{UiState, UiAction};
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
                    ui.active_modal = Some(crate::ui::ModalType::CommandPalette);
                    ui.command_palette_query.clear();
                    ui.command_palette_selected = 0;
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
    if ui.show_search_panel {
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
                            };
                            state.inactive_panes.push(new_pane);
                        } else {
                            let initial_tab = crate::app::Tab {
                                path: None,
                                buffer: Buffer::new(),
                                cursor: Cursor::new(),
                                scroll_x: 0,
                                scroll_y: 0,
                            };
                            let new_pane = crate::app::state::Pane {
                                tabs: vec![initial_tab],
                                active_tab_idx: 0,
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
                    ui.active_modal = Some(crate::ui::ModalType::CommandPalette);
                    ui.command_palette_query.clear();
                    ui.command_palette_selected = 0;
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
                ui.search_focus_replace = !ui.search_focus_replace;
                window.request_redraw();
                return;
            }
            Key::Named(NamedKey::Enter) => {
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
                    ui.perform_search(state);
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
                    if !ui.search_focus_replace {
                        ui.perform_search(state);
                    }
                    window.request_redraw();
                    return;
                }
            }
            _ => {}
        }
    }

    // Handle typing inside the SidebarInput modal
    if ui.active_modal == Some(crate::ui::ModalType::SidebarInput) {
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
        let bytes_to_write: Option<Vec<u8>> = match logical_key {
            Key::Character(text) => {
                let ctrl = state.modifiers.control_key();
                if ctrl && text.len() == 1 {
                    let c = text.chars().next().unwrap();
                    if c.is_ascii_alphabetic() {
                        let code = c.to_ascii_uppercase() as u8 - b'A' + 1;
                        Some(vec![code])
                    } else {
                        Some(text.as_bytes().to_vec())
                    }
                } else {
                    Some(text.as_bytes().to_vec())
                }
            }
            Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
            Key::Named(NamedKey::Space) => Some(vec![b' ']),
            Key::Named(NamedKey::Backspace) => Some(vec![127]),
            Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
            Key::Named(NamedKey::Escape) => Some(vec![27]),
            Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
            Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
            Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
            Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
            Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
            Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
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
    if let Some(crate::ui::ModalType::CommandPalette) = ui.active_modal {
        match logical_key {
            Key::Named(NamedKey::Escape) => {
                ui.active_modal = None;
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
                         ui.execute_command(cmd, &mut active_tab.buffer, &mut active_tab.cursor)
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
                if text.chars().count() == 1 {
                    let c = text.chars().next().unwrap();
                    if c.is_ascii_graphic() || c == ' ' {
                        ui.command_palette_query.push(c);
                        ui.command_palette_selected = 0;
                        window.request_redraw();
                    }
                }
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
                crate::editor::actions::Action::Escape => true,
                _ => false,
            };

            let is_document_action = !is_navigation_action && !is_global_action;

            if is_navigation_action {
                match &action {
                    crate::editor::actions::Action::MoveUp { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        if active_tab.cursor.line > 0 {
                            active_tab.cursor.line -= 1;
                        }
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
                        });
                        active_tab.cursor.col = active_tab.cursor.col.min(line_len);
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    }
                    crate::editor::actions::Action::MoveDown { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        if !visual_lines.is_empty() && active_tab.cursor.line < visual_lines.len() - 1 {
                            active_tab.cursor.line += 1;
                        }
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
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
                            let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                            let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                                crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                                _ => 0,
                            });
                            active_tab.cursor.col = line_len;
                        }
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    }
                    crate::editor::actions::Action::MoveRight { .. } => {
                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        active_tab.cursor.clear_selection();
                        let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
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
                        let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                        let line_len = visual_lines.get(active_tab.cursor.line).map_or(0, |vl| match vl {
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => line_content.chars().count(),
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.chars().count() + 10,
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => diag.message.chars().count() + 10,
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
                    let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);
                    let current_visual_line = {
                        let active_tab = &state.tabs[state.active_tab_idx];
                        visual_lines.get(active_tab.cursor.line).cloned()
                    };

                    let path_opt = current_visual_line.as_ref().map(|vl| match vl {
                        crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, .. } => path.clone(),
                        crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { path, .. } => path.clone(),
                        crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { path, .. } => path.clone(),
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
                                scroll_x: 0,
                                scroll_y: 0,
                            });
                            state.tabs.len() - 1
                        };

                        let (target_line, target_col) = match current_visual_line.as_ref().unwrap() {
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_idx, line_content, .. } => {
                                let line_idx = *line_idx;
                                let target_line = line_idx.min(state.tabs[target_tab_idx].buffer.len().saturating_sub(1));
                                let line_len = state.tabs[target_tab_idx].buffer.lines().get(target_line).map_or(0, |l| l.chars().count());
                                let target_col = {
                                    let active_tab = &state.tabs[state.active_tab_idx];
                                    active_tab.cursor.col.min(line_content.chars().count()).min(line_len)
                                };
                                (target_line, target_col)
                            }
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { line, col, .. } => {
                                (*line, *col)
                            }
                            crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => {
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

                        let visual_lines_new = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui);

                        let active_tab = &mut state.tabs[state.active_tab_idx];
                        if let crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_idx: orig_line_idx, .. } = current_visual_line.as_ref().unwrap() {
                            if new_line == *orig_line_idx {
                                active_tab.cursor.col = new_col;
                                active_tab.cursor.intended_col = new_col;
                            } else if let Some(new_v_idx) = visual_lines_new.iter().position(|vl| {
                                match vl {
                                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { path: p, line_idx: li, .. } => {
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
                        };
                        state.inactive_panes.push(new_pane);
                    } else {
                        let initial_tab = crate::app::Tab {
                            path: None,
                            buffer: Buffer::new(),
                            cursor: Cursor::new(),
                            scroll_x: 0,
                            scroll_y: 0,
                        };
                        let new_pane = crate::app::state::Pane {
                            tabs: vec![initial_tab],
                            active_tab_idx: 0,
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
                ui.active_modal = Some(crate::ui::ModalType::CommandPalette);
                ui.command_palette_query.clear();
                ui.command_palette_selected = 0;
            }
            crate::editor::actions::Action::SaveFile => {
                handle_action(ui, state, UiAction::SaveFile, window, elwt, gpu, atlas, font_bytes);
            }
            crate::editor::actions::Action::Escape => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.clear_selection();
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
                if word {
                    active_tab.cursor.move_word_left(&active_tab.buffer, select);
                } else {
                    active_tab.cursor.move_left(&active_tab.buffer, select);
                }
            }
            crate::editor::actions::Action::MoveRight { select, word } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                if word {
                    active_tab.cursor.move_word_right(&active_tab.buffer, select);
                } else {
                    active_tab.cursor.move_right(&active_tab.buffer, select);
                }
            }
            crate::editor::actions::Action::MoveUp { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_up(&active_tab.buffer, select);
            }
            crate::editor::actions::Action::MoveDown { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_down(&active_tab.buffer, select);
            }
            crate::editor::actions::Action::MoveToLineStart { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_to_line_start(select);
            }
            crate::editor::actions::Action::MoveToLineEnd { select } => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.move_to_line_end(&active_tab.buffer, select);
            }
            crate::editor::actions::Action::SelectAll => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                active_tab.cursor.selection_anchor = Some((0, 0));
                active_tab.cursor.line = active_tab.buffer.len() - 1;
                active_tab.cursor.col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                active_tab.cursor.intended_col = active_tab.cursor.col;
            }
            crate::editor::actions::Action::Copy => {
                let text = {
                    let active_tab = &state.tabs[state.active_tab_idx];
                    active_tab.cursor.selection_range().map(|(s_l, s_c, e_l, e_c)| {
                        active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c)
                    })
                };
                if let Some(text) = text {
                    state.copy_to_clipboard(text);
                }
            }
            crate::editor::actions::Action::Cut => {
                let selection = {
                    let active_tab = &state.tabs[state.active_tab_idx];
                    active_tab.cursor.selection_range().map(|(s_l, s_c, e_l, e_c)| {
                        let text = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
                        (s_l, s_c, e_l, e_c, text)
                    })
                };
                if let Some((s_l, s_c, e_l, e_c, text)) = selection {
                    state.copy_to_clipboard(text);
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    active_tab.buffer.commit_transaction();
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.intended_col = s_c;
                    active_tab.cursor.clear_selection();
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::Paste => {
                let clipboard_text = state.paste_from_clipboard();
                if !clipboard_text.is_empty() {
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    active_tab.buffer.commit_transaction();
                    active_tab.buffer.start_transaction();
                    if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                        active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                        active_tab.cursor.line = s_l;
                        active_tab.cursor.col = s_c;
                        active_tab.cursor.clear_selection();
                    }
                    active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &clipboard_text);
  
                    let parts = clipboard_text.split('\n').collect::<Vec<&str>>();
                    if parts.len() == 1 {
                        active_tab.cursor.col += clipboard_text.chars().count();
                    } else {
                        active_tab.cursor.line += parts.len() - 1;
                        active_tab.cursor.col = parts.last().unwrap().chars().count();
                    }
                    active_tab.cursor.intended_col = active_tab.cursor.col;
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
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.intended_col = s_c;
                    active_tab.cursor.clear_selection();
                    active_tab.buffer.commit_transaction();
                } else if active_tab.cursor.col > 0 || active_tab.cursor.line > 0 {
                    active_tab.buffer.start_transaction();
                    let is_paired = if active_tab.cursor.col > 0 {
                        let line_chars: Vec<char> = active_tab.buffer.lines()[active_tab.cursor.line].chars().collect();
                        if active_tab.cursor.col < line_chars.len() {
                            let left_char = line_chars[active_tab.cursor.col - 1];
                            let right_char = line_chars[active_tab.cursor.col];
                            match (left_char, right_char) {
                                ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"') | ('\'', '\'') => true,
                                _ => false,
                            }
                        } else { false }
                    } else { false };
  
                    if is_paired {
                        active_tab.buffer.delete(active_tab.cursor.line, active_tab.cursor.col - 1, active_tab.cursor.line, active_tab.cursor.col + 1);
                        active_tab.cursor.col -= 1;
                        active_tab.cursor.intended_col = active_tab.cursor.col;
                    } else {
                        let mut prev_cursor = active_tab.cursor;
                        prev_cursor.move_left(&active_tab.buffer, false);
                        active_tab.buffer.delete(prev_cursor.line, prev_cursor.col, active_tab.cursor.line, active_tab.cursor.col);
                        active_tab.cursor = prev_cursor;
                    }
                    active_tab.buffer.commit_transaction();
                }
            }
            crate::editor::actions::Action::DeleteRight => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.intended_col = s_c;
                    active_tab.cursor.clear_selection();
                    active_tab.buffer.commit_transaction();
                } else {
                    let line_len = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                    if active_tab.cursor.col < line_len || active_tab.cursor.line < active_tab.buffer.len() - 1 {
                        active_tab.buffer.start_transaction();
                        let mut next_cursor = active_tab.cursor;
                        next_cursor.move_right(&active_tab.buffer, false);
                        active_tab.buffer.delete(active_tab.cursor.line, active_tab.cursor.col, next_cursor.line, next_cursor.col);
                        active_tab.buffer.commit_transaction();
                    }
                }
            }
            crate::editor::actions::Action::InsertNewLine => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.clear_selection();
                }
                active_tab.buffer.start_transaction();
                active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, "\n");
                active_tab.cursor.line += 1;
                active_tab.cursor.col = 0;
                active_tab.cursor.intended_col = 0;
                active_tab.buffer.commit_transaction();
            }
            crate::editor::actions::Action::InsertTab => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                active_tab.buffer.commit_transaction();
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    active_tab.buffer.start_transaction();
                    active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                    active_tab.cursor.line = s_l;
                    active_tab.cursor.col = s_c;
                    active_tab.cursor.clear_selection();
                }
                active_tab.buffer.start_transaction();
                active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, "    ");
                active_tab.cursor.col += 4;
                active_tab.cursor.intended_col = active_tab.cursor.col;
                active_tab.buffer.commit_transaction();
            }
            crate::editor::actions::Action::InsertChar(c) => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let had_selection = active_tab.cursor.selection_range().is_some();
                let step_over = if active_tab.cursor.selection_range().is_none() && (c == ')' || c == ']' || c == '}' || c == '"' || c == '\'') {
                    let line_chars: Vec<char> = active_tab.buffer.lines()[active_tab.cursor.line].chars().collect();
                    if active_tab.cursor.col < line_chars.len() && line_chars[active_tab.cursor.col] == c {
                        true
                    } else { false }
                } else { false };
  
                if step_over {
                    active_tab.cursor.col += 1;
                    active_tab.cursor.intended_col = active_tab.cursor.col;
                } else {
                    let wrapped = if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                        let matching_close = match c {
                            '(' => Some(')'),
                            '[' => Some(']'),
                            '{' => Some('}'),
                            '"' => Some('"'),
                            '\'' => Some('\''),
                            _ => None,
                        };
  
                        if let Some(close_char) = matching_close {
                            active_tab.buffer.commit_transaction();
                            active_tab.buffer.start_transaction();
                            active_tab.buffer.insert(s_l, s_c, &c.to_string());
                            let adjusted_e_c = if s_l == e_l { e_c + 1 } else { e_c };
                            active_tab.buffer.insert(e_l, adjusted_e_c, &close_char.to_string());
                            
                            if active_tab.cursor.selection_anchor.unwrap().0 == s_l && active_tab.cursor.selection_anchor.unwrap().1 == s_c {
                                active_tab.cursor.selection_anchor = Some((s_l, s_c + 1));
                                active_tab.cursor.line = e_l;
                                active_tab.cursor.col = adjusted_e_c;
                            } else {
                                active_tab.cursor.selection_anchor = Some((e_l, adjusted_e_c));
                                active_tab.cursor.line = s_l;
                                active_tab.cursor.col = s_c + 1;
                            }
                            active_tab.cursor.intended_col = active_tab.cursor.col;
                            active_tab.buffer.commit_transaction();
                            true
                        } else { false }
                    } else { false };
  
                    if !wrapped {
                        let paired = if active_tab.cursor.selection_range().is_none() {
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
                                        let line_chars: Vec<char> = active_tab.buffer.lines()[active_tab.cursor.line].chars().collect();
                                        let preceded_by_alpha = if active_tab.cursor.col > 0 {
                                            line_chars[active_tab.cursor.col - 1].is_alphanumeric()
                                        } else {
                                            false
                                        };
                                        let followed_by_alpha = if active_tab.cursor.col < line_chars.len() {
                                            line_chars[active_tab.cursor.col].is_alphanumeric()
                                        } else {
                                            false
                                        };
                                        !preceded_by_alpha && !followed_by_alpha
                                    }
                                    _ => false,
                                };
  
                                if should_pair {
                                    active_tab.buffer.start_transaction();
                                    let pair_str = format!("{}{}", c, close_char);
                                    active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &pair_str);
                                    active_tab.cursor.col += 1;
                                    active_tab.cursor.intended_col = active_tab.cursor.col;
                                    active_tab.buffer.commit_transaction();
                                    true
                                } else { false }
                            } else { false }
                        } else { false };
  
                        if !paired {
                            if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                                active_tab.buffer.start_transaction();
                                active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                                active_tab.cursor.line = s_l;
                                active_tab.cursor.col = s_c;
                                active_tab.cursor.clear_selection();
                            }
                            active_tab.buffer.start_transaction();
                            active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &c.to_string());
                            active_tab.cursor.col += 1;
                            active_tab.cursor.intended_col = active_tab.cursor.col;
                            if c.is_whitespace() || c.is_ascii_punctuation() || had_selection {
                                active_tab.buffer.commit_transaction();
                            }
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
    update_cursor_icon(window, ui, &active_tab.buffer, state.mouse_x, state.mouse_y);
    window.request_redraw();
}
