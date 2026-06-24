use super::state::{AppState, Tab};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::{UiAction, UiState};
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::GpuContext;
use std::sync::Arc;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

pub fn handle_action(
    ui: &mut UiState,
    state: &mut AppState,
    action: UiAction,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
) {
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

    let line_to_jump = match &action {
        UiAction::OpenFileAt(_, l) => Some(*l),
        _ => None,
    };

    match action {
        UiAction::OpenFile(path) | UiAction::OpenFileAt(path, _) => {
            let current_dir = std::env::current_dir().unwrap_or_default();
            let path_str = path.to_string_lossy().to_string();
            let path_str =
                if path_str.starts_with("diagnostics://") || path_str.starts_with("search://") {
                    path_str
                } else {
                    let normalized_path = if path.is_absolute() {
                        if let Ok(rel) = path.strip_prefix(&current_dir) {
                            rel.to_path_buf()
                        } else {
                            path.clone()
                        }
                    } else {
                        path.clone()
                    };
                    let normalized_path = crate::editor::normalize_path(&normalized_path);
                    normalized_path.to_string_lossy().to_string()
                };
            let _is_new = if let Some(existing_idx) = state.tabs.iter().position(|t| {
                t.path.as_ref().is_some_and(|p| {
                    if p.starts_with("diagnostics://") || p.starts_with("search://") {
                        p == &path_str
                    } else {
                        let p_buf = std::path::PathBuf::from(p);
                        let p_norm = if p_buf.is_absolute() {
                            p_buf
                                .strip_prefix(&current_dir)
                                .map(|r| r.to_path_buf())
                                .unwrap_or(p_buf)
                        } else {
                            p_buf
                        };
                        crate::editor::normalize_path(&p_norm).to_string_lossy() == path_str
                    }
                })
            }) {
                state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
                state.tabs[state.active_tab_idx].scroll_y = ui.scroll_y;
                state.active_tab_idx = existing_idx;
                ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
                ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
                false
            } else {
                let mut new_buf = Buffer::new();
                if !path_str.starts_with("diagnostics://")
                    && !path_str.starts_with("search://")
                    && let Err(e) = new_buf.load_file(&path_str)
                {
                    log::warn!("Failed to load file '{}': {}", path_str, e);
                }
                state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
                state.tabs[state.active_tab_idx].scroll_y = ui.scroll_y;
                state.tabs.push(Tab {
                    path: Some(path_str),
                    buffer: new_buf,
                    cursor: Cursor::new(),
                    secondary_cursors: Vec::new(),
                    scroll_x: 0,
                    scroll_y: 0,
                });
                state.active_tab_idx = state.tabs.len() - 1;
                ui.scroll_x = 0;
                ui.scroll_y = 0;
                true
            };
            if let Some(ref active_path) = state.tabs[state.active_tab_idx].path {
                ui.selected_file = Some(std::path::PathBuf::from(active_path));
                if active_path == "search://project" {
                    ui.global_search_focused = true;
                }
                if !active_path.starts_with("diagnostics://")
                    && !active_path.starts_with("search://")
                {
                    let abs_path = crate::editor::get_absolute_path(active_path);
                    ui.diagnostics_file_cache.insert(
                        abs_path,
                        state.tabs[state.active_tab_idx].buffer.lines().to_vec(),
                    );
                    ui.update_git_diff(Some(active_path));
                    ui.update_git_file_blame(Some(active_path));
                    ui.update_git_statuses();
                }
            } else {
                ui.selected_file = None;
            }
            let tab_paths: Vec<Option<String>> =
                state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            let visible_width = if state.inactive_panes.is_empty() || state.is_split_horizontal {
                size.width as f32 - ui.sidebar_width
            } else {
                ((size.width as f32 - ui.sidebar_width) / 2.0).round()
            };
            state.tab_scroll_x = ui.scroll_to_tab(
                state.active_tab_idx,
                &tab_paths,
                visible_width,
                state.tab_scroll_x,
            );

            // Handle jumping to specific line
            if let Some(line) = line_to_jump {
                let active_tab = &mut state.tabs[state.active_tab_idx];
                let max_line = active_tab.buffer.len().saturating_sub(1);
                active_tab.cursor.line = line.min(max_line);
                active_tab.cursor.col = 0;
                active_tab.cursor.intended_col = 0;
                active_tab.cursor.selection_anchor = None;

                ui.scroll_to_cursor(
                    &active_tab.cursor,
                    active_tab.buffer.len(),
                    size.width as f32,
                    size.height as f32,
                );
            }
        }
        UiAction::SaveFile => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            let path_to_save = active_tab.path.clone().unwrap_or_else(|| {
                let default_path = "./untitled.txt".to_string();
                active_tab.path = Some(default_path.clone());
                default_path
            });
            if let Err(e) = active_tab.buffer.save_file(&path_to_save) {
                log::error!("Failed to save file: {:?}", e);
            } else {
                active_tab.buffer.mark_saved();
                let path_buf = std::path::PathBuf::from(&path_to_save);
                ui.unsaved_project_search_files.remove(&path_buf);
                ui.invalidate_search_render_items();
                ui.rebuild_tree();
                ui.update_git_diff(Some(&path_to_save));
                ui.update_git_file_blame(Some(&path_to_save));
                ui.update_git_statuses();
                ui.external_change_warnings.remove(&path_to_save);
            }
        }
        UiAction::Undo => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            if let Some((line, col)) = active_tab.buffer.undo() {
                active_tab.cursor.line = line.min(active_tab.buffer.len() - 1);
                let max_col = active_tab.buffer.lines()[active_tab.cursor.line]
                    .chars()
                    .count();
                active_tab.cursor.col = col.min(max_col);
                active_tab.cursor.intended_col = active_tab.cursor.col;
            }
            active_tab.cursor.clear_selection();
        }
        UiAction::Redo => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            if let Some((line, col)) = active_tab.buffer.redo() {
                active_tab.cursor.line = line.min(active_tab.buffer.len() - 1);
                let max_col = active_tab.buffer.lines()[active_tab.cursor.line]
                    .chars()
                    .count();
                active_tab.cursor.col = col.min(max_col);
                active_tab.cursor.intended_col = active_tab.cursor.col;
            }
            active_tab.cursor.clear_selection();
        }
        UiAction::Find => {
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
        UiAction::FindInProject => {
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
        UiAction::ToggleSidebar => {
            let preferred = if ui.config.sidebar_width > 0.0 {
                ui.config.sidebar_width
            } else {
                200.0
            };
            ui.target_sidebar_width = if ui.target_sidebar_width > 0.0 {
                0.0
            } else {
                preferred
            };
            ui.sidebar_width = ui.target_sidebar_width;
            if ui.target_sidebar_width > 0.0 {
                ui.config.sidebar_width = ui.target_sidebar_width;
                ui.config.save_in_background();
            }
        }
        UiAction::ShowSettings => {
            ui.open_modal(crate::machkit::ModalType::Settings);
        }
        UiAction::ShowAbout => {
            ui.open_modal(crate::machkit::ModalType::About);
        }
        UiAction::ShowCommandPalette => {
            ui.open_modal(crate::machkit::ModalType::CommandPalette);
        }
        UiAction::CloseModal => {
            ui.close_modal();
        }
        UiAction::ChangeBufferFontSize(delta) => {
            let new_size = (ui.buffer_font_size + delta).clamp(8.0, 36.0);
            ui.update_buffer_font_size(&atlas.font, new_size);
            ui.config.buffer_font_size = new_size;
            ui.config.save_in_background();
        }
        UiAction::ChangeUiFontSize(delta) => {
            let new_size = (ui.ui_font_size + delta).clamp(8.0, 24.0);
            ui.update_ui_font_size(&atlas.font, new_size);
            ui.config.ui_font_size = new_size;
            ui.config.save_in_background();
        }
        UiAction::ChangeSidebarWidth(delta) => {
            let new_width = (ui.config.sidebar_width + delta).clamp(100.0, 400.0);
            ui.config.sidebar_width = new_width;
            ui.target_sidebar_width = new_width;
            ui.sidebar_width = new_width;
            ui.config.save_in_background();
        }
        UiAction::ChangeTheme(theme_name) => {
            let selected_theme = crate::editor::config::Theme::get_by_name(&theme_name);
            ui.config.theme = selected_theme;
            ui.config.save_in_background();
        }
        UiAction::ChangeGitBlame(enabled) => {
            ui.config.show_git_blame = enabled;
            ui.config.save_in_background();
        }
        UiAction::ChangeGitBranch(enabled) => {
            ui.config.show_git_branch = enabled;
            if !enabled {
                ui.git_branch = None;
            }
            ui.config.save_in_background();
        }
        UiAction::ChangeBackend(backend) => {
            let mut new_config = ui.config.clone();
            let forced_backends = match backend {
                wgpu::Backend::Vulkan => wgpu::Backends::VULKAN,
                wgpu::Backend::Gl => wgpu::Backends::GL,
                _ => wgpu::Backends::all(),
            };
            *gpu = None;
            let mut new_gpu =
                pollster::block_on(GpuContext::new(window.clone(), Some(forced_backends)));

            let actual_backend_str = match new_gpu.backend {
                wgpu::Backend::Vulkan => "Vulkan",
                wgpu::Backend::Gl => "OpenGL",
                _ => "Vulkan",
            };
            new_config.backend = actual_backend_str.to_string();
            if let Err(e) = new_config.save() {
                log::warn!("Failed to save config on settings change: {:?}", e);
            } else {
                log::warn!(
                    "Successfully saved backend '{}' to config from settings.",
                    actual_backend_str
                );
            }

            if let Ok(new_atlas) = FontAtlas::new(&new_gpu.device, &new_gpu.queue, font_bytes) {
                *atlas = new_atlas;
                new_gpu.update_bind_group(&atlas.texture, &atlas.sampler);

                let mut new_ui = UiState::new(
                    atlas,
                    &new_gpu.queue,
                    new_config,
                    ui.event_loop_proxy.clone(),
                    ui.experimental,
                );
                new_ui.active_device_name = new_gpu.device_name.clone();

                let old_expanded = ui.expanded_dirs.clone();
                let old_selected = ui.selected_file.clone();
                let old_sidebar_w = ui.sidebar_width;
                let old_target_sidebar_w = ui.target_sidebar_width;
                *ui = new_ui;
                ui.expanded_dirs = old_expanded;
                ui.selected_file = old_selected;
                ui.sidebar_width = old_sidebar_w;
                ui.target_sidebar_width = old_target_sidebar_w;
                ui.open_modal(crate::machkit::ModalType::Settings);
                ui.rebuild_tree();
            }
            *gpu = Some(new_gpu);
        }
        UiAction::SelectTab(idx) => {
            state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
            state.tabs[state.active_tab_idx].scroll_y = ui.scroll_y;
            state.active_tab_idx = idx;
            ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
            ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
            if let Some(ref path) = state.tabs[idx].path {
                ui.selected_file = Some(std::path::PathBuf::from(path));
            } else {
                ui.selected_file = None;
            }
            let tab_paths: Vec<Option<String>> =
                state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            let visible_width = if state.inactive_panes.is_empty() || state.is_split_horizontal {
                size.width as f32 - ui.sidebar_width
            } else {
                ((size.width as f32 - ui.sidebar_width) / 2.0).round()
            };
            state.tab_scroll_x = ui.scroll_to_tab(
                state.active_tab_idx,
                &tab_paths,
                visible_width,
                state.tab_scroll_x,
            );
        }
        UiAction::CloseTab(idx) => {
            let autosave_setting = &ui.config.autosave;
            if crate::app::autosave::should_save_on_close(autosave_setting)
                && state.tabs[idx].path.is_some()
            {
                crate::app::autosave::save_tab(ui, &mut state.tabs[idx]);
            }

            if state.tabs[idx].buffer.is_modified {
                ui.tab_to_close = Some(idx);
                ui.open_modal(crate::machkit::ModalType::UnsavedChanges);
            } else {
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
                        state.tabs.push(Tab {
                            path: None,
                            buffer: Buffer::new(),
                            cursor: Cursor::new(),
                            secondary_cursors: Vec::new(),
                            scroll_x: 0,
                            scroll_y: 0,
                        });
                    }
                }
                state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
                ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
                ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
                if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                    ui.selected_file = Some(std::path::PathBuf::from(path));
                } else {
                    ui.selected_file = None;
                }
                let tab_paths: Vec<Option<String>> =
                    state.tabs.iter().map(|t| t.path.clone()).collect();
                let size = window.inner_size();
                let visible_width = if state.inactive_panes.is_empty() || state.is_split_horizontal
                {
                    size.width as f32 - ui.sidebar_width
                } else {
                    ((size.width as f32 - ui.sidebar_width) / 2.0).round()
                };
                state.tab_scroll_x = ui.scroll_to_tab(
                    state.active_tab_idx,
                    &tab_paths,
                    visible_width,
                    state.tab_scroll_x,
                );
                crate::app::autosave::save_session_and_dirty_buffers(state);
            }
        }
        UiAction::ForceCloseTab(idx) => {
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
                    state.tabs.push(Tab {
                        path: None,
                        buffer: Buffer::new(),
                        cursor: Cursor::new(),
                        secondary_cursors: Vec::new(),
                        scroll_x: 0,
                        scroll_y: 0,
                    });
                }
            }
            state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
            ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
            ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
            if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                ui.selected_file = Some(std::path::PathBuf::from(path));
            } else {
                ui.selected_file = None;
            }
            ui.tab_to_close = None;
            ui.close_modal();
            let tab_paths: Vec<Option<String>> =
                state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            let visible_width = if state.inactive_panes.is_empty() || state.is_split_horizontal {
                size.width as f32 - ui.sidebar_width
            } else {
                ((size.width as f32 - ui.sidebar_width) / 2.0).round()
            };
            state.tab_scroll_x = ui.scroll_to_tab(
                state.active_tab_idx,
                &tab_paths,
                visible_width,
                state.tab_scroll_x,
            );
            crate::app::autosave::save_session_and_dirty_buffers(state);
        }
        UiAction::SaveAndCloseTab(idx) => {
            let tab_to_save = &mut state.tabs[idx];
            let path_to_save = tab_to_save.path.clone().unwrap_or_else(|| {
                let default_path = "./untitled.txt".to_string();
                tab_to_save.path = Some(default_path.clone());
                default_path
            });
            if let Err(e) = tab_to_save.buffer.save_file(&path_to_save) {
                log::error!("Failed to save file: {:?}", e);
            } else {
                tab_to_save.buffer.mark_saved();
                ui.external_change_warnings.remove(&path_to_save);
            }

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
                    state.tabs.push(Tab {
                        path: None,
                        buffer: Buffer::new(),
                        cursor: Cursor::new(),
                        secondary_cursors: Vec::new(),
                        scroll_x: 0,
                        scroll_y: 0,
                    });
                }
            }
            state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
            ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
            ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
            if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                ui.selected_file = Some(std::path::PathBuf::from(path));
            } else {
                ui.selected_file = None;
            }
            ui.tab_to_close = None;
            ui.close_modal();
            ui.rebuild_tree();
            let tab_paths: Vec<Option<String>> =
                state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            let visible_width = if state.inactive_panes.is_empty() || state.is_split_horizontal {
                size.width as f32 - ui.sidebar_width
            } else {
                ((size.width as f32 - ui.sidebar_width) / 2.0).round()
            };
            state.tab_scroll_x = ui.scroll_to_tab(
                state.active_tab_idx,
                &tab_paths,
                visible_width,
                state.tab_scroll_x,
            );
            crate::app::autosave::save_session_and_dirty_buffers(state);
        }
        UiAction::MinimizeWindow => {
            window.set_minimized(true);
        }
        UiAction::MaximizeWindow => {
            let is_max = window.is_maximized();
            window.set_maximized(!is_max);
        }
        UiAction::ToggleDock => {
            ui.show_dock = !ui.show_dock;
            if !ui.show_dock {
                state.terminal_focus = false;
            } else {
                let size = window.inner_size();
                let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;

                if state.dock_terminals.is_empty() {
                    if let Ok(term) = crate::terminal::TerminalInstance::new(
                        cols,
                        rows,
                        window.clone(),
                        ui.event_loop_proxy.clone(),
                    ) {
                        state.dock_terminals.push(term);
                        state.active_terminal_idx = 0;
                    }
                } else {
                    let active_term = &mut state.dock_terminals[state.active_terminal_idx];
                    active_term.grid.resize(cols, rows);
                    active_term.resize_pty(cols, rows);
                }
            }
        }
        UiAction::NewTerminal => {
            let size = window.inner_size();
            let width_content = size.width as f32 - ui.sidebar_width - 16.0;
            let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
            let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
            let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
            if let Ok(term) = crate::terminal::TerminalInstance::new(
                cols,
                rows,
                window.clone(),
                ui.event_loop_proxy.clone(),
            ) {
                state.dock_terminals.push(term);
                state.active_terminal_idx = state.dock_terminals.len() - 1;
            }
            state.terminal_focus = true;
        }
        UiAction::CloseTerminal(idx) => {
            if idx < state.dock_terminals.len() {
                state.dock_terminals.remove(idx);
                if state.dock_terminals.is_empty() {
                    let size = window.inner_size();
                    let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                    let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                    let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                    let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                    if let Ok(term) = crate::terminal::TerminalInstance::new(
                        cols,
                        rows,
                        window.clone(),
                        ui.event_loop_proxy.clone(),
                    ) {
                        state.dock_terminals.push(term);
                    }
                }
                state.active_terminal_idx = state
                    .active_terminal_idx
                    .min(state.dock_terminals.len() - 1);
            }
        }
        UiAction::SelectTerminal(idx) => {
            if idx < state.dock_terminals.len() {
                state.active_terminal_idx = idx;
                state.terminal_focus = true;
            }
        }
        UiAction::SplitVertical => {
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
                state.is_split_horizontal = false;
                state.switch_pane(1);
            } else {
                let target_pane = if state.active_pane_idx == 0 { 1 } else { 0 };
                state.is_split_horizontal = false;
                state.switch_pane(target_pane);
            }
        }
        UiAction::SplitHorizontal => {
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
                state.is_split_horizontal = true;
                state.switch_pane(1);
            } else {
                let target_pane = if state.active_pane_idx == 0 { 1 } else { 0 };
                state.is_split_horizontal = true;
                state.switch_pane(target_pane);
            }
        }
        UiAction::Exit => {
            elwt.exit();
        }
        UiAction::None => {}
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
}
