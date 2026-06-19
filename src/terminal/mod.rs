use portable_pty::{Child, CommandBuilder, PtyPair, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, channel};
use std::thread;

const COLOR_PALETTE: [[f32; 4]; 16] = [
    // Normal colors
    [0.18, 0.18, 0.18, 1.0], // Black
    [0.80, 0.30, 0.30, 1.0], // Red
    [0.30, 0.70, 0.30, 1.0], // Green
    [0.80, 0.70, 0.20, 1.0], // Yellow
    [0.30, 0.50, 0.90, 1.0], // Blue
    [0.70, 0.40, 0.80, 1.0], // Magenta
    [0.30, 0.70, 0.70, 1.0], // Cyan
    [0.85, 0.85, 0.85, 1.0], // White
    // Bright colors
    [0.30, 0.30, 0.30, 1.0], // Bright Black (Grey)
    [0.90, 0.40, 0.40, 1.0], // Bright Red
    [0.40, 0.80, 0.40, 1.0], // Bright Green
    [0.90, 0.80, 0.30, 1.0], // Bright Yellow
    [0.45, 0.65, 1.00, 1.0], // Bright Blue
    [0.80, 0.50, 0.90, 1.0], // Bright Magenta
    [0.40, 0.80, 0.80, 1.0], // Bright Cyan
    [0.98, 0.98, 0.98, 1.0], // Bright White
];

pub const DEFAULT_FG: [f32; 4] = [0.85, 0.85, 0.85, 1.0];
pub const DEFAULT_BG: [f32; 4] = [0.0, 0.0, 0.0, 0.0]; // transparent, fallback to theme

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub c: char,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
        }
    }
}

pub struct TerminalGrid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    pub alt_cells: Vec<Cell>,
    pub use_alt_screen: bool,
    pub decckm: bool,
    pub show_cursor: bool,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub current_fg: [f32; 4],
    pub current_bg: [f32; 4],
    pub bold: bool,
    pub inverse: bool,
    pub title: String,
    pub scrollback: Vec<Vec<Cell>>,
    pub scroll_offset: usize,
    pub saved_cursor_x: Option<usize>,
    pub saved_cursor_y: Option<usize>,
}

