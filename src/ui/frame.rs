use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;
use crate::terminal::TerminalInstance;
use super::{UiState, UiAction, ModalType};

impl UiState {
    pub fn get_max_line_len(&mut self, buffer: &Buffer, active_file_path: Option<&str>, cursor_line: usize) -> usize {
        let mut max_len = 0;
        for (line_idx, line) in buffer.lines().iter().enumerate() {
            let mut len = line.chars().count();
            if self.config.show_git_blame && line_idx == cursor_line {
                if let Some(blame_str) = self.get_or_update_blame(active_file_path, line_idx) {
                    if blame_str != "Loading blame..." && !blame_str.is_empty() {
                        len += 4 + blame_str.chars().count();
                    }
                }
            }
            if len > max_len {
                max_len = len;
            }
        }
        max_len
    }

    /// Push a solid rectangle (quad) into the vertex and index vectors
    pub fn push_quad(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        white_uv: [f32; 2],
        color: [f32; 4],
    ) {
        // Round panel coordinates to integer pixels for crisp borders
        let rx = x.round();
        let ry = y.round();
        let rw = w.round();
        let rh = h.round();

        let start = vertices.len() as u16;
        vertices.push(Vertex {
            position: [rx, ry],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [rx + rw, ry],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [rx + rw, ry + rh],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [rx, ry + rh],
            tex_coords: white_uv,
            color,
        });
        indices.extend_from_slice(&[start, start + 1, start + 2, start + 2, start + 3, start]);
    }

    /// Push a single text character glyph using the font atlas
    pub fn push_char(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        c: char,
        pen_x: f32,
        baseline_y: f32,
        color: [f32; 4],
        font_size: f32,
        char_width: f32,
    ) -> f32 {
        if let Some(info) = atlas.get_or_rasterize(queue, c, font_size) {
            if info.width == 0.0 || info.height == 0.0 {
                return char_width;
            }

            // CRITICAL: Round coordinates to exact integer pixels to eliminate bilinear filtering blur!
            let x = (pen_x + info.bearing_x).round();
            let y = (baseline_y - info.bearing_y - info.height).round();
            let w = info.width.round();
            let h = info.height.round();

            let start = vertices.len() as u16;
            vertices.push(Vertex {
                position: [x, y],
                tex_coords: info.uv_min,
                color,
            });
            vertices.push(Vertex {
                position: [x + w, y],
                tex_coords: [info.uv_max[0], info.uv_min[1]],
                color,
            });
            vertices.push(Vertex {
                position: [x + w, y + h],
                tex_coords: info.uv_max,
                color,
            });
            vertices.push(Vertex {
                position: [x, y + h],
                tex_coords: [info.uv_min[0], info.uv_max[1]],
                color,
            });
            indices.extend_from_slice(&[start, start + 1, start + 2, start + 2, start + 3, start]);
        }
        char_width
    }

    pub fn push_icon(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        icon_path: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        size: f32,
    ) -> f32 {
        if let Some(info) = atlas.get_or_rasterize_icon(queue, icon_path, size) {
            let start = vertices.len() as u16;
            let rx = x.round();
            let ry = y.round();
            let rw = info.width.round();
            let rh = info.height.round();

            vertices.push(Vertex {
                position: [rx, ry],
                tex_coords: [info.uv_min[0], info.uv_min[1]],
                color,
            });
            vertices.push(Vertex {
                position: [rx + rw, ry],
                tex_coords: [info.uv_max[0], info.uv_min[1]],
                color,
            });
            vertices.push(Vertex {
                position: [rx + rw, ry + rh],
                tex_coords: [info.uv_max[0], info.uv_max[1]],
                color,
            });
            vertices.push(Vertex {
                position: [rx, ry + rh],
                tex_coords: [info.uv_min[0], info.uv_max[1]],
                color,
            });

            indices.extend_from_slice(&[start, start + 1, start + 2, start + 2, start + 3, start]);
            rw
        } else {
            0.0
        }
    }

    pub fn push_str(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        text: &str,
        mut x: f32,
        y: f32,
        color: [f32; 4],
        font_size: f32,
        char_width: f32,
    ) -> f32 {
        let start_x = x;
        for c in text.chars() {
            x += self.push_char(vertices, indices, atlas, queue, c, x, y, color, font_size, char_width);
        }
        x - start_x
    }

