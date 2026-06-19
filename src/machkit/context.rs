use crate::editor::config::Theme;
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;

pub struct UiContext<'a> {
    pub vertices: &'a mut Vec<Vertex>,
    pub indices: &'a mut Vec<u16>,
    pub atlas: &'a mut FontAtlas,
    pub queue: &'a wgpu::Queue,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub theme: &'a Theme,
    pub white_uv: [f32; 2],
    pub ui_font_size: f32,
    pub ui_char_width: f32,
    pub ui_font_ascent: f32,
    pub ui_line_height: f32,
    pub buffer_font_size: f32,
    pub buffer_font_ascent: f32,
    pub buffer_line_height: f32,
}

pub fn push_quad_raw(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    white_uv: [f32; 2],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
) {
    let rx = x.round();
    let ry = y.round();
    let rw = (x + w).round() - rx;
    let rh = (y + h).round() - ry;

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

impl<'a> UiContext<'a> {
    pub fn push_quad(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        push_quad_raw(
            self.vertices,
            self.indices,
            self.white_uv,
            x,
            y,
            w,
            h,
            color,
        );
    }

    pub fn push_char(
        &mut self,
        c: char,
        pen_x: f32,
        baseline_y: f32,
        color: [f32; 4],
        font_size: f32,
        char_width: f32,
    ) -> f32 {
        let white_uv = self.white_uv;
        if let Some(info) = self.atlas.get_or_rasterize(self.queue, c, font_size) {
            if info.width == 0.0 || info.height == 0.0 {
                return char_width;
            }

            let cp = c as u32;
            let is_box_drawing = c >= '\u{2500}' && c <= '\u{257f}';
            let is_powerline = c >= '\u{e0b0}' && c <= '\u{e0d4}';

            let is_pua = (cp >= 0xE000 && cp <= 0xF8FF)
                || (cp >= 0xF0000 && cp <= 0xFFFFF)
                || (cp >= 0x100000 && cp <= 0x10FFFF);
            let is_emoji = (cp >= 0x1F300 && cp <= 0x1F9FF)
                || (cp >= 0x1F600 && cp <= 0x1F64F)
                || (cp >= 0x2600 && cp <= 0x27BF);
            let is_special_icon = (is_pua || is_emoji) && !is_powerline;

            let (x, y, w, h) = if is_box_drawing || is_powerline {
                let x_min = pen_x.round();
                let x_max = (pen_x + char_width).round();

                let (ascent, line_h) = if (font_size - self.buffer_font_size).abs() < 0.1 {
                    (self.buffer_font_ascent, self.buffer_line_height)
                } else {
                    (self.ui_font_ascent, self.ui_line_height)
                };

                let y_min = (baseline_y - ascent).round();
                let y_max = (baseline_y - ascent + line_h).round();

                (x_min, y_min, x_max - x_min, y_max - y_min)
            } else if is_special_icon {
                let (ascent, line_h) = if (font_size - self.buffer_font_size).abs() < 0.1 {
                    (self.buffer_font_ascent, self.buffer_line_height)
                } else {
                    (self.ui_font_ascent, self.ui_line_height)
                };

                let max_w = char_width * 0.9;
                let max_h = line_h * 0.9;

                let scale_w = max_w / info.width;
                let scale_h = max_h / info.height;
                let scale = scale_w.min(scale_h).min(1.0);

                let w_val = (info.width * scale).round();
                let h_val = (info.height * scale).round();

                let x_val = (pen_x + (char_width - w_val) / 2.0).round();
                let y_val = (baseline_y - ascent + (line_h - h_val) / 2.0).round();

                (x_val, y_val, w_val, h_val)
            } else {
                let x = (pen_x + info.bearing_x).round();
                let y = (baseline_y - info.bearing_y - info.height).round();
                let w = info.width.round();
                let h = info.height.round();
                (x, y, w, h)
            };

            // Draw under-fill solid quad for powerline solid separators to prevent any horizontal gaps
            if is_powerline {
                let sliver_w = 1.5;
                if cp % 4 == 0 {
                    push_quad_raw(
                        self.vertices,
                        self.indices,
                        white_uv,
                        x,
                        y,
                        sliver_w,
                        h,
                        color,
                    );
                } else if cp % 4 == 2 {
                    push_quad_raw(
                        self.vertices,
                        self.indices,
                        white_uv,
                        x + w - sliver_w,
                        y,
                        sliver_w,
                        h,
                        color,
                    );
                }
            }

            let start = self.vertices.len() as u16;
            self.vertices.push(Vertex {
                position: [x, y],
                tex_coords: info.uv_min,
                color,
            });
            self.vertices.push(Vertex {
                position: [x + w, y],
                tex_coords: [info.uv_max[0], info.uv_min[1]],
                color,
            });
            self.vertices.push(Vertex {
                position: [x + w, y + h],
                tex_coords: info.uv_max,
                color,
            });
            self.vertices.push(Vertex {
                position: [x, y + h],
                tex_coords: [info.uv_min[0], info.uv_max[1]],
                color,
            });

            self.indices.extend_from_slice(&[
                start,
                start + 1,
                start + 2,
                start + 2,
                start + 3,
                start,
            ]);
        }
        char_width
    }

    pub fn push_icon(
        &mut self,
        icon_path: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        size: f32,
    ) -> f32 {
        if let Some(info) = self
            .atlas
            .get_or_rasterize_icon(self.queue, icon_path, size)
        {
            let start = self.vertices.len() as u16;
            let rx = x.round();
            let ry = y.round();
            let rw = info.width.round();
            let rh = info.height.round();

            self.vertices.push(Vertex {
                position: [rx, ry],
                tex_coords: [info.uv_min[0], info.uv_min[1]],
                color,
            });
            self.vertices.push(Vertex {
                position: [rx + rw, ry],
                tex_coords: [info.uv_max[0], info.uv_min[1]],
                color,
            });
            self.vertices.push(Vertex {
                position: [rx + rw, ry + rh],
                tex_coords: [info.uv_max[0], info.uv_max[1]],
                color,
            });
            self.vertices.push(Vertex {
                position: [rx, ry + rh],
                tex_coords: [info.uv_min[0], info.uv_max[1]],
                color,
            });

            self.indices.extend_from_slice(&[
                start,
                start + 1,
                start + 2,
                start + 2,
                start + 3,
                start,
            ]);
            rw
        } else {
            0.0
        }
    }

    pub fn push_str(&mut self, text: &str, x: f32, y: f32, color: [f32; 4], font_size: f32) -> f32 {
        self.push_str_spaced(text, x, y, color, font_size, self.ui_char_width)
    }

    pub fn push_str_spaced(
        &mut self,
        text: &str,
        mut x: f32,
        y: f32,
        color: [f32; 4],
        font_size: f32,
        spacing: f32,
    ) -> f32 {
        let start_x = x;
        for c in text.chars() {
            x += self.push_char(c, x, y, color, font_size, spacing);
        }
        x - start_x
    }
}