impl TerminalGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            alt_cells: vec![Cell::default(); cols * rows],
            use_alt_screen: false,
            decckm: false,
            show_cursor: true,
            cursor_x: 0,
            cursor_y: 0,
            current_fg: DEFAULT_FG,
            current_bg: DEFAULT_BG,
            bold: false,
            inverse: false,
            title: String::new(),
            scrollback: Vec::new(),
            scroll_offset: 0,
            saved_cursor_x: None,
            saved_cursor_y: None,
        }
    }

    pub fn get_cells(&self) -> &Vec<Cell> {
        if self.use_alt_screen {
            &self.alt_cells
        } else {
            &self.cells
        }
    }

    pub fn get_cells_mut(&mut self) -> &mut Vec<Cell> {
        if self.use_alt_screen {
            &mut self.alt_cells
        } else {
            &mut self.cells
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }

        // Clear cursor line in old grid to prevent ghost prompt duplicates on resize
        if self.cursor_y < self.rows {
            let start_idx = self.cursor_y * self.cols;
            for x in 0..self.cols {
                if start_idx + x < self.cells.len() {
                    self.cells[start_idx + x] = Cell::default();
                }
                if start_idx + x < self.alt_cells.len() {
                    self.alt_cells[start_idx + x] = Cell::default();
                }
            }
        }

        let mut new_cells = vec![Cell::default(); new_cols * new_rows];
        let mut new_alt_cells = vec![Cell::default(); new_cols * new_rows];

        // Calculate vertical shift if the height is shrinking and the cursor would be off-screen
        let shift_y = if new_rows < self.rows && self.cursor_y >= new_rows {
            self.cursor_y - new_rows + 1
        } else {
            0
        };

        // If shifting, save the shifted out lines to scrollback (only for normal cells)
        for y in 0..shift_y {
            let mut row = vec![Cell::default(); self.cols];
            for x in 0..self.cols {
                row[x] = self.cells[y * self.cols + x];
            }
            self.scrollback.push(row);
        }
        if self.scrollback.len() > 1000 {
            let to_remove = self.scrollback.len() - 1000;
            self.scrollback.drain(0..to_remove);
        }

        // Copy cells from old grid with y-offset shift_y
        for y in 0..new_rows {
            let old_y = y + shift_y;
            if old_y < self.rows {
                let copy_cols = self.cols.min(new_cols);
                for x in 0..copy_cols {
                    new_cells[y * new_cols + x] = self.cells[old_y * self.cols + x];
                    new_alt_cells[y * new_cols + x] = self.alt_cells[old_y * self.cols + x];
                }
            }
        }

        self.cells = new_cells;
        self.alt_cells = new_alt_cells;
        self.cols = new_cols;
        self.rows = new_rows;
        self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
        self.cursor_y = self
            .cursor_y
            .saturating_sub(shift_y)
            .min(new_rows.saturating_sub(1));
        self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
    }

    fn scroll_up(&mut self) {
        if self.use_alt_screen {
            // Shift all rows up by 1
            for y in 1..self.rows {
                for x in 0..self.cols {
                    self.alt_cells[(y - 1) * self.cols + x] = self.alt_cells[y * self.cols + x];
                }
            }
            // Clear bottom row
            let last_row_start = (self.rows - 1) * self.cols;
            for x in 0..self.cols {
                self.alt_cells[last_row_start + x] = Cell::default();
            }
        } else {
            // Save the top row to scrollback
            let mut top_row = vec![Cell::default(); self.cols];
            for x in 0..self.cols {
                top_row[x] = self.cells[x];
            }
            self.scrollback.push(top_row);
            if self.scrollback.len() > 1000 {
                self.scrollback.remove(0);
            }

            // Shift all rows up by 1
            for y in 1..self.rows {
                for x in 0..self.cols {
                    self.cells[(y - 1) * self.cols + x] = self.cells[y * self.cols + x];
                }
            }
            // Clear bottom row
            let last_row_start = (self.rows - 1) * self.cols;
            for x in 0..self.cols {
                self.cells[last_row_start + x] = Cell::default();
            }
        }
    }

    fn newline(&mut self) {
        if self.cursor_y + 1 >= self.rows {
            self.scroll_up();
            self.cursor_y = self.rows - 1;
        } else {
            self.cursor_y += 1;
        }
    }
}

impl vte::Perform for TerminalGrid {
    fn print(&mut self, c: char) {
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.newline();
        }
        let idx = self.cursor_y * self.cols + self.cursor_x;
        let fg = if self.inverse {
            self.current_bg
        } else {
            self.current_fg
        };
        let bg = if self.inverse {
            self.current_fg
        } else {
            self.current_bg
        };

