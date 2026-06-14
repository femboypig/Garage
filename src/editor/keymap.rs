use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use winit::keyboard::{Key, NamedKey, PhysicalKey, KeyCode};
use super::actions::Action;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyContextBindings {
    pub context: Option<String>,
    pub bindings: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    pub contexts: Vec<KeyContextBindings>,
    pub defaults: Vec<KeyContextBindings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Keystroke {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl Keystroke {
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.to_lowercase();
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        
        let mut remainder = s.as_str();
        loop {
            if remainder.starts_with("ctrl-") {
                ctrl = true;
                remainder = &remainder[5..];
            } else if remainder.starts_with("shift-") {
                shift = true;
                remainder = &remainder[6..];
            } else if remainder.starts_with("alt-") {
                alt = true;
                remainder = &remainder[4..];
            } else {
                break;
            }
        }
        
        if remainder.is_empty() {
            return Err(format!("Invalid keystroke: {}", s));
        }
        
        Ok(Self {
            ctrl,
            shift,
            alt,
            key: remainder.to_string(),
        })
    }
}

const DEFAULT_KEYMAPS_JSON: &str = r#"[
  {
    "context": "Workspace",
    "bindings": {
      "ctrl-=": "workspace::ZoomIn",
      "ctrl-plus": "workspace::ZoomIn",
      "ctrl--": "workspace::ZoomOut",
      "ctrl-shift-p": "workspace::CommandPalette",
      "ctrl-f": "workspace::Find"
    }
  },
  {
    "context": "Editor",
    "bindings": {
      "ctrl-s": "editor::Save",
      "ctrl-a": "editor::SelectAll",
      "ctrl-c": "editor::Copy",
      "ctrl-x": "editor::Cut",
      "ctrl-v": "editor::Paste",
      "ctrl-z": "editor::Undo",
      "ctrl-y": "editor::Redo",
      "escape": "editor::Escape",
      "backspace": "editor::DeleteLeft",
      "delete": "editor::DeleteRight",
      "enter": "editor::InsertNewLine",
      "tab": "editor::InsertTab",
      "left": "editor::MoveLeft",
      "right": "editor::MoveRight",
      "up": "editor::MoveUp",
      "down": "editor::MoveDown",
      "shift-left": "editor::SelectLeft",
      "shift-right": "editor::SelectRight",
      "shift-up": "editor::SelectUp",
      "shift-down": "editor::SelectDown",
      "ctrl-left": "editor::MoveWordLeft",
      "ctrl-right": "editor::MoveWordRight",
      "ctrl-shift-left": "editor::SelectWordLeft",
      "ctrl-shift-right": "editor::SelectWordRight",
      "home": "editor::MoveToLineStart",
      "shift-home": "editor::SelectToLineStart",
      "end": "editor::MoveToLineEnd",
      "shift-end": "editor::SelectToLineEnd",
      "alt-up": "editor::MoveLineUp",
      "alt-down": "editor::MoveLineDown",
      "shift-alt-up": "editor::DuplicateLine",
      "shift-alt-down": "editor::DuplicateLine",
      "ctrl-shift-k": "editor::DeleteLine"
    }
  }
]"#;

