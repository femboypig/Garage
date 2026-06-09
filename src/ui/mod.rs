use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;

pub struct UiState {
    pub char_width: f32,
    pub line_height: f32,
    pub font_ascent: f32,
    pub scroll_y: usize, // Scroll offset in lines
    pub scroll_x: usize, // Scroll offset in columns
}

impl UiState {
    pub fn new(atlas: &mut FontAtlas, queue: &wgpu::Queue) -> Self {
        // Measure character width using 'm' as standard monospace width
        let glyph_m = atlas.get_or_rasterize(queue, 'm').expect("Failed to rasterize reference character");
        let char_width = glyph_m.width.max(8.0); // Safe fallback

        // Retrieve font metrics for baseline and line height calculation
        let font_metrics = atlas.font.horizontal_line_metrics(atlas.font_size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: atlas.font_size * 0.8,
                descent: -atlas.font_size * 0.2,
                line_gap: atlas.font_size * 0.2,
                new_line_size: atlas.font_size * 1.2,
            });

        let line_height = font_metrics.new_line_size;
        let font_ascent = font_metrics.ascent;

        Self {
            char_width,
            line_height,
            font_ascent,
            scroll_y: 0,
            scroll_x: 0,
        }
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
        let start = vertices.len() as u16;
        vertices.push(Vertex {
            position: [x, y],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [x + w, y],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [x + w, y + h],
            tex_coords: white_uv,
            color,
        });
        vertices.push(Vertex {
            position: [x, y + h],
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
    ) -> f32 {
        if let Some(info) = atlas.get_or_rasterize(queue, c) {
            // Handle space and non-visual chars
            if info.width == 0.0 || info.height == 0.0 {
                return self.char_width;
            }

            // Calculate exact raster glyph position relative to baseline
            let x = pen_x + info.bearing_x;
            let y = baseline_y - info.bearing_y - info.height;
            let w = info.width;
            let h = info.height;

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
        self.char_width
    }

    /// Helper to render a full string of text
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
    ) {
        for c in text.chars() {
            x += self.push_char(vertices, indices, atlas, queue, c, x, y, color);
        }
    }

    /// Build entire UI frame (gutter, selections, text, cursor, status bar)
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
    ) {
        let white_uv = atlas.white_pixel_uv();
        let status_height = 24.0;
        let main_height = height - status_height;

        // Calculate dynamic line number gutter width (at least 4 digits wide)
        let max_line_digits = buffer.len().to_string().len().max(3);
        let gutter_width = (max_line_digits as f32 + 2.0) * self.char_width;
        let text_area_x = gutter_width;

        // Keep scroll inside bounds
        let visible_lines = (main_height / self.line_height).floor() as usize;
        if cursor.line < self.scroll_y {
            self.scroll_y = cursor.line;
        } else if cursor.line >= self.scroll_y + visible_lines {
            self.scroll_y = cursor.line - visible_lines + 1;
        }

        // Draw Gutter Background
        self.push_quad(
            vertices,
            indices,
            0.0,
            0.0,
            gutter_width,
            main_height,
            white_uv,
            [0.08, 0.08, 0.1, 1.0], // Slightly lighter sidebar color
        );

        // Draw Gutter separator line
        self.push_quad(
            vertices,
            indices,
            gutter_width - 1.0,
            0.0,
            1.0,
            main_height,
            white_uv,
            [0.18, 0.18, 0.22, 1.0],
        );

        // Render line-by-line contents
        let start_idx = self.scroll_y;
        let end_idx = (start_idx + visible_lines).min(buffer.len());

        for line_idx in start_idx..end_idx {
            let row_y = (line_idx - start_idx) as f32 * self.line_height;
            let baseline_y = row_y + self.font_ascent;

            // 1. Highlight current active line
            if line_idx == cursor.line {
                self.push_quad(
                    vertices,
                    indices,
                    gutter_width,
                    row_y,
                    width - gutter_width,
                    self.line_height,
                    white_uv,
                    [0.12, 0.12, 0.15, 1.0], // Subtle highlight
                );
            }

            // 2. Draw line number text in gutter
            let line_num_str = format!("{:>width$}", line_idx + 1, width = max_line_digits);
            let num_color = if line_idx == cursor.line {
                [0.7, 0.7, 0.7, 1.0] // Brighter color for current line
            } else {
                [0.3, 0.3, 0.35, 1.0] // Dimmer for others
            };
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &line_num_str,
                self.char_width,
                baseline_y,
                num_color,
            );

            // 3. Render selections (if any range is selected)
            if let Some((s_line, s_col, e_line, e_col)) = cursor.selection_range() {
                if line_idx >= s_line && line_idx <= e_line {
                    let line_chars_count = buffer.lines()[line_idx].chars().count();
                    
                    let col_start = if line_idx == s_line { s_col } else { 0 };
                    let col_end = if line_idx == e_line { e_col } else { line_chars_count };

                    if col_start < col_end || (s_line != e_line && line_idx < e_line) {
                        let sel_x = text_area_x + col_start as f32 * self.char_width;
                        let sel_w = ((col_end - col_start) as f32).max(0.5) * self.char_width;
                        self.push_quad(
                            vertices,
                            indices,
                            sel_x,
                            row_y,
                            sel_w,
                            self.line_height,
                            white_uv,
                            [0.18, 0.28, 0.45, 0.6], // Sleek semi-transparent blue
                        );
                    }
                }
            }

            // 4. Draw source code text characters
            let line_text = &buffer.lines()[line_idx];
            let mut pen_x = text_area_x;
            
            for c in line_text.chars() {
                // Simple lexer/syntax highlighting rules
                let char_color = match c {
                    '0'..='9' => [0.85, 0.65, 0.45, 1.0],   // Orange numbers
                    '{' | '}' | '(' | ')' | '[' | ']' => [0.75, 0.75, 0.4, 1.0], // Yellow brackets
                    _ => [0.85, 0.85, 0.9, 1.0],            // Off-white text
                };

                pen_x += self.push_char(vertices, indices, atlas, queue, c, pen_x, baseline_y, char_color);
            }
        }

