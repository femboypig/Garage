use super::context::UiContext;

pub struct Input<'a> {
    value: &'a str,
    placeholder: &'a str,
    focused: bool,
    active: bool,
    has_border: bool,
    icon: Option<&'a str>,
    right_padding: Option<f32>,
}

impl<'a> Default for Input<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Input<'a> {
    pub fn new() -> Self {
        Self {
            value: "",
            placeholder: "",
            focused: false,
            active: false,
            has_border: true,
            icon: None,
            right_padding: None,
        }
    }

    pub fn value(mut self, val: &'a str) -> Self {
        self.value = val;
        self
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn has_border(mut self, has_border: bool) -> Self {
        self.has_border = has_border;
        self
    }

    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn right_padding(mut self, padding: f32) -> Self {
        self.right_padding = Some(padding);
        self
    }

    pub fn draw(self, ctx: &mut UiContext, x: f32, y: f32, w: f32, h: f32) {
        // Background
        ctx.push_quad(x, y, w, h, ctx.theme.editor_bg);

        // Border
        if self.has_border {
            let border_color = if self.focused {
                ctx.theme.cursor_color
            } else {
                ctx.theme.modal_border
            };
            ctx.push_quad(x, y, w, 1.0, border_color); // top
            ctx.push_quad(x, y + h - 1.0, w, 1.0, border_color); // bottom
            ctx.push_quad(x, y, 1.0, h, border_color); // left
            ctx.push_quad(x + w - 1.0, y, 1.0, h, border_color); // right
        }

        let mut start_x = x + 6.0;

        // Icon inside input box
        if let Some(ic) = self.icon {
            let icon_sz = 16.0f32;
            let icon_y = (y + (h - icon_sz) / 2.0).round();
            ctx.push_icon(ic, start_x, icon_y, ctx.theme.modal_text_muted, icon_sz);
            start_x += icon_sz + 6.0;
        }

        let baseline_y = (y + h / 2.0 + ctx.ui_font_ascent / 2.0 - 1.0).round();
        let padding_r = self
            .right_padding
            .unwrap_or(if self.icon.is_some() { 28.0 } else { 12.0 });
        let left_offset = start_x - x;
        let max_chars = ((w - padding_r - left_offset) / ctx.ui_char_width)
            .floor()
            .max(1.0) as usize;

        // Value or placeholder text
        if self.value.is_empty() {
            let display_placeholder = if self.placeholder.chars().count() > max_chars {
                self.placeholder.chars().take(max_chars).collect::<String>()
            } else {
                self.placeholder.to_string()
            };
            ctx.push_str(
                &display_placeholder,
                start_x,
                baseline_y,
                ctx.theme.syntax_comment,
                ctx.ui_font_size,
            );
        } else {
            let display_val = if self.value.chars().count() > max_chars {
                if self.focused {
                    self.value
                        .chars()
                        .skip(self.value.chars().count() - max_chars)
                        .collect::<String>()
                } else {
                    self.value.chars().take(max_chars).collect::<String>()
                }
            } else {
                self.value.to_string()
            };

            ctx.push_str(
                &display_val,
                start_x,
                baseline_y,
                ctx.theme.modal_text_normal,
                ctx.ui_font_size,
            );

            // Cursor
            if self.focused {
                let cursor_x = start_x + display_val.chars().count() as f32 * ctx.ui_char_width;
                if cursor_x < x + w - padding_r.min(5.0) {
                    ctx.push_quad(cursor_x, y + 3.0, 1.5, h - 6.0, ctx.theme.cursor_color);
                }
            }
        }
    }
}
