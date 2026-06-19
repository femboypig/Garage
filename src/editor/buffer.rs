use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

fn is_binary_file(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8192);
    let slice = &bytes[..check_len];

    // Check for UTF-16 or UTF-32 BOMs
    if slice.starts_with(&[0xfe, 0xff]) || slice.starts_with(&[0xff, 0xfe]) {
        return false; // UTF-16 text
    }
    if slice.starts_with(&[0x00, 0x00, 0xfe, 0xff]) || slice.starts_with(&[0xff, 0xfe, 0x00, 0x00])
    {
        return false; // UTF-32 text
    }

    // If it's valid UTF-8, it's text
    if std::str::from_utf8(slice).is_ok() {
        return false;
    }

    // Check percentage of control/non-printable characters (excluding whitespace/tabs/newlines)
    let mut control_count = 0;
    for &b in slice {
        if b == 0 {
            control_count += 1;
        } else if b < 9 || (b > 13 && b < 32) || b == 127 {
            control_count += 1;
        }
    }

    // If more than 5% of characters are non-printable control chars, it's binary
    if !slice.is_empty() && (control_count * 100 / slice.len()) > 5 {
        return true;
    }

    false
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Insert {
        line: usize,
        col: usize,
        text: String,
    },
    Delete {
        line: usize,
        col: usize,
        text: String,
    },
}

#[derive(Clone)]
pub struct Buffer {
    lines: Vec<String>,
    undo_stack: Vec<Vec<Action>>,
    redo_stack: Vec<Vec<Action>>,
    current_transaction: Option<Vec<Action>>,
    pub is_modified: bool,
    max_line_len: usize,
    saved_undo_len: Option<usize>,
    pub revision: usize,
    pub line_ending: String,
}

