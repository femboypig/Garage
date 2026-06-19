use super::context::UiContext;

pub struct Cursor {
    color: Option<[f32; 4]>,
    width: f32,
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            color: None,
            width: 1.5,
        }
    }

    pub fn color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn draw(self, ctx: &mut UiContext, x: f32, y: f32, h: f32) {
        let color = self.color.unwrap_or(ctx.theme.cursor_color);
        ctx.push_quad(x, y, self.width, h, color);
    }
}
