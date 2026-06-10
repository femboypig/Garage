use std::sync::Arc;
use std::io::Write;
use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::{GpuContext, Vertex};
use crate::ui::{UiState, UiAction};

pub fn run_editor(file_path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize env_logger (warn level by default to not pollute output)
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let event_loop = EventLoop::new()?;
    let mut window = Arc::new(
        WindowBuilder::new()
            .with_title("Garage")
            .with_decorations(false)
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 800))
            .build(&event_loop)?,
    );

    // Initialize wgpu rendering context and pipeline synchronously
    // Load configuration at startup
    let mut config = crate::editor::config::AppConfig::load();

    // Select backend based on config
    let initial_backends = match config.backend.as_str() {
        "Vulkan" => Some(wgpu::Backends::VULKAN),
        "OpenGL" => Some(wgpu::Backends::GL),
        _ => None,
    };

    // Initialize wgpu rendering context and pipeline synchronously
    let mut gpu = Some(pollster::block_on(GpuContext::new(window.clone(), initial_backends)));

    let actual_backend_str = match gpu.as_ref().unwrap().backend {
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Gl => "OpenGL",
        _ => "Vulkan",
    };
    if config.backend != actual_backend_str {
        config.backend = actual_backend_str.to_string();
        if let Err(e) = config.save() {
            log::warn!("Failed to save config on startup fallback: {:?}", e);
        } else {
            log::warn!("Successfully saved fallback backend '{}' to config.", actual_backend_str);
        }
    }

    // Load bundled IBM Plex Mono font bytes
    let font_bytes = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");

    // Initialize Font Atlas using wgpu device/queue
    let mut atlas = FontAtlas::new(&gpu.as_ref().unwrap().device, &gpu.as_ref().unwrap().queue, font_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Update bind group to use actual font texture and sampler
    gpu.as_mut().unwrap().update_bind_group(&atlas.texture, &atlas.sampler);

    // Initialize layout and state
    let mut ui = UiState::new(&mut atlas, &gpu.as_ref().unwrap().queue, config);
    ui.active_device_name = gpu.as_ref().unwrap().device_name.clone();

    // Define Tab structure for multi-tab support
    struct Tab {
        path: Option<String>,
        buffer: Buffer,
        cursor: Cursor,
        scroll_x: usize,
        scroll_y: usize,
    }

    let mut tabs: Vec<Tab> = Vec::new();
    let mut active_tab_idx: usize = 0;

    // Load initial file or start with empty tab
    {
        let mut buffer = Buffer::new();
        let save_path = if let Some(ref path) = file_path {
            if let Err(e) = buffer.load_file(path) {
                log::warn!("Failed to load file '{}': {}. Starting with empty buffer.", path, e);
            }
            Some(path.clone())
        } else {
            None
        };
        let cursor = Cursor::new();
        tabs.push(Tab {
            path: save_path,
            buffer,
            cursor,
            scroll_x: 0,
            scroll_y: 0,
        });
    }

    // Editor state
    let mut modifiers = winit::keyboard::ModifiersState::default();
    let mut is_dragging = false;
    let mut is_dragging_scroll = false;
    let mut is_dragging_horizontal_scroll = false;
    let mut is_dragging_minimap = false;
    let mut scroll_drag_offset_y = 0.0f32;
    let mut scroll_drag_offset_x = 0.0f32;
    let mut is_dragging_sidebar = false;
    let mut is_dragging_dock_border = false;
    let mut internal_clipboard = String::new();
    
    // Terminal Dock state
    let mut dock_terminals: Vec<crate::terminal::TerminalInstance> = Vec::new();
    let mut active_terminal_idx: usize = 0;
    let mut terminal_focus = false;
    let mut last_click_time: Option<std::time::Instant> = None;

    // Create default initial terminal with correct dimensions matching the startup size of the dock area
    let initial_term_size = window.inner_size();
    let initial_width_content = initial_term_size.width as f32 - ui.sidebar_width - 16.0;
    let initial_height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
    let initial_cols = (initial_width_content / ui.buffer_char_width).floor().max(10.0) as usize;
    let initial_rows = (initial_height_content / ui.buffer_line_height).floor().max(2.0) as usize;

    if let Ok(term) = crate::terminal::TerminalInstance::new(initial_cols, initial_rows, window.clone()) {
        dock_terminals.push(term);
    }
    
    // Helper to update cursor icon
    let update_cursor_icon = |window: &winit::window::Window, ui: &UiState, buffer: &Buffer, mouse_x: f32, mouse_y: f32| {
        let size = window.inner_size();
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        
        let on_sidebar_border = ui.sidebar_width > 0.0 && (mouse_x - ui.sidebar_width).abs() <= 4.0;
        
        let main_y = ui.titlebar_height;
        let mut dock_start_y = size.height as f32 - ui.status_height;
        if ui.show_dock {
            dock_start_y = (size.height as f32 - ui.status_height - ui.dock_height).max(main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0);
        }
        let on_dock_border = ui.show_dock && (mouse_y - dock_start_y).abs() <= 4.0;
        
        if on_sidebar_border {
            window.set_cursor_icon(winit::window::CursorIcon::ColResize);
        } else if on_dock_border {
            window.set_cursor_icon(winit::window::CursorIcon::RowResize);
        } else {
            let scrollbar_width = ui.scrollbar_width();
            let minimap_width = ui.minimap_width();
            let sb_x = size.width as f32 - scrollbar_width;
            let minimap_x = sb_x - minimap_width;

            let is_in_editor = ui.active_modal.is_none()
                && ui.active_menu.is_none()
                && mouse_x >= text_area_x
                && mouse_x < minimap_x
                && mouse_y >= ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height
                && mouse_y < dock_start_y - 14.0;
                
            if is_in_editor {
                window.set_cursor_icon(winit::window::CursorIcon::Text);
            } else {
                window.set_cursor_icon(winit::window::CursorIcon::Default);
            }
        }
    };
    
    // Track mouse pixel coordinates
    let mut mouse_x = 0.0f32;
    let mut mouse_y = 0.0f32;

    // Track dynamic vertices and indices
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    // Run the event loop reactively to save power/CPU/GPU cycles when idle
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        macro_rules! handle_action {
            ($act:expr) => {
                match $act {
                    UiAction::OpenFile(path) => {
                        let path_str = path.to_string_lossy().to_string();
                        if let Some(existing_idx) = tabs.iter().position(|t| t.path.as_ref() == Some(&path_str)) {
                            tabs[active_tab_idx].scroll_x = ui.scroll_x;
                            tabs[active_tab_idx].scroll_y = ui.scroll_y;
                            active_tab_idx = existing_idx;
                            ui.scroll_x = tabs[active_tab_idx].scroll_x;
                            ui.scroll_y = tabs[active_tab_idx].scroll_y;
                        } else {
                            let mut new_buf = Buffer::new();
                            if let Err(e) = new_buf.load_file(&path_str) {
                                log::warn!("Failed to load file '{}': {}", path_str, e);
                            }
                            tabs[active_tab_idx].scroll_x = ui.scroll_x;
                            tabs[active_tab_idx].scroll_y = ui.scroll_y;
                            tabs.push(Tab {
                                path: Some(path_str),
                                buffer: new_buf,
                                cursor: Cursor::new(),
                                scroll_x: 0,
                                scroll_y: 0,
                            });
                            active_tab_idx = tabs.len() - 1;
                            ui.scroll_x = 0;
                            ui.scroll_y = 0;
                        }
                    }
                    UiAction::SaveFile => {
                        let active_tab = &mut tabs[active_tab_idx];
                        let path_to_save = active_tab.path.clone().unwrap_or_else(|| {
                            let default_path = "./untitled.txt".to_string();
                            active_tab.path = Some(default_path.clone());
                            default_path
                        });
                        if let Err(e) = active_tab.buffer.save_file(&path_to_save) {
                            log::error!("Failed to save file: {:?}", e);
                        } else {
                            active_tab.buffer.is_modified = false;
                            ui.rebuild_tree();
                        }
                    }
                    UiAction::Undo => {
                        let active_tab = &mut tabs[active_tab_idx];
                        if let Some((line, col)) = active_tab.buffer.undo() {
                            active_tab.cursor.line = line.min(active_tab.buffer.len() - 1);
                            let max_col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                            active_tab.cursor.col = col.min(max_col);
                            active_tab.cursor.intended_col = active_tab.cursor.col;
                        }
                        active_tab.cursor.clear_selection();
                    }
                    UiAction::Redo => {
                        let active_tab = &mut tabs[active_tab_idx];
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
                        gpu = None;
                        window.set_visible(false);
                        let current_size = window.inner_size();
                        let new_win_res = WindowBuilder::new()
                            .with_title("Garage")
                            .with_decorations(false)
                            .with_inner_size(current_size)
                            .build(elwt);
                        match new_win_res {
                            Ok(w) => {
                                window = Arc::new(w);
                                window.set_visible(true);
                            }
                            Err(e) => {
                                log::error!("Failed to recreate window: {:?}", e);
                            }
                        }
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
                            atlas = new_atlas;
                            new_gpu.update_bind_group(&atlas.texture, &atlas.sampler);

                            let mut new_ui = UiState::new(&mut atlas, &new_gpu.queue, new_config);
                            new_ui.active_device_name = new_gpu.device_name.clone();

                            let old_expanded = ui.expanded_dirs.clone();
                            let old_selected = ui.selected_file.clone();
                            let old_sidebar_w = ui.sidebar_width;
                            let old_target_sidebar_w = ui.target_sidebar_width;
                            ui = new_ui;
                            ui.expanded_dirs = old_expanded;
                            ui.selected_file = old_selected;
                            ui.sidebar_width = old_sidebar_w;
                            ui.target_sidebar_width = old_target_sidebar_w;
                            ui.active_modal = Some(crate::ui::ModalType::Settings);
                            ui.rebuild_tree();
                        }
                        gpu = Some(new_gpu);
                    }
                    UiAction::SelectTab(idx) => {
                        tabs[active_tab_idx].scroll_x = ui.scroll_x;
                        tabs[active_tab_idx].scroll_y = ui.scroll_y;
                        active_tab_idx = idx;
                        ui.scroll_x = tabs[active_tab_idx].scroll_x;
                        ui.scroll_y = tabs[active_tab_idx].scroll_y;
                    }
                    UiAction::CloseTab(idx) => {
                        if tabs[idx].buffer.is_modified {
                            ui.tab_to_close = Some(idx);
                            ui.active_modal = Some(crate::ui::ModalType::UnsavedChanges);
                        } else {
                            tabs.remove(idx);
                            if tabs.is_empty() {
                                tabs.push(Tab {
                                    path: None,
                                    buffer: Buffer::new(),
                                    cursor: Cursor::new(),
                                    scroll_x: 0,
                                    scroll_y: 0,
                                });
                            }
                            active_tab_idx = active_tab_idx.min(tabs.len() - 1);
                            ui.scroll_x = tabs[active_tab_idx].scroll_x;
                            ui.scroll_y = tabs[active_tab_idx].scroll_y;
                        }
                    }
                    UiAction::ForceCloseTab(idx) => {
                        tabs.remove(idx);
                        if tabs.is_empty() {
                            tabs.push(Tab {
                                path: None,
                                buffer: Buffer::new(),
                                cursor: Cursor::new(),
                                scroll_x: 0,
                                scroll_y: 0,
                            });
                        }
                        active_tab_idx = active_tab_idx.min(tabs.len() - 1);
                        ui.scroll_x = tabs[active_tab_idx].scroll_x;
                        ui.scroll_y = tabs[active_tab_idx].scroll_y;
                        ui.tab_to_close = None;
                        ui.active_modal = None;
                    }
                    UiAction::SaveAndCloseTab(idx) => {
                        let tab_to_save = &mut tabs[idx];
                        let path_to_save = tab_to_save.path.clone().unwrap_or_else(|| {
                            let default_path = "./untitled.txt".to_string();
                            tab_to_save.path = Some(default_path.clone());
                            default_path
                        });
                        if let Err(e) = tab_to_save.buffer.save_file(&path_to_save) {
                            log::error!("Failed to save file: {:?}", e);
                        } else {
                            tab_to_save.buffer.is_modified = false;
                        }
                        
                        tabs.remove(idx);
                        if tabs.is_empty() {
                            tabs.push(Tab {
                                path: None,
                                buffer: Buffer::new(),
                                cursor: Cursor::new(),
                                scroll_x: 0,
                                scroll_y: 0,
                            });
                        }
                        active_tab_idx = active_tab_idx.min(tabs.len() - 1);
                        ui.scroll_x = tabs[active_tab_idx].scroll_x;
                        ui.scroll_y = tabs[active_tab_idx].scroll_y;
                        ui.tab_to_close = None;
                        ui.active_modal = None;
                        ui.rebuild_tree();
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
                            terminal_focus = false;
                        } else if !dock_terminals.is_empty() {
                            let size = window.inner_size();
                            let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                            let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                            let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                            let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                            let active_term = &mut dock_terminals[active_terminal_idx];
                            active_term.grid.resize(cols, rows);
                            active_term.resize_pty(cols, rows);
                        }
                    }
                    UiAction::NewTerminal => {
                        let size = window.inner_size();
                        let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                        let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                        let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                        let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                        if let Ok(term) = crate::terminal::TerminalInstance::new(cols, rows, window.clone()) {
                            dock_terminals.push(term);
                            active_terminal_idx = dock_terminals.len() - 1;
                        }
                        terminal_focus = true;
                    }
                    UiAction::CloseTerminal(idx) => {
                        if idx < dock_terminals.len() {
                            dock_terminals.remove(idx);
                            if dock_terminals.is_empty() {
                                let size = window.inner_size();
                                let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                                let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                                let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                                let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                                if let Ok(term) = crate::terminal::TerminalInstance::new(cols, rows, window.clone()) {
                                    dock_terminals.push(term);
                                }
                            }
                            active_terminal_idx = active_terminal_idx.min(dock_terminals.len() - 1);
                        }
                    }
                    UiAction::SelectTerminal(idx) => {
                        if idx < dock_terminals.len() {
                            active_terminal_idx = idx;
                            terminal_focus = true;
                        }
                    }
                    UiAction::Exit => {
                        elwt.exit();
                    }
                    UiAction::None => {}
                }
            };
        }

        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),

                WindowEvent::Resized(physical_size) => {
                    gpu.as_mut().unwrap().resize(physical_size);
                    window.request_redraw();
                }

                WindowEvent::ScaleFactorChanged { .. } => {
                    let physical_size = window.inner_size();
                    gpu.as_mut().unwrap().resize(physical_size);
                    window.request_redraw();
                }

                WindowEvent::RedrawRequested => {
                    let size = window.inner_size();
                    if ui.show_dock && !dock_terminals.is_empty() {
                        let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                        let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                        let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                        let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                        let active_term = &mut dock_terminals[active_terminal_idx];
                        if active_term.grid.cols != cols || active_term.grid.rows != rows {
                            active_term.grid.resize(cols, rows);
                            active_term.resize_pty(cols, rows);
                        }
                    }

                    // Read data from PTY channels for all terminals and parse ANSI sequences
                    for term in &mut dock_terminals {
                        let mut parser = vte::Parser::new();
                        while let Ok(bytes) = term.rx.try_recv() {
                            for b in bytes {
                                parser.advance(&mut term.grid, b);
                            }
                        }
                    }

                    // Sync active tab scroll offsets
                    tabs[active_tab_idx].scroll_x = ui.scroll_x;
                    tabs[active_tab_idx].scroll_y = ui.scroll_y;

                    // Clear dynamic buffers
                    vertices.clear();
                    indices.clear();

                    let size = window.inner_size();
                    
                    let tab_paths: Vec<Option<String>> = tabs.iter().map(|t| t.path.clone()).collect();
                    let tab_modified: Vec<bool> = tabs.iter().map(|t| t.buffer.is_modified).collect();

                    // Compile the current editor frame into GPU vertices
                    ui.build_frame(
                        &mut vertices,
                        &mut indices,
                        &mut atlas,
                        &gpu.as_ref().unwrap().queue,
                        &tabs[active_tab_idx].buffer,
                        &tabs[active_tab_idx].cursor,
                        size.width as f32,
                        size.height as f32,
                        mouse_x,
                        mouse_y,
                        gpu.as_ref().unwrap().backend,
                        &tab_paths,
                        &tab_modified,
                        active_tab_idx,
                        &dock_terminals,
                        active_terminal_idx,
                        terminal_focus,
                        window.is_maximized(),
                    );

                    // Update cursor icon when screen redraws
                    update_cursor_icon(&window, &ui, &tabs[active_tab_idx].buffer, mouse_x, mouse_y);

                    // Render to swapchain
                    if let Err(e) = gpu.as_mut().unwrap().render(&vertices, &indices) {
                        log::error!("Rendering error: {:?}", e);
                    }
                }

                WindowEvent::ModifiersChanged(new_modifiers) => {
                    modifiers = new_modifiers.state();
                }

                WindowEvent::CursorMoved { position, .. } => {
                    mouse_x = position.x as f32;
                    mouse_y = position.y as f32;

                    let size = window.inner_size();

                    if is_dragging_sidebar {
                        let new_width = if mouse_x < 30.0 { 0.0 } else { mouse_x.clamp(50.0, 600.0) };
                        ui.sidebar_width = new_width;
                        ui.target_sidebar_width = new_width;
                    } else if is_dragging_dock_border {
                        let main_y = ui.titlebar_height;
                        let max_y = size.height as f32 - ui.status_height - 50.0;
                        let min_y = main_y + ui.tabbar_height + ui.breadcrumb_height + 50.0;
                        let target_y = mouse_y.clamp(min_y, max_y);
                        let new_height = size.height as f32 - ui.status_height - target_y;
                        ui.dock_height = new_height;
                        
                        // Resize active terminal PTY
                        if !dock_terminals.is_empty() {
                            let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                            let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                            let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                            let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                            let active_term = &mut dock_terminals[active_terminal_idx];
                            active_term.grid.resize(cols, rows);
                            active_term.resize_pty(cols, rows);
                        }
                    } else if is_dragging_scroll {
                        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                        let status_y = (size.height as f32 - ui.status_height).round();
                        let editor_height = status_y - editor_top - 14.0;
                        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                        let ratio = visible_lines as f32 / tabs[active_tab_idx].buffer.len() as f32;
                        let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
                        let max_scroll = (tabs[active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0) as f32;
                        let relative_y = mouse_y - editor_top - scroll_drag_offset_y;
                        let scroll_range = editor_height - thumb_h;
                        let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                        ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                    } else if is_dragging_horizontal_scroll {
                        let max_line_digits = tabs[active_tab_idx].buffer.len().to_string().len().max(3);
                        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                        let text_area_x = ui.sidebar_width + gutter_width;
                        let scrollbar_width = ui.scrollbar_width();
                        let minimap_width = ui.minimap_width();
                        let sb_x = size.width as f32 - scrollbar_width;
                        let minimap_x = sb_x - minimap_width;
                        let text_viewport_w = (minimap_x - text_area_x).max(10.0);

                        let max_line_len = ui.get_max_line_len(&tabs[active_tab_idx].buffer, tabs[active_tab_idx].path.as_deref(), tabs[active_tab_idx].cursor.line);
                        let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
                        let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
                        let thumb_w = (text_viewport_w * ratio_x).clamp(20.0, text_viewport_w);
                        let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
                        let relative_x = mouse_x - text_area_x - scroll_drag_offset_x;
                        let scroll_range = text_viewport_w - thumb_w;
                        let scroll_ratio = if scroll_range > 0.0 { (relative_x / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                        ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
                    } else if is_dragging_minimap {
                        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                        let status_y = (size.height as f32 - ui.status_height).round();
                        let total_editor_height = status_y - editor_top;
                        let editor_height = total_editor_height - 14.0;
                        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                        let max_scroll = (tabs[active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0) as f32;
                        let relative_y = mouse_y - editor_top;
                        
                        let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
                        let minimap_total_h = tabs[active_tab_idx].buffer.len() as f32 * minimap_line_height;
                        
                        if minimap_total_h > total_editor_height {
                            let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
                            ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                        } else {
                            let line_idx = (relative_y / minimap_line_height).floor() as usize;
                            ui.scroll_y = line_idx.saturating_sub(visible_lines / 2).min(max_scroll as usize);
                        }
                    } else if is_dragging {
                        let max_line_digits = tabs[active_tab_idx].buffer.len().to_string().len().max(3);
                        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                        let text_area_x = ui.sidebar_width + gutter_width;
                        let scrollbar_width = ui.scrollbar_width();
                        let minimap_width = ui.minimap_width();
                        let sb_x = size.width as f32 - scrollbar_width;
                        let minimap_x = sb_x - minimap_width;

                        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                        let line_idx = if mouse_y >= editor_top {
                            ((mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y
                        } else {
                            ui.scroll_y
                        };
                        let line_idx = line_idx.min(tabs[active_tab_idx].buffer.len() - 1);

                        let mouse_x_clamped = mouse_x.min(minimap_x);
                        let col_idx = if mouse_x_clamped > text_area_x {
                            ((mouse_x_clamped - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x
                        } else {
                            0
                        };
                        let line_chars = tabs[active_tab_idx].buffer.lines()[line_idx].chars().count();
                        let col_idx = col_idx.min(line_chars);

                        tabs[active_tab_idx].cursor.line = line_idx;
                        tabs[active_tab_idx].cursor.col = col_idx;
                        tabs[active_tab_idx].cursor.intended_col = col_idx;

                        ui.scroll_to_cursor(&tabs[active_tab_idx].cursor, tabs[active_tab_idx].buffer.len(), size.width as f32, size.height as f32);
                    }

                    update_cursor_icon(&window, &ui, &tabs[active_tab_idx].buffer, mouse_x, mouse_y);
                    window.request_redraw();
                }

                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left {
                        let size = window.inner_size();
                        if state == ElementState::Pressed {
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
                            let is_titlebar_drag_zone = mouse_y < ui.titlebar_height
                                && mouse_x >= menu_width
                                && mouse_x < max_drag_x;

                            if is_titlebar_drag_zone {
                                let now = std::time::Instant::now();
                                let is_double_click = if let Some(last) = last_click_time {
                                    now.duration_since(last) < std::time::Duration::from_millis(300)
                                } else {
                                    false
                                };
                                last_click_time = Some(now);

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
                            if ui.show_dock && mouse_x >= ui.sidebar_width && mouse_y >= dock_start_y {
                                terminal_focus = true;
                            } else {
                                terminal_focus = false;
                            }

                            // Check if click is on dock resize border
                            let on_dock_border = ui.show_dock && (mouse_y - dock_start_y).abs() <= 4.0;

                            if on_dock_border {
                                is_dragging_dock_border = true;
                            } else if ui.active_modal.is_some() {
                                let tab_paths: Vec<Option<String>> = tabs.iter().map(|t| t.path.clone()).collect();
                                let tab_modified: Vec<bool> = tabs.iter().map(|t| t.buffer.is_modified).collect();
                                let action = {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    ui.handle_click(
                                        mouse_x,
                                        mouse_y,
                                        size.width as f32,
                                        size.height as f32,
                                        &mut active_tab.buffer,
                                        &mut active_tab.cursor,
                                        &tab_paths,
                                        &tab_modified,
                                        dock_terminals.len(),
                                    )
                                };
                                handle_action!(action);
                            } else {
                                // Check if click is on sidebar resize border
                                let on_sidebar_border = ui.sidebar_width > 0.0 && (mouse_x - ui.sidebar_width).abs() <= 4.0;
                                if on_sidebar_border {
                                    is_dragging_sidebar = true;
                                } else {
                                    let tab_paths: Vec<Option<String>> = tabs.iter().map(|t| t.path.clone()).collect();
                                    let tab_modified: Vec<bool> = tabs.iter().map(|t| t.buffer.is_modified).collect();
                                    let action = {
                                        let active_tab = &mut tabs[active_tab_idx];
                                        ui.handle_click(
                                            mouse_x,
                                            mouse_y,
                                            size.width as f32,
                                            size.height as f32,
                                            &mut active_tab.buffer,
                                            &mut active_tab.cursor,
                                            &tab_paths,
                                            &tab_modified,
                                            dock_terminals.len(),
                                        )
                                    };

                                    match action {
                                        UiAction::None => {
                                            let active_tab = &mut tabs[active_tab_idx];
                                            let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                                            let status_y = (size.height as f32 - ui.status_height).round();
                                            let editor_height = status_y - editor_top - 14.0;
                                            
                                            let max_line_digits = active_tab.buffer.len().to_string().len().max(3);
                                            let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                                            let text_area_x = ui.sidebar_width + gutter_width;
                                            let scrollbar_width = ui.scrollbar_width();
                                            let minimap_width = ui.minimap_width();
                                            let sb_x = size.width as f32 - scrollbar_width;
                                            let minimap_x = sb_x - minimap_width;
                                            let text_viewport_w = (minimap_x - text_area_x).max(10.0);

                                            // 1. Check if click is on minimap
                                            if mouse_x >= minimap_x && mouse_x < sb_x && mouse_y >= editor_top && mouse_y < size.height as f32 - ui.status_height {
                                                is_dragging_minimap = true;
                                                let total_editor_height = status_y - editor_top;
                                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                                let max_scroll = (active_tab.buffer.len() as isize - visible_lines as isize).max(0) as f32;
                                                let relative_y = mouse_y - editor_top;
                                                
                                                let minimap_line_height = (ui.buffer_font_size * 0.22).round().max(2.0);
                                                let minimap_total_h = active_tab.buffer.len() as f32 * minimap_line_height;
                                                
                                                if minimap_total_h > total_editor_height {
                                                    let scroll_ratio = (relative_y / total_editor_height).clamp(0.0, 1.0);
                                                    ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                                                } else {
                                                    let line_idx = (relative_y / minimap_line_height).floor() as usize;
                                                    ui.scroll_y = line_idx.saturating_sub(visible_lines / 2).min(max_scroll as usize);
                                                }
                                            }
                                            // 2. Check if click is on scrollbar
                                            else if mouse_x >= sb_x && mouse_y >= editor_top && mouse_y < size.height as f32 - ui.status_height {
                                                is_dragging_scroll = true;
                                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                                let ratio = visible_lines as f32 / active_tab.buffer.len() as f32;
                                                let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
                                                let max_scroll = (active_tab.buffer.len() as isize - visible_lines as isize).max(0) as f32;
                                                
                                                let scroll_ratio = if max_scroll > 0.0 { ui.scroll_y as f32 / max_scroll } else { 0.0 };
                                                let thumb_y = editor_top + scroll_ratio * (editor_height - thumb_h);
                                                
                                                if mouse_y >= thumb_y && mouse_y < thumb_y + thumb_h {
                                                    scroll_drag_offset_y = mouse_y - thumb_y;
                                                } else {
                                                    scroll_drag_offset_y = thumb_h / 2.0;
                                                    let relative_y = mouse_y - editor_top - scroll_drag_offset_y;
                                                    let scroll_range = editor_height - thumb_h;
                                                    let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                                                    ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                                                }
                                            }
                                            // 3. Check if click is on horizontal scrollbar
                                            else if mouse_x >= text_area_x && mouse_x < minimap_x && mouse_y >= size.height as f32 - ui.status_height - 14.0 && mouse_y < size.height as f32 - ui.status_height {
                                                is_dragging_horizontal_scroll = true;
                                                let max_line_len = ui.get_max_line_len(&active_tab.buffer, active_tab.path.as_deref(), active_tab.cursor.line);
                                                let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;
                                                let ratio_x = visible_cols as f32 / max_line_len.max(1) as f32;
                                                let thumb_w = (text_viewport_w * ratio_x).clamp(20.0, text_viewport_w);
                                                let max_scroll_x = (max_line_len as isize - visible_cols as isize).max(0) as f32;
                                                
                                                let scroll_ratio_x = if max_scroll_x > 0.0 { ui.scroll_x as f32 / max_scroll_x } else { 0.0 };
                                                let thumb_x = text_area_x + scroll_ratio_x * (text_viewport_w - thumb_w);
                                                
                                                if mouse_x >= thumb_x && mouse_x < thumb_x + thumb_w {
                                                    scroll_drag_offset_x = mouse_x - thumb_x;
                                                } else {
                                                    scroll_drag_offset_x = thumb_w / 2.0;
                                                    let relative_x = mouse_x - text_area_x - scroll_drag_offset_x;
                                                    let scroll_range = text_viewport_w - thumb_w;
                                                    let scroll_ratio = if scroll_range > 0.0 { (relative_x / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                                                    ui.scroll_x = (scroll_ratio * max_scroll_x).round() as usize;
                                                }
                                            } else {
                                                 // Click inside editor area
                                                 if mouse_x >= text_area_x && mouse_x < minimap_x && mouse_y >= editor_top && mouse_y < size.height as f32 - ui.status_height - 14.0 {
                                                     active_tab.buffer.commit_transaction();
                                                     is_dragging = true;

                                                     let line_idx = ((mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y;
                                                     let line_idx = line_idx.min(active_tab.buffer.len() - 1);

                                                     let col_idx = ((mouse_x - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x;
                                                     let line_chars = active_tab.buffer.lines()[line_idx].chars().count();
                                                     let col_idx = col_idx.min(line_chars);

                                                     let extend_selection = modifiers.shift_key();
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
                                        act => {
                                            handle_action!(act);
                                        }
                                    }
                                }
                            }
                        } else {
                            let was_dragging_sidebar = is_dragging_sidebar;
                            is_dragging = false;
                            is_dragging_scroll = false;
                            is_dragging_horizontal_scroll = false;
                            is_dragging_minimap = false;
                            is_dragging_sidebar = false;
                            is_dragging_dock_border = false;
                            if was_dragging_sidebar {
                                ui.config.sidebar_width = ui.sidebar_width;
                                ui.config.save_in_background();
                            }
                            if let Some((s_l, s_c, e_l, e_c)) = tabs[active_tab_idx].cursor.selection_range() {
                                if s_l == e_l && s_c == e_c {
                                    tabs[active_tab_idx].cursor.clear_selection();
                                }
                            }
                        }
                        update_cursor_icon(&window, &ui, &tabs[active_tab_idx].buffer, mouse_x, mouse_y);
                        window.request_redraw();
                    }
                }

                WindowEvent::MouseWheel { delta, .. } => {
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

                    // Handle Sidebar Scroll
                    let size = window.inner_size();
                    let sidebar_top = ui.titlebar_height;
                    let sidebar_bottom = size.height as f32 - ui.status_height;
                    if ui.sidebar_width > 0.0 && mouse_x >= 0.0 && mouse_x < ui.sidebar_width && mouse_y >= sidebar_top && mouse_y < sidebar_bottom {
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

                    let is_shift = modifiers.shift_key();
                    if is_shift {
                        let scroll_cols = match delta {
                            MouseScrollDelta::LineDelta(dx, _) if dx != 0.0 => -dx as isize * 3,
                            MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
                            MouseScrollDelta::PixelDelta(pos) => {
                                let val = if pos.x.abs() > pos.y.abs() { pos.x } else { pos.y };
                                ((val / (ui.buffer_char_width as f64)) * 3.0) as isize * -1
                            }
                        };
                        let max_line_digits = tabs[active_tab_idx].buffer.len().to_string().len().max(3);
                        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                        let text_area_x = ui.sidebar_width + gutter_width;
                        let scrollbar_width = ui.scrollbar_width();
                        let minimap_width = ui.minimap_width();
                        let size = window.inner_size();
                        let sb_x = size.width as f32 - scrollbar_width;
                        let minimap_x = sb_x - minimap_width;
                        let text_viewport_w = (minimap_x - text_area_x).max(10.0);
                        let visible_cols = (text_viewport_w / ui.buffer_char_width).floor() as usize;

                        let max_line_len = tabs[active_tab_idx].buffer.lines().iter().map(|l| l.chars().count()).max().unwrap_or(0);
                        let max_scroll = (max_line_len as isize - visible_cols as isize).max(0);
                        let new_scroll = ui.scroll_x as isize + scroll_cols;
                        ui.scroll_x = new_scroll.clamp(0, max_scroll) as usize;
                        tabs[active_tab_idx].scroll_x = ui.scroll_x;
                        window.request_redraw();
                        return;
                    }

                    let scroll_lines = match delta {
                        MouseScrollDelta::LineDelta(_, dy) => -dy as isize * 3,
                        MouseScrollDelta::PixelDelta(pos) => ((pos.y / (ui.buffer_line_height as f64)) * 3.0) as isize * -1,
                    };

                    let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                    let status_y = (window.inner_size().height as f32 - ui.status_height).round();
                    let editor_height = status_y - editor_top - 14.0;
                    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                    let max_scroll = (tabs[active_tab_idx].buffer.len() as isize - visible_lines as isize).max(0);

                    let new_scroll = ui.scroll_y as isize + scroll_lines;
                    ui.scroll_y = new_scroll.clamp(0, max_scroll) as usize;

                    window.request_redraw();
                }

                 WindowEvent::KeyboardInput { event: kb_event, .. } => {
                    if terminal_focus && !dock_terminals.is_empty() {
                        if kb_event.state == ElementState::Pressed {
                            let active_term = &mut dock_terminals[active_terminal_idx];
                            let bytes_to_write: Option<Vec<u8>> = match &kb_event.logical_key {
                                Key::Character(text) => {
                                    let ctrl = modifiers.control_key();
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
                            }
                            window.request_redraw();
                        }
                        return;
                    }

                    if kb_event.state == ElementState::Pressed {
                        let ctrl = modifiers.control_key();
                        let shift = modifiers.shift_key();
                        let alt = modifiers.alt_key();

                        // 1. If CommandPalette modal is active, handle it specifically
                        if let Some(crate::ui::ModalType::CommandPalette) = ui.active_modal {
                            match &kb_event.logical_key {
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
                                        
                                        let action = {
                                             let active_tab = &mut tabs[active_tab_idx];
                                             ui.execute_command(cmd, &mut active_tab.buffer, &mut active_tab.cursor)
                                         };
                                         handle_action!(action);
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
                            if let Key::Named(NamedKey::Escape) = &kb_event.logical_key {
                                ui.active_modal = None;
                                window.request_redraw();
                            }
                            return;
                        }

                        // 3. Otherwise map key input to Action
                        if let Some(action) = crate::editor::keymap::map_key(&kb_event.logical_key, kb_event.physical_key, ctrl, shift, alt) {
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
                                    handle_action!(UiAction::SaveFile);
                                }
                                crate::editor::actions::Action::Escape => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    active_tab.cursor.clear_selection();
                                }
                                crate::editor::actions::Action::MoveLeft { select, word } => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    if word {
                                        active_tab.cursor.move_word_left(&active_tab.buffer, select);
                                    } else {
                                        active_tab.cursor.move_left(&active_tab.buffer, select);
                                    }
                                }
                                crate::editor::actions::Action::MoveRight { select, word } => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    if word {
                                        active_tab.cursor.move_word_right(&active_tab.buffer, select);
                                    } else {
                                        active_tab.cursor.move_right(&active_tab.buffer, select);
                                    }
                                }
                                crate::editor::actions::Action::MoveUp { select } => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    active_tab.cursor.move_up(&active_tab.buffer, select);
                                }
                                crate::editor::actions::Action::MoveDown { select } => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    active_tab.cursor.move_down(&active_tab.buffer, select);
                                }
                                crate::editor::actions::Action::MoveToLineStart { select } => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    active_tab.cursor.move_to_line_start(select);
                                }
                                crate::editor::actions::Action::MoveToLineEnd { select } => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    active_tab.cursor.move_to_line_end(&active_tab.buffer, select);
                                }
                                crate::editor::actions::Action::SelectAll => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    active_tab.buffer.commit_transaction();
                                    active_tab.cursor.selection_anchor = Some((0, 0));
                                    active_tab.cursor.line = active_tab.buffer.len() - 1;
                                    active_tab.cursor.col = active_tab.buffer.lines()[active_tab.cursor.line].chars().count();
                                    active_tab.cursor.intended_col = active_tab.cursor.col;
                                }
                                crate::editor::actions::Action::Copy => {
                                    let active_tab = &tabs[active_tab_idx];
                                    if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                                        internal_clipboard = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
                                    }
                                }
                                crate::editor::actions::Action::Cut => {
                                    let active_tab = &mut tabs[active_tab_idx];
                                    if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                                        internal_clipboard = active_tab.buffer.get_range_text(s_l, s_c, e_l, e_c);
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
                                    if !internal_clipboard.is_empty() {
                                        let active_tab = &mut tabs[active_tab_idx];
                                        active_tab.buffer.start_transaction();
                                        if let Some((s_l, s_c, e_l, e_c)) = active_tab.cursor.selection_range() {
                                            active_tab.buffer.delete(s_l, s_c, e_l, e_c);
                                            active_tab.cursor.line = s_l;
                                            active_tab.cursor.col = s_c;
                                            active_tab.cursor.clear_selection();
                                        }
                                        active_tab.buffer.insert(active_tab.cursor.line, active_tab.cursor.col, &internal_clipboard);

                                        let parts = internal_clipboard.split('\n').collect::<Vec<&str>>();
                                        if parts.len() == 1 {
                                            active_tab.cursor.col += internal_clipboard.chars().count();
                                        } else {
                                            active_tab.cursor.line += parts.len() - 1;
                                            active_tab.cursor.col = parts.last().unwrap().chars().count();
                                        }
                                        active_tab.cursor.intended_col = active_tab.cursor.col;
                                        active_tab.buffer.commit_transaction();
                                    }
                                }
                                crate::editor::actions::Action::Undo => {
                                    handle_action!(UiAction::Undo);
                                    tabs[active_tab_idx].cursor.intended_col = tabs[active_tab_idx].cursor.col;
                                }
                                crate::editor::actions::Action::Redo => {
                                    handle_action!(UiAction::Redo);
                                    tabs[active_tab_idx].cursor.intended_col = tabs[active_tab_idx].cursor.col;
                                }
                                crate::editor::actions::Action::DeleteLeft => {
                                    let active_tab = &mut tabs[active_tab_idx];
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
                                    let active_tab = &mut tabs[active_tab_idx];
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
                                    let active_tab = &mut tabs[active_tab_idx];
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
                                    let active_tab = &mut tabs[active_tab_idx];
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
                                    let active_tab = &mut tabs[active_tab_idx];
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
                        
                        let active_tab = &tabs[active_tab_idx];
                        ui.scroll_to_cursor(&active_tab.cursor, active_tab.buffer.len(), window.inner_size().width as f32, window.inner_size().height as f32);
                        update_cursor_icon(&window, &ui, &active_tab.buffer, mouse_x, mouse_y);
                        window.request_redraw();
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                if terminal_focus {
                    elwt.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + std::time::Duration::from_millis(15)
                    ));
                    window.request_redraw();
                } else {
                    elwt.set_control_flow(ControlFlow::Wait);
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}