use std::path::Path;
use super::{UiState, FileNode};

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

fn scan_dir_recursive(dir: &Path, depth: usize, expanded_dirs: &std::collections::HashSet<std::path::PathBuf>, visible_nodes: &mut Vec<FileNode>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut entries_vec = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Skip large/ignored folders to optimize directory scanning performance
                if name == ".git" || name == "target" || name == ".gemini" {
                    continue;
                }
                
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
}
