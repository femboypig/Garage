use std::sync::Arc;
use winit::window::Window;
use winit::event_loop::EventLoopWindowTarget;
use crate::ui::{UiState, UiAction};
use crate::renderer::wgpu::GpuContext;
use crate::renderer::atlas::FontAtlas;
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use super::state::{AppState, Tab};

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
    match action {
        UiAction::OpenFile(path) => {
            let path_str = path.to_string_lossy().to_string();
            let is_new = if let Some(existing_idx) = state.tabs.iter().position(|t| t.path.as_ref() == Some(&path_str)) {
                state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
                state.tabs[state.active_tab_idx].scroll_y = ui.scroll_y;
                state.active_tab_idx = existing_idx;
                ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
                ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
                false
            } else {
                let mut new_buf = Buffer::new();
                if let Err(e) = new_buf.load_file(&path_str) {
                    log::warn!("Failed to load file '{}': {}", path_str, e);
                }
                state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
                state.tabs[state.active_tab_idx].scroll_y = ui.scroll_y;
                state.tabs.push(Tab {
                    path: Some(path_str),
                    buffer: new_buf,
                    cursor: Cursor::new(),
                    scroll_x: 0,
                    scroll_y: 0,
                });
                state.active_tab_idx = state.tabs.len() - 1;
                ui.scroll_x = 0;
                ui.scroll_y = 0;
                true
            };
            if let Some(ref active_path) = state.tabs[state.active_tab_idx].path {
                ui.update_git_diff(Some(active_path));
                ui.update_git_file_blame(Some(active_path));
                ui.update_git_statuses();
                if let Some(ref lsp) = state.lsp_client {
                    if is_new {
                        lsp.notify_open(active_path, state.tabs[state.active_tab_idx].buffer.lines().join("\n"));
                    }
                    lsp.notify_active_file(active_path);
                }
            }
            let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            ui.scroll_to_tab(state.active_tab_idx, &tab_paths, size.width as f32);
        }
        UiAction::SaveFile => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            let was_untitled = active_tab.path.is_none();
            let path_to_save = active_tab.path.clone().unwrap_or_else(|| {
                let default_path = "./untitled.txt".to_string();
                active_tab.path = Some(default_path.clone());
                default_path
            });
            if let Err(e) = active_tab.buffer.save_file(&path_to_save) {
                log::error!("Failed to save file: {:?}", e);
            } else {
                active_tab.buffer.mark_saved();
                ui.rebuild_tree();
                ui.update_git_diff(Some(&path_to_save));
                ui.update_git_file_blame(Some(&path_to_save));
                ui.update_git_statuses();
                if let Some(ref lsp) = state.lsp_client {
                    if was_untitled {
                        lsp.notify_open(&path_to_save, active_tab.buffer.lines().join("\n"));
                    }
                    lsp.notify_save(&path_to_save);
                }
            }
        }
        UiAction::Undo => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            if let Some((line, col)) = active_tab.buffer.undo() {
                active_tab.cursor.line = line.min(active_tab.buffer.len() - 1);
                let max_col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                active_tab.cursor.col = col.min(max_col);
                active_tab.cursor.intended_col = active_tab.cursor.col;
            }
            active_tab.cursor.clear_selection();
        }
        UiAction::Redo => {
            let active_tab = &mut state.tabs[state.active_tab_idx];
            if let Some((line, col)) = active_tab.buffer.redo() {
                active_tab.cursor.line = line.min(active_tab.buffer.len() - 1);
                let max_col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                active_tab.cursor.col = col.min(max_col);
                active_tab.cursor.intended_col = active_tab.cursor.col;
            }
            active_tab.cursor.clear_selection();
        }
        UiAction::ToggleSidebar => {
            let preferred = if ui.config.sidebar_width > 0.0 { ui.config.sidebar_width } else { 200.0 };
            ui.target_sidebar_width = if ui.target_sidebar_width > 0.0 { 0.0 } else { preferred };
            ui.sidebar_width = ui.target_sidebar_width;
            if ui.target_sidebar_width > 0.0 {
                ui.config.sidebar_width = ui.target_sidebar_width;
                ui.config.save_in_background();
            }
        }
        UiAction::ShowSettings => {
            ui.active_modal = Some(crate::ui::ModalType::Settings);
        }
        UiAction::ShowAbout => {
            ui.active_modal = Some(crate::ui::ModalType::About);
        }
        UiAction::ShowCommandPalette => {
            ui.active_modal = Some(crate::ui::ModalType::CommandPalette);
            ui.command_palette_query.clear();
            ui.command_palette_selected = 0;
        }
        UiAction::CloseModal => {
            ui.active_modal = None;
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
            let mut new_gpu = pollster::block_on(GpuContext::new(window.clone(), Some(forced_backends)));

            let actual_backend_str = match new_gpu.backend {
                wgpu::Backend::Vulkan => "Vulkan",
                wgpu::Backend::Gl => "OpenGL",
                _ => "Vulkan",
            };
            new_config.backend = actual_backend_str.to_string();
            if let Err(e) = new_config.save() {
                log::warn!("Failed to save config on settings change: {:?}", e);
            } else {
                log::warn!("Successfully saved backend '{}' to config from settings.", actual_backend_str);
            }

            if let Ok(new_atlas) = FontAtlas::new(&new_gpu.device, &new_gpu.queue, font_bytes) {
                *atlas = new_atlas;
                new_gpu.update_bind_group(&atlas.texture, &atlas.sampler);

                let mut new_ui = UiState::new(atlas, &new_gpu.queue, new_config, ui.event_loop_proxy.clone());
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
                ui.active_modal = Some(crate::ui::ModalType::Settings);
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
            let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            ui.scroll_to_tab(state.active_tab_idx, &tab_paths, size.width as f32);
            if let Some(ref lsp) = state.lsp_client {
                if let Some(ref path) = state.tabs[idx].path {
                    lsp.notify_active_file(path);
                } else {
                    lsp.notify_active_file("");
                }
            }
        }
        UiAction::CloseTab(idx) => {
            if state.tabs[idx].buffer.is_modified {
                ui.tab_to_close = Some(idx);
                ui.active_modal = Some(crate::ui::ModalType::UnsavedChanges);
            } else {
                state.tabs.remove(idx);
                if state.tabs.is_empty() {
                    state.tabs.push(Tab {
                        path: None,
                        buffer: Buffer::new(),
                        cursor: Cursor::new(),
                        scroll_x: 0,
                        scroll_y: 0,
                    });
                }
                state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
                ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
                ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
                let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                let size = window.inner_size();
                ui.scroll_to_tab(state.active_tab_idx, &tab_paths, size.width as f32);
                if let Some(ref lsp) = state.lsp_client {
                    if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                        lsp.notify_active_file(path);
                    } else {
                        lsp.notify_active_file("");
                    }
                }
            }
        }
        UiAction::ForceCloseTab(idx) => {
            state.tabs.remove(idx);
            if state.tabs.is_empty() {
                state.tabs.push(Tab {
                    path: None,
                    buffer: Buffer::new(),
                    cursor: Cursor::new(),
                    scroll_x: 0,
                    scroll_y: 0,
                });
            }
            state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
            ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
            ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
            ui.tab_to_close = None;
            ui.active_modal = None;
            let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            ui.scroll_to_tab(state.active_tab_idx, &tab_paths, size.width as f32);
            if let Some(ref lsp) = state.lsp_client {
                if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                    lsp.notify_active_file(path);
                } else {
                    lsp.notify_active_file("");
                }
            }
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
            }
            
            state.tabs.remove(idx);
            if state.tabs.is_empty() {
                state.tabs.push(Tab {
                    path: None,
                    buffer: Buffer::new(),
                    cursor: Cursor::new(),
                    scroll_x: 0,
                    scroll_y: 0,
                });
            }
            state.active_tab_idx = state.active_tab_idx.min(state.tabs.len() - 1);
            ui.scroll_x = state.tabs[state.active_tab_idx].scroll_x;
            ui.scroll_y = state.tabs[state.active_tab_idx].scroll_y;
            ui.tab_to_close = None;
            ui.active_modal = None;
            ui.rebuild_tree();
            let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
            let size = window.inner_size();
            ui.scroll_to_tab(state.active_tab_idx, &tab_paths, size.width as f32);
            if let Some(ref lsp) = state.lsp_client {
                if let Some(ref path) = state.tabs[state.active_tab_idx].path {
                    lsp.notify_active_file(path);
                } else {
                    lsp.notify_active_file("");
                }
            }
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
                    if let Ok(term) = crate::terminal::TerminalInstance::new(cols, rows, window.clone(), ui.event_loop_proxy.clone()) {
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
            if let Ok(term) = crate::terminal::TerminalInstance::new(cols, rows, window.clone(), ui.event_loop_proxy.clone()) {
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
                    if let Ok(term) = crate::terminal::TerminalInstance::new(cols, rows, window.clone(), ui.event_loop_proxy.clone()) {
                        state.dock_terminals.push(term);
                    }
                }
                state.active_terminal_idx = state.active_terminal_idx.min(state.dock_terminals.len() - 1);
            }
        }
        UiAction::SelectTerminal(idx) => {
            if idx < state.dock_terminals.len() {
                state.active_terminal_idx = idx;
                state.terminal_focus = true;
            }
        }
        UiAction::Exit => {
            elwt.exit();
        }
        UiAction::None => {}
    }
}
