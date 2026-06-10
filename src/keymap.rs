use winit::keyboard::{Key, NamedKey, PhysicalKey, KeyCode};
use crate::actions::Action;

pub fn map_key(
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> Option<Action> {
    // 1. Check control shortcuts first
    if ctrl {
        if let PhysicalKey::Code(keycode) = physical_key {
            match keycode {
                KeyCode::Equal | KeyCode::NumpadAdd => return Some(Action::ZoomIn),
                KeyCode::Minus | KeyCode::NumpadSubtract => return Some(Action::ZoomOut),
                KeyCode::KeyP if shift => return Some(Action::CommandPalette),
                KeyCode::KeyS => return Some(Action::SaveFile),
                KeyCode::KeyA => return Some(Action::SelectAll),
                KeyCode::KeyC => return Some(Action::Copy),
                KeyCode::KeyX => return Some(Action::Cut),
                KeyCode::KeyV => return Some(Action::Paste),
                KeyCode::KeyZ => return Some(Action::Undo),
                KeyCode::KeyY => return Some(Action::Redo),
                _ => {}
            }
        }
    }

    // 2. Navigation & Special Keys
    match logical_key {
        Key::Named(NamedKey::ArrowLeft) => {
            Some(Action::MoveLeft { select: shift, word: ctrl })
        }
        Key::Named(NamedKey::ArrowRight) => {
            Some(Action::MoveRight { select: shift, word: ctrl })
        }
        Key::Named(NamedKey::ArrowUp) => {
            Some(Action::MoveUp { select: shift })
        }
        Key::Named(NamedKey::ArrowDown) => {
            Some(Action::MoveDown { select: shift })
        }
        Key::Named(NamedKey::Home) => {
            Some(Action::MoveToLineStart { select: shift })
        }
        Key::Named(NamedKey::End) => {
            Some(Action::MoveToLineEnd { select: shift })
        }
        Key::Named(NamedKey::Escape) => {
            Some(Action::Escape)
        }
        Key::Named(NamedKey::Backspace) => {
            Some(Action::DeleteLeft)
        }
        Key::Named(NamedKey::Delete) => {
            Some(Action::DeleteRight)
        }
        Key::Named(NamedKey::Enter) => {
            Some(Action::InsertNewLine)
        }
        Key::Named(NamedKey::Tab) => {
            Some(Action::InsertTab)
        }
        Key::Named(NamedKey::Space) => {
            if !ctrl && !alt {
                Some(Action::InsertChar(' '))
            } else {
                None
            }
        }
        Key::Character(text) => {
            if !ctrl && !alt && text.chars().count() == 1 {
                Some(Action::InsertChar(text.chars().next().unwrap()))
            } else {
                None
            }
        }
        _ => None,
    }
}
