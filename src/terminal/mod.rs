use portable_pty::{native_pty_system, CommandBuilder, PtySize, PtyPair, Child};
use std::sync::mpsc::{channel, Receiver};
use std::io::{Read, Write};
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
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub current_fg: [f32; 4],
    pub current_bg: [f32; 4],
    pub bold: bool,
    pub title: String,
    pub scrollback: Vec<Vec<Cell>>,
    pub scroll_offset: usize,
}

impl TerminalGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            cursor_x: 0,
            cursor_y: 0,
            current_fg: DEFAULT_FG,
            current_bg: DEFAULT_BG,
            bold: false,
            title: String::new(),
            scrollback: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }

        let mut new_cells = vec![Cell::default(); new_cols * new_rows];
        
        // Calculate vertical shift if the height is shrinking and the cursor would be off-screen
        let shift_y = if new_rows < self.rows && self.cursor_y >= new_rows {
            self.cursor_y - new_rows + 1
        } else {
            0
        };

        // If shifting, save the shifted out lines to scrollback
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
                }
            }
        }

        self.cells = new_cells;
        self.cols = new_cols;
        self.rows = new_rows;
        self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
        self.cursor_y = self.cursor_y.saturating_sub(shift_y).min(new_rows.saturating_sub(1));
        self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
    }

    fn scroll_up(&mut self) {
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
        if idx < self.cells.len() {
            self.cells[idx] = Cell {
                c,
                fg: self.current_fg,
                bg: self.current_bg,
            };
        }
        self.cursor_x += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            10 => { // LF (Line Feed)
                self.newline();
            }
            13 => { // CR (Carriage Return)
                self.cursor_x = 0;
            }
            8 => { // BS (Backspace)
                self.cursor_x = self.cursor_x.saturating_sub(1);
            }
            9 => { // TAB
                let tab_width = 8;
                let next_tab = ((self.cursor_x / tab_width) + 1) * tab_width;
                self.cursor_x = next_tab.min(self.cols - 1);
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &vte::Params, _intermediates: &[u8], _ignore: bool, action: char) {
        match action {
            'm' => { // SGR (Select Graphic Rendition)
                if params.is_empty() {
                    self.current_fg = DEFAULT_FG;
                    self.current_bg = DEFAULT_BG;
                    self.bold = false;
                    return;
                }
                
                let mut params_iter = params.iter();
                while let Some(param) = params_iter.next() {
                    let p = param.get(0).copied().unwrap_or(0);
                    match p {
                        0 => { // Reset
                            self.current_fg = DEFAULT_FG;
                            self.current_bg = DEFAULT_BG;
                            self.bold = false;
                        }
                        1 => { // Bold
                            self.bold = true;
                        }
                        22 => { // Normal color or intensity
                            self.bold = false;
                        }
                        30..=37 => { // Foreground color
                            let idx = (p - 30) as usize;
                            self.current_fg = COLOR_PALETTE[idx];
                        }
                        38 => { // Extended Foreground (256-color or RGB)
                            // We can skip or parse basic 256 colors if next params allow.
                            // For simplicity, support 256 colors basic mapping
                            if let Some(next_param) = params_iter.next() {
                                let mode = next_param.get(0).copied().unwrap_or(0);
                                if mode == 5 { // 256 color
                                    if let Some(color_idx_param) = params_iter.next() {
                                        let c_idx = color_idx_param.get(0).copied().unwrap_or(0) as usize;
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
                                } else if mode == 2 { // RGB
                                    if let (Some(r_p), Some(g_p), Some(b_p)) = (params_iter.next(), params_iter.next(), params_iter.next()) {
                                        let r = r_p.get(0).copied().unwrap_or(0) as f32 / 255.0;
                                        let g = g_p.get(0).copied().unwrap_or(0) as f32 / 255.0;
                                        let b = b_p.get(0).copied().unwrap_or(0) as f32 / 255.0;
                                        self.current_fg = [r, g, b, 1.0];
                                    }
                                }
                            }
                        }
                        39 => { // Default foreground
                            self.current_fg = DEFAULT_FG;
                        }
                        40..=47 => { // Background color
                            let idx = (p - 40) as usize;
                            self.current_bg = COLOR_PALETTE[idx];
                        }
                        48 => { // Extended Background
                            if let Some(next_param) = params_iter.next() {
                                let mode = next_param.get(0).copied().unwrap_or(0);
                                if mode == 5 {
                                    if let Some(color_idx_param) = params_iter.next() {
                                        let c_idx = color_idx_param.get(0).copied().unwrap_or(0) as usize;
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
                                } else if mode == 2 {
                                    if let (Some(r_p), Some(g_p), Some(b_p)) = (params_iter.next(), params_iter.next(), params_iter.next()) {
                                        let r = r_p.get(0).copied().unwrap_or(0) as f32 / 255.0;
                                        let g = g_p.get(0).copied().unwrap_or(0) as f32 / 255.0;
                                        let b = b_p.get(0).copied().unwrap_or(0) as f32 / 255.0;
                                        self.current_bg = [r, g, b, 1.0];
                                    }
                                }
                            }
                        }
                        49 => { // Default background
                            self.current_bg = DEFAULT_BG;
                        }
                        90..=97 => { // Bright foreground
                            let idx = (p - 90 + 8) as usize;
                            self.current_fg = COLOR_PALETTE[idx];
                        }
                        100..=107 => { // Bright background
                            let idx = (p - 100 + 8) as usize;
                            self.current_bg = COLOR_PALETTE[idx];
                        }
                        _ => {}
                    }
                }
            }
            'J' => { // Erase in Display (ED)
                let mode = params.iter().next().and_then(|p| p.get(0)).copied().unwrap_or(0);
                if mode == 2 || mode == 3 {
                    // Clear entire screen
                    for cell in &mut self.cells {
                        *cell = Cell::default();
                    }
                    self.cursor_x = 0;
                    self.cursor_y = 0;
                    if mode == 3 {
                        self.scrollback.clear();
                        self.scroll_offset = 0;
                    }
                }
            }
            'K' => { // Erase in Line (EL)
                let mode = params.iter().next().and_then(|p| p.get(0)).copied().unwrap_or(0);
                let start_idx = self.cursor_y * self.cols;
                match mode {
                    0 => { // Clear from cursor to end of line
                        for x in self.cursor_x..self.cols {
                            self.cells[start_idx + x] = Cell::default();
                        }
                    }
                    1 => { // Clear from start of line to cursor
                        for x in 0..=self.cursor_x.min(self.cols - 1) {
                            self.cells[start_idx + x] = Cell::default();
                        }
                    }
                    2 => { // Clear entire line
                        for x in 0..self.cols {
                            self.cells[start_idx + x] = Cell::default();
                        }
                    }
                    _ => {}
                }
            }
            'H' | 'f' => { // Cursor Position (CUP)
                let mut iter = params.iter();
                let row = iter.next().and_then(|p| p.get(0)).copied().unwrap_or(1) as usize;
                let col = iter.next().and_then(|p| p.get(0)).copied().unwrap_or(1) as usize;
                self.cursor_y = (row.saturating_sub(1)).min(self.rows - 1);
                self.cursor_x = (col.saturating_sub(1)).min(self.cols - 1);
            }
            'A' => { // Cursor Up
                let count = params.iter().next().and_then(|p| p.get(0)).copied().unwrap_or(1) as usize;
                self.cursor_y = self.cursor_y.saturating_sub(count);
            }
            'B' => { // Cursor Down
                let count = params.iter().next().and_then(|p| p.get(0)).copied().unwrap_or(1) as usize;
                self.cursor_y = (self.cursor_y + count).min(self.rows - 1);
            }
            'C' => { // Cursor Forward
                let count = params.iter().next().and_then(|p| p.get(0)).copied().unwrap_or(1) as usize;
                self.cursor_x = (self.cursor_x + count).min(self.cols - 1);
            }
            'D' => { // Cursor Backward
                let count = params.iter().next().and_then(|p| p.get(0)).copied().unwrap_or(1) as usize;
                self.cursor_x = self.cursor_x.saturating_sub(count);
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2 {
            let action = params[0];
            if action == b"0" || action == b"2" {
                if let Ok(title) = std::str::from_utf8(params[1]) {
                    self.title = title.to_string();
                }
            }
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
}

impl TerminalInstance {
    pub fn new(cols: usize, rows: usize, window: std::sync::Arc<winit::window::Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pty_pair = pty_system.openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        
        if let Ok(current_dir) = std::env::current_dir() {
            cmd.cwd(current_dir);
        }

        let child = pty_pair.slave.spawn_command(cmd)?;

        let mut pty_reader = pty_pair.master.try_clone_reader()?;
        let pty_writer = pty_pair.master.take_writer()?;

        let (tx, rx) = channel();

        // Background reader thread
        let win = window.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = pty_reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
                win.request_redraw();
            }
        });

        let grid = TerminalGrid::new(cols, rows);

        Ok(Self {
            name: "terminal".to_string(),
            pty_writer,
            rx,
            grid,
            pty_pair,
            child,
        })
    }

    pub fn resize_pty(&self, cols: usize, rows: usize) {
        let _ = self.pty_pair.master.resize(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}
