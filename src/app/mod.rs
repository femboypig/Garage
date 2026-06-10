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
use crate::renderer::gpu::{GpuContext, Vertex};
use crate::ui::{UiState, UiAction};

use self::state::{AppState, Tab};

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

    // Load bundled IBM Plex Mono font bytes (relative to this file: src/app/mod.rs)
    let font_bytes = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

    // Initialize Font Atlas using wgpu device/queue
    let mut atlas = FontAtlas::new(&gpu.as_ref().unwrap().device, &gpu.as_ref().unwrap().queue, font_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Update bind group to use actual font texture and sampler
    gpu.as_mut().unwrap().update_bind_group(&atlas.texture, &atlas.sampler);

    // Initialize layout and state
    let mut ui = UiState::new(&mut atlas, &gpu.as_ref().unwrap().queue, config);
    ui.active_device_name = gpu.as_ref().unwrap().device_name.clone();

    // Load initial file or start with empty tab
    let initial_tab = {
        let mut buffer = Buffer::new();
        let save_path = if let Some(ref path) = file_path {
            if let Err(e) = buffer.load_file(path) {
                log::warn!("Failed to load file '{}': {}. Starting with empty buffer.", path, e);
            }
            Some(path.clone())
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

    // Create default initial terminal with correct dimensions matching the startup size of the dock area
    let initial_term_size = window.inner_size();
    let initial_width_content = initial_term_size.width as f32 - ui.sidebar_width - 16.0;
    let initial_height_content = ui.dock_height - 28.0 - 1.0 - 12.0;
    let initial_cols = (initial_width_content / ui.buffer_char_width).floor().max(10.0) as usize;
    let initial_rows = (initial_height_content / ui.buffer_line_height).floor().max(2.0) as usize;

    if let Ok(term) = crate::terminal::TerminalInstance::new(initial_cols, initial_rows, window.clone()) {
        state.dock_terminals.push(term);
    }

    // Track dynamic vertices and indices
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

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
                    if ui.show_dock && !state.dock_terminals.is_empty() {
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

                    // Read data from PTY channels for all terminals and parse ANSI sequences
                    for term in &mut state.dock_terminals {
                        let mut parser = vte::Parser::new();
                        while let Ok(bytes) = term.rx.try_recv() {
                            for b in bytes {
                                parser.advance(&mut term.grid, b);
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
                }
                _ => {}
            },
            Event::AboutToWait => {
                if state.terminal_focus {
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