impl Buffer {
    /// Create a new, empty Buffer with at least one empty line.
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_transaction: None,
            is_modified: false,
            max_line_len: 0,
            saved_undo_len: Some(0),
            revision: 0,
            line_ending: "LF".to_string(),
        }
    }

    pub fn from_text(text: &str) -> Self {
        if text.is_empty() {
            return Self::new();
        }
        let line_count = text.lines().count();
        let mut max_len = 0;
        let mut lines = Vec::with_capacity(line_count);
        let has_crlf = text.contains("\r\n");
        let line_ending = if has_crlf {
            "CRLF".to_string()
        } else {
            "LF".to_string()
        };
        for s in text.lines() {
            let line = if s.contains('\t') {
                s.replace('\t', "    ")
            } else {
                s.to_string()
            };
            let count = if line.is_ascii() {
                line.len()
            } else {
                line.chars().count()
            };
            if count > max_len {
                max_len = count;
            }
            lines.push(line);
        }
        Self {
            lines,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_transaction: None,
            is_modified: false,
            max_line_len: max_len,
            saved_undo_len: Some(0),
            revision: 0,
            line_ending,
        }
    }

    /// Load a file into the buffer.
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut bytes)?;

        if is_binary_file(&bytes) {
            let error_msg =
                "[Error: Binary file detected. Garage does not support editing binary files.]"
                    .to_string();
            self.max_line_len = error_msg.chars().count();
            self.lines = vec![error_msg];
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.current_transaction = None;
            self.is_modified = false;
            self.saved_undo_len = Some(0);
            return Ok(());
        }

        let text = String::from_utf8_lossy(&bytes);
        let has_crlf = bytes.windows(2).any(|w| w == b"\r\n");
        self.line_ending = if has_crlf {
            "CRLF".to_string()
        } else {
            "LF".to_string()
        };
        let line_count = bytes.iter().filter(|&&b| b == b'\n').count() + 1;
        let mut max_len = 0;
        let mut loaded_lines = Vec::with_capacity(line_count);
        for s in text.split('\n') {
            let mut s = s;
            if s.ends_with('\r') {
                s = &s[..s.len() - 1];
            }
            let line = if s.contains('\t') {
                s.replace('\t', "    ")
            } else {
                s.to_string()
            };
            let count = if line.is_ascii() {
                line.len()
            } else {
                line.chars().count()
            };
            if count > max_len {
                max_len = count;
            }
            loaded_lines.push(line);
        }

        if loaded_lines.is_empty() {
            loaded_lines.push(String::new());
        }

        self.lines = loaded_lines;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_transaction = None;
        self.is_modified = false;
        self.saved_undo_len = Some(0);
        self.max_line_len = max_len;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Mark the buffer as saved by syncing the initial lines.
    pub fn mark_saved(&mut self) {
        self.saved_undo_len = Some(self.undo_stack.len());
        self.is_modified = false;
    }

    /// Save the buffer contents to a file.
    pub fn save_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        let le_bytes: &[u8] = if self.line_ending == "CRLF" {
            b"\r\n"
        } else {
            b"\n"
        };
        for (i, line) in self.lines.iter().enumerate() {
            file.write_all(line.as_bytes())?;
            if i < self.lines.len() - 1 {
                file.write_all(le_bytes)?;
            }
        }
        Ok(())
    }

    /// Get a reference to the lines of text.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Get the number of lines in the buffer.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Start a group of edits that should be undone/redone together.
    pub fn start_transaction(&mut self) {
        if self.current_transaction.is_none() {
            self.current_transaction = Some(Vec::new());
        }
    }

    /// Commit the current group of edits.
    pub fn commit_transaction(&mut self) {
        if let Some(tx) = self.current_transaction.take()
            && !tx.is_empty()
        {
            self.push_undo(tx);
            self.redo_stack.clear();
        }
    }

    /// Insert text at a specific line and column.
    pub fn insert(&mut self, line: usize, col: usize, text: &str) {
        if line >= self.lines.len() {
            return;
        }

        // Keep track of the edit in the current transaction
        let action = Action::Insert {
            line,
            col,
            text: text.to_string(),
        };
        if let Some(ref mut tx) = self.current_transaction {
            tx.push(action);
        } else {
            self.push_undo(vec![action]);
            self.redo_stack.clear();
        }

        self.insert_raw(line, col, text);
    }

    /// Perform raw insertion without touching undo/redo stacks.
    fn insert_raw(&mut self, line: usize, col: usize, text: &str) {
        self.revision = self.revision.wrapping_add(1);
        let cur_line = &mut self.lines[line];

        // Clamp column to line boundaries
        let col = col.min(cur_line.chars().count());
        let byte_idx = cur_line
            .char_indices()
            .map(|(i, _)| i)
            .nth(col)
            .unwrap_or(cur_line.len());

        let left = &cur_line[..byte_idx];
        let right = &cur_line[byte_idx..];

        let parts = text.split('\n').collect::<Vec<&str>>();
        if parts.len() == 1 {
            // Single line insert
            let new_line = format!("{}{}{}", left, parts[0], right);
            self.lines[line] = new_line;
        } else {
            // Multi-line insert
            let first_line = format!("{}{}", left, parts[0]);
            let last_line = format!("{}{}", parts.last().unwrap(), right);

            self.lines[line] = first_line;

            // Insert middle lines
            let mut insert_idx = line + 1;
            for mid in &parts[1..parts.len() - 1] {
                self.lines.insert(insert_idx, mid.to_string());
                insert_idx += 1;
            }
            self.lines.insert(insert_idx, last_line);
        }
        self.is_modified = true;

        // Incremental max line length check
        let mut max_new_len = 0;
        for i in 0..parts.len() {
            let l_idx = line + i;
            if l_idx < self.lines.len() {
                max_new_len = max_new_len.max(self.lines[l_idx].chars().count());
            }
        }
        self.max_line_len = self.max_line_len.max(max_new_len);
    }

    /// Delete text from start coordinates to end coordinates.
    /// Returns the deleted text.
    pub fn delete(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> String {
        let (s_line, s_col, e_line, e_col) =
            self.normalize_range(start_line, start_col, end_line, end_col);

        let deleted_text = self.get_range_text(s_line, s_col, e_line, e_col);

        let action = Action::Delete {
            line: s_line,
            col: s_col,
            text: deleted_text.clone(),
        };

        if let Some(ref mut tx) = self.current_transaction {
            tx.push(action);
        } else {
            self.push_undo(vec![action]);
            self.redo_stack.clear();
        }

        self.delete_raw(s_line, s_col, e_line, e_col);
        deleted_text
    }

    /// Perform raw deletion without touching undo/redo stacks.
    fn delete_raw(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) {
        self.revision = self.revision.wrapping_add(1);
        if start_line >= self.lines.len() || end_line >= self.lines.len() {
            return;
        }

        let mut was_max = false;
        for i in start_line..=end_line {
            if self.lines[i].chars().count() >= self.max_line_len {
                was_max = true;
                break;
            }
        }

        let start_byte = self.char_to_byte_idx(start_line, start_col);
        let end_byte = self.char_to_byte_idx(end_line, end_col);

        if start_line == end_line {
            let line_text = &self.lines[start_line];
            let new_text = format!("{}{}", &line_text[..start_byte], &line_text[end_byte..]);
            self.lines[start_line] = new_text;
        } else {
            let first_line_text = &self.lines[start_line];
            let last_line_text = &self.lines[end_line];

            let new_first = format!(
                "{}{}",
                &first_line_text[..start_byte],
                &last_line_text[end_byte..]
            );
            self.lines[start_line] = new_first;

            // Remove middle and end lines
            self.lines.drain((start_line + 1)..=end_line);
        }

        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.is_modified = true;

        if was_max {
            self.recalculate_max_line_len();
        }
    }

    /// Undo the last transaction. Returns true if successful.
    pub fn undo(&mut self) -> Option<(usize, usize)> {
        if let Some(tx) = self.undo_stack.pop() {
            let mut redo_tx = Vec::new();
            let mut edit_pos = None;
            // Apply in reverse order
            for action in tx.iter().rev() {
                match action {
                    Action::Insert { line, col, text } => {
                        let lines_count = text.split('\n').count();
                        let end_line = line + lines_count - 1;
                        let end_col = if lines_count == 1 {
                            col + text.chars().count()
                        } else {
                            text.split('\n').next_back().unwrap().chars().count()
                        };
                        self.delete_raw(*line, *col, end_line, end_col);
                        redo_tx.push(Action::Delete {
                            line: *line,
                            col: *col,
                            text: text.clone(),
                        });
                        edit_pos = Some((*line, *col));
                    }
                    Action::Delete { line, col, text } => {
                        self.insert_raw(*line, *col, text);
                        redo_tx.push(Action::Insert {
                            line: *line,
                            col: *col,
                            text: text.clone(),
                        });
                        edit_pos = Some((*line, *col));
                    }
                }
            }
            self.push_redo(redo_tx);
            self.is_modified = Some(self.undo_stack.len()) != self.saved_undo_len;
            edit_pos
        } else {
            None
        }
    }

    /// Redo the last undone transaction. Returns the edit position if successful.
    pub fn redo(&mut self) -> Option<(usize, usize)> {
        if let Some(tx) = self.redo_stack.pop() {
            let mut undo_tx = Vec::new();
            let mut edit_pos = None;
            // Apply in reverse order
            for action in tx.iter().rev() {
                match action {
                    Action::Insert { line, col, text } => {
                        let lines_count = text.split('\n').count();
                        let end_line = line + lines_count - 1;
                        let end_col = if lines_count == 1 {
                            col + text.chars().count()
                        } else {
                            text.split('\n').next_back().unwrap().chars().count()
                        };
                        self.delete_raw(*line, *col, end_line, end_col);
                        undo_tx.push(Action::Delete {
                            line: *line,
                            col: *col,
                            text: text.clone(),
                        });
                        edit_pos = Some((*line, *col));
                    }
                    Action::Delete { line, col, text } => {
                        self.insert_raw(*line, *col, text);
                        undo_tx.push(Action::Insert {
                            line: *line,
                            col: *col,
                            text: text.clone(),
                        });
                        edit_pos = Some((*line, *col));
                    }
                }
            }
            self.push_undo(undo_tx);
            self.is_modified = Some(self.undo_stack.len()) != self.saved_undo_len;
            edit_pos
        } else {
            None
        }
    }

    fn push_undo(&mut self, tx: Vec<Action>) {
        if let Some(saved_len) = self.saved_undo_len
            && self.undo_stack.len() < saved_len
        {
            self.saved_undo_len = None;
        }
        self.undo_stack.push(tx);
        if self.undo_stack.len() > 1000 {
            if let Some(saved_len) = self.saved_undo_len {
                let drained_count = self.undo_stack.len() - 1000;
                if saved_len < drained_count {
                    self.saved_undo_len = None;
                } else {
                    self.saved_undo_len = Some(saved_len - drained_count);
                }
            }
            self.undo_stack.drain(0..(self.undo_stack.len() - 1000));
        }
        self.is_modified = Some(self.undo_stack.len()) != self.saved_undo_len;
    }

    fn push_redo(&mut self, tx: Vec<Action>) {
        self.redo_stack.push(tx);
        if self.redo_stack.len() > 1000 {
            self.redo_stack.drain(0..(self.redo_stack.len() - 1000));
        }
    }

    // --- Helper Methods ---

    /// Helper to convert character index to byte index in a specific line.
    fn char_to_byte_idx(&self, line: usize, char_col: usize) -> usize {
        let line_text = &self.lines[line];
        line_text
            .char_indices()
            .map(|(i, _)| i)
            .nth(char_col)
            .unwrap_or(line_text.len())
    }

    /// Ensure start comes before end.
    fn normalize_range(
        &self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> (usize, usize, usize, usize) {
        if start_line < end_line {
            (start_line, start_col, end_line, end_col)
        } else if start_line > end_line {
            (end_line, end_col, start_line, start_col)
        } else {
            (
                start_line,
                start_col.min(end_col),
                start_line,
                start_col.max(end_col),
            )
        }
    }

    /// Retrieve the text within a specific coordinate range.
    pub fn get_range_text(
        &self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> String {
        let (s_line, s_col, e_line, e_col) =
            self.normalize_range(start_line, start_col, end_line, end_col);

        if s_line >= self.lines.len() {
            return String::new();
        }

        let s_byte = self.char_to_byte_idx(s_line, s_col);
        let e_byte = self.char_to_byte_idx(e_line, e_col);

        if s_line == e_line {
            self.lines[s_line][s_byte..e_byte].to_string()
        } else {
            let mut result = String::new();
            result.push_str(&self.lines[s_line][s_byte..]);
            result.push('\n');

            for line in &self.lines[(s_line + 1)..e_line] {
                result.push_str(line);
                result.push('\n');
            }

            result.push_str(&self.lines[e_line][..e_byte]);
            result
        }
    }

    pub fn max_line_len(&self) -> usize {
        self.max_line_len
    }

    fn recalculate_max_line_len(&mut self) {
        self.max_line_len = self
            .lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_single_line() {
        let mut buf = Buffer::new();
        buf.insert(0, 0, "Hello");
        assert_eq!(buf.lines(), &["Hello".to_string()]);

        buf.insert(0, 5, " World");
        assert_eq!(buf.lines(), &["Hello World".to_string()]);

        buf.insert(0, 5, ",");
        assert_eq!(buf.lines(), &["Hello, World".to_string()]);
    }

    #[test]
    fn test_insert_multi_line() {
        let mut buf = Buffer::new();
        buf.insert(0, 0, "Hello\nWorld\nRust");
        assert_eq!(
            buf.lines(),
            &["Hello".to_string(), "World".to_string(), "Rust".to_string()]
        );
    }

    #[test]
    fn test_delete_single_line() {
        let mut buf = Buffer::new();
        buf.insert(0, 0, "Hello World");
        let deleted = buf.delete(0, 5, 0, 11);
        assert_eq!(deleted, " World");
        assert_eq!(buf.lines(), &["Hello".to_string()]);
    }

    #[test]
    fn test_delete_multi_line() {
        let mut buf = Buffer::new();
        buf.insert(0, 0, "Hello\nWorld\nRust");
        let deleted = buf.delete(0, 3, 2, 2);
        assert_eq!(deleted, "lo\nWorld\nRu");
        assert_eq!(buf.lines(), &["Helst".to_string()]);
    }

    #[test]
    fn test_undo_redo() {
        let mut buf = Buffer::new();
        buf.insert(0, 0, "Hello");
        assert_eq!(buf.lines(), &["Hello".to_string()]);

        buf.undo();
        assert_eq!(buf.lines(), &["".to_string()]);

        buf.redo();
        assert_eq!(buf.lines(), &["Hello".to_string()]);
    }

    #[test]
    fn test_transaction() {
        let mut buf = Buffer::new();
        buf.start_transaction();
        buf.insert(0, 0, "H");
        buf.insert(0, 1, "e");
        buf.insert(0, 2, "l");
        buf.insert(0, 3, "l");
        buf.insert(0, 4, "o");
        buf.commit_transaction();

        assert_eq!(buf.lines(), &["Hello".to_string()]);
        buf.undo();
        assert_eq!(buf.lines(), &["".to_string()]);
    }
}
