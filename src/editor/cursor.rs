use crate::editor::buffer::Buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
    pub intended_col: usize,
    pub selection_anchor: Option<(usize, usize)>, // (line, col)
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            line: 0,
            col: 0,
            intended_col: 0,
            selection_anchor: None,
        }
    }

    /// Reset selection anchor.
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Set or update the selection anchor.
    pub fn update_selection(&mut self, select: bool) {
        if select {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some((self.line, self.col));
            }
        } else {
            self.selection_anchor = None;
        }
    }

    /// Get current selection range. Returns Option<(start_line, start_col, end_line, end_col)>.
    pub fn selection_range(&self) -> Option<(usize, usize, usize, usize)> {
        self.selection_anchor.map(|(anchor_line, anchor_col)| {
            if anchor_line < self.line {
                (anchor_line, anchor_col, self.line, self.col)
            } else if anchor_line > self.line {
                (self.line, self.col, anchor_line, anchor_col)
            } else if anchor_col < self.col {
                (anchor_line, anchor_col, self.line, self.col)
            } else {
                (self.line, self.col, anchor_line, anchor_col)
            }
        })
    }

    /// Move left by one character.
    pub fn move_left(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);

        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }

        if self.col > 0 {
            self.col -= 1;
        } else if self.line > 0 {
            self.line -= 1;
            self.col = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
        }

        self.intended_col = self.col;
    }

    /// Move right by one character.
    pub fn move_right(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);

        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }

        let line_len = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
        if self.col < line_len {
            self.col += 1;
        } else if self.line < buffer.len() - 1 {
            self.line += 1;
            self.col = 0;
        }

        self.intended_col = self.col;
    }

    /// Move up by one line, retaining intended column.
    pub fn move_up(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);

        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }

        if self.line > 0 {
            self.line -= 1;
            let line_len = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
            self.col = self.intended_col.min(line_len);
        } else {
            self.col = 0;
            self.intended_col = 0;
        }
    }

    /// Move down by one line, retaining intended column.
    pub fn move_down(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);

        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }

        if self.line < buffer.len() - 1 {
            self.line += 1;
            let line_len = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
            self.col = self.intended_col.min(line_len);
        } else {
            let line_len = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
            self.col = line_len;
            self.intended_col = line_len;
        }
    }

    /// Move to the start of the current line.
    pub fn move_to_line_start(&mut self, select: bool) {
        self.update_selection(select);
        self.col = 0;
        self.intended_col = 0;
    }

    /// Move to the end of the current line.
    pub fn move_to_line_end(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);
        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }
        let line_len = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
        self.col = line_len;
        self.intended_col = line_len;
    }

    /// Move left by one word.
    pub fn move_word_left(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);

        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }

        if self.col == 0 {
            if self.line > 0 {
                self.line -= 1;
                self.col = buffer.lines().get(self.line).map_or(0, |l| l.chars().count());
            } else {
                return;
            }
        }

        let line_chars: Vec<char> = buffer.lines().get(self.line).map_or(Vec::new(), |l| l.chars().collect());
        let mut idx = self.col;

        // Skip leading whitespace leftwards
        while idx > 0 && idx - 1 < line_chars.len() && line_chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        if idx > 0 && idx - 1 < line_chars.len() {
            let start_is_alphanumeric = line_chars[idx - 1].is_alphanumeric();
            while idx > 0 && idx - 1 < line_chars.len() && line_chars[idx - 1].is_alphanumeric() == start_is_alphanumeric && !line_chars[idx - 1].is_whitespace() {
                idx -= 1;
            }
        }

        self.col = idx;
        self.intended_col = idx;
    }

    /// Move right by one word.
    pub fn move_word_right(&mut self, buffer: &Buffer, select: bool) {
        self.update_selection(select);

        if self.line >= buffer.len() {
            self.line = buffer.len().saturating_sub(1);
        }

        let line_chars: Vec<char> = buffer.lines().get(self.line).map_or(Vec::new(), |l| l.chars().collect());
        let line_len = line_chars.len();

        if self.col >= line_len {
            if self.line < buffer.len() - 1 {
                self.line += 1;
                self.col = 0;
            }
            return;
        }

        let mut idx = self.col;

        // Skip leading whitespace rightwards
        while idx < line_len && line_chars[idx].is_whitespace() {
            idx += 1;
        }

        if idx < line_len {
            let start_is_alphanumeric = line_chars[idx].is_alphanumeric();
            while idx < line_len && line_chars[idx].is_alphanumeric() == start_is_alphanumeric && !line_chars[idx].is_whitespace() {
                idx += 1;
            }
        }

        self.col = idx;
        self.intended_col = idx;
    }
}