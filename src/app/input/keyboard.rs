use std::sync::Arc;
use winit::window::Window;
use winit::keyboard::{Key, PhysicalKey, NamedKey};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::ui::UiState;
use crate::renderer::atlas::FontAtlas;
use crate::app::state::AppState;

pub mod terminal;
pub mod palette;
pub mod diagnostics;
pub mod editor;

pub fn handle_keyboard_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: Key,
    physical_key: PhysicalKey,
) {
    // 1. Delegate to terminal input handler if terminal is focused
    if terminal::handle_terminal_input(state, window, &logical_key) {
        return;
    }

    let (active_path_start, old_content) = {
        if state.active_tab_idx < state.tabs.len() {
            let active_tab = &state.tabs[state.active_tab_idx];
            if let Some(ref path) = active_tab.path {
                if !path.starts_with("diagnostics://") {
                    (Some(path.clone()), Some(active_tab.buffer.lines().to_vec()))
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

    let ctrl = state.modifiers.control_key();
    let shift = state.modifiers.shift_key();
    let alt = state.modifiers.alt_key();

    // 2. Delegate to command palette modal handler if command palette is active
    if palette::handle_command_palette_input(ui, state, window, elwt, gpu, atlas, font_bytes, &logical_key) {
        return;
    }

    // 3. Delegate to modal handler (Escape key behavior for other modals)
    if ui.active_modal.is_some() {
        if let Key::Named(NamedKey::Escape) = &logical_key {
            ui.active_modal = None;
            window.request_redraw();
        }
        return;
    }

    // 4. Delegate to virtual diagnostics view handler if active
    if diagnostics::handle_diagnostics_keyboard(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
    ) {
        return;
    }

    // 5. Delegate to core editor handler
    editor::handle_editor_keyboard(
        ui,
        state,
        window,
        elwt,
        gpu,
        atlas,
        font_bytes,
        &logical_key,
        physical_key,
        ctrl,
        shift,
        alt,
        active_path_start,
        old_content,
    );
}
