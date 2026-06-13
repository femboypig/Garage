pub mod handler;
pub mod input;
pub mod state;

use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::{GpuContext, Vertex};
use crate::ui::UiState;

use self::state::{AppState, Tab};

pub fn run_editor(file_path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = env_logger::Builder::from_default_env();
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Warn);
    }
    builder.init();

    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Garage")
            .with_decorations(false)
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 800))
            // .with_visible(false)
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
    // window.set_visible(true);

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

    // Load bundled IBM Plex Mono font bytes (relative to this file: src/app/mod.rs)
    let font_bytes = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

    // Initialize Font Atlas using wgpu device/queue
    let mut atlas = FontAtlas::new(&gpu.as_ref().unwrap().device, &gpu.as_ref().unwrap().queue, font_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Update bind group to use actual font texture and sampler
    gpu.as_mut().unwrap().update_bind_group(&atlas.texture, &atlas.sampler);


    let proxy = event_loop.create_proxy();

    // Initialize layout and state
    let mut ui = UiState::new(&mut atlas, &gpu.as_ref().unwrap().queue, config, proxy.clone());
    ui.active_device_name = gpu.as_ref().unwrap().device_name.clone();

    // Load initial file or start with empty tab
    let initial_tab = {
        let mut buffer = Buffer::new();
        let save_path = if let Some(ref path) = file_path {
            if !path.starts_with("diagnostics://") {
                if let Err(e) = buffer.load_file(path) {
                    log::warn!("Failed to load file '{}': {}. Starting with empty buffer.", path, e);
                }
            }
            if path.starts_with("diagnostics://") {
                Some(path.clone())
            } else {
                let path_buf = std::path::PathBuf::from(path);
                let normalized = if path_buf.is_absolute() {
                    let current_dir = std::env::current_dir().unwrap_or_default();
                    if let Ok(rel) = path_buf.strip_prefix(&current_dir) {
                        rel.to_path_buf()
                    } else {
                        path_buf
                    }
                } else {
                    path_buf
                };
                let normalized = crate::editor::normalize_path(&normalized);
                Some(normalized.to_string_lossy().to_string())
            }
        } else {
            None
        };
        Tab {
            path: save_path,
            buffer,
            cursor: Cursor::new(),
            scroll_x: 0,
            scroll_y: 0,
        }
    };

    let mut state = AppState::new(initial_tab);
    if let Some(ref path) = state.tabs[0].path {
        ui.selected_file = Some(std::path::PathBuf::from(path));
    }

    // Track dynamic vertices and indices
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut first_frame_rendered = false;

    // Run the event loop reactively to save power/CPU/GPU cycles when idle
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

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
                    if ui.show_dock && !state.dock_terminals.is_empty() && !state.is_dragging_sidebar && !state.is_dragging_dock_border {
                        let width_content = size.width as f32 - ui.sidebar_width - 16.0;
                        let height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
                        let cols = (width_content / ui.buffer_char_width).floor().max(10.0) as usize;
                        let rows = (height_content / ui.buffer_line_height).floor().max(2.0) as usize;
                        let active_term = &mut state.dock_terminals[state.active_terminal_idx];
                        if active_term.grid.cols != cols || active_term.grid.rows != rows {
                            active_term.grid.resize(cols, rows);
                            active_term.resize_pty(cols, rows);
                        }
                    }


                    // Drain Tree scan channel
                    if let Some(ref rx) = ui.tree_rx {
                        while let Ok(nodes) = rx.try_recv() {
                            ui.visible_nodes = nodes;
                        }
                    }

                    // Read data from PTY channels for all terminals and parse ANSI sequences using persistent parser
                    for term in &mut state.dock_terminals {
                        while let Ok(bytes) = term.rx.try_recv() {
                            for b in bytes {
                                term.parser.advance(&mut term.grid, b);
                            }
                        }
                    }

                    // Sync active tab scroll offsets
                    state.tabs[state.active_tab_idx].scroll_x = ui.scroll_x;
                    state.tabs[state.active_tab_idx].scroll_y = ui.scroll_y;

                    // Clear dynamic buffers
                    vertices.clear();
                    indices.clear();

                    let size = window.inner_size();
                    
                    // Sync active tab buffers to diagnostics file cache
                    for tab in &state.tabs {
                        if let Some(ref path) = tab.path {
                            if !path.starts_with("diagnostics://") {
                                let abs_path = crate::editor::get_absolute_path(path);
                                let tab_lines = tab.buffer.lines();
                                let mut needs_update = false;
                                if let Some(cached) = ui.diagnostics_file_cache.get(&abs_path) {
                                    if cached.len() != tab_lines.len() || cached != tab_lines {
                                        needs_update = true;
                                    }
                                } else {
                                    needs_update = true;
                                }
                                if needs_update {
                                    ui.diagnostics_file_cache.insert(abs_path, tab_lines.to_vec());
                                    ui.diagnostics_changed = true;
                                }
                            }
                        }
                    }

                    // Rebuild and sync the diagnostics://project tab buffer itself to match visual diagnostic lines
                    if ui.diagnostics_changed {
                        ui.diagnostics_changed = false;
                        if let Some(diag_tab_idx) = state.tabs.iter().position(|t| t.path.as_deref() == Some("diagnostics://project")) {
                            let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(&mut ui);
                            let mut text_lines = Vec::new();
                            for vl in &visual_lines {
                                match vl {
                                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Header { path, line, col } => {
                                        text_lines.push(format!("▶ {} (Line {}, Col {})", path, line + 1, col + 1));
                                    }
                                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => {
                                        text_lines.push(line_content.clone());
                                    }
                                    crate::ui::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => {
                                        text_lines.push(format!("  └─ [{}] {}", match diag.severity { 1 => "Error", 2 => "Warning", 3 => "Info", _ => "Hint" }, diag.message));
                                    }
                                }
                            }
                            if text_lines.is_empty() {
                                text_lines.push("No problems found in the workspace".to_string());
                            }
                            let new_text = text_lines.join("\n");
                            let current_text = state.tabs[diag_tab_idx].buffer.lines().join("\n");
                            if current_text != new_text {
                                state.tabs[diag_tab_idx].buffer = crate::editor::buffer::Buffer::from_text(&new_text);
                            }
                        }
                    }

                    let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                    let tab_modified: Vec<bool> = state.tabs.iter().map(|t| t.buffer.is_modified).collect();

                    // Compile the current editor frame into GPU vertices
                    ui.build_frame(
                        &mut vertices,
                        &mut indices,
                        &mut atlas,
                        &gpu.as_ref().unwrap().queue,
                        &state.tabs[state.active_tab_idx].buffer,
                        &state.tabs[state.active_tab_idx].cursor,
                        size.width as f32,
                        size.height as f32,
                        state.mouse_x,
                        state.mouse_y,
                        gpu.as_ref().unwrap().backend,
                        &tab_paths,
                        &tab_modified,
                        state.active_tab_idx,
                        &state.dock_terminals,
                        state.active_terminal_idx,
                        state.terminal_focus,
                        window.is_maximized(),
                    );

                    // Update cursor icon when screen redraws
                    input::update_cursor_icon(
                        &window,
                        &ui,
                        &state.tabs[state.active_tab_idx].buffer,
                        state.mouse_x,
                        state.mouse_y,
                    );

                    // Render to swapchain
                    if let Err(e) = gpu.as_mut().unwrap().render(&vertices, &indices) {
                        log::error!("Rendering error: {:?}", e);
                    } else {
                        first_frame_rendered = true;
                    }
                }

                WindowEvent::ModifiersChanged(new_modifiers) => {
                    state.modifiers = new_modifiers.state();
                }

                WindowEvent::CursorMoved { position, .. } => {
                    input::handle_cursor_moved(
                        &mut ui,
                        &mut state,
                        &window,
                        position.x as f32,
                        position.y as f32,
                    );
                    window.request_redraw();
                }

                WindowEvent::MouseInput { state: input_state, button, .. } => {
                    input::handle_mouse_input(
                        &mut ui,
                        &mut state,
                        &mut window.clone(),
                        elwt,
                        &mut gpu,
                        &mut atlas,
                        font_bytes,
                        input_state,
                        button,
                    );
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    input::handle_mouse_wheel(&mut ui, &mut state, &window, delta);
                }

                WindowEvent::KeyboardInput { event: key_event, .. } => {
                    if key_event.state == winit::event::ElementState::Pressed {
                        input::handle_keyboard_input(
                            &mut ui,
                            &mut state,
                            &mut window.clone(),
                            elwt,
                            &mut gpu,
                            &mut atlas,
                            font_bytes,
                            key_event.logical_key,
                            key_event.physical_key,
                        );

                        if !state.terminal_focus {
                            let active_tab = &state.tabs[state.active_tab_idx];
                            if let Some(ref path) = active_tab.path {
                                let abs_path = crate::editor::get_absolute_path(path);
                                ui.diagnostics_file_cache.insert(abs_path, active_tab.buffer.lines().to_vec());
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::UserEvent(()) => {
                window.request_redraw();
            }
            Event::AboutToWait => {
                if !first_frame_rendered {
                    window.request_redraw();
                }
                
                let scheduled_wakeup = false;

                if !scheduled_wakeup {
                    if state.terminal_focus {
                        elwt.set_control_flow(ControlFlow::WaitUntil(
                            std::time::Instant::now() + std::time::Duration::from_millis(15)
                        ));
                        window.request_redraw();
                    } else {
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}

fn is_hidden(path: &std::path::Path) -> bool {
    path.components().any(|comp| {
        if let std::path::Component::Normal(name) = comp {
            name.to_string_lossy().starts_with('.')
        } else {
            false
        }
    })
}


