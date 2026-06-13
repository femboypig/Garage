use std::sync::Arc;
use winit::window::Window;
use winit::keyboard::{Key, NamedKey};
use winit::event_loop::EventLoopWindowTarget;

use crate::renderer::wgpu::GpuContext;
use crate::ui::UiState;
use crate::renderer::atlas::FontAtlas;
use crate::app::state::AppState;
use crate::app::handler::handle_action;

pub fn handle_command_palette_input(
    ui: &mut UiState,
    state: &mut AppState,
    window: &mut Arc<Window>,
    elwt: &EventLoopWindowTarget<()>,
    gpu: &mut Option<GpuContext>,
    atlas: &mut FontAtlas,
    font_bytes: &[u8],
    logical_key: &Key,
) -> bool {
    if let Some(crate::ui::ModalType::CommandPalette) = ui.active_modal {
        match logical_key {
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
                    
                    let action_res = {
                         let active_tab = &mut state.tabs[state.active_tab_idx];
                         ui.execute_command(cmd, &mut active_tab.buffer, &mut active_tab.cursor)
                     };
                     handle_action(ui, state, action_res, window, elwt, gpu, atlas, font_bytes);
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
        true
    } else {
        false
    }
}
