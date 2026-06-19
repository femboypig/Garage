use super::context::UiContext;

pub struct Button<'a> {
    text: Option<&'a str>,
    icon: Option<&'a str>,
    active: bool,
    bg_color: Option<[f32; 4]>,
    text_color: Option<[f32; 4]>,
    border: bool,
    border_color: Option<[f32; 4]>,
    hover_bg: Option<[f32; 4]>,
}

impl<'a> Button<'a> {
    pub fn new() -> Self {
        Self {
            text: None,
            icon: None,
            active: false,
            bg_color: None,
            text_color: None,
            border: false,
            border_color: None,
            hover_bg: None,
        }
    }

    pub fn text(mut self, text: &'a str) -> Self {
        self.text = Some(text);
        self
    }

    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn bg_color(mut self, color: [f32; 4]) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn text_color(mut self, color: [f32; 4]) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn border(mut self, border: bool) -> Self {
        self.border = border;
        self
    }

    pub fn border_color(mut self, color: [f32; 4]) -> Self {
        self.border_color = Some(color);
        self
    }

    pub fn hover_bg(mut self, color: [f32; 4]) -> Self {
        self.hover_bg = Some(color);
        self
    }

    pub fn draw(self, ctx: &mut UiContext, x: f32, y: f32, w: f32, h: f32) -> bool {
        let hover = ctx.mouse_x >= x && ctx.mouse_x < x + w && ctx.mouse_y >= y && ctx.mouse_y < y + h;

        // Determine background color
        let bg = if self.active {
            ctx.theme.selection_bg
        } else if hover {
            self.hover_bg.unwrap_or(ctx.theme.button_hover_bg)
        } else {
            self.bg_color.unwrap_or([0.0, 0.0, 0.0, 0.0])
        };

        // Draw background
        ctx.push_quad(x, y, w, h, bg);

        // Draw border if requested
        if self.border {
            let border_color = self.border_color.unwrap_or(ctx.theme.button_border);
            ctx.push_quad(x, y, w, 1.0, border_color); // top
            ctx.push_quad(x, y + h - 1.0, w, 1.0, border_color); // bottom
            ctx.push_quad(x, y, 1.0, h, border_color); // left
            ctx.push_quad(x + w - 1.0, y, 1.0, h, border_color); // right
        }

        // Draw icon/text content
        let t_color = self.text_color.unwrap_or(ctx.theme.button_text);
        
        let mut content_w = 0.0;
        if let Some(txt) = self.text {
            let text_len = txt.chars().count() as f32 * ctx.ui_char_width;
            content_w += text_len;
        }

        let icon_sz = (h * 0.6).round().min(16.0);
        if self.icon.is_some() {
            if self.text.is_some() {
                content_w += icon_sz + 4.0;
            } else {
                content_w += icon_sz;
            }
        }

        let start_x = x + ((w - content_w) / 2.0).round();
        let mut current_x = start_x;

        if let Some(ic) = self.icon {
            let icon_y = (y + (h - icon_sz) / 2.0).round();
            ctx.push_icon(ic, current_x, icon_y, t_color, icon_sz);
            if self.text.is_some() {
                current_x += icon_sz + 4.0;
            }
        }

        if let Some(txt) = self.text {
            let baseline_y = (y + h / 2.0 + ctx.ui_font_ascent / 2.0 - 1.0).round();
            ctx.push_str(txt, current_x, baseline_y, t_color, ctx.ui_font_size);
        }

        hover
    }
}
