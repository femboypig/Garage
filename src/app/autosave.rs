use crate::app::state::{AppState, Tab};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TabSession {
    pub path: Option<String>,
    pub cursor: Cursor,
    pub secondary_cursors: Vec<Cursor>,
    pub scroll_x: usize,
    pub scroll_y: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionState {
    pub tabs: Vec<TabSession>,
    pub active_tab_idx: usize,
}

pub fn get_autosave_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("garage")
        .join("autosave")
}

pub fn get_autosave_path(path: Option<&str>, tab_idx: usize) -> PathBuf {
    let dir = get_autosave_dir();
    match path {
        Some(p) => {
            let abs_path = crate::editor::get_absolute_path(p);
            let hex_name: String = abs_path.bytes().map(|b| format!("{:02x}", b)).collect();
            dir.join(hex_name)
        }
        None => dir.join(format!("untitled_{}", tab_idx)),
    }
}

pub fn delete_autosave(path: Option<&str>, tab_idx: usize) {
    let path_buf = get_autosave_path(path, tab_idx);
    if path_buf.exists() {
        let _ = fs::remove_file(path_buf);
    }
}

pub fn save_session_and_dirty_buffers(state: &AppState) {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_dir = PathBuf::from(home).join(".config").join("garage");
    let session_path = config_dir.join("session.json");

    // Ensure directories exist
    let autosave_dir = get_autosave_dir();
    if autosave_dir.exists() {
        let _ = fs::remove_dir_all(&autosave_dir);
    }
    let _ = fs::create_dir_all(&autosave_dir);
    let _ = fs::create_dir_all(&config_dir);

    // Save dirty buffers
    for (i, tab) in state.tabs.iter().enumerate() {
        if let Some(ref path) = tab.path {
            if path.starts_with("diagnostics://") {
                continue;
            }
        }
        let autosave_file = get_autosave_path(tab.path.as_deref(), i);
        if tab.buffer.is_modified {
            let content = tab.buffer.lines().join("\n");
            let _ = fs::write(autosave_file, content);
        } else {
            if autosave_file.exists() {
                let _ = fs::remove_file(autosave_file);
            }
        }
    }

    // Save session
    let mut tab_sessions = Vec::new();
    for tab in &state.tabs {
        if let Some(ref path) = tab.path {
            if path.starts_with("diagnostics://") {
                continue;
            }
        }
        tab_sessions.push(TabSession {
            path: tab.path.clone(),
            cursor: tab.cursor,
            secondary_cursors: tab.secondary_cursors.clone(),
            scroll_x: tab.scroll_x,
            scroll_y: tab.scroll_y,
        });
    }

    let session_state = SessionState {
        tabs: tab_sessions,
        active_tab_idx: state.active_tab_idx,
    };

    if let Ok(json_str) = serde_json::to_string_pretty(&session_state) {
        let _ = fs::write(session_path, json_str);
    }
}

pub fn load_session_and_restore_buffers() -> Option<(Vec<Tab>, usize)> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let session_path = PathBuf::from(home)
        .join(".config")
        .join("garage")
        .join("session.json");
    if !session_path.exists() {
        return None;
    }

    let json_str = fs::read_to_string(session_path).ok()?;
    let session_state: SessionState = serde_json::from_str(&json_str).ok()?;

    if session_state.tabs.is_empty() {
        return None;
    }

    let mut restored_tabs = Vec::new();
    for (i, ts) in session_state.tabs.iter().enumerate() {
        let mut buffer = Buffer::new();
        let mut load_success = false;

        if let Some(ref p) = ts.path {
            let abs_path = crate::editor::get_absolute_path(p);
            if buffer.load_file(&abs_path).is_ok() {
                load_success = true;
            }
        }

        let autosave_file = get_autosave_path(ts.path.as_deref(), i);
        let mut is_modified = false;
        if autosave_file.exists() {
            if let Ok(autosave_content) = fs::read_to_string(&autosave_file) {
                buffer = Buffer::from_text(&autosave_content);
                is_modified = true;
                load_success = true;
            }
        }

        if ts.path.is_none() && !autosave_file.exists() {
            load_success = true;
        }

        if load_success {
            if is_modified {
                buffer.is_modified = true;
            }
            restored_tabs.push(Tab {
                path: ts.path.clone(),
                buffer,
                cursor: ts.cursor,
                secondary_cursors: ts.secondary_cursors.clone(),
                scroll_x: ts.scroll_x,
                scroll_y: ts.scroll_y,
            });
        }
    }

    if restored_tabs.is_empty() {
        None
    } else {
        let active_tab_idx = session_state.active_tab_idx.min(restored_tabs.len() - 1);
        Some((restored_tabs, active_tab_idx))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosaveTrigger {
    FocusChange,
    WindowChange,
    Delay,
}

pub fn should_save_on_close(autosave: &crate::editor::config::AutosaveSetting) -> bool {
    matches!(
        autosave,
        crate::editor::config::AutosaveSetting::OnFocusChange
            | crate::editor::config::AutosaveSetting::OnWindowChange
            | crate::editor::config::AutosaveSetting::AfterDelay { .. }
    )
}

pub fn save_tab(ui: &mut crate::machkit::UiState, tab: &mut Tab) {
    if let Some(ref path_to_save) = tab.path {
        if path_to_save.starts_with("diagnostics://") || path_to_save.starts_with("search://") {
            return;
        }
        if tab.buffer.is_modified {
            if let Err(e) = tab.buffer.save_file(path_to_save) {
                log::error!("Failed to auto-save file '{}': {:?}", path_to_save, e);
            } else {
                tab.buffer.mark_saved();
                ui.rebuild_tree();
                ui.update_git_diff(Some(path_to_save));
                ui.update_git_file_blame(Some(path_to_save));
                ui.update_git_statuses();
                ui.external_change_warnings.remove(path_to_save);
            }
        }
    }
}

pub fn run_autosave_if_needed(
    ui: &mut crate::machkit::UiState,
    state: &mut AppState,
    trigger: AutosaveTrigger,
) {
    let autosave_setting = &ui.config.autosave;
    match autosave_setting {
        crate::editor::config::AutosaveSetting::Off => {}
        crate::editor::config::AutosaveSetting::AfterDelay { .. } => {
            // Save active tab or any modified tabs when trigger is Delay, FocusChange or WindowChange
            for tab in &mut state.tabs {
                save_tab(ui, tab);
            }
            for pane in &mut state.inactive_panes {
                for tab in &mut pane.tabs {
                    save_tab(ui, tab);
                }
            }
        }
        crate::editor::config::AutosaveSetting::OnFocusChange => {
            if trigger == AutosaveTrigger::FocusChange || trigger == AutosaveTrigger::WindowChange {
                for tab in &mut state.tabs {
                    save_tab(ui, tab);
                }
                for pane in &mut state.inactive_panes {
                    for tab in &mut pane.tabs {
                        save_tab(ui, tab);
                    }
                }
            }
        }
        crate::editor::config::AutosaveSetting::OnWindowChange => {
            if trigger == AutosaveTrigger::WindowChange {
                for tab in &mut state.tabs {
                    save_tab(ui, tab);
                }
                for pane in &mut state.inactive_panes {
                    for tab in &mut pane.tabs {
                        save_tab(ui, tab);
                    }
                }
            }
        }
    }
}