    /// Parse enclosing function/struct backwards from cursor line
    pub fn find_current_function(&self, buffer: &Buffer, cursor_line: usize) -> Option<String> {
        for i in (0..=cursor_line).rev() {
            if i >= buffer.len() {
                continue;
            }
            let line = &buffer.lines()[i];
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") || trimmed.starts_with("pub(crate) fn ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for (idx, &part) in parts.iter().enumerate() {
                    if part == "fn" && idx + 1 < parts.len() {
                        let fn_name_full = parts[idx + 1];
                        let fn_name = fn_name_full.split('(').next().unwrap_or(fn_name_full);
                        return Some(format!("fn {}", fn_name));
                    }
                }
            } else if trimmed.starts_with("impl ") || trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for (idx, &part) in parts.iter().enumerate() {
                    if (part == "struct" || part == "impl") && idx + 1 < parts.len() {
                        let name_full = parts[idx + 1];
                        let name = name_full.split('{').next().unwrap_or(name_full);
                        return Some(format!("{} {}", part, name));
                    }
                }
            }
        }
        None
    }

    /// Basic fallback syntax highlighting when no LSP semantic tokens are available
    pub fn get_line_char_colors(&self, line_text: &str, path_opt: Option<&str>) -> Vec<[f32; 4]> {
        let chars: Vec<char> = line_text.chars().collect();
        let len = chars.len();
        let mut colors = vec![self.config.theme.syntax_default; len];
        if len == 0 {
            return colors;
        }

        let ext = path_opt
            .and_then(|p| std::path::Path::new(p).extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "toml" => {
                let mut i = 0;
                let mut in_value = false;
                
                let trimmed = line_text.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    for j in 0..len {
                        colors[j] = self.config.theme.syntax_keyword;
                    }
                    return colors;
                }

                while i < len {
                    if chars[i].is_whitespace() {
                        i += 1;
                        continue;
                    }
                    if chars[i] == '#' {
                        for j in i..len {
                            colors[j] = self.config.theme.syntax_comment;
                        }
                        break;
                    }
                    if chars[i] == '"' || chars[i] == '\'' {
                        let quote = chars[i];
                        colors[i] = self.config.theme.syntax_string;
                        let mut j = i + 1;
                        while j < len {
                            colors[j] = self.config.theme.syntax_string;
                            if chars[j] == quote && (j == 0 || chars[j - 1] != '\\') {
                                break;
                            }
                            j += 1;
                        }
                        i = j + 1;
                        continue;
                    }
                    if chars[i] == '=' {
                        colors[i] = self.config.theme.syntax_operator;
                        in_value = true;
                        i += 1;
                        continue;
                    }
                    if !in_value {
                        let start = i;
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-') {
                            colors[i] = self.config.theme.syntax_property;
                            i += 1;
                        }
                        if start == i {
                            i += 1;
                        }
                    } else {
                        if chars[i].is_ascii_digit() || chars[i] == '-' || chars[i] == '+' {
                            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_' || chars[i] == '-') {
                                colors[i] = self.config.theme.syntax_number;
                                i += 1;
                            }
                            continue;
                        }
                        if chars[i].is_alphabetic() {
                            let start = i;
                            while i < len && chars[i].is_alphabetic() {
                                i += 1;
                            }
                            let word: String = chars[start..i].iter().collect();
                            let color = match word.as_str() {
                                "true" | "false" => self.config.theme.syntax_keyword,
                                _ => self.config.theme.syntax_default,
                            };
                            for j in start..i {
                                colors[j] = color;
                            }
                            continue;
                        }
                        i += 1;
                    }
                }
            }
            "json" => {
                let mut i = 0;
                while i < len {
                    if chars[i].is_whitespace() {
                        i += 1;
                        continue;
                    }
                    if chars[i] == '"' {
                        let mut j = i + 1;
                        while j < len {
                            if chars[j] == '"' && (j == 0 || chars[j - 1] != '\\') {
                                break;
                            }
                            j += 1;
                        }
                        let mut next = j + 1;
                        while next < len && chars[next].is_whitespace() {
                            next += 1;
                        }
                        let is_key = next < len && chars[next] == ':';
                        let color = if is_key { self.config.theme.syntax_property } else { self.config.theme.syntax_string };
                        for k in i..=(j.min(len - 1)) {
                            colors[k] = color;
                        }
                        i = j + 1;
                        continue;
                    }
                    if chars[i] == ':' || chars[i] == ',' || chars[i] == '{' || chars[i] == '}' || chars[i] == '[' || chars[i] == ']' {
                        colors[i] = self.config.theme.syntax_operator;
                        i += 1;
                        continue;
                    }
                    if chars[i].is_ascii_digit() || chars[i] == '-' {
                        while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '-' || chars[i] == '+') {
                            colors[i] = self.config.theme.syntax_number;
                            i += 1;
                        }
                        continue;
                    }
                    if chars[i].is_alphabetic() {
                        let start = i;
                        while i < len && chars[i].is_alphabetic() {
                            i += 1;
                        }
                        let word: String = chars[start..i].iter().collect();
                        let color = match word.as_str() {
                            "true" | "false" | "null" => self.config.theme.syntax_keyword,
                            _ => self.config.theme.syntax_default,
                        };
                        for j in start..i {
                            colors[j] = color;
                        }
                        continue;
                    }
                    i += 1;
                }
            }
            "md" | "markdown" => {
                if line_text.starts_with('#') {
                    for j in 0..len {
                        colors[j] = self.config.theme.syntax_keyword;
                    }
                    return colors;
                }
                let mut i = 0;
                while i < len {
                    if chars[i] == '`' {
                        let mut j = i + 1;
                        while j < len && chars[j] != '`' {
                            j += 1;
                        }
                        let end = (j + 1).min(len);
                        for k in i..end {
                            colors[k] = self.config.theme.syntax_attribute;
                        }
                        i = end;
                        continue;
                    }
                    if chars[i] == '[' {
                        let mut j = i + 1;
                        while j < len && chars[j] != ']' {
                            j += 1;
                        }
                        if j < len {
                            colors[i] = self.config.theme.syntax_operator;
                            colors[j] = self.config.theme.syntax_operator;
                            for k in (i+1)..j {
                                colors[k] = self.config.theme.syntax_string;
                            }
                            i = j + 1;
                            continue;
                        }
                    }
                    if chars[i] == '(' {
                        let mut j = i + 1;
                        while j < len && chars[j] != ')' {
                            j += 1;
                        }
                        if j < len {
                            colors[i] = self.config.theme.syntax_operator;
                            colors[j] = self.config.theme.syntax_operator;
                            for k in (i+1)..j {
                                colors[k] = self.config.theme.syntax_type;
                            }
                            i = j + 1;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
            _ => {
                let is_python = ext == "py" || ext == "python";
                let mut i = 0;
                while i < len {
                    if is_python && chars[i] == '#' {
                        for j in i..len {
                            colors[j] = self.config.theme.syntax_comment;
                        }
                        break;
                    }
                    if !is_python && i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
                        for j in i..len {
                            colors[j] = self.config.theme.syntax_comment;
                        }
                        break;
                    }
                    if chars[i] == '"' || chars[i] == '\'' {
                        let quote = chars[i];
                        colors[i] = self.config.theme.syntax_string;
                        let mut j = i + 1;
                        while j < len {
                            colors[j] = self.config.theme.syntax_string;
                            if chars[j] == quote && (j == 0 || chars[j - 1] != '\\') {
                                break;
                            }
                            j += 1;
                        }
                        i = j + 1;
                        continue;
                    }
                    if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric() && chars[i - 1] != '_') {
                        while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.' || chars[i] == 'x' || chars[i] == 'b' || chars[i] == 'o') {
                            colors[i] = self.config.theme.syntax_number;
                            i += 1;
                        }
                        continue;
                    }
                    if chars[i].is_alphabetic() || chars[i] == '_' {
                        let start = i;
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            i += 1;
                        }
                        let word: String = chars[start..i].iter().collect();
                        let color = match word.as_str() {
                            "fn" | "let" | "mut" | "const" | "static" | "if" | "else" | "match" | "for" |
                            "while" | "loop" | "break" | "continue" | "return" | "pub" | "mod" | "use" |
                            "struct" | "enum" | "impl" | "trait" | "type" | "where" | "as" | "in" |
                            "ref" | "self" | "Self" | "super" | "crate" | "async" | "await" | "move" |
                            "dyn" | "unsafe" | "extern" | "true" | "false" |
                            "def" | "class" | "import" | "from" | "elif" | "try" | "except" | "finally" |
                            "with" | "pass" | "lambda" | "is" | "and" | "or" | "not" | "yield" | "global" |
                            "var" | "function" | "const_cast" | "dynamic_cast" | "reinterpret_cast" |
                            "static_cast" | "typename" | "namespace" | "using" | "template" | "inline" |
                            "virtual" | "override" | "final" | "package" | "import_path" | "go" | "chan" |
                            "select" | "nil" | "null" | "True" | "False" => self.config.theme.syntax_keyword,
                            
                            "String" | "Vec" | "Option" | "Result" | "Box" | "Arc" | "Mutex" | "HashMap" |
                            "HashSet" | "PathBuf" | "Path" | "Rc" | "Cell" | "RefCell" | "Cow" |
                            "u8" | "u16" | "u32" | "u64" | "u128" | "usize" |
                            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" |
                            "f32" | "f64" | "bool" | "char" | "str" |
                            "int" | "float" | "double" | "void" | "string" | "boolean" | "number" |
                            "object" | "any" | "unknown" | "never" => self.config.theme.syntax_type,
                            
                            "Some" | "None" | "Ok" | "Err" => self.config.theme.syntax_enum_member,
                            _ => {
                                let mut next = i;
                                while next < len && chars[next].is_whitespace() {
                                    next += 1;
                                }
                                if next < len && chars[next] == '(' {
                                    self.config.theme.syntax_attribute
                                } else if chars[start].is_uppercase() {
                                    self.config.theme.syntax_type
                                } else {
                                    self.config.theme.syntax_default
                                }
                            }
                        };
                        for j in start..i {
                            colors[j] = color;
                        }
                        continue;
                    }
                    if chars[i] == '#' && !is_python {
                        while i < len && !chars[i].is_whitespace() && chars[i] != ';' {
                            colors[i] = self.config.theme.syntax_attribute;
                            i += 1;
                        }
                        continue;
                    }
                    if chars[i] == '@' && is_python {
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '@') {
                            colors[i] = self.config.theme.syntax_attribute;
                            i += 1;
                        }
                        continue;
                    }
                    if chars[i] == '+' || chars[i] == '-' || chars[i] == '*' || chars[i] == '/' || chars[i] == '=' || chars[i] == '!' || chars[i] == '<' || chars[i] == '>' || chars[i] == '&' || chars[i] == '|' || chars[i] == '^' || chars[i] == '%' {
                        colors[i] = self.config.theme.syntax_operator;
                    }
                    i += 1;
                }
            }
        }

        colors
    }


    pub fn update_git_branch(&mut self) {
        let tx = self.git_branch_tx.clone();
        let proxy = self.event_loop_proxy.clone();
        std::thread::spawn(move || {
            let output = std::process::Command::new("git")
                .args(&["rev-parse", "--abbrev-ref", "HEAD"])
                .output();
            
            if let Ok(out) = output {
                if out.status.success() {
                    let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !branch.is_empty() {
                        let _ = tx.send(branch);
                        let _ = proxy.send_event(());
                    }
                }
            }
        });
    }

    pub fn update_git_statuses(&mut self) {
        let tx = self.git_status_tx.clone();
        let proxy = self.event_loop_proxy.clone();
        std::thread::spawn(move || {
            let output = std::process::Command::new("git")
                .args(&["status", "--porcelain"])
                .output();
            
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut map = std::collections::HashMap::new();
                    for line in stdout.lines() {
                        if line.len() > 3 {
                            let status = line[0..2].to_string();
                            let file_path = std::path::PathBuf::from(line[3..].trim().to_string());
                            map.insert(file_path, status);
                        }
                    }
                    let _ = tx.send(map);
                    let _ = proxy.send_event(());
                }
            }
        });
    }

    pub fn update_git_diff(&mut self, file_path: Option<&str>) {
        let file_path = match file_path {
            Some(p) => p.to_string(),
            None => return,
        };
        let tx = self.git_diff_tx.clone();
        let proxy = self.event_loop_proxy.clone();
        std::thread::spawn(move || {
            let output = std::process::Command::new("git")
                .args(&["diff", "--no-ext-diff", "-U0", "--", &file_path])
                .output();
            
            if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut hunks = Vec::new();
                
                for line in stdout.lines() {
                    if line.starts_with("@@ ") {
                        let parts: Vec<&str> = line.split("@@").collect();
                        if parts.len() >= 2 {
                            let header = parts[1].trim();
                            let specs: Vec<&str> = header.split_whitespace().collect();
                            if specs.len() >= 2 {
                                let new_spec = specs[1];
                                if new_spec.starts_with('+') {
                                    let content = &new_spec[1..];
                                    let subparts: Vec<&str> = content.split(',').collect();
                                    if !subparts.is_empty() {
                                        let line_idx = subparts[0].parse::<usize>().unwrap_or(1).saturating_sub(1);
                                        let count = if subparts.len() >= 2 {
                                            subparts[1].parse::<usize>().unwrap_or(1)
                                        } else {
                                            1
                                        };
                                        
                                        let old_spec = specs[0];
                                        let old_count = if old_spec.starts_with('-') {
                                            let old_content = &old_spec[1..];
                                            let old_subparts: Vec<&str> = old_content.split(',').collect();
                                            if old_subparts.len() >= 2 {
                                                old_subparts[1].parse::<usize>().unwrap_or(1)
                                            } else {
                                                1
                                            }
                                        } else {
                                            1
                                        };

                                        if old_count == 0 {
                                            hunks.push(crate::ui::types::GitDiffHunk::Added { line: line_idx, count });
                                        } else if count == 0 {
                                            hunks.push(crate::ui::types::GitDiffHunk::Deleted { line: line_idx });
                                        } else {
                                            hunks.push(crate::ui::types::GitDiffHunk::Modified { line: line_idx, count });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = tx.send((file_path, hunks));
                let _ = proxy.send_event(());
            }
        });
    }

    pub fn get_or_update_blame(&self, file_path: Option<&str>, line_idx: usize) -> Option<String> {
        let file_path = file_path?;
        if let Some(blame_map) = self.git_file_blames.get(file_path) {
            blame_map.get(&line_idx).cloned()
        } else {
            None
        }
    }

    pub fn update_git_file_blame(&mut self, file_path: Option<&str>) {
        let file_path = match file_path {
            Some(p) => p.to_string(),
            None => return,
        };
        let tx = self.git_blame_file_tx.clone();
        let proxy = self.event_loop_proxy.clone();
        std::thread::spawn(move || {
            let output = std::process::Command::new("git")
                .args(&["blame", "--porcelain", &file_path])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    
                    struct CommitInfo {
                        author: String,
                        time: u64,
                        summary: String,
                    }

                    let mut commits = std::collections::HashMap::<String, CommitInfo>::new();
                    let mut line_commits = std::collections::HashMap::<usize, String>::new();
                    
                    let mut lines = stdout.lines();
                    while let Some(line) = lines.next() {
                        if line.starts_with('\t') {
                            continue;
                        }
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }
                        let first_part = parts[0];
                        if first_part.len() == 40 && parts.len() >= 3 {
                            let commit_hash = first_part.to_string();
                            if let Ok(result_line) = parts[2].parse::<usize>() {
                                line_commits.insert(result_line, commit_hash.clone());
                                if !commits.contains_key(&commit_hash) {
                                    let mut author = None;
                                    let mut author_time = None;
                                    let mut summary = None;
                                    
                                    while let Some(hdr_line) = lines.next() {
                                        if hdr_line.starts_with('\t') {
                                            break;
                                        }
                                        if hdr_line.starts_with("author ") {
                                            author = Some(hdr_line["author ".len()..].trim().to_string());
                                        } else if hdr_line.starts_with("author-time ") {
                                            author_time = hdr_line["author-time ".len()..].trim().parse::<u64>().ok();
                                        } else if hdr_line.starts_with("summary ") {
                                            summary = Some(hdr_line["summary ".len()..].trim().to_string());
                                        }
                                    }
                                    
                                    if let (Some(auth), Some(time), Some(sum)) = (author, author_time, summary) {
                                        commits.insert(commit_hash, CommitInfo {
                                            author: auth,
                                            time,
                                            summary: sum,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let mut file_blame_map = std::collections::HashMap::new();
                    for (result_line, commit_hash) in line_commits {
                        if let Some(info) = commits.get(&commit_hash) {
                            let blame_str = if info.author == "Not Committed Yet" {
                                "Not Committed Yet".to_string()
                            } else {
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let diff = now.saturating_sub(info.time);
                                let time_str = if diff < 60 {
                                    "just now".to_string()
                                } else if diff < 3600 {
                                    format!("{}m ago", diff / 60)
                                } else if diff < 86400 {
                                    format!("{}h ago", diff / 3600)
                                } else if diff < 2592000 {
                                    let days = diff / 86400;
                                    if days == 1 { "yesterday".to_string() } else { format!("{} days ago", days) }
                                } else if diff < 31536000 {
                                    let months = diff / 2592000;
                                    if months == 1 { "1 month ago".to_string() } else { format!("{} months ago", months) }
                                } else {
                                    let years = diff / 31536000;
                                    if years == 1 { "1 year ago".to_string() } else { format!("{} years ago", years) }
                                };
                                format!("{} • {} • {}", info.author, time_str, info.summary)
                            };
                            file_blame_map.insert(result_line - 1, blame_str);
                        }
                    }

                    let _ = tx.send((file_path, file_blame_map));
                    let _ = proxy.send_event(());
                }
            }
        });
    }

    pub fn get_all_commands(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Theme: Light Theme", "Switch to the Light Theme"),
            ("Theme: Dark Theme", "Switch to the default Dark Theme"),
            ("Sidebar: Toggle Visibility", "Show or hide the file tree sidebar"),
            ("Font Size: Increase Editor Font", "Increase the text size of the editor"),
            ("Font Size: Decrease Editor Font", "Decrease the text size of the editor"),
            ("Git Blame: Toggle Inline Annotations", "Enable/disable inline git blame"),
            ("Git Branch: Toggle Branch Statusbar", "Enable/disable git branch status"),
            ("Settings: Open settings modal", "Configure editor options"),
            ("About: Open about dialog", "View editor information"),
            ("Exit: Quit Garage", "Close the code editor"),
        ]
    }

    pub fn get_filtered_commands(&self) -> Vec<(&'static str, &'static str)> {
        let query = self.command_palette_query.to_lowercase();
        if query.is_empty() {
            return self.get_all_commands();
        }
        self.get_all_commands()
            .into_iter()
            .filter(|(name, desc)| {
                name.to_lowercase().contains(&query) || desc.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn execute_command(
        &mut self,
        cmd: (&'static str, &'static str),
        _buffer: &mut Buffer,
        _cursor: &mut Cursor,
    ) -> UiAction {
        match cmd.0 {
            "Theme: Light Theme" => UiAction::ChangeTheme("Light Theme".to_string()),
            "Theme: Dark Theme" => UiAction::ChangeTheme("Dark Theme".to_string()),
            "Sidebar: Toggle Visibility" => UiAction::ToggleSidebar,
            "Font Size: Increase Editor Font" => UiAction::ChangeBufferFontSize(1.0),
            "Font Size: Decrease Editor Font" => UiAction::ChangeBufferFontSize(-1.0),
            "Git Blame: Toggle Inline Annotations" => {
                let enabled = !self.config.show_git_blame;
                UiAction::ChangeGitBlame(enabled)
            }
            "Git Branch: Toggle Branch Statusbar" => {
                let enabled = !self.config.show_git_branch;
                UiAction::ChangeGitBranch(enabled)
            }
            "Settings: Open settings modal" => {
                self.active_modal = Some(ModalType::Settings);
                UiAction::None
            }
            "About: Open about dialog" => {
                self.active_modal = Some(ModalType::About);
                UiAction::None
            }
            "Exit: Quit Garage" => UiAction::Exit,
            _ => UiAction::None,
        }
    }

    /// Build entire UI frame (Titlebar, Sidebar, Scrollbar, Dropdowns, Modals)
    pub fn build_frame(
        &mut self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        atlas: &mut FontAtlas,
        queue: &wgpu::Queue,
        buffer: &Buffer,
        cursor: &Cursor,
        width: f32,
        height: f32,
        mouse_x: f32,
        mouse_y: f32,
        current_backend: wgpu::Backend,
        tab_paths: &[Option<String>],
        tab_modified: &[bool],
        active_tab_idx: usize,
        terminals: &[TerminalInstance],
        active_terminal_idx: usize,
        terminal_focus: bool,
        _is_window_maximized: bool,
    ) {
        self.active_dock_tab = active_terminal_idx;

        // Drain git branch channel
        if let Some(ref rx) = self.git_branch_rx {
            while let Ok(branch) = rx.try_recv() {
                self.git_branch = Some(branch);
            }
        }

        // Drain git file blame channel
        if let Some(ref rx) = self.git_blame_file_rx {
            while let Ok((file, blame_map)) = rx.try_recv() {
                self.git_file_blames.insert(file, blame_map);
            }
        }

        // Drain git status channel
        if let Some(ref rx) = self.git_status_rx {
            while let Ok(statuses) = rx.try_recv() {
                self.git_statuses = statuses;
            }
        }

        // Drain git diff channel
        if let Some(ref rx) = self.git_diff_rx {
            while let Ok((file, hunks)) = rx.try_recv() {
                self.git_diffs.insert(file, hunks);
            }
        }

        // Throttled git branch, status and diff check (every 1 second)
        if self.last_branch_check.is_none() || self.last_branch_check.unwrap().elapsed() > std::time::Duration::from_secs(1) {
            if self.config.show_git_branch {
                self.update_git_branch();
            }
            self.update_git_statuses();
            if active_tab_idx < tab_paths.len() {
                if let Some(ref file_path) = tab_paths[active_tab_idx] {
                    self.update_git_diff(Some(file_path));
                }
            }
            self.last_branch_check = Some(std::time::Instant::now());
        }

        if active_tab_idx < tab_paths.len() {
            if let Some(ref file_path) = tab_paths[active_tab_idx] {
                if !self.git_file_blames.contains_key(file_path) {
                    self.update_git_file_blame(Some(file_path));
                }
            }
        }
        let main_y = self.titlebar_height;
        let main_height = height - self.titlebar_height - self.status_height;

        // Instant expand/collapse sidebar width (no animation delay)
        self.sidebar_width = self.target_sidebar_width;

        let mut dock_y = height - self.status_height;
        if self.show_dock {
            dock_y = (height - self.status_height - self.dock_height).max(main_y + self.tabbar_height + self.breadcrumb_height + 50.0);
        }
        let status_y = if self.show_dock { dock_y.round() } else { (height - self.status_height).round() };
        let dock_start_y = status_y;

        // --- 1. Draw Titlebar Menu Headers (Light Theme) ---
        crate::ui::components::titlebar::draw_titlebar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            mouse_x,
            mouse_y,
        );

        // --- 2. Draw Sidebar Panel (Light Theme) ---
        crate::ui::components::sidebar::draw_sidebar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            main_y,
            main_height,
            mouse_x,
            mouse_y,
            tab_paths,
            tab_modified,
        );

        // --- 3. Draw Editor Tabbar, Breadcrumbs, Text Area, Gutter, Scrollbars & Minimap ---
        crate::ui::components::editor_view::draw_editor_view(
            self,
            vertices,
            indices,
            atlas,
            queue,
            buffer,
            cursor,
            width,
            mouse_x,
            mouse_y,
            tab_paths,
            tab_modified,
            active_tab_idx,
            status_y,
        );

        // --- 4.5. Draw Bottom Dock ---
        crate::ui::components::dock::draw_dock(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            height,
            mouse_x,
            mouse_y,
            terminals,
            terminal_focus,
            dock_start_y,
        );

        // --- 5. Draw Statusbar ---
        let active_path = tab_paths.get(active_tab_idx).and_then(|p| p.as_deref());
        crate::ui::components::statusbar::draw_statusbar(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            height,
            buffer,
            cursor,
            mouse_x,
            mouse_y,
            active_path,
        );

        // --- 6. Draw Context Dropdown Menus & 7. Modal Dialogs ---
        crate::ui::components::modals::draw_modals(
            self,
            vertices,
            indices,
            atlas,
            queue,
            width,
            height,
            mouse_x,
            mouse_y,
            current_backend,
            tab_paths,
        );
    }
}