        // Draw Active Text Cursor
        if cursor.line >= self.scroll_y && cursor.line < self.scroll_y + visible_lines {
            let cur_row_y = (cursor.line - self.scroll_y) as f32 * self.line_height;
            let cur_x = text_area_x + cursor.col as f32 * self.char_width;
            
            // Solid, bright teal cursor (classic tech aesthetic)
            self.push_quad(
                vertices,
                indices,
                cur_x,
                cur_row_y + 1.0,
                2.0,
                self.line_height - 2.0,
                white_uv,
                [0.0, 0.9, 0.8, 1.0],
            );
        }

        // Draw Status Bar Background
        self.push_quad(
            vertices,
            indices,
            0.0,
            main_height,
            width,
            status_height,
            white_uv,
            [0.1, 0.1, 0.12, 1.0],
        );

        // Draw Status Bar border
        self.push_quad(
            vertices,
            indices,
            0.0,
            main_height,
            width,
            1.0,
            white_uv,
            [0.18, 0.18, 0.22, 1.0],
        );

        // Status bar text
        let status_y = main_height + self.font_ascent + 2.0;
        let status_left = format!(" GARAGE | Line {}, Col {}", cursor.line + 1, cursor.col + 1);
        let status_right = format!("Lines: {} | IBM Plex Mono ", buffer.len());

        self.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &status_left,
            10.0,
            status_y,
            [0.6, 0.6, 0.65, 1.0],
        );

        let right_text_width = status_right.chars().count() as f32 * self.char_width;
        let right_x = width - right_text_width - 15.0;
        if right_x > width / 2.0 {
            self.push_str(
                vertices,
                indices,
                atlas,
                queue,
                &status_right,
                right_x,
                status_y,
                [0.5, 0.5, 0.55, 1.0],
            );
        }
    }
}
