use std::path::{Component, Path, PathBuf};

use crate::machkit::SidebarInputMode;

pub fn is_safe_sidebar_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

pub fn apply_sidebar_input(mode: SidebarInputMode, target: &Path, value: &str) {
    if !is_safe_sidebar_name(value) {
        return;
    }

    match mode {
        SidebarInputMode::NewFile => {
            let parent = sidebar_parent(target);
            let _ = std::fs::File::create(parent.join(value));
        }
        SidebarInputMode::NewFolder => {
            let parent = sidebar_parent(target);
            let _ = std::fs::create_dir_all(parent.join(value));
        }
        SidebarInputMode::Rename => {
            if let Some(parent) = target.parent() {
                let _ = std::fs::rename(target, parent.join(value));
            }
        }
        SidebarInputMode::Delete => {
            let expected = target
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if value == expected {
                if target.is_dir() {
                    let _ = std::fs::remove_dir_all(target);
                } else {
                    let _ = std::fs::remove_file(target);
                }
            }
        }
    }
}

fn sidebar_parent(target: &Path) -> PathBuf {
    if target.is_dir() {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map(|path| path.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
