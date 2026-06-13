use std::sync::Arc;
use winit::window::Window;
use winit::keyboard::{Key, PhysicalKey, NamedKey};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::ui::UiState;
use crate::renderer::atlas::FontAtlas;
use crate::app::state::AppState;
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

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
        if let Some(action) = crate::editor::keymap::map_key(logical_key, physical_key, ctrl, shift, alt) {
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

                        super::handle_keyboard_input(ui, state, window, elwt, gpu, atlas, font_bytes, logical_key.clone(), physical_key);

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