        let cells = self.get_cells_mut();
        if idx < cells.len() {
            cells[idx] = Cell { c, fg, bg };
        }
        self.cursor_x += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            10 => {
                // LF (Line Feed)
                self.newline();
            }
            13 => {
                // CR (Carriage Return)
                self.cursor_x = 0;
            }
            8 => {
                // BS (Backspace)
                self.cursor_x = self.cursor_x.saturating_sub(1);
            }
            9 => {
                // TAB
                let tab_width = 8;
                let next_tab = ((self.cursor_x / tab_width) + 1) * tab_width;
                self.cursor_x = next_tab.min(self.cols - 1);
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            'm' => {
                // SGR (Select Graphic Rendition)
                if params.is_empty() {
                    self.current_fg = DEFAULT_FG;
                    self.current_bg = DEFAULT_BG;
                    self.bold = false;
                    return;
                }

                let mut params_iter = params.iter();
                while let Some(param) = params_iter.next() {
                    let p = param.first().copied().unwrap_or(0);
                    match p {
                        0 => {
                            // Reset
                            self.current_fg = DEFAULT_FG;
                            self.current_bg = DEFAULT_BG;
                            self.bold = false;
                            self.inverse = false;
                        }
                        1 => {
                            // Bold
                            self.bold = true;
                        }
                        7 => {
                            // Reverse video
                            self.inverse = true;
                        }
                        22 => {
                            // Normal color or intensity
                            self.bold = false;
                        }
                        27 => {
                            // Positive image (normal video)
                            self.inverse = false;
                        }
                        30..=37 => {
                            // Foreground color
                            let idx = (p - 30) as usize;
                            self.current_fg = COLOR_PALETTE[idx];
                        }
                        38 => {
                            // Extended Foreground (256-color or RGB)
                            // We can skip or parse basic 256 colors if next params allow.
                            // For simplicity, support 256 colors basic mapping
                            if let Some(next_param) = params_iter.next() {
                                let mode = next_param.first().copied().unwrap_or(0);
                                if mode == 5 {
                                    // 256 color
                                    if let Some(color_idx_param) = params_iter.next() {
                                        let c_idx =
                                            color_idx_param.first().copied().unwrap_or(0) as usize;
                                        if c_idx < 16 {
                                            self.current_fg = COLOR_PALETTE[c_idx];
                                        } else if c_idx >= 232 {
                                            // grayscale ramp
                                            let val = (c_idx - 232) as f32 / 23.0;
                                            self.current_fg = [val, val, val, 1.0];
                                        } else {
                                            // 6x6x6 color cube
                                            let code = c_idx - 16;
                                            let r = ((code / 36) % 6) as f32 / 5.0;
                                            let g = ((code / 6) % 6) as f32 / 5.0;
                                            let b = (code % 6) as f32 / 5.0;
                                            self.current_fg = [r, g, b, 1.0];
                                        }
                                    }
                                } else if mode == 2 {
                                    // RGB
                                    if let (Some(r_p), Some(g_p), Some(b_p)) =
                                        (params_iter.next(), params_iter.next(), params_iter.next())
                                    {
                                        let r = r_p.first().copied().unwrap_or(0) as f32 / 255.0;
                                        let g = g_p.first().copied().unwrap_or(0) as f32 / 255.0;
                                        let b = b_p.first().copied().unwrap_or(0) as f32 / 255.0;
                                        self.current_fg = [r, g, b, 1.0];
                                    }
                                }
                            }
                        }
                        39 => {
                            // Default foreground
                            self.current_fg = DEFAULT_FG;
                        }
                        40..=47 => {
                            // Background color
                            let idx = (p - 40) as usize;
                            self.current_bg = COLOR_PALETTE[idx];
                        }
                        48 => {
                            // Extended Background
                            if let Some(next_param) = params_iter.next() {
                                let mode = next_param.first().copied().unwrap_or(0);
                                if mode == 5 {
                                    if let Some(color_idx_param) = params_iter.next() {
                                        let c_idx =
                                            color_idx_param.first().copied().unwrap_or(0) as usize;
                                        if c_idx < 16 {
                                            self.current_bg = COLOR_PALETTE[c_idx];
                                        } else if c_idx >= 232 {
                                            let val = (c_idx - 232) as f32 / 23.0;
                                            self.current_bg = [val, val, val, 1.0];
                                        } else {
                                            let code = c_idx - 16;
                                            let r = ((code / 36) % 6) as f32 / 5.0;
                                            let g = ((code / 6) % 6) as f32 / 5.0;
                                            let b = (code % 6) as f32 / 5.0;
                                            self.current_bg = [r, g, b, 1.0];
                                        }
                                    }
                                } else if mode == 2
                                    && let (Some(r_p), Some(g_p), Some(b_p)) =
                                        (params_iter.next(), params_iter.next(), params_iter.next())
                                    {
                                        let r = r_p.first().copied().unwrap_or(0) as f32 / 255.0;
                                        let g = g_p.first().copied().unwrap_or(0) as f32 / 255.0;
                                        let b = b_p.first().copied().unwrap_or(0) as f32 / 255.0;
                                        self.current_bg = [r, g, b, 1.0];
                                    }
                            }
                        }
                        49 => {
                            // Default background
                            self.current_bg = DEFAULT_BG;
                        }
                        90..=97 => {
                            // Bright foreground
                            let idx = (p - 90 + 8) as usize;
                            self.current_fg = COLOR_PALETTE[idx];
                        }
                        100..=107 => {
                            // Bright background
                            let idx = (p - 100 + 8) as usize;
                            self.current_bg = COLOR_PALETTE[idx];
                        }
                        _ => {}
                    }
                }
            }
            'J' => {
                // Erase in Display (ED)
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                if mode == 0 {
                    let cursor_x = self.cursor_x;
                    let cursor_y = self.cursor_y;
                    let cols = self.cols;
                    let rows = self.rows;
                    let cells = self.get_cells_mut();

                    // Clear current line from cursor to end
                    let start_idx = cursor_y * cols;
                    for x in cursor_x..cols {
                        if start_idx + x < cells.len() {
                            cells[start_idx + x] = Cell::default();
                        }
                    }
                    // Clear all lines below cursor
                    for y in (cursor_y + 1)..rows {
                        let line_start = y * cols;
                        for x in 0..cols {
                            if line_start + x < cells.len() {
                                cells[line_start + x] = Cell::default();
                            }
                        }
                    }
                } else if mode == 1 {
                    let cursor_x = self.cursor_x;
                    let cursor_y = self.cursor_y;
                    let cols = self.cols;
                    let cells = self.get_cells_mut();

                    // Clear all lines above cursor
                    for y in 0..cursor_y {
                        let line_start = y * cols;
                        for x in 0..cols {
                            if line_start + x < cells.len() {
                                cells[line_start + x] = Cell::default();
                            }
                        }
                    }
                    // Clear current line from start to cursor
                    let start_idx = cursor_y * cols;
                    for x in 0..=cursor_x.min(cols - 1) {
                        if start_idx + x < cells.len() {
                            cells[start_idx + x] = Cell::default();
                        }
                    }
                } else if mode == 2 || mode == 3 {
                    // Clear entire screen
                    let cells = self.get_cells_mut();
                    for cell in cells {
                        *cell = Cell::default();
                    }
                    self.cursor_x = 0;
                    self.cursor_y = 0;
                    if mode == 3 && !self.use_alt_screen {
                        self.scrollback.clear();
                        self.scroll_offset = 0;
                    }
                }
            }
            'K' => {
                // Erase in Line (EL)
                let mode = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                let start_idx = self.cursor_y * self.cols;
                let cursor_x = self.cursor_x;
                let cols = self.cols;
                let cells = self.get_cells_mut();
                match mode {
                    0 => {
                        // Clear from cursor to end of line
                        for x in cursor_x..cols {
                            cells[start_idx + x] = Cell::default();
                        }
                    }
                    1 => {
                        // Clear from start of line to cursor
                        for x in 0..=cursor_x.min(cols - 1) {
                            cells[start_idx + x] = Cell::default();
                        }
                    }
                    2 => {
                        // Clear entire line
                        for x in 0..cols {
                            cells[start_idx + x] = Cell::default();
                        }
                    }
                    _ => {}
                }
            }
            'H' | 'f' => {
                // Cursor Position (CUP)
                let mut iter = params.iter();
                let row = iter.next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                let col = iter.next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                self.cursor_y = (row.saturating_sub(1)).min(self.rows - 1);
                self.cursor_x = (col.saturating_sub(1)).min(self.cols - 1);
            }
            'A' => {
                // Cursor Up
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_y = self.cursor_y.saturating_sub(count);
            }
            'B' => {
                // Cursor Down
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_y = (self.cursor_y + count).min(self.rows - 1);
            }
            'C' => {
                // Cursor Forward
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_x = (self.cursor_x + count).min(self.cols - 1);
            }
            'D' => {
                // Cursor Backward
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_x = self.cursor_x.saturating_sub(count);
            }
            'G' => {
                // Cursor Horizontal Absolute (CHA)
                let col = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_x = (col.saturating_sub(1)).min(self.cols - 1);
            }
            'd' => {
                // Cursor Vertical Absolute (VPA)
                let row = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_y = (row.saturating_sub(1)).min(self.rows - 1);
            }
            'e' => {
                // Cursor Down Line
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                self.cursor_y = (self.cursor_y + count).min(self.rows - 1);
            }
            'X' => {
                // Erase Character (ECH)
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                let start_idx = self.cursor_y * self.cols;
                let cursor_x = self.cursor_x;
                let cols = self.cols;
                let fg = self.current_fg;
                let bg = self.current_bg;
                let cells = self.get_cells_mut();
                for offset in 0..count {
                    let x = cursor_x + offset;
                    if x < cols {
                        cells[start_idx + x] = Cell { c: ' ', fg, bg };
                    }
                }
            }
            'P' => {
                // Delete Character (DCH)
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                let start_idx = self.cursor_y * self.cols;
                let cursor_x = self.cursor_x;
                let cols = self.cols;
                let move_count = cols - (cursor_x + count);
                let cells = self.get_cells_mut();
                for i in 0..move_count {
                    cells[start_idx + cursor_x + i] = cells[start_idx + cursor_x + count + i];
                }
                for i in (cols - count)..cols {
                    cells[start_idx + i] = Cell::default();
                }
            }
            '@' => {
                // Insert Character (ICH)
                let count = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1) as usize;
                let start_idx = self.cursor_y * self.cols;
                let cursor_x = self.cursor_x;
                let cols = self.cols;
                let shift_limit = cols - cursor_x;
                let cells = self.get_cells_mut();
                for i in (count..shift_limit).rev() {
                    cells[start_idx + cursor_x + i] = cells[start_idx + cursor_x + i - count];
                }
                for i in 0..count.min(shift_limit) {
                    cells[start_idx + cursor_x + i] = Cell::default();
                }
            }
            'h' => {
                // SM (Set Mode)
                let is_private = _intermediates.contains(&b'?');
                for param in params.iter() {
                    let p = param.first().copied().unwrap_or(0);
                    if is_private {
                        match p {
                            1 => self.decckm = true,
                            25 => self.show_cursor = true,
                            1049 => {
                                self.use_alt_screen = true;
                                for cell in &mut self.alt_cells {
                                    *cell = Cell::default();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            'l' => {
                // RM (Reset Mode)
                let is_private = _intermediates.contains(&b'?');
                for param in params.iter() {
                    let p = param.first().copied().unwrap_or(0);
                    if is_private {
                        match p {
                            1 => self.decckm = false,
                            25 => self.show_cursor = false,
                            1049 => {
                                self.use_alt_screen = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            's' => {
                // Save Cursor Position
                self.saved_cursor_x = Some(self.cursor_x);
                self.saved_cursor_y = Some(self.cursor_y);
            }
            'u' => {
                // Restore Cursor Position
                if let (Some(x), Some(y)) = (self.saved_cursor_x, self.saved_cursor_y) {
                    self.cursor_x = x.min(self.cols - 1);
                    self.cursor_y = y.min(self.rows - 1);
                }
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2 {
            let action = params[0];
            if action == b"0" || action == b"1" || action == b"2" {
                if let Ok(title) = std::str::from_utf8(params[1]) {
                    self.title = title.to_string();
                }
            } else if action == b"7"
                && let Ok(url_str) = std::str::from_utf8(params[1]) {
                    // OSC 7: file://hostname/path — extract path component
                    let path_str = if let Some(path_part) = url_str.strip_prefix("file://") {
                        // Skip hostname part to get the path
                        path_part.find('/').map(|slash_idx| &path_part[slash_idx..])
                    } else {
                        // Treat as raw path
                        Some(url_str)
                    };
                    if let Some(path) = path_str {
                        let path_buf = std::path::Path::new(path);
                        // Use the directory name as the tab title (like "src" or "Garage")
                        if let Some(dir_name) = path_buf.file_name().and_then(|f| f.to_str()) {
                            self.title = dir_name.to_string();
                        } else if path == "/" {
                            self.title = "/".to_string();
                        }
                    }
                }
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'7' => {
                // Save cursor position (DECSC)
                self.saved_cursor_x = Some(self.cursor_x);
                self.saved_cursor_y = Some(self.cursor_y);
            }
            b'8' => {
                // Restore cursor position (DECRC)
                if let (Some(x), Some(y)) = (self.saved_cursor_x, self.saved_cursor_y) {
                    self.cursor_x = x.min(self.cols - 1);
                    self.cursor_y = y.min(self.rows - 1);
                }
            }
            _ => {}
        }
    }
}

pub struct TerminalInstance {
    pub name: String,
    pub pty_writer: Box<dyn Write + Send>,
    pub rx: Receiver<Vec<u8>>,
    pub grid: TerminalGrid,
    pub pty_pair: PtyPair,
    pub child: Box<dyn Child>,
    pub parser: vte::Parser,
}

impl TerminalInstance {
    pub fn new(
        cols: usize,
        rows: usize,
        window: std::sync::Arc<winit::window::Window>,
        event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pty_pair = pty_system.openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            if std::path::Path::new("/bin/bash").exists() {
                "/bin/bash".to_string()
            } else {
                "/bin/sh".to_string()
            }
        });
        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if std::env::var("LANG").is_err() {
            cmd.env("LANG", "C.UTF-8");
        }
        if std::env::var("LC_ALL").is_err() {
            cmd.env("LC_ALL", "C.UTF-8");
        }
        cmd.arg("--login");
        // Tell bash to emit OSC 7 (current directory) after each prompt
        if shell.ends_with("bash") {
            cmd.env(
                "PROMPT_COMMAND",
                r#"printf "\033]7;file://%s%s\007" "$(hostname)" "$(pwd)""#,
            );
        }

        if let Ok(current_dir) = std::env::current_dir() {
            cmd.cwd(current_dir);
        }

        let child = pty_pair.slave.spawn_command(cmd)?;

        let mut pty_reader = pty_pair.master.try_clone_reader()?;
        let pty_writer = pty_pair.master.take_writer()?;

        let (tx, rx) = channel();

        // Background reader thread
        let win = window.clone();
        let proxy = event_loop_proxy.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = pty_reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
                let _ = proxy.send_event(());
                win.request_redraw();
            }
        });

        let grid = TerminalGrid::new(cols, rows);
        let parser = vte::Parser::new();

        Ok(Self {
            name: "terminal".to_string(),
            pty_writer,
            rx,
            grid,
            pty_pair,
            child,
            parser,
        })
    }
}

impl Drop for TerminalInstance {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

impl TerminalInstance {
    pub fn resize_pty(&self, cols: usize, rows: usize) {
        let _ = self.pty_pair.master.resize(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn get_process_name(&self) -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            if let Some(pid) = self.child.process_id() {
                if let Ok(stat_content) = std::fs::read_to_string(format!("/proc/{}/stat", pid))
                    && let Some(last_paren) = stat_content.rfind(')') {
                        let post_paren = &stat_content[last_paren + 1..];
                        let parts: Vec<&str> = post_paren.split_whitespace().collect();
                        if parts.len() > 5
                            && let Ok(tpgid) = parts[5].parse::<i32>()
                                && tpgid > 0
                                    && let Ok(comm) =
                                        std::fs::read_to_string(format!("/proc/{}/comm", tpgid))
                                    {
                                        let name = comm.trim().to_string();
                                        if !name.is_empty() {
                                            return Some(name);
                                        }
                                    }
                    }
                if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                    let name = comm.trim().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
        None
    }

    pub fn get_display_name(&self, idx: usize) -> String {
        let raw_name = if let Some(proc_name) = self.get_process_name() {
            proc_name
        } else if self.grid.title.is_empty() {
            format!("terminal-{}", idx + 1)
        } else {
            self.grid.title.clone()
        };

        let mut name = raw_name;
        if name.chars().count() > 20 {
            let prefix: String = name.chars().take(17).collect();
            name = format!("{}...", prefix);
        }
        name
    }
}
