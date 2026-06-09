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
    let mut gpu = pollster::block_on(GpuContext::new(window.clone()));

    // Load bundled IBM Plex Mono font bytes
    let font_bytes = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");

    // Initialize Font Atlas using wgpu device/queue
    let mut atlas = FontAtlas::new(&gpu.device, &gpu.queue, font_bytes, 16.0)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Update bind group to use actual font texture and sampler
    gpu.update_bind_group(&atlas.texture, &atlas.sampler);

    // Initialize layout and state
    let mut ui = UiState::new(&mut atlas, &gpu.queue);

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
    let mut internal_clipboard = String::new();
    
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
                    gpu.resize(physical_size);
                    window.request_redraw();
                }

                WindowEvent::ScaleFactorChanged { .. } => {
                    let physical_size = window.inner_size();
                    gpu.resize(physical_size);
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
                        &gpu.queue,
                        &buffer,
                        &cursor,
                        size.width as f32,
                        size.height as f32,
                        mouse_x,
                        mouse_y,
                    );

                    // Render to swapchain
                    if let Err(e) = gpu.render(&vertices, &indices) {
                        log::error!("Rendering error: {:?}", e);
                    }
                }

                WindowEvent::ModifiersChanged(new_modifiers) => {
                    modifiers = new_modifiers.state();
                }

                WindowEvent::CursorMoved { position, .. } => {
                    mouse_x = position.x as f32;
                    mouse_y = position.y as f32;

                    if is_dragging {
                        // Calculate dynamic line number gutter width
                        let max_line_digits = buffer.len().to_string().len().max(3);
                        let gutter_width = (max_line_digits as f32 + 2.0) * ui.char_width;
                        let text_area_x = ui.sidebar_width + gutter_width;

                        // Calculate line under mouse pointer
                        let line_idx = if mouse_y >= ui.titlebar_height {
                            ((mouse_y - ui.titlebar_height) / ui.line_height).floor() as usize + ui.scroll_y
                        } else {
                            ui.scroll_y
                        };
                        let line_idx = line_idx.min(buffer.len() - 1);

                        // Calculate column under mouse pointer
                        let col_idx = if mouse_x > text_area_x {
                            ((mouse_x - text_area_x) / ui.char_width).round() as usize + ui.scroll_x
                        } else {
                            0
                        };
                        let line_chars = buffer.lines()[line_idx].chars().count();
                        let col_idx = col_idx.min(line_chars);

                        // Update cursor active selection focus
                        cursor.line = line_idx;
                        cursor.col = col_idx;
                        cursor.intended_col = col_idx;
                    }
                    window.request_redraw();
                }

                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left {
                        let size = window.inner_size();
                        if state == ElementState::Pressed {
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
                                    if let Some(ref path) = save_path {
                                        if let Err(e) = buffer.save_file(path) {
                                            log::error!("Failed to save file: {:?}", e);
                                        } else {
                                            ui.rebuild_tree();
                                        }
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
                                    ui.target_sidebar_width = if ui.target_sidebar_width == 200.0 { 0.0 } else { 200.0 };
                                }
                                UiAction::Exit => {
                                    elwt.exit();
                                }
                                UiAction::None => {
                                    let max_line_digits = buffer.len().to_string().len().max(3);
                                    let gutter_width = (max_line_digits as f32 + 2.0) * ui.char_width;
                                    let text_area_x = ui.sidebar_width + gutter_width;

                                    if mouse_x >= text_area_x && mouse_y >= ui.titlebar_height && mouse_y < size.height as f32 - ui.status_height {
                                        buffer.commit_transaction();
                                        is_dragging = true;

                                        let extend_selection = modifiers.shift_key();
                                        cursor.update_selection(extend_selection);

                                        let line_idx = ((mouse_y - ui.titlebar_height) / ui.line_height).floor() as usize + ui.scroll_y;
                                        let line_idx = line_idx.min(buffer.len() - 1);

                                        let col_idx = ((mouse_x - text_area_x) / ui.char_width).round() as usize + ui.scroll_x;
                                        let line_chars = buffer.lines()[line_idx].chars().count();
                                        let col_idx = col_idx.min(line_chars);

                                        cursor.line = line_idx;
                                        cursor.col = col_idx;
                                        cursor.intended_col = col_idx;
                                    }
                                }
                            }
                        } else {
                            is_dragging = false;
                            if let Some((s_l, s_c, e_l, e_c)) = cursor.selection_range() {
                                if s_l == e_l && s_c == e_c {
                                    cursor.clear_selection();
                                }
                            }
                        }
                        window.request_redraw();
                    }
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    let scroll_lines = match delta {
                        MouseScrollDelta::LineDelta(_, dy) => -dy as isize,
                        MouseScrollDelta::PixelDelta(pos) => (pos.y / (ui.line_height as f64)) as isize * -1,
                    };

                    let new_scroll = ui.scroll_y as isize + scroll_lines;
                    ui.scroll_y = new_scroll.clamp(0, buffer.len() as isize - 1) as usize;

                    window.request_redraw();
                }

                WindowEvent::KeyboardInput { event: kb_event, .. } => {
                    if kb_event.state == ElementState::Pressed {
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
                                // Delete selection or char before cursor
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
                                // Delete selection or char after cursor
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
                                    // Control shortcuts
                                    match text.as_str() {
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
                                            if let Some(ref path) = save_path {
                                                if let Err(e) = buffer.save_file(path) {
                                                    log::error!("Failed to save file: {:?}", e);
                                                }
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
                                    // Normal character input
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
                    }
                }
                _ => {}
            },
            _ => {}
        }
    })?;

    Ok(())
}
