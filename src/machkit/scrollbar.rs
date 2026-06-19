use super::context::UiContext;

pub struct Scrollbar {
    is_vertical: bool,
    virtual_len: usize,
    visible_count: usize,
    scroll_pos: usize,
    hovered: bool,
}

impl Default for Scrollbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Scrollbar {
    pub fn new() -> Self {
        Self {
            is_vertical: true,
            virtual_len: 0,
            visible_count: 0,
            scroll_pos: 0,
            hovered: false,
        }
    }

    pub fn vertical(mut self, vertical: bool) -> Self {
        self.is_vertical = vertical;
        self
    }

    pub fn virtual_len(mut self, len: usize) -> Self {
        self.virtual_len = len;
        self
    }

    pub fn visible_count(mut self, count: usize) -> Self {
        self.visible_count = count;
        self
    }

    pub fn scroll_pos(mut self, pos: usize) -> Self {
        self.scroll_pos = pos;
        self
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn draw(self, ctx: &mut UiContext, x: f32, y: f32, w: f32, h: f32) {
        if self.virtual_len <= self.visible_count {
            if self.is_vertical {
                // Editor background in scrollbar area to avoid a black gap
                ctx.push_quad(x - 1.0, y, w + 1.0, h, ctx.theme.editor_bg);
            }
            return;
        }

        // Draw track background
        ctx.push_quad(x, y, w, h, ctx.theme.scrollbar_track);

        // Draw track border separator
        if self.is_vertical {
            ctx.push_quad(x - 1.0, y, 1.0, h, ctx.theme.scrollbar_border);
        } else {
            ctx.push_quad(x, y, w, 1.0, ctx.theme.scrollbar_border);
        }

        let thumb_color = if self.hovered {
            ctx.theme.scrollbar_thumb_hover
        } else {
            ctx.theme.scrollbar_thumb
        };

        if self.is_vertical {
            let ratio = self.visible_count as f32 / self.virtual_len as f32;
            let thumb_h = (h * ratio).clamp(20.0f32.min(h), h);
            let max_scroll_f =
                (self.virtual_len as isize - self.visible_count as isize).max(0) as f32;
            let scroll_ratio = if max_scroll_f > 0.0 {
                self.scroll_pos as f32 / max_scroll_f
            } else {
                0.0
            };
            let thumb_y = y + scroll_ratio * (h - thumb_h);

            // Draw Thumb
            ctx.push_quad(x + 2.0, thumb_y, w - 4.0, thumb_h, thumb_color);
        } else {
            let ratio = self.visible_count as f32 / self.virtual_len as f32;
            let thumb_w = (w * ratio).clamp(20.0f32.min(w), w);
            let max_scroll_f =
                (self.virtual_len as isize - self.visible_count as isize).max(0) as f32;
            let scroll_ratio = if max_scroll_f > 0.0 {
                self.scroll_pos as f32 / max_scroll_f
            } else {
                0.0
            };
            let thumb_x = x + scroll_ratio * (w - thumb_w);

            // Draw Thumb
            ctx.push_quad(thumb_x, y + 2.0, thumb_w, h - 4.0, thumb_color);
        }
    }
}
