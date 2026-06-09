use std::sync::Arc;
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
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Garage Code Editor")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 800))
            .build(&event_loop)?,
    );

    // Initialize wgpu rendering context and pipeline synchronously
    // Load configuration at startup
    let config = crate::config::AppConfig::load();

    // Select backend based on config
    let initial_backends = match config.backend.as_str() {
        "Vulkan" => Some(wgpu::Backends::VULKAN),
        "OpenGL" => Some(wgpu::Backends::GL),
        _ => None,
    };

    // Initialize wgpu rendering context and pipeline synchronously
    let mut gpu = Some(pollster::block_on(GpuContext::new(window.clone(), initial_backends)));

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

    // Initialize text buffer and load file if provided
    let mut buffer = Buffer::new();
    let mut save_path = if let Some(ref path) = file_path {
        if let Err(e) = buffer.load_file(path) {
            log::warn!("Failed to load file '{}': {}. Starting with empty buffer.", path, e);
        }
        Some(path.clone())
    } else {
        None
    };

    // Initialize cursor navigation state
    let mut cursor = Cursor::new();

    // Editor state
    let mut modifiers = winit::keyboard::ModifiersState::default();
    let mut is_dragging = false;
    let mut is_dragging_scroll = false;
    let mut scroll_drag_offset_y = 0.0f32;
    let mut is_dragging_sidebar = false;
    let mut internal_clipboard = String::new();
    
    // Helper to update cursor icon
    let update_cursor_icon = |window: &winit::window::Window, ui: &UiState, buffer: &Buffer, mouse_x: f32, mouse_y: f32| {
        let size = window.inner_size();
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
        let text_area_x = ui.sidebar_width + gutter_width;
        
        let on_sidebar_border = ui.sidebar_width > 0.0 && (mouse_x - ui.sidebar_width).abs() <= 4.0;
        
        if on_sidebar_border {
            window.set_cursor_icon(winit::window::CursorIcon::ColResize);
        } else {
            let is_in_editor = ui.active_modal.is_none()
                && ui.active_menu.is_none()
                && mouse_x >= text_area_x
                && mouse_x < size.width as f32 - 12.0
                && mouse_y >= ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height
                && mouse_y < size.height as f32 - ui.status_height;
                
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
                    // Clear dynamic buffers
                    vertices.clear();
                    indices.clear();

                    let size = window.inner_size();
                    // Compile the current editor frame into GPU vertices
                    ui.build_frame(
                        &mut vertices,
                        &mut indices,
                        &mut atlas,
                        &gpu.as_ref().unwrap().queue,
                        &buffer,
                        &cursor,
                        size.width as f32,
                        size.height as f32,
                        mouse_x,
                        mouse_y,
                        gpu.as_ref().unwrap().backend,
                    );

                    // Update cursor icon when screen redraws
                    update_cursor_icon(&window, &ui, &buffer, mouse_x, mouse_y);

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
                    } else if is_dragging_scroll {
                        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                        let editor_height = size.height as f32 - editor_top - ui.status_height;
                        let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                        let ratio = visible_lines as f32 / buffer.len() as f32;
                        let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
                        let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
                        let relative_y = mouse_y - editor_top - scroll_drag_offset_y;
                        let scroll_range = editor_height - thumb_h;
                        let scroll_ratio = if scroll_range > 0.0 { (relative_y / scroll_range).clamp(0.0, 1.0) } else { 0.0 };
                        ui.scroll_y = (scroll_ratio * max_scroll).round() as usize;
                    } else if is_dragging {
                        let max_line_digits = buffer.len().to_string().len().max(3);
                        let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                        let text_area_x = ui.sidebar_width + gutter_width;

                        let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                        let line_idx = if mouse_y >= editor_top {
                            ((mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y
                        } else {
                            ui.scroll_y
                        };
                        let line_idx = line_idx.min(buffer.len() - 1);

                        let col_idx = if mouse_x > text_area_x {
                            ((mouse_x - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x
                        } else {
                            0
                        };
                        let line_chars = buffer.lines()[line_idx].chars().count();
                        let col_idx = col_idx.min(line_chars);

                        cursor.line = line_idx;
                        cursor.col = col_idx;
                        cursor.intended_col = col_idx;

                        ui.scroll_to_cursor(&cursor, buffer.len(), size.height as f32);
                    }

                    update_cursor_icon(&window, &ui, &buffer, mouse_x, mouse_y);
                    window.request_redraw();
                }

                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left {
                        let size = window.inner_size();
                        if state == ElementState::Pressed {
                            if ui.active_modal.is_some() {
                                let action = ui.handle_click(
                                    mouse_x,
                                    mouse_y,
                                    size.width as f32,
                                    size.height as f32,
                                    &mut buffer,
                                    &mut cursor,
                                );

                                match action {
                                    UiAction::CloseModal => {
                                        ui.active_modal = None;
                                    }
                                    UiAction::ChangeBufferFontSize(delta) => {
                                        let new_size = (ui.buffer_font_size + delta).clamp(8.0, 36.0);
                                        ui.update_buffer_font_size(&atlas.font, new_size);
                                        ui.config.buffer_font_size = new_size;
                                        let _ = ui.config.save();
                                    }
                                    UiAction::ChangeUiFontSize(delta) => {
                                        let new_size = (ui.ui_font_size + delta).clamp(8.0, 24.0);
                                        ui.update_ui_font_size(&atlas.font, new_size);
                                        ui.config.ui_font_size = new_size;
                                        let _ = ui.config.save();
                                    }
                                    UiAction::ChangeBackend(backend) => {
                                        let mut new_config = ui.config.clone();
                                        let requested_str = match backend {
                                            wgpu::Backend::Vulkan => "Vulkan",
                                            wgpu::Backend::Gl => "OpenGL",
                                            _ => "Vulkan",
                                        };
                                        new_config.backend = requested_str.to_string();
                                        let _ = new_config.save();

                                        let forced_backends = match backend {
                                            wgpu::Backend::Vulkan => wgpu::Backends::VULKAN,
                                            wgpu::Backend::Gl => wgpu::Backends::GL,
                                            _ => wgpu::Backends::all(),
                                        };
                                        gpu = None;
                                        let mut new_gpu = pollster::block_on(GpuContext::new(window.clone(), Some(forced_backends)));
                                        if let Ok(new_atlas) = FontAtlas::new(&new_gpu.device, &new_gpu.queue, font_bytes) {
                                            atlas = new_atlas;
                                            new_gpu.update_bind_group(&atlas.texture, &atlas.sampler);

                                            let actual_str = match new_gpu.backend {
                                                wgpu::Backend::Vulkan => "Vulkan",
                                                wgpu::Backend::Gl => "OpenGL",
                                                _ => requested_str,
                                            };
                                            new_config.backend = actual_str.to_string();
                                            let _ = new_config.save();

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
                                    _ => {}
                                }
                            } else {
                                // Check if click is on sidebar resize border
                                let on_sidebar_border = ui.sidebar_width > 0.0 && (mouse_x - ui.sidebar_width).abs() <= 4.0;
                                if on_sidebar_border {
                                    is_dragging_sidebar = true;
                                } else {
                                    let action = ui.handle_click(
                                        mouse_x,
                                        mouse_y,
                                        size.width as f32,
                                        size.height as f32,
                                        &mut buffer,
                                        &mut cursor,
                                    );

                                    match action {
                                        UiAction::OpenFile(path) => {
                                            if let Err(e) = buffer.load_file(&path) {
                                                log::error!("Failed to load file: {:?}", e);
                                            } else {
                                                save_path = Some(path.to_string_lossy().to_string());
                                                cursor.line = 0;
                                                cursor.col = 0;
                                                cursor.intended_col = 0;
                                                cursor.clear_selection();
                                                ui.scroll_y = 0;
                                            }
                                        }
                                        UiAction::SaveFile => {
                                            let path_to_save = save_path.clone().unwrap_or_else(|| {
                                                let default_path = "./untitled.txt".to_string();
                                                save_path = Some(default_path.clone());
                                                default_path
                                            });
                                            println!("Saving file to: {}", path_to_save);
                                            if let Err(e) = buffer.save_file(&path_to_save) {
                                                log::error!("Failed to save file: {:?}", e);
                                            } else {
                                                ui.rebuild_tree();
                                            }
                                        }
                                        UiAction::Undo => {
                                            buffer.undo();
                                            cursor.clear_selection();
                                            cursor.line = cursor.line.min(buffer.len() - 1);
                                            let max_col = buffer.lines()[cursor.line].chars().count();
                                            cursor.col = cursor.col.min(max_col);
                                        }
                                        UiAction::Redo => {
                                            buffer.redo();
                                            cursor.clear_selection();
                                            cursor.line = cursor.line.min(buffer.len() - 1);
                                            let max_col = buffer.lines()[cursor.line].chars().count();
                                            cursor.col = cursor.col.min(max_col);
                                        }
                                        UiAction::ToggleSidebar => {
                                            ui.target_sidebar_width = if ui.target_sidebar_width > 0.0 { 0.0 } else { 200.0 };
                                            ui.sidebar_width = ui.target_sidebar_width;
                                            ui.config.sidebar_width = ui.target_sidebar_width;
                                            let _ = ui.config.save();
                                        }
                                        UiAction::ShowSettings => {
                                            ui.active_modal = Some(crate::ui::ModalType::Settings);
                                        }
                                        UiAction::ShowAbout => {
                                            ui.active_modal = Some(crate::ui::ModalType::About);
                                        }
                                        UiAction::CloseModal => {
                                            ui.active_modal = None;
                                        }
                                        UiAction::ChangeBufferFontSize(delta) => {
                                            let new_size = (ui.buffer_font_size + delta).clamp(8.0, 36.0);
                                            ui.update_buffer_font_size(&atlas.font, new_size);
                                            ui.config.buffer_font_size = new_size;
                                            let _ = ui.config.save();
                                        }
                                        UiAction::ChangeUiFontSize(delta) => {
                                            let new_size = (ui.ui_font_size + delta).clamp(8.0, 24.0);
                                            ui.update_ui_font_size(&atlas.font, new_size);
                                            ui.config.ui_font_size = new_size;
                                            let _ = ui.config.save();
                                        }
                                        UiAction::ChangeBackend(backend) => {
                                            let mut new_config = ui.config.clone();
                                            let requested_str = match backend {
                                                wgpu::Backend::Vulkan => "Vulkan",
                                                wgpu::Backend::Gl => "OpenGL",
                                                _ => "Vulkan",
                                            };
                                            new_config.backend = requested_str.to_string();
                                            let _ = new_config.save();

                                            let forced_backends = match backend {
                                                wgpu::Backend::Vulkan => wgpu::Backends::VULKAN,
                                                wgpu::Backend::Gl => wgpu::Backends::GL,
                                                _ => wgpu::Backends::all(),
                                            };
                                            gpu = None;
                                            let mut new_gpu = pollster::block_on(GpuContext::new(window.clone(), Some(forced_backends)));
                                            if let Ok(new_atlas) = FontAtlas::new(&new_gpu.device, &new_gpu.queue, font_bytes) {
                                                atlas = new_atlas;
                                                new_gpu.update_bind_group(&atlas.texture, &atlas.sampler);

                                                let actual_str = match new_gpu.backend {
                                                    wgpu::Backend::Vulkan => "Vulkan",
                                                    wgpu::Backend::Gl => "OpenGL",
                                                    _ => requested_str,
                                                };
                                                new_config.backend = actual_str.to_string();
                                                let _ = new_config.save();

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
                                        UiAction::Exit => {
                                            elwt.exit();
                                        }
                                        UiAction::None => {
                                            let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                                            let editor_height = size.height as f32 - editor_top - ui.status_height;
                                            // Check if click is on scrollbar
                                            let sb_x = size.width as f32 - 12.0;
                                            if mouse_x >= sb_x && mouse_y >= editor_top && mouse_y < size.height as f32 - ui.status_height {
                                                is_dragging_scroll = true;
                                                let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                                                let ratio = visible_lines as f32 / buffer.len() as f32;
                                                let thumb_h = (editor_height * ratio).clamp(20.0, editor_height);
                                                let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0) as f32;
                                                
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
                                            } else {
                                                // Click inside editor area
                                                let max_line_digits = buffer.len().to_string().len().max(3);
                                                let gutter_width = (max_line_digits as f32 + 2.0) * ui.buffer_char_width;
                                                let text_area_x = ui.sidebar_width + gutter_width;

                                                if mouse_x >= text_area_x && mouse_y >= editor_top && mouse_y < size.height as f32 - ui.status_height {
                                                    buffer.commit_transaction();
                                                    is_dragging = true;

                                                    let line_idx = ((mouse_y - editor_top) / ui.buffer_line_height).floor() as usize + ui.scroll_y;
                                                    let line_idx = line_idx.min(buffer.len() - 1);

                                                    let col_idx = ((mouse_x - text_area_x) / ui.buffer_char_width).round() as usize + ui.scroll_x;
                                                    let line_chars = buffer.lines()[line_idx].chars().count();
                                                    let col_idx = col_idx.min(line_chars);

                                                    let extend_selection = modifiers.shift_key();
                                                    if extend_selection {
                                                        if cursor.selection_anchor.is_none() {
                                                            cursor.selection_anchor = Some((cursor.line, cursor.col));
                                                        }
                                                    } else {
                                                        cursor.selection_anchor = Some((line_idx, col_idx));
                                                    }

                                                    cursor.line = line_idx;
                                                    cursor.col = col_idx;
                                                    cursor.intended_col = col_idx;

                                                    ui.scroll_to_cursor(&cursor, buffer.len(), size.height as f32);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            let was_dragging_sidebar = is_dragging_sidebar;
                            is_dragging = false;
                            is_dragging_scroll = false;
                            is_dragging_sidebar = false;
                            if was_dragging_sidebar {
                                ui.config.sidebar_width = ui.sidebar_width;
                                let _ = ui.config.save();
                            }
                            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                if s_l == e_l && s_c == e_c {
                                    cursor.clear_selection();
                                }
                            }
                        }
                        update_cursor_icon(&window, &ui, &buffer, mouse_x, mouse_y);
                        window.request_redraw();
                    }
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    if ui.active_modal.is_some() {
                        return;
                    }
                    let scroll_lines = match delta {
                        MouseScrollDelta::LineDelta(_, dy) => -dy as isize,
                        MouseScrollDelta::PixelDelta(pos) => (pos.y / (ui.buffer_line_height as f64)) as isize * -1,
                    };

                    let editor_top = ui.titlebar_height + ui.tabbar_height + ui.breadcrumb_height;
                    let editor_height = window.inner_size().height as f32 - editor_top - ui.status_height;
                    let visible_lines = (editor_height / ui.buffer_line_height).floor() as usize;
                    let max_scroll = (buffer.len() as isize - visible_lines as isize).max(0);

                    let new_scroll = ui.scroll_y as isize + scroll_lines;
                    ui.scroll_y = new_scroll.clamp(0, max_scroll) as usize;

                    window.request_redraw();
                }

                WindowEvent::KeyboardInput { event: kb_event, .. } => {
                    if kb_event.state == ElementState::Pressed {
                        if ui.active_modal.is_some() {
                            let ctrl = modifiers.control_key();
                            match &kb_event.logical_key {
                                Key::Named(NamedKey::Escape) => {
                                    ui.active_modal = None;
                                    window.request_redraw();
                                }
                                Key::Character(text) if ctrl => {
                                    match text.as_str() {
                                        "+" | "=" => {
                                            let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
                                            ui.update_buffer_font_size(&atlas.font, new_size);
                                            window.request_redraw();
                                        }
                                        "-" => {
                                            let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
                                            ui.update_buffer_font_size(&atlas.font, new_size);
                                            window.request_redraw();
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                            return;
                        }

                        let shift = modifiers.shift_key();
                        let ctrl = modifiers.control_key();

                        match &kb_event.logical_key {
                            Key::Named(NamedKey::ArrowLeft) => {
                                buffer.commit_transaction();
                                if ctrl {
                                    cursor.move_word_left(&buffer, shift);
                                } else {
                                    cursor.move_left(&buffer, shift);
                                }
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                buffer.commit_transaction();
                                if ctrl {
                                    cursor.move_word_right(&buffer, shift);
                                } else {
                                    cursor.move_right(&buffer, shift);
                                }
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::ArrowUp) => {
                                buffer.commit_transaction();
                                cursor.move_up(&buffer, shift);
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::ArrowDown) => {
                                buffer.commit_transaction();
                                cursor.move_down(&buffer, shift);
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Home) => {
                                buffer.commit_transaction();
                                cursor.move_to_line_start(shift);
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::End) => {
                                buffer.commit_transaction();
                                cursor.move_to_line_end(&buffer, shift);
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Escape) => {
                                buffer.commit_transaction();
                                cursor.clear_selection();
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Backspace) => {
                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                    buffer.start_transaction();
                                    buffer.delete(s_l, s_c, e_l, e_c);
                                    cursor.line = s_l;
                                    cursor.col = s_c;
                                    cursor.intended_col = s_c;
                                    cursor.clear_selection();
                                    buffer.commit_transaction();
                                } else if cursor.col > 0 || cursor.line > 0 {
                                    buffer.start_transaction();
                                    let mut prev_cursor = cursor;
                                    prev_cursor.move_left(&buffer, false);
                                    buffer.delete(prev_cursor.line, prev_cursor.col, cursor.line, cursor.col);
                                    cursor = prev_cursor;
                                    buffer.commit_transaction();
                                }
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Delete) => {
                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                    buffer.start_transaction();
                                    buffer.delete(s_l, s_c, e_l, e_c);
                                    cursor.line = s_l;
                                    cursor.col = s_c;
                                    cursor.intended_col = s_c;
                                    cursor.clear_selection();
                                    buffer.commit_transaction();
                                } else {
                                    let line_len = buffer.lines()[cursor.line].chars().count();
                                    if cursor.col < line_len || cursor.line < buffer.len() - 1 {
                                        buffer.start_transaction();
                                        let mut next_cursor = cursor;
                                        next_cursor.move_right(&buffer, false);
                                        buffer.delete(cursor.line, cursor.col, next_cursor.line, next_cursor.col);
                                        buffer.commit_transaction();
                                    }
                                }
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Space) => {
                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                    buffer.start_transaction();
                                    buffer.delete(s_l, s_c, e_l, e_c);
                                    cursor.line = s_l;
                                    cursor.col = s_c;
                                    cursor.clear_selection();
                                }
                                buffer.start_transaction();
                                buffer.insert(cursor.line, cursor.col, " ");
                                cursor.col += 1;
                                cursor.intended_col = cursor.col;
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Enter) => {
                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                    buffer.start_transaction();
                                    buffer.delete(s_l, s_c, e_l, e_c);
                                    cursor.line = s_l;
                                    cursor.col = s_c;
                                    cursor.clear_selection();
                                }
                                buffer.start_transaction();
                                buffer.insert(cursor.line, cursor.col, "\n");
                                cursor.line += 1;
                                cursor.col = 0;
                                cursor.intended_col = 0;
                                buffer.commit_transaction();
                                window.request_redraw();
                            }
                            Key::Named(NamedKey::Tab) => {
                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                    buffer.start_transaction();
                                    buffer.delete(s_l, s_c, e_l, e_c);
                                    cursor.line = s_l;
                                    cursor.col = s_c;
                                    cursor.clear_selection();
                                }
                                buffer.start_transaction();
                                buffer.insert(cursor.line, cursor.col, "    ");
                                cursor.col += 4;
                                cursor.intended_col = cursor.col;
                                buffer.commit_transaction();
                                window.request_redraw();
                            }
                            Key::Character(text) => {
                                if ctrl {
                                    match text.as_str() {
                                         "+" | "=" => {
                                             let new_size = (ui.buffer_font_size + 1.0).clamp(8.0, 36.0);
                                             ui.update_buffer_font_size(&atlas.font, new_size);
                                         }
                                         "-" => {
                                             let new_size = (ui.buffer_font_size - 1.0).clamp(8.0, 36.0);
                                             ui.update_buffer_font_size(&atlas.font, new_size);
                                         }
                                        "z" | "Z" => {
                                            buffer.commit_transaction();
                                            if buffer.undo() {
                                                cursor.clear_selection();
                                                cursor.line = cursor.line.min(buffer.len() - 1);
                                                let max_col = buffer.lines()[cursor.line].chars().count();
                                                cursor.col = cursor.col.min(max_col);
                                                cursor.intended_col = cursor.col;
                                            }
                                        }
                                        "y" | "Y" => {
                                            buffer.commit_transaction();
                                            if buffer.redo() {
                                                cursor.clear_selection();
                                                cursor.line = cursor.line.min(buffer.len() - 1);
                                                let max_col = buffer.lines()[cursor.line].chars().count();
                                                cursor.col = cursor.col.min(max_col);
                                                cursor.intended_col = cursor.col;
                                            }
                                        }
                                        "s" | "S" => {
                                            buffer.commit_transaction();
                                            let path_to_save = save_path.clone().unwrap_or_else(|| {
                                                let default_path = "./untitled.txt".to_string();
                                                save_path = Some(default_path.clone());
                                                default_path
                                            });
                                            println!("Saving file to: {}", path_to_save);
                                            if let Err(e) = buffer.save_file(&path_to_save) {
                                                log::error!("Failed to save file: {:?}", e);
                                            }
                                        }
                                        "c" | "C" => {
                                            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                                internal_clipboard = buffer.get_range_text(s_l, s_c, e_l, e_c);
                                            }
                                        }
                                        "x" | "X" => {
                                            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                                internal_clipboard = buffer.get_range_text(s_l, s_c, e_l, e_c);
                                                buffer.start_transaction();
                                                buffer.delete(s_l, s_c, e_l, e_c);
                                                cursor.line = s_l;
                                                cursor.col = s_c;
                                                cursor.intended_col = s_c;
                                                cursor.clear_selection();
                                                buffer.commit_transaction();
                                            }
                                        }
                                        "v" | "V" => {
                                            if !internal_clipboard.is_empty() {
                                                buffer.start_transaction();
                                                if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                                    buffer.delete(s_l, s_c, e_l, e_c);
                                                    cursor.line = s_l;
                                                    cursor.col = s_c;
                                                    cursor.clear_selection();
                                                }
                                                buffer.insert(cursor.line, cursor.col, &internal_clipboard);

                                                let parts = internal_clipboard.split('\n').collect::<Vec<&str>>();
                                                if parts.len() == 1 {
                                                    cursor.col += internal_clipboard.chars().count();
                                                } else {
                                                    cursor.line += parts.len() - 1;
                                                    cursor.col = parts.last().unwrap().chars().count();
                                                }
                                                cursor.intended_col = cursor.col;
                                                buffer.commit_transaction();
                                            }
                                        }
                                        "a" | "A" => {
                                            buffer.commit_transaction();
                                            cursor.selection_anchor = Some((0, 0));
                                            cursor.line = buffer.len() - 1;
                                            cursor.col = buffer.lines()[cursor.line].chars().count();
                                            cursor.intended_col = cursor.col;
                                        }
                                        _ => {}
                                    }
                                } else {
                                    if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                        buffer.start_transaction();
                                        buffer.delete(s_l, s_c, e_l, e_c);
                                        cursor.line = s_l;
                                        cursor.col = s_c;
                                        cursor.clear_selection();
                                    }
                                    buffer.start_transaction();
                                    buffer.insert(cursor.line, cursor.col, text);
                                    cursor.col += text.chars().count();
                                    cursor.intended_col = cursor.col;
                                }
                                window.request_redraw();
                            }
                            _ => {}
                        }
                        ui.scroll_to_cursor(&cursor, buffer.len(), window.inner_size().height as f32);
                        update_cursor_icon(&window, &ui, &buffer, mouse_x, mouse_y);
                        window.request_redraw();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    })?;

    Ok(())
}
