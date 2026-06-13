pub mod buffer;
pub mod cursor;
pub mod config;
pub mod actions;
pub mod keymap;
pub mod lsp;

#[derive(Debug, Clone)]
pub struct DiagnosticDetail {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub severity: u32,
    pub message: String,
}

pub fn detect_language_id(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust",
        "toml" => "toml",
        "json" => "json",
        "md" => "markdown",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "yaml" | "yml" => "yaml",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "cpp",
        "go" => "go",
        "sh" | "bash" => "shellscript",
        "lua" => "lua",
        "rb" => "ruby",
        "java" => "java",
        "xml" => "xml",
        "sql" => "sql",
        _ => "plaintext",
    }
}

pub fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};

    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek() {
        let buf = PathBuf::from(c.as_os_str());
        components.next();
        buf
    } else {
        PathBuf::new()
    };

    let mut normalized = Vec::new();
    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(Component::RootDir.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(c) => {
                normalized.push(c);
            }
        }
    }
    for component in normalized {
        ret.push(component);
    }
    ret
}

pub fn get_absolute_path(path: &str) -> String {
    let path_buf = std::path::PathBuf::from(path);
    let abs_path = if path_buf.is_absolute() {
        path_buf
    } else {
        std::env::current_dir().unwrap_or_default().join(path_buf)
    };
    normalize_path(&abs_path)
        .to_string_lossy()
        .to_string()
}

