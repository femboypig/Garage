use std::sync::Arc;
use std::io::Write;
use winit::window::Window;
use winit::keyboard::{Key, NamedKey};
use crate::app::state::AppState;

pub fn handle_terminal_input(
    state: &mut AppState,
    window: &Arc<Window>,
    logical_key: &Key,
) -> bool {
    if state.terminal_focus && !state.dock_terminals.is_empty() {
        let active_term = &mut state.dock_terminals[state.active_terminal_idx];
        let bytes_to_write: Option<Vec<u8>> = match logical_key {
            Key::Character(text) => {
                let ctrl = state.modifiers.control_key();
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
            active_term.grid.scroll_offset = 0;
        }
        window.request_redraw();
        true
    } else {
        false
    }
}