fn keycode_to_string(keycode: KeyCode) -> Option<&'static str> {
    match keycode {
        KeyCode::KeyA => Some("a"),
        KeyCode::KeyB => Some("b"),
        KeyCode::KeyC => Some("c"),
        KeyCode::KeyD => Some("d"),
        KeyCode::KeyE => Some("e"),
        KeyCode::KeyF => Some("f"),
        KeyCode::KeyG => Some("g"),
        KeyCode::KeyH => Some("h"),
        KeyCode::KeyI => Some("i"),
        KeyCode::KeyJ => Some("j"),
        KeyCode::KeyK => Some("k"),
        KeyCode::KeyL => Some("l"),
        KeyCode::KeyM => Some("m"),
        KeyCode::KeyN => Some("n"),
        KeyCode::KeyO => Some("o"),
        KeyCode::KeyP => Some("p"),
        KeyCode::KeyQ => Some("q"),
        KeyCode::KeyR => Some("r"),
        KeyCode::KeyS => Some("s"),
        KeyCode::KeyT => Some("t"),
        KeyCode::KeyU => Some("u"),
        KeyCode::KeyV => Some("v"),
        KeyCode::KeyW => Some("w"),
        KeyCode::KeyX => Some("x"),
        KeyCode::KeyY => Some("y"),
        KeyCode::KeyZ => Some("z"),
        KeyCode::Digit0 => Some("0"),
        KeyCode::Digit1 => Some("1"),
        KeyCode::Digit2 => Some("2"),
        KeyCode::Digit3 => Some("3"),
        KeyCode::Digit4 => Some("4"),
        KeyCode::Digit5 => Some("5"),
        KeyCode::Digit6 => Some("6"),
        KeyCode::Digit7 => Some("7"),
        KeyCode::Digit8 => Some("8"),
        KeyCode::Digit9 => Some("9"),
        KeyCode::Equal => Some("="),
        KeyCode::Minus => Some("-"),
        KeyCode::Slash => Some("/"),
        KeyCode::Backslash => Some("\\"),
        KeyCode::Semicolon => Some(";"),
        KeyCode::Quote => Some("'"),
        KeyCode::Comma => Some(","),
        KeyCode::Period => Some("."),
        KeyCode::BracketLeft => Some("["),
        KeyCode::BracketRight => Some("]"),
        KeyCode::Backquote => Some("`"),
        KeyCode::NumpadAdd => Some("+"),
        KeyCode::NumpadSubtract => Some("-"),
        KeyCode::Numpad0 => Some("0"),
        KeyCode::Numpad1 => Some("1"),
        KeyCode::Numpad2 => Some("2"),
        KeyCode::Numpad3 => Some("3"),
        KeyCode::Numpad4 => Some("4"),
        KeyCode::Numpad5 => Some("5"),
        KeyCode::Numpad6 => Some("6"),
        KeyCode::Numpad7 => Some("7"),
        KeyCode::Numpad8 => Some("8"),
        KeyCode::Numpad9 => Some("9"),
        KeyCode::ArrowLeft => Some("left"),
        KeyCode::ArrowRight => Some("right"),
        KeyCode::ArrowUp => Some("up"),
        KeyCode::ArrowDown => Some("down"),
        KeyCode::Home => Some("home"),
        KeyCode::End => Some("end"),
        KeyCode::Escape => Some("escape"),
        KeyCode::Backspace => Some("backspace"),
        KeyCode::Delete => Some("delete"),
        KeyCode::Enter => Some("enter"),
        KeyCode::Tab => Some("tab"),
        KeyCode::Space => Some("space"),
        KeyCode::PageUp => Some("pageup"),
        KeyCode::PageDown => Some("pagedown"),
        _ => None,
    }
}

fn normalize_key_name(key: &str) -> String {
    match key {
        "arrowleft" => "left".to_string(),
        "arrowright" => "right".to_string(),
        "arrowup" => "up".to_string(),
        "arrowdown" => "down".to_string(),
        "esc" => "escape".to_string(),
        "del" => "delete".to_string(),
        "return" => "enter".to_string(),
        "plus" => "+".to_string(),
        _ => key.to_string(),
    }
}

