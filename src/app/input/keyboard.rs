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
use super::mouse::update_cursor_icon;


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
    if state.terminal_focus && !state.dock_terminals.is_empty() {
        let active_term = &mut state.dock_terminals[state.active_terminal_idx];
        let bytes_to_write: Option<Vec<u8>> = match &logical_key {
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
        return;
    }
 
    let ctrl = state.modifiers.control_key();
    let shift = state.modifiers.shift_key();
    let alt = state.modifiers.alt_key();
 
    // 1. If CommandPalette modal is active, handle it specifically
    if let Some(crate::ui::ModalType::CommandPalette) = ui.active_modal {
        match &logical_key {
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
        return;
    }
 
    // 2. If any other modal is active, Escape closes it
    if ui.active_modal.is_some() {
        if let Key::Named(NamedKey::Escape) = &logical_key {
            ui.active_modal = None;
            window.request_redraw();
        }
        return;
    }
 
    // 3. Otherwise map key input to Action
    if let Some(action) = crate::editor::keymap::map_key(&logical_key, physical_key, ctrl, shift, alt) {
        match action {
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
                let active_tab = &state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    state.internal_clipboard = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
                }
            }
            crate::editor::actions::Action::Cut => {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                    state.internal_clipboard = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
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
                if !state.internal_clipboard.is_empty() {
                    let active_tab = &mut state.tabs[state.active_tab_idx];
                    active_tab.buffer.start_transaction();
                    if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                        active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                        active_tab.cursor.line = s_l;
                        active_tab.cursor.col = s_c;
                        active_tab.cursor.clear_selection();
                    }
                    active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &state.internal_clipboard);
 
                    let parts = state.internal_clipboard.split('\n').collect::<Vec<&str>>();
                    if parts.len() == 1 {
                        active_tab.cursor.col += state.internal_clipboard.chars().count();
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
                                active_tab.buffer.start_transaction();
                                let pair_str = format!("{}{}", c, close_char);
                                active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &pair_str);
                                active_tab.cursor.col += 1;
                                active_tab.cursor.intended_col = active_tab.cursor.col;
                                active_tab.buffer.commit_transaction();
                                true
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
                            active_tab.buffer.commit_transaction();
                        }
                    }
                }
            }
        }
    }
    
    let active_tab = &state.tabs[state.active_tab_idx];
    ui.scroll_to_cursor(&active_tab.cursor, active_tab.buffer.len(), window.inner_size().width as f32, window.inner_size().height as f32);
    update_cursor_icon(window, ui, &active_tab.buffer, state.mouse_x, state.mouse_y);
    window.request_redraw();
}
