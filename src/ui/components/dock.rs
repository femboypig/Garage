use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::terminal::TerminalInstance;
use crate::ui::UiState;

pub fn draw_dock(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    height: f32,
    mouse_x: f32,
    mouse_y: f32,
    terminals: &[TerminalInstance],
    terminal_focus: bool,
    dock_start_y: f32,
) {
    if !ui.show_dock {
        return;
    }

    let white_uv = atlas.white_pixel_uv();
    let dock_w = width - ui.sidebar_width;
    let dock_h = (height - ui.status_height - dock_start_y).max(0.0);
    
    // 1. Draw top border
    ui.push_quad(
        vertices,
        indices,
        ui.sidebar_width,
        dock_start_y,
        dock_w,
        1.0,
        white_uv,
        ui.config.theme.tabbar_border,
    );

    // 2. Draw dock tab bar
    let dock_tabbar_h = 28.0f32;
    ui.push_quad(
        vertices,
        indices,
        ui.sidebar_width,
        dock_start_y + 1.0,
        dock_w,
        dock_tabbar_h - 1.0,
        white_uv,
        ui.config.theme.tabbar_bg,
    );

    // 2.5. Calculate active terminal tab layout details beforehand
    let mut active_dock_x = 0.0f32;
    let mut active_dock_w = 0.0f32;
    let mut has_active_dock = false;
    let mut temp_x = ui.sidebar_width + 10.0f32;
    for idx in 0..terminals.len() {
        let term_name = format!("terminal-{}", idx + 1);
        let term_name_w = term_name.chars().count() as f32 * ui.ui_char_width * 0.9;
        let icon_sz = 12.0f32;
        let close_sz = 10.0f32;
        let tab_w = 12.0 + icon_sz + 6.0 + term_name_w + 8.0 + close_sz + 10.0;
        if idx == ui.active_dock_tab {
            active_dock_x = temp_x;
            active_dock_w = tab_w;
            has_active_dock = true;
        }
        temp_x += tab_w;
    }

    // Draw dock tabbar bottom border (skipping the active tab)
    let dock_tabbar_border_y = dock_start_y + dock_tabbar_h;
    let border_start_x = ui.sidebar_width;
    if has_active_dock {
        if active_dock_x > border_start_x {
            ui.push_quad(
                vertices,
                indices,
                border_start_x,
                dock_tabbar_border_y,
                active_dock_x - border_start_x,
                1.0,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
        let border_end_x = active_dock_x + active_dock_w;
        if border_end_x < width {
            ui.push_quad(
                vertices,
                indices,
                border_end_x,
                dock_tabbar_border_y,
                width - border_end_x,
                1.0,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
    } else {
        ui.push_quad(
            vertices,
            indices,
            border_start_x,
            dock_tabbar_border_y,
            dock_w,
            1.0,
            white_uv,
            ui.config.theme.tabbar_border,
        );
    }

    // 3. Draw active/inactive terminal tabs
    let mut cur_x = ui.sidebar_width + 10.0f32;
    let tab_y = dock_start_y + 1.0;
    let tab_h = dock_tabbar_h - 1.0;
    let tab_font_sz = ui.ui_font_size * 0.9;
    let icon_sz = 12.0f32;
    let close_sz = 10.0f32;

    for idx in 0..terminals.len() {
        let is_active = idx == ui.active_dock_tab;
        let term_name = format!("terminal-{}", idx + 1);
        let term_name_w = term_name.chars().count() as f32 * ui.ui_char_width * 0.9;
        let tab_w = 12.0 + icon_sz + 6.0 + term_name_w + 8.0 + close_sz + 10.0;
        
        // Draw tab background
        let bg_color = if is_active {
            ui.config.theme.tab_active_bg
        } else {
            ui.config.theme.tabbar_bg
        };
        let current_tab_h = if is_active {
            dock_tabbar_h
        } else {
            dock_tabbar_h - 1.0
        };
        ui.push_quad(vertices, indices, cur_x, tab_y, tab_w, current_tab_h, white_uv, bg_color);
        
        // Draw separators/borders like in editor tabbar
        if idx > 0 {
            ui.push_quad(
                vertices,
                indices,
                cur_x,
                tab_y,
                1.0,
                dock_tabbar_h,
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
        ui.push_quad(
            vertices,
            indices,
            cur_x + tab_w - 1.0,
            tab_y,
            1.0,
            dock_tabbar_h,
            white_uv,
            ui.config.theme.tabbar_border,
        );

        // Draw terminal icon
        let icon_color = if is_active {
            ui.config.theme.tab_text
        } else {
            let mut c = ui.config.theme.tab_text;
            c[3] *= 0.6;
            c
        };
        let cur_tab_h_for_calc = dock_tabbar_h - 1.0;
        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "terminal",
            cur_x + 8.0,
            (tab_y + (cur_tab_h_for_calc - icon_sz) / 2.0).round(),
            icon_color,
            icon_sz,
        );

        // Draw text
        let tab_baseline = (tab_y + cur_tab_h_for_calc / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &term_name,
            cur_x + 8.0 + icon_sz + 6.0,
            tab_baseline,
            icon_color,
            tab_font_sz,
            ui.ui_char_width * 0.9,
        );

        // Draw tab close button
        let close_x = cur_x + tab_w - 8.0 - close_sz;
        let close_y = (tab_y + (cur_tab_h_for_calc - close_sz) / 2.0).round();
        
        let is_close_hover = ui.active_modal.is_none() && mouse_x >= close_x - 3.0 && mouse_x < close_x + close_sz + 3.0 && mouse_y >= close_y - 3.0 && mouse_y < close_y + close_sz + 3.0;
        let close_color = if is_close_hover {
            [1.0, 0.3, 0.3, 1.0]
        } else {
            let mut c = icon_color;
            c[3] *= 0.5;
            c
        };

        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "close",
            close_x,
            close_y,
            close_color,
            close_sz,
        );

        cur_x += tab_w;
    }

    // Draw '+' button to add new terminal
    let add_btn_w = 28.0f32;
    let add_btn_x = cur_x;
    let is_add_hover = ui.active_modal.is_none() && mouse_x >= add_btn_x && mouse_x < add_btn_x + add_btn_w && mouse_y >= tab_y && mouse_y < tab_y + tab_h;
    let add_bg = if is_add_hover {
        ui.config.theme.titlebar_hover_bg
    } else {
        ui.config.theme.tabbar_bg
    };
    ui.push_quad(vertices, indices, add_btn_x, tab_y, add_btn_w, tab_h, white_uv, add_bg);
    
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        "+",
        add_btn_x + 10.0,
        (tab_y + tab_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
        ui.config.theme.tab_text,
        ui.ui_font_size,
        ui.ui_char_width,
    );

    // Draw Close dock button
    let close_dock_w = 28.0f32;
    let close_dock_x = width - 10.0 - close_dock_w;
    let is_close_dock_hover = ui.active_modal.is_none() && mouse_x >= close_dock_x && mouse_x < close_dock_x + close_dock_w && mouse_y >= tab_y && mouse_y < tab_y + tab_h;
    let close_dock_bg = if is_close_dock_hover {
        ui.config.theme.titlebar_hover_bg
    } else {
        ui.config.theme.tabbar_bg
    };
    ui.push_quad(vertices, indices, close_dock_x, tab_y, close_dock_w, tab_h, white_uv, close_dock_bg);
    ui.push_icon(
        vertices,
        indices,
        atlas,
        queue,
        "close",
        close_dock_x + 8.0,
        (tab_y + (tab_h - 12.0) / 2.0).round(),
        ui.config.theme.tab_text,
        12.0,
    );

    // 4. Draw Terminal Content Area
    let content_y = dock_start_y + dock_tabbar_h + 1.0;
    let content_h = dock_h - dock_tabbar_h - 1.0;
    ui.push_quad(
        vertices,
        indices,
        ui.sidebar_width,
        content_y,
        dock_w,
        content_h,
        white_uv,
        ui.config.theme.editor_bg,
    );

    // 5. Draw active terminal grid cells
    if !terminals.is_empty() {
        let term = &terminals[ui.active_dock_tab.min(terminals.len() - 1)];
        let grid = &term.grid;
        
        let term_font_sz = ui.buffer_font_size;
        let term_char_w = ui.buffer_char_width;
        let term_line_h = ui.buffer_line_height;
        let term_font_ascent = ui.buffer_font_ascent;

        let term_pad_x = 8.0f32;
        let term_pad_y = 6.0f32;

        for ty in 0..grid.rows {
            let cell_y = content_y + term_pad_y + ty as f32 * term_line_h;
            if cell_y + term_line_h > content_y + content_h {
                break;
            }
            
            let cell_baseline = (cell_y + term_font_ascent).round();

            for tx in 0..grid.cols {
                let cell_x = ui.sidebar_width + term_pad_x + tx as f32 * term_char_w;
                if cell_x + term_char_w > width {
                    break;
                }

                let cell = grid.cells[ty * grid.cols + tx];

                // Draw non-default background
                if cell.bg != crate::terminal::DEFAULT_BG {
                    ui.push_quad(
                        vertices,
                        indices,
                        cell_x,
                        cell_y,
                        term_char_w,
                        term_line_h,
                        white_uv,
                        cell.bg,
                    );
                }

                // Draw character if not space
                if cell.c != ' ' {
                    let mut color = cell.fg;
                    if grid.bold && color == crate::terminal::DEFAULT_FG {
                        color = [1.0, 1.0, 1.0, 1.0];
                    }
                    
                    let mut buf = [0u8; 4];
                    let c_str = cell.c.encode_utf8(&mut buf);
                    
                    ui.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        c_str,
                        cell_x,
                        cell_baseline,
                        color,
                        term_font_sz,
                        term_char_w,
                    );
                }
            }
        }

        // Draw Cursor
        let cursor_x = ui.sidebar_width + term_pad_x + grid.cursor_x as f32 * term_char_w;
        let cursor_y = content_y + term_pad_y + grid.cursor_y as f32 * term_line_h;
        
        if cursor_x + term_char_w <= width && cursor_y + term_line_h <= content_y + content_h {
            if terminal_focus {
                ui.push_quad(
                    vertices,
                    indices,
                    cursor_x,
                    cursor_y,
                    term_char_w,
                    term_line_h,
                    white_uv,
                    [0.7, 0.7, 0.7, 0.6],
                );
            } else {
                ui.push_quad(vertices, indices, cursor_x, cursor_y, term_char_w, 1.5, white_uv, [0.6, 0.6, 0.6, 0.8]);
                ui.push_quad(vertices, indices, cursor_x, cursor_y + term_line_h - 1.5, term_char_w, 1.5, white_uv, [0.6, 0.6, 0.6, 0.8]);
                ui.push_quad(vertices, indices, cursor_x, cursor_y, 1.5, term_line_h, white_uv, [0.6, 0.6, 0.6, 0.8]);
                ui.push_quad(vertices, indices, cursor_x + term_char_w - 1.5, cursor_y, 1.5, term_line_h, white_uv, [0.6, 0.6, 0.6, 0.8]);
            }
        }
    }
}