fn get_key_name(logical_key: &Key, physical_key: PhysicalKey, ctrl: bool) -> String {
    if ctrl {
        if let PhysicalKey::Code(keycode) = physical_key {
            if let Some(name) = keycode_to_string(keycode) {
                return name.to_string();
            }
        }
    }
    
    match logical_key {
        Key::Character(text) => {
            text.to_lowercase()
        }
        Key::Named(named_key) => {
            match named_key {
                NamedKey::ArrowLeft => "left".to_string(),
                NamedKey::ArrowRight => "right".to_string(),
                NamedKey::ArrowUp => "up".to_string(),
                NamedKey::ArrowDown => "down".to_string(),
                NamedKey::Home => "home".to_string(),
                NamedKey::End => "end".to_string(),
                NamedKey::Escape => "escape".to_string(),
                NamedKey::Backspace => "backspace".to_string(),
                NamedKey::Delete => "delete".to_string(),
                NamedKey::Enter => "enter".to_string(),
                NamedKey::Tab => "tab".to_string(),
                NamedKey::Space => "space".to_string(),
                NamedKey::PageUp => "pageup".to_string(),
                NamedKey::PageDown => "pagedown".to_string(),
                _ => format!("{:?}", named_key).to_lowercase(),
            }
        }
        _ => {
            if let PhysicalKey::Code(keycode) = physical_key {
                if let Some(name) = keycode_to_string(keycode) {
                    return name.to_string();
                }
            }
            "".to_string()
        }
    }
}

pub fn parse_action(action_str: &str) -> Option<Action> {
    match action_str {
        "workspace::ZoomIn" => Some(Action::ZoomIn),
        "workspace::ZoomOut" => Some(Action::ZoomOut),
        "workspace::CommandPalette" | "workspace::ToggleCommandPalette" => Some(Action::CommandPalette),
        "editor::Save" => Some(Action::SaveFile),
        "editor::Escape" | "workspace::Escape" => Some(Action::Escape),
        "editor::SelectAll" => Some(Action::SelectAll),
        "editor::Copy" => Some(Action::Copy),
        "editor::Cut" => Some(Action::Cut),
        "editor::Paste" => Some(Action::Paste),
        "editor::Undo" => Some(Action::Undo),
        "editor::Redo" => Some(Action::Redo),
        "editor::DeleteLeft" | "editor::Backspace" => Some(Action::DeleteLeft),
        "editor::DeleteRight" | "editor::Delete" => Some(Action::DeleteRight),
        "editor::InsertNewLine" | "editor::Newline" => Some(Action::InsertNewLine),
        "editor::InsertTab" | "editor::Tab" => Some(Action::InsertTab),
        
        "editor::MoveLeft" => Some(Action::MoveLeft { select: false, word: false }),
        "editor::SelectLeft" => Some(Action::MoveLeft { select: true, word: false }),
        "editor::MoveWordLeft" => Some(Action::MoveLeft { select: false, word: true }),
        "editor::SelectWordLeft" => Some(Action::MoveLeft { select: true, word: true }),
        
        "editor::MoveRight" => Some(Action::MoveRight { select: false, word: false }),
        "editor::SelectRight" => Some(Action::MoveRight { select: true, word: false }),
        "editor::MoveWordRight" => Some(Action::MoveRight { select: false, word: true }),
        "editor::SelectWordRight" => Some(Action::MoveRight { select: true, word: true }),
        
        "editor::MoveUp" => Some(Action::MoveUp { select: false }),
        "editor::SelectUp" => Some(Action::MoveUp { select: true }),
        
        "editor::MoveDown" => Some(Action::MoveDown { select: false }),
        "editor::SelectDown" => Some(Action::MoveDown { select: true }),
        
        "editor::MoveToLineStart" => Some(Action::MoveToLineStart { select: false }),
        "editor::SelectToLineStart" => Some(Action::MoveToLineStart { select: true }),
        
        "editor::MoveToLineEnd" => Some(Action::MoveToLineEnd { select: false }),
        "editor::SelectToLineEnd" => Some(Action::MoveToLineEnd { select: true }),
        
        "editor::MoveLineUp" => Some(Action::MoveLineUp),
        "editor::MoveLineDown" => Some(Action::MoveLineDown),
        "editor::DuplicateLine" => Some(Action::DuplicateLine),
        "editor::DeleteLine" => Some(Action::DeleteLine),
        "workspace::Find" | "editor::Find" => Some(Action::Find),
        
        _ => None,
    }
}

