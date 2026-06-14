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
        builder.filter(Some("wgpu_hal"), log::LevelFilter::Error);
        builder.filter(Some("wgpu_core"), log::LevelFilter::Error);
    }
    builder.init();

    let event_loop = EventLoop::new()?;
    
    // Spawn background thread immediately to pre-load Vulkan / GL drivers and config/fonts
    let (init_tx, init_rx) = std::sync::mpsc::channel::<(GpuContext, FontAtlas, UiState)>();
    let (window_tx, window_rx) = std::sync::mpsc::channel::<Arc<winit::window::Window>>();
    
    let font_bytes = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");
    let proxy = event_loop.create_proxy();
    let proxy_clone = proxy.clone();
    
    std::thread::spawn(move || {
        let mut config = crate::editor::config::AppConfig::load();
        crate::experiments::startup::record_step("Config Load");

        let initial_backends = match config.backend.as_str() {
            "Vulkan" => Some(wgpu::Backends::VULKAN),
            "OpenGL" => Some(wgpu::Backends::GL),
            _ => None,
        };

        // Pre-create the instance (loads Vulkan shared library in parallel with winit Window creation!)
        let instance_backends = initial_backends.unwrap_or(wgpu::Backends::all());
        let flags = (wgpu::InstanceFlags::default() | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER)
            & !wgpu::InstanceFlags::VALIDATION & !wgpu::InstanceFlags::DEBUG;
        let instance = Arc::new(wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: instance_backends,
            flags,
            ..Default::default()
        }));

        // Wait for window to be created on the main thread
        let window = match window_rx.recv() {
            Ok(w) => w,
            Err(_) => return,
        };

        let mut gpu = pollster::block_on(GpuContext::new_with_instance(window, initial_backends, Some(instance)));
        crate::experiments::startup::record_step("WGPU Context Creation");

        let actual_backend_str = match gpu.backend {
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

        // Initialize Font Atlas using wgpu device/queue
        let mut atlas = match FontAtlas::new(&gpu.device, &gpu.queue, font_bytes) {
            Ok(a) => a,
            Err(e) => {
                log::error!("Failed to create FontAtlas: {:?}", e);
                panic!("FontAtlas creation failed: {:?}", e);
            }
        };

        // Update bind group to use actual font texture and sampler
        gpu.update_bind_group(&atlas.texture, &atlas.sampler);
        crate::experiments::startup::record_step("Font Atlas & Texture upload");

        // Initialize layout and state
        let mut ui = UiState::new(&mut atlas, &gpu.queue, config, proxy_clone.clone());
        ui.active_device_name = gpu.device_name.clone();
        crate::experiments::startup::record_step("UI State Initialization");

        let _ = init_tx.send((gpu, atlas, ui));
        let _ = proxy_clone.send_event(());
    });

    // Build the window on the main thread in parallel with Vulkan driver loading
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Garage")
            .with_decorations(false)
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 800))
            .with_visible(true)
            .build(&event_loop)?,
    );
    crate::experiments::startup::record_step("Window Creation");

    // Pass the window to the background thread
    let _ = window_tx.send(window.clone());

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
    crate::experiments::startup::record_step("Initial File Load & State Setup");

    // Track dynamic vertices and indices
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut first_frame_rendered = false;

    let mut gpu: Option<GpuContext> = None;
    let mut atlas: Option<FontAtlas> = None;
    let mut ui: Option<UiState> = None;

    window.set_visible(true);
    crate::experiments::startup::record_step("Window Visibility Set");

    let mut redraw_requested = false;

    // Run the event loop reactively to save power/CPU/GPU cycles when idle
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::NewEvents(winit::event::StartCause::Init) => {
                redraw_requested = true;
            }

            Event::UserEvent(()) => {
                if let Ok((g, a, mut u)) = init_rx.try_recv() {
                    if let Some(ref path) = state.tabs[0].path {
                        u.selected_file = Some(std::path::PathBuf::from(path));
                    }
                    gpu = Some(g);
                    atlas = Some(a);
                    ui = Some(u);

                    // Render the first frame immediately!
                    let gpu_ref = gpu.as_mut().unwrap();
                    let ui_ref = ui.as_mut().unwrap();
                    let atlas_ref = atlas.as_mut().unwrap();
                    let size = window.inner_size();
                    
                    vertices.clear();
                    indices.clear();
                    
                    let tab_paths: Vec<Option<String>> = state.tabs.iter().map(|t| t.path.clone()).collect();
                    let tab_modified: Vec<bool> = state.tabs.iter().map(|t| t.buffer.is_modified).collect();
                    
                    ui_ref.build_frame(
                        &mut vertices,
                        &mut indices,
                        atlas_ref,
                        &gpu_ref.queue,
                        &state.tabs[state.active_tab_idx].buffer,
                        &state.tabs[state.active_tab_idx].cursor,
                        size.width as f32,
                        size.height as f32,
                        state.mouse_x,
                        state.mouse_y,
                        gpu_ref.backend,
                        &tab_paths,
                        &tab_modified,
                        state.active_tab_idx,
                        state.dragged_tab_idx,
                        &state.inactive_panes,
                        state.active_pane_idx,
                        &state.dock_terminals,
                        state.active_terminal_idx,
                        state.terminal_focus,
                        window.is_maximized(),
                    );
                    
                    if let Err(e) = gpu_ref.render(&vertices, &indices) {
                        log::error!("First frame rendering error: {:?}", e);
                    } else {
                        first_frame_rendered = true;
                        crate::experiments::startup::report_startup_complete();
                    }
                }
                redraw_requested = true;
            }

            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                if let WindowEvent::CloseRequested = event {
                    elwt.exit();
                    return;
                }

                if gpu.is_none() || ui.is_none() || atlas.is_none() {
                    return;
                }

                let gpu_ref = gpu.as_mut().unwrap();
                let ui_ref = ui.as_mut().unwrap();
                let atlas_ref = atlas.as_mut().unwrap();

                match event {
                    WindowEvent::CloseRequested => unreachable!(),

                    WindowEvent::Resized(physical_size) => {
                        gpu_ref.resize(physical_size);
                        redraw_requested = true;
                    }

                    WindowEvent::ScaleFactorChanged { .. } => {
                        let physical_size = window.inner_size();
                        gpu_ref.resize(physical_size);
                        redraw_requested = true;
                    }

                    WindowEvent::RedrawRequested => {
                        redraw_requested = true;
                    }

                    WindowEvent::ModifiersChanged(new_modifiers) => {
                        state.modifiers = new_modifiers.state();
                    }

                    WindowEvent::CursorMoved { position, .. } => {
                        input::handle_cursor_moved(
                            ui_ref,
                            &mut state,
                            &window,
                            position.x as f32,
                            position.y as f32,
                        );
                        redraw_requested = true;
                    }

                    WindowEvent::MouseInput { state: input_state, button, .. } => {
                        input::handle_mouse_input(
                            ui_ref,
                            &mut state,
                            &mut window.clone(),
                            elwt,
                            &mut gpu,
                            atlas_ref,
                            font_bytes,
                            input_state,
                            button,
                        );
                        redraw_requested = true;
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        input::handle_mouse_wheel(ui_ref, &mut state, &window, delta);
                        redraw_requested = true;
                    }

                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if key_event.state == winit::event::ElementState::Pressed {
                            input::handle_keyboard_input(
                                ui_ref,
                                &mut state,
                                &mut window.clone(),
                                elwt,
                                &mut gpu,
                                atlas_ref,
                                font_bytes,
                                key_event.logical_key,
                                key_event.physical_key,
                            );

                            if !state.terminal_focus {
                                let active_tab = &state.tabs[state.active_tab_idx];
                                if let Some(ref path) = active_tab.path {
                                    let abs_path = crate::editor::get_absolute_path(path);
                                    ui_ref.diagnostics_file_cache.insert(abs_path, active_tab.buffer.lines().to_vec());
                                }
                            }
                            redraw_requested = true;
                        }
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                if !first_frame_rendered && gpu.is_some() {
                    redraw_requested = true;
                }
                
                if gpu.is_some() && ui.is_some() && atlas.is_some() {
                    let ui_ref = ui.as_mut().unwrap();
                    let gpu_ref = gpu.as_mut().unwrap();
                    let atlas_ref = atlas.as_mut().unwrap();

                    // Throttled git branch, status and diff check (every 1 second)
                    if ui_ref.last_branch_check.is_none() || ui_ref.last_branch_check.unwrap().elapsed() > std::time::Duration::from_secs(1) {
                        if ui_ref.config.show_git_branch {
                            ui_ref.update_git_branch();
                        }
                        ui_ref.update_git_statuses();
                        if state.active_tab_idx < state.tabs.len() {
                            if let Some(ref file_path) = state.tabs[state.active_tab_idx].path {
                                ui_ref.update_git_diff(Some(file_path));
                            }
                        }
                        ui_ref.last_branch_check = Some(std::time::Instant::now());
                    }

                    if state.active_tab_idx < state.tabs.len() {
                        if let Some(ref file_path) = state.tabs[state.active_tab_idx].path {
                            if !ui_ref.git_file_blames.contains_key(file_path) {
                                ui_ref.update_git_file_blame(Some(file_path));
                            }
                        }
                    }

                    // Drain Tree scan channel
                    if let Some(ref rx) = ui_ref.tree_rx {
                        while let Ok(nodes) = rx.try_recv() {
                            ui_ref.visible_nodes = nodes;
                            redraw_requested = true;
                        }
                    }

                    // Drain diagnostics file channel
                    if let Some(ref rx) = ui_ref.diagnostics_file_rx {
                        while let Ok((path, lines)) = rx.try_recv() {
                            ui_ref.diagnostics_file_cache.insert(path, lines);
                            redraw_requested = true;
                        }
                    }

                    // Read data from PTY channels for all terminals and parse ANSI sequences using persistent parser
                    for term in &mut state.dock_terminals {
                        while let Ok(bytes) = term.rx.try_recv() {
                            for b in bytes {
                                term.parser.advance(&mut term.grid, b);
                            }
                            redraw_requested = true;
                        }
                    }

                    if redraw_requested {
                        let size = window.inner_size();
                        if ui_ref.show_dock && !state.dock_terminals.is_empty() && !state.is_dragging_sidebar && !state.is_dragging_dock_border {
                            let width_content = size.width as f32 - ui_ref.sidebar_width - 16.0;
                            let height_content = ui_ref.dock_height - 28.0 - 1.0 - 12.0;
                            let cols = (width_content / ui_ref.buffer_char_width).floor().max(10.0) as usize;
                            let rows = (height_content / ui_ref.buffer_line_height).floor().max(2.0) as usize;
                            let active_term = &mut state.dock_terminals[state.active_terminal_idx];
                            if active_term.grid.cols != cols || active_term.grid.rows != rows {
                                active_term.grid.resize(cols, rows);
                                active_term.resize_pty(cols, rows);
                            }
                        }

                        // Sync active tab scroll offsets
                        state.tabs[state.active_tab_idx].scroll_x = ui_ref.scroll_x;
                        state.tabs[state.active_tab_idx].scroll_y = ui_ref.scroll_y;

                        // Clear dynamic buffers
                        vertices.clear();
                        indices.clear();

                        // Sync active tab buffers to diagnostics file cache
                        for tab in &state.tabs {
                            if let Some(ref path) = tab.path {
                                if !path.starts_with("diagnostics://") {
                                    let abs_path = crate::editor::get_absolute_path(path);
                                    let tab_revision = tab.buffer.revision;
                                    let mut needs_update = false;
                                    if let Some(&synced_rev) = ui_ref.synced_revisions.get(&abs_path) {
                                        if synced_rev != tab_revision {
                                            needs_update = true;
                                        }
                                    } else {
                                        needs_update = true;
                                    }
                                    if needs_update {
                                        let tab_lines = tab.buffer.lines();
                                        ui_ref.diagnostics_file_cache.insert(abs_path.clone(), tab_lines.to_vec());
                                        ui_ref.synced_revisions.insert(abs_path, tab_revision);
                                        ui_ref.diagnostics_changed = true;
                                    }
                                }
                            }
                        }

                        // Rebuild and sync the diagnostics://project tab buffer itself to match visual diagnostic lines
                        if ui_ref.diagnostics_changed {
                            ui_ref.diagnostics_changed = false;
                            if let Some(diag_tab_idx) = state.tabs.iter().position(|t| t.path.as_deref() == Some("diagnostics://project")) {
                                let visual_lines = crate::ui::components::editor::text_area::get_visual_diagnostic_lines(ui_ref);
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
                        ui_ref.build_frame(
                            &mut vertices,
                            &mut indices,
                            atlas_ref,
                            &gpu_ref.queue,
                            &state.tabs[state.active_tab_idx].buffer,
                            &state.tabs[state.active_tab_idx].cursor,
                            size.width as f32,
                            size.height as f32,
                            state.mouse_x,
                            state.mouse_y,
                            gpu_ref.backend,
                            &tab_paths,
                            &tab_modified,
                            state.active_tab_idx,
                            state.dragged_tab_idx,
                            &state.inactive_panes,
                            state.active_pane_idx,
                            &state.dock_terminals,
                            state.active_terminal_idx,
                            state.terminal_focus,
                            window.is_maximized(),
                        );

                        // Update cursor icon when screen redraws
                        input::update_cursor_icon(
                            &window,
                            ui_ref,
                            &state,
                        );

                        // Render to swapchain
                        if let Err(e) = gpu_ref.render(&vertices, &indices) {
                            log::error!("Rendering error: {:?}", e);
                        } else {
                            if !first_frame_rendered {
                                first_frame_rendered = true;
                                crate::experiments::startup::report_startup_complete();
                            }
                        }

                        redraw_requested = false;
                    }
                }

                let scheduled_wakeup = false;

                if !scheduled_wakeup {
                    if state.terminal_focus && gpu.is_some() {
                        elwt.set_control_flow(ControlFlow::WaitUntil(
                            std::time::Instant::now() + std::time::Duration::from_millis(15)
                        ));
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




