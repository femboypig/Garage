use std::path::Path;
use super::ui_state::UiState;
use super::types::FileNode;

impl UiState {
    /// Re-scan the directory to populate the project tree asynchronously
    pub fn rebuild_tree(&mut self) {
        let expanded = self.expanded_dirs.clone();
        let tx = self.tree_tx.clone();
        let proxy = self.event_loop_proxy.clone();

        std::thread::spawn(move || {
            let mut nodes = Vec::new();
            scan_dir_recursive(Path::new("."), 0, &expanded, &mut nodes);
            let _ = tx.send(nodes);
            let _ = proxy.send_event(());
        });
    }
}

fn is_hidden(path: &Path) -> bool {
    path.components().any(|comp| {
        if let std::path::Component::Normal(name) = comp {
            name.to_string_lossy().starts_with('.')
        } else {
            false
        }
    })
}

fn scan_dir_recursive(dir: &Path, depth: usize, expanded_dirs: &std::collections::HashSet<std::path::PathBuf>, visible_nodes: &mut Vec<FileNode>) {
    let walker = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(true)
        .git_ignore(true)
        .parents(true)
        .build();

    let mut entries_vec = Vec::new();
    for result in walker {
        if let Ok(entry) = result {
            if entry.depth() == 0 {
                continue;
            }
            let path = entry.path().to_path_buf();
            if is_hidden(&path) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            entries_vec.push((path, name, is_dir));
        }
    }

    // Sort: directories first, then files alphabetically
    entries_vec.sort_by(|a, b| {
        if a.2 != b.2 {
            b.2.cmp(&a.2)
        } else {
            a.1.cmp(&b.1)
        }
    });

    for (path, name, is_dir) in entries_vec {
        let is_expanded = expanded_dirs.contains(&path);
        visible_nodes.push(FileNode {
            path: path.clone(),
            name,
            is_dir,
            depth,
        });
        if is_dir && is_expanded {
            scan_dir_recursive(&path, depth + 1, expanded_dirs, visible_nodes);
        }
    }
}