impl Keymap {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("garage").join("keymaps.json")
    }

    pub fn load() -> Self {
        let default_contexts: Vec<KeyContextBindings> = serde_json::from_str(DEFAULT_KEYMAPS_JSON)
            .expect("Failed to parse built-in default keymaps");

        let path = Self::config_path();
        let contexts = if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(loaded) = serde_json::from_str::<Vec<KeyContextBindings>>(&content) {
                    loaded
                } else {
                    log::error!("Failed to parse keymaps.json. Falling back to defaults.");
                    default_contexts.clone()
                }
            } else {
                default_contexts.clone()
            }
        } else {
            // Try to write the default keymaps file
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::write(&path, DEFAULT_KEYMAPS_JSON) {
                log::error!("Failed to write default keymaps.json: {:?}", e);
            }
            default_contexts.clone()
        };

        Self {
            contexts,
            defaults: default_contexts,
        }
    }

    pub fn lookup(&self, keystroke: &Keystroke, active_contexts: &[&str]) -> Option<Action> {
        // 1. Try user-defined keymap first
        if let Some(action) = self.lookup_in_bindings(&self.contexts, keystroke, active_contexts) {
            return Some(action);
        }
        
        // 2. Fall back to default keymap
        self.lookup_in_bindings(&self.defaults, keystroke, active_contexts)
    }

    fn lookup_in_bindings(&self, contexts: &[KeyContextBindings], keystroke: &Keystroke, active_contexts: &[&str]) -> Option<Action> {
        for context_name in active_contexts {
            for context_bindings in contexts {
                let context_matches = match &context_bindings.context {
                    Some(c) => c.to_lowercase() == context_name.to_lowercase(),
                    None => context_name.to_lowercase() == "workspace",
                };
                if context_matches {
                    let normalized_key = normalize_key_name(&keystroke.key);
                    
                    let mut key_str = String::new();
                    if keystroke.ctrl {
                        key_str.push_str("ctrl-");
                    }
                    if keystroke.alt {
                        key_str.push_str("alt-");
                    }
                    if keystroke.shift {
                        key_str.push_str("shift-");
                    }
                    key_str.push_str(&normalized_key);
                    
                    if let Some(action_str) = context_bindings.bindings.get(&key_str) {
                        if let Some(action) = parse_action(action_str) {
                            return Some(action);
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn map_key(
    keymap: &Keymap,
    logical_key: &Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
    contexts: &[&str],
) -> Option<Action> {
    let keystroke = Keystroke {
        ctrl,
        shift,
        alt,
        key: get_key_name(logical_key, physical_key, ctrl),
    };
    
    if let Some(action) = keymap.lookup(&keystroke, contexts) {
        return Some(action);
    }
    
    if contexts.contains(&"Editor") {
        if let Key::Character(text) = logical_key {
            if !ctrl && !alt && text.chars().count() == 1 {
                return Some(Action::InsertChar(text.chars().next().unwrap()));
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keystroke_parse() {
        let k = Keystroke::parse("ctrl-shift-p").unwrap();
        assert_eq!(k.ctrl, true);
        assert_eq!(k.shift, true);
        assert_eq!(k.alt, false);
        assert_eq!(k.key, "p");

        let k2 = Keystroke::parse("ctrl--").unwrap();
        assert_eq!(k2.ctrl, true);
        assert_eq!(k2.shift, false);
        assert_eq!(k2.alt, false);
        assert_eq!(k2.key, "-");
    }

    #[test]
    fn test_keymap_lookup() {
        let keymap = Keymap::load();
        let k = Keystroke {
            ctrl: true,
            shift: false,
            alt: false,
            key: "s".to_string(),
        };
        let action = keymap.lookup(&k, &["Editor"]);
        assert_eq!(action, Some(Action::SaveFile));
    }
}
