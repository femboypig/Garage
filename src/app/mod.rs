pub mod autosave;
pub mod handler;
pub mod input;
pub mod ipc;
pub mod state;
#[cfg(target_os = "macos")]
mod macos_window;

use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowBuilderExtMacOS;

use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::machkit::{FrameInput, UiState};
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::{GpuContext, Vertex};

use self::state::{AppState, Tab};

pub fn run_editor(
    file_path: Option<String>,
    experimental: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = env_logger::Builder::from_default_env();
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Warn);
        builder.filter(Some("wgpu_hal"), log::LevelFilter::Error);
        builder.filter(Some("wgpu_core"), log::LevelFilter::Error);
    }
    builder.init();

    // 1. Load config immediately on main thread (fast, ~5ms)
    let config = crate::editor::config::AppConfig::load();
    crate::experiments::startup::record_step("Config Load");

    // 2. Initialize Event Loop
    let event_loop = EventLoop::new()?;

    // 3. Pre-create the instance on the main thread (fast, <1ms)
    let initial_backends = match config.backend.as_str() {
        "Vulkan" => Some(wgpu::Backends::VULKAN),
        "OpenGL" => Some(wgpu::Backends::GL),
        "Metal" => Some(wgpu::Backends::METAL),
        _ => None,
    };
    let instance_backends = initial_backends.unwrap_or(wgpu::Backends::all());
    let flags = (wgpu::InstanceFlags::default()
        | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER)
        & !wgpu::InstanceFlags::VALIDATION
        & !wgpu::InstanceFlags::DEBUG;
    let instance = Arc::new(wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: instance_backends,
        flags,
        ..Default::default()
    }));

    // 4. Build the window with visibility initially FALSE
    //
    // macOS: we want native traffic-light buttons (close/minimize/zoom) with our
    // custom titlebar rendering underneath them. The key constraint is that
    // with_transparent(true) sets NSWindow.opaque=NO which breaks AppKit's
    // compositor layer ordering — the Metal CAMetalLayer ends up above the
    // NSThemeFrame buttons. So on macOS we do NOT use with_transparent and
    // instead configure the surface alpha mode at the wgpu level.
    //
    // Linux: keep with_transparent(true) and full decorations as before.
    #[allow(unused_mut)]
    let mut builder = WindowBuilder::new()
        .with_title("Garage")
        .with_inner_size(winit::dpi::PhysicalSize::new(1280, 800))
        .with_visible(false);

    #[cfg(target_os = "macos")]
    {
        // Our titlebar_height = (ui_line_height * 1.45).max(25.0).
        // At default font size 13 that's ~29 logical px. Traffic-light circles
        // are 12 px diameter; standard macOS puts their centre at y≈11 from the
        // top of NSThemeFrame. For our taller bar: centre = 29/2 = 14.5, so
        // inset_y = 14.5 - 6 (radius) = ~8.5. We round to 9.
        // The actual inset is applied via ObjC runtime in macos_window::center_traffic_lights()
        // because winit 0.29 does not expose with_traffic_light_inset().
        builder = builder
            .with_decorations(true)            // keep OS chrome → traffic lights visible
            .with_titlebar_transparent(true)   // titlebar area becomes part of our content
            .with_fullsize_content_view(true)  // our Metal surface fills the whole window
            .with_title_hidden(true);          // hide title text, traffic lights stay
        // Note: no with_transparent(true) on macOS — it breaks AppKit compositing.
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder
            .with_decorations(true)
            .with_transparent(true);
    }

    let window = Arc::new(builder.build(&event_loop)?);

    // On macOS: vertically center traffic-light buttons in our custom titlebar.
    // The default titlebar_height at font size 13 is ~29 logical px.
    #[cfg(target_os = "macos")]
    macos_window::center_traffic_lights(&window, 29.0);

    crate::experiments::startup::record_step("Window Creation");

    // 5. Create Surface on main thread (fast, ~1ms)
    let surface = match instance.create_surface(window.clone()) {
        Ok(s) => s,
        Err(e) => {
            return Err(Box::new(std::io::Error::other(format!(
                "Failed to create surface: {:?}",
                e
            ))));
        }
    };

    // Spawn background thread immediately to do all heavy GPU init and precompilation
    let (init_tx, init_rx) = std::sync::mpsc::channel::<(GpuContext, FontAtlas, UiState)>();
    let font_bytes = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");
    let proxy = event_loop.create_proxy();
    let proxy_clone = proxy.clone();

    let window_for_bg = window.clone();
    let instance_for_bg = instance.clone();
    let config_clone = config.clone();

    std::thread::spawn(move || {
        // Parallel pre-initialization of Adapter, Device, Queue, and compilation of Shader module
        let pre_init = pollster::block_on(crate::renderer::wgpu::GpuContext::pre_initialize(
            instance_for_bg.clone(),
            &surface,
            initial_backends,
        ));
        crate::experiments::startup::record_step("WGPU Context Pre-Init");

        // Complete initialization, configure surface and create bind groups/buffers
        let mut gpu =
            pollster::block_on(crate::renderer::wgpu::GpuContext::complete_initialization(
                window_for_bg,
                surface,
                pre_init,
                initial_backends,
                instance_for_bg,
            ));
        crate::experiments::startup::record_step("WGPU Context Complete & Surface Creation");

        let actual_backend_str = match gpu.backend {
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Gl => "OpenGL",
            wgpu::Backend::Metal => "Metal",
            _ => "Vulkan",
        };
        let mut saved_config = config_clone;
        if saved_config.backend != actual_backend_str {
            saved_config.backend = actual_backend_str.to_string();
            if let Err(e) = saved_config.save() {
                log::warn!("Failed to save config on startup fallback: {:?}", e);
            } else {
                log::warn!(
                    "Successfully saved fallback backend '{}' to config.",
                    actual_backend_str
                );
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

        // Pre-rasterize ASCII characters for UI and buffer font sizes to prevent frame hitching/input lag during rendering
        atlas.pre_rasterize_ascii(
            &gpu.queue,
            &[saved_config.ui_font_size, saved_config.buffer_font_size],
        );

        crate::experiments::startup::record_step("Font Atlas & Texture upload");

        // Initialize layout and state
        let mut ui = UiState::new(
            &mut atlas,
            &gpu.queue,
            saved_config,
            proxy_clone.clone(),
            experimental,
        );
        ui.active_device_name = gpu.device_name.clone();
        crate::experiments::startup::record_step("UI State Initialization");

        let _ = init_tx.send((gpu, atlas, ui));
        let _ = proxy_clone.send_event(());
    });

    let mut state = if let Some((mut restored_tabs, active_tab_idx)) =
        autosave::load_session_and_restore_buffers()
    {
        if let Some(ref path) = file_path {
            let abs_path = crate::editor::get_absolute_path(path);
            let position = restored_tabs.iter().position(|t| {
                t.path
                    .as_ref()
                    .map(|tp| crate::editor::get_absolute_path(tp))
                    == Some(abs_path.clone())
            });
            let final_active_idx = if let Some(idx) = position {
                idx
            } else {
                let mut buffer = Buffer::new();
                if !path.starts_with("diagnostics://") {
                    let _ = buffer.load_file(&abs_path);
                }
                restored_tabs.push(Tab {
                    path: Some(path.clone()),
                    buffer,
                    cursor: Cursor::new(),
                    secondary_cursors: Vec::new(),
                    scroll_x: 0,
                    scroll_y: 0,
                });
                restored_tabs.len() - 1
            };
            let mut s = AppState::new(restored_tabs[0].clone());
            s.tabs = restored_tabs;
            s.active_tab_idx = final_active_idx;
            s
        } else {
            let mut s = AppState::new(restored_tabs[0].clone());
            s.tabs = restored_tabs;
            s.active_tab_idx = active_tab_idx;
            s
        }
    } else {
        let initial_tab = {
            let mut buffer = Buffer::new();
            let save_path = if let Some(ref path) = file_path {
                if !path.starts_with("diagnostics://")
                    && let Err(e) = buffer.load_file(path)
                {
                    log::warn!(
                        "Failed to load file '{}': {}. Starting with empty buffer.",
                        path,
                        e
                    );
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
                secondary_cursors: Vec::new(),
                scroll_x: 0,
                scroll_y: 0,
            }
        };
        AppState::new(initial_tab)
    };
    crate::experiments::startup::record_step("Initial File Load & State Setup");

    let window_clone = window.clone();
    let pending_files_clone = state.pending_open_files.clone();
    let proxy_clone_ipc = event_loop.create_proxy();
    std::thread::spawn(move || {
        let socket_path = ipc::start_ipc_server(pending_files_clone, proxy_clone_ipc);
        ipc::register_window(&window_clone, &socket_path);
    });
    struct IpcGuard;
    impl Drop for IpcGuard {
        fn drop(&mut self) {
            ipc::unregister_window();
        }
    }
    let _ipc_guard = IpcGuard;

    // Track dynamic vertices and indices
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut first_frame_rendered = false;

    let mut gpu: Option<GpuContext> = None;
    let mut atlas: Option<FontAtlas> = None;
    let mut ui: Option<UiState> = None;

    let mut last_autosave = std::time::Instant::now();

    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
    let proxy_for_watcher = event_loop.create_proxy();

    // Debounce watcher proxy calls: instead of send_event on every fs event (which
    // can be hundreds/sec during a cargo build or git operation), we use a shared
    // AtomicBool flag. The watcher sets the flag; a lightweight debounce thread
    // checks it every 80 ms and fires at most one proxy wake-up per interval.
    let watcher_pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watcher_pending_w = watcher_pending.clone();
    let proxy_debounce = proxy_for_watcher.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(80));
        if watcher_pending_w.swap(false, std::sync::atomic::Ordering::Relaxed) {
            let _ = proxy_debounce.send_event(());
        }
    });

    // Spawn file watcher setup in a background thread to prevent disk/kernel API blocking the main thread
    let (watcher_keepalive_tx, watcher_keepalive_rx) =
        std::sync::mpsc::channel::<Option<notify::RecommendedWatcher>>();
    let watcher_pending_setup = watcher_pending.clone();
    std::thread::spawn(move || {
        let watcher_pending_inner = watcher_pending_setup.clone();
        let mut watcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if watcher_tx.send(res).is_ok() {
                    // Set flag; the debounce thread will fire one proxy event per 80ms window.
                    watcher_pending_inner.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }) {
                Ok(w) => Some(w),
                Err(e) => {
                    log::warn!("Failed to initialize file watcher: {:?}", e);
                    None
                }
            };

        if let Some(ref mut w) = watcher {
            use notify::Watcher;
            // Exclude build artifacts and .git internals to avoid noise from
            // cargo builds and git operations flooding the event queue.
            let _ = w.watch(std::path::Path::new("."), notify::RecursiveMode::Recursive);
            for skip in &["./target", "./.git"] {
                let _ = w.unwatch(std::path::Path::new(skip));
            }
        }
        let _ = watcher_keepalive_tx.send(watcher);
    });

    let mut watcher: Option<notify::RecommendedWatcher> = None;

    // Run the event loop reactively to save power/CPU/GPU cycles when idle
    event_loop.run(move |event, elwt| {
        if watcher.is_none()
            && let Ok(w) = watcher_keepalive_rx.try_recv() {
                watcher = w;
            }
        if !first_frame_rendered {
            // Use WaitUntil with a short timeout instead of Poll to avoid
            // spinning 100% CPU while the GPU init background thread works.
            let wake = std::time::Instant::now() + std::time::Duration::from_millis(8);
            elwt.set_control_flow(ControlFlow::WaitUntil(wake));
        } else {
            elwt.set_control_flow(ControlFlow::Wait);
        }

        match event {
            Event::NewEvents(winit::event::StartCause::Init) => {
                window.request_redraw();
            }

            Event::UserEvent(()) => {
                // Check if GPU initialization just completed.
                let gpu_just_ready = if let Ok((g, a, mut u)) = init_rx.try_recv() {
                    if let Some(ref path) = state.tabs[0].path {
                        u.selected_file = Some(std::path::PathBuf::from(path));
                    }
                    gpu = Some(g);
                    atlas = Some(a);
                    ui = Some(u);
                    true
                } else {
                    false
                };

                // Handle pending IPC file-open requests.
                let mut files = Vec::new();
                if let Ok(mut pending) = state.pending_open_files.lock() {
                    files = std::mem::take(&mut *pending);
                }
                let had_new_files = !files.is_empty();
                if gpu.is_some() && ui.is_some() && atlas.is_some() {
                    let ui_ref = ui.as_mut().unwrap();
                    let atlas_ref = atlas.as_mut().unwrap();
                    let mut mut_window = window.clone();
                    for f in files {
                        let open_action = crate::machkit::UiAction::OpenFile(std::path::PathBuf::from(f));
                        crate::app::handler::handle_action(
                            ui_ref,
                            &mut state,
                            open_action,
                            &mut mut_window,
                            elwt,
                            &mut gpu,
                            atlas_ref,
                            font_bytes,
                        );
                    }
                }
                // Only redraw for GPU init or new IPC files.
                // Git/watcher proxy wake-ups are handled by drain_background_channels
                // in AboutToWait which only redraws when data actually changed.
                if gpu_just_ready || had_new_files {
                    window.request_redraw();
                }
            }

            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                if let WindowEvent::CloseRequested = event {
                    autosave::save_session_and_dirty_buffers(&state);
                    elwt.exit();
                    return;
                }

                if gpu.is_none() || ui.is_none() || atlas.is_none() {
                    return;
                }

                let gpu_ref = gpu.as_mut().unwrap();
                let ui_ref = ui.as_mut().unwrap();
                let atlas_ref = atlas.as_mut().unwrap();

                let old_tab_idx = state.active_tab_idx;
                let old_pane_idx = state.active_pane_idx;
                let old_term_focus = state.terminal_focus;

                match event {
                    WindowEvent::CloseRequested => unreachable!(),

                    WindowEvent::Resized(physical_size) => {
                        gpu_ref.resize(physical_size);
                        ipc::update_window_geometry(&window);
                        #[cfg(target_os = "macos")]
                        {
                            macos_window::center_traffic_lights(&window, ui_ref.titlebar_height);
                        }
                        window.request_redraw();
                    }
                    WindowEvent::Moved(_) => {
                        ipc::update_window_geometry(&window);
                    }

                    WindowEvent::ScaleFactorChanged { .. } => {
                        let physical_size = window.inner_size();
                        gpu_ref.resize(physical_size);
                        #[cfg(target_os = "macos")]
                        {
                            macos_window::center_traffic_lights(&window, ui_ref.titlebar_height);
                        }
                        window.request_redraw();
                    }

                    WindowEvent::RedrawRequested => {
                        #[cfg(target_os = "macos")]
                        {
                            macos_window::center_traffic_lights(&window, ui_ref.titlebar_height);
                        }
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
                            if let Some(ref path) = tab.path
                                && !path.starts_with("diagnostics://") {
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

                        // Rebuild and sync the diagnostics://project tab buffer itself to match visual diagnostic lines
                        if ui_ref.diagnostics_changed {
                            ui_ref.diagnostics_changed = false;
                            if let Some(diag_tab_idx) = state.tabs.iter().position(|t| t.path.as_deref() == Some("diagnostics://project")) {
                                let visual_lines = crate::machkit::components::editor::text_area::get_visual_diagnostic_lines(ui_ref);
                                let mut text_lines = Vec::new();
                                for vl in &visual_lines {
                                    match vl {
                                        crate::machkit::components::editor::text_area::VisualDiagnosticLine::Header { path, line, col } => {
                                            text_lines.push(format!("▶ {} (Line {}, Col {})", path, line + 1, col + 1));
                                        }
                                        crate::machkit::components::editor::text_area::VisualDiagnosticLine::Code { line_content, .. } => {
                                            text_lines.push(line_content.clone());
                                        }
                                        crate::machkit::components::editor::text_area::VisualDiagnosticLine::Banner { diag, .. } => {
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
                            FrameInput {
                                buffer: &state.tabs[state.active_tab_idx].buffer,
                                cursor: &state.tabs[state.active_tab_idx].cursor,
                                secondary_cursors: &state.tabs[state.active_tab_idx].secondary_cursors,
                                width: size.width as f32,
                                height: size.height as f32,
                                mouse_x: state.mouse_x,
                                mouse_y: state.mouse_y,
                                current_backend: gpu_ref.backend,
                                tab_paths: &tab_paths,
                                tab_modified: &tab_modified,
                                active_tab_idx: state.active_tab_idx,
                                dragged_tab_idx: if state.is_actually_dragging_tab() { state.dragged_tab_idx } else { None },
                                inactive_panes: &state.inactive_panes,
                                active_pane_idx: state.active_pane_idx,
                                is_split_horizontal: state.is_split_horizontal,
                                terminals: &state.dock_terminals,
                                active_terminal_idx: state.active_terminal_idx,
                                terminal_focus: state.terminal_focus,
                                is_window_maximized: window.is_maximized(),
                                is_fullscreen: {
                                    #[cfg(target_os = "macos")]
                                    { macos_window::is_fullscreen(&window) }
                                    #[cfg(not(target_os = "macos"))]
                                    { window.fullscreen().is_some() }
                                },
                                tab_scroll_x: state.tab_scroll_x,
                            },
                        );

                        // Sync active tab scroll offsets back from ui_ref after drawing
                        state.tabs[state.active_tab_idx].scroll_x = ui_ref.scroll_x;
                        state.tabs[state.active_tab_idx].scroll_y = ui_ref.scroll_y;

                        // Update cursor icon when screen redraws
                        input::update_cursor_icon(
                            &window,
                            ui_ref,
                            &state,
                        );

                        // Render to swapchain
                        match gpu_ref.render(&vertices, &indices) {
                            Ok(_) => {
                                if !first_frame_rendered {
                                    first_frame_rendered = true;
                                    window.set_visible(true);
                                    crate::experiments::startup::record_step("Window Visibility Set");
                                    crate::experiments::startup::report_startup_complete();
                                }
                            }
                            Err(wgpu::SurfaceError::Timeout) => {
                                // A timeout occurs when the frame takes too long to acquire.
                                // Log as warning to prevent noise, and skip drawing this frame.
                                log::warn!("Rendering error: Timeout");
                            }
                            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                                // Reconfigure the surface if it's outdated or lost.
                                let size = window.inner_size();
                                gpu_ref.resize(size);
                                window.request_redraw();
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                log::error!("Rendering error: Out of memory! Exiting event loop.");
                                elwt.exit();
                            }
                        }
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
                        window.request_redraw();
                    }

                    WindowEvent::MouseInput { state: input_state, button, .. } => {
                        let was_modified_state = if state.active_tab_idx < state.tabs.len() {
                            let tab = &state.tabs[state.active_tab_idx];
                            Some((tab.buffer.revision, tab.buffer.is_modified))
                        } else {
                            None
                        };

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

                        let is_modified_state = if state.active_tab_idx < state.tabs.len() {
                            let tab = &state.tabs[state.active_tab_idx];
                            Some((tab.buffer.revision, tab.buffer.is_modified))
                        } else {
                            None
                        };

                        if was_modified_state != is_modified_state {
                            state.last_edit_time = Some(std::time::Instant::now());
                        }

                        window.request_redraw();
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        input::handle_mouse_wheel(ui_ref, &mut state, &window, delta);
                        window.request_redraw();
                    }

                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if key_event.state == winit::event::ElementState::Pressed {
                            let was_modified_state = if state.active_tab_idx < state.tabs.len() {
                                let tab = &state.tabs[state.active_tab_idx];
                                Some((tab.buffer.revision, tab.buffer.is_modified))
                            } else {
                                None
                            };

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

                            let is_modified_state = if state.active_tab_idx < state.tabs.len() {
                                let tab = &state.tabs[state.active_tab_idx];
                                Some((tab.buffer.revision, tab.buffer.is_modified))
                            } else {
                                None
                            };

                            if was_modified_state != is_modified_state {
                                state.last_edit_time = Some(std::time::Instant::now());
                            }

                            if !state.terminal_focus {
                                let active_tab = &state.tabs[state.active_tab_idx];
                                if let Some(ref path) = active_tab.path {
                                    let abs_path = crate::editor::get_absolute_path(path);
                                    ui_ref.diagnostics_file_cache.insert(abs_path, active_tab.buffer.lines().to_vec());
                                }
                            }
                            window.request_redraw();
                        }
                    }

                    WindowEvent::Focused(focused) => {
                        if !focused {
                            autosave::run_autosave_if_needed(ui_ref, &mut state, autosave::AutosaveTrigger::WindowChange);
                        }
                    }
                    _ => {}
                }

                if state.active_tab_idx != old_tab_idx || state.active_pane_idx != old_pane_idx || state.terminal_focus != old_term_focus {
                    autosave::run_autosave_if_needed(ui_ref, &mut state, autosave::AutosaveTrigger::FocusChange);
                }
            }
            Event::AboutToWait => {
                if !first_frame_rendered && gpu.is_some() {
                    window.request_redraw();
                }

                if gpu.is_some() && ui.is_some() && atlas.is_some() {
                    let ui_ref = ui.as_mut().unwrap();

                    // Delay-based autosave check
                    if let crate::editor::config::AutosaveSetting::AfterDelay { milliseconds } = ui_ref.config.autosave
                        && let Some(last_edit) = state.last_edit_time {
                            let delay = std::time::Duration::from_millis(milliseconds);
                            if last_edit.elapsed() >= delay {
                                autosave::run_autosave_if_needed(ui_ref, &mut state, autosave::AutosaveTrigger::Delay);
                                state.last_edit_time = None;
                                window.request_redraw();
                            } else {
                                let wake_time = last_edit + delay;
                                elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(wake_time));
                            }
                        }

                    // Periodic auto-save (every 2 seconds) — silent background write,
                    // no redraw needed since the visual state hasn't changed.
                    if last_autosave.elapsed() >= std::time::Duration::from_secs(2) {
                        autosave::save_session_and_dirty_buffers(&state);
                        last_autosave = std::time::Instant::now();
                    }

                    // Drain file watcher events
                    let mut watcher_updated = false;
                    while let Ok(res) = watcher_rx.try_recv() {
                        if let Ok(event) = res {
                            if matches!(event.kind, notify::EventKind::Access(_)) {
                                continue;
                            }
                            for p in event.paths {
                                let normalized = crate::editor::normalize_path(&p);
                                let abs_str = normalized.to_string_lossy().to_string();

                                for tab in &mut state.tabs {
                                    if let Some(ref tab_path) = tab.path {
                                        if tab_path.starts_with("diagnostics://") {
                                            continue;
                                        }
                                        let tab_abs = crate::editor::get_absolute_path(tab_path);
                                        if tab_abs == abs_str {
                                            if let Ok(content) = std::fs::read_to_string(&abs_str) {
                                                let disk_lines: Vec<&str> = content.lines().collect();
                                                let tab_lines = tab.buffer.lines();

                                                let is_different = if disk_lines.len() != tab_lines.len() {
                                                    true
                                                } else {
                                                    disk_lines.iter().zip(tab_lines.iter()).any(|(d, t)| d != t)
                                                };

                                                if is_different {
                                                    if !tab.buffer.is_modified {
                                                        if let Err(e) = tab.buffer.load_file(&abs_str) {
                                                            log::warn!("Failed to reload file '{}': {:?}", abs_str, e);
                                                        } else {
                                                            tab.buffer.mark_saved();
                                                            ui_ref.external_change_warnings.remove(tab_path);

                                                            let max_line = tab.buffer.len() - 1;
                                                            tab.cursor.line = tab.cursor.line.min(max_line);
                                                            let max_col = tab.buffer.lines()[tab.cursor.line].chars().count();
                                                            tab.cursor.col = tab.cursor.col.min(max_col);
                                                            tab.cursor.intended_col = tab.cursor.col;
                                                            tab.secondary_cursors.clear();

                                                            watcher_updated = true;
                                                        }
                                                    } else {
                                                        if ui_ref.external_change_warnings.insert(tab_path.clone()) {
                                                            watcher_updated = true;
                                                        }
                                                    }
                                                } else {
                                                    if ui_ref.external_change_warnings.remove(tab_path) {
                                                        watcher_updated = true;
                                                    }
                                                }
                                            } else {
                                                if !std::path::Path::new(&abs_str).exists()
                                                    && ui_ref.external_change_warnings.insert(tab_path.clone()) {
                                                        watcher_updated = true;
                                                    }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if watcher_updated {
                        window.request_redraw();
                    }

                    // Drain background git/search channels and schedule periodic polling.
                    // Returns true only if data actually changed → only then do we redraw.
                    let tab_paths: Vec<Option<String>> = state.tabs.iter()
                        .map(|t| t.path.clone())
                        .collect();
                    if ui_ref.drain_background_channels(state.active_tab_idx, &tab_paths) {
                        window.request_redraw();
                    }

                    // Drain Tree scan channel
                    let mut tree_updated = false;
                    if let Some(ref rx) = ui_ref.tree_rx {
                        while let Ok(nodes) = rx.try_recv() {
                            ui_ref.visible_nodes = nodes;
                            tree_updated = true;
                        }
                    }
                    if tree_updated {
                        window.request_redraw();
                    }

                    // Drain diagnostics file channel
                    let mut diagnostics_updated = false;
                    if let Some(ref rx) = ui_ref.diagnostics_file_rx {
                        while let Ok((path, lines)) = rx.try_recv() {
                            ui_ref.diagnostics_file_cache.insert(path, lines);
                            diagnostics_updated = true;
                        }
                    }
                    if diagnostics_updated {
                        window.request_redraw();
                    }

                    // Read data from PTY channels for all terminals and parse ANSI sequences using persistent parser
                    let mut pty_updated = false;
                    for term in &mut state.dock_terminals {
                        while let Ok(bytes) = term.rx.try_recv() {
                            for b in bytes {
                                term.parser.advance(&mut term.grid, b);
                            }
                            pty_updated = true;
                        }
                    }
                    if pty_updated {
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}
