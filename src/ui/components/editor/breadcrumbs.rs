use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

pub fn draw_breadcrumbs(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    cursor: &Cursor,
    width: f32,
    main_y: f32,
    activity_bar_width: f32,
    active_file_path: Option<&str>,
) {
    let white_uv = atlas.white_pixel_uv();
    
    // Breadcrumb Bar background
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        main_y + ui.tabbar_height,
        width - (activity_bar_width + ui.sidebar_width),
        ui.breadcrumb_height,
        white_uv,
        ui.config.theme.breadcrumb_bg,
    );
    // Breadcrumb bottom border
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        main_y + ui.tabbar_height + ui.breadcrumb_height - 1.0,
        width - (activity_bar_width + ui.sidebar_width),
        1.0,
        white_uv,
        ui.config.theme.breadcrumb_border,
    );
    
    // Construct breadcrumb text: relative_path > current_function
    let relative_path = active_file_path
        .unwrap_or("Untitled");
    
    let current_fn = ui.find_current_function(buffer, cursor.line);
    let breadcrumb_text = if let Some(ref func) = current_fn {
        format!("{} > {}", relative_path, func)
    } else {
        relative_path.to_string()
    };
    ui.push_str(
        vertices,
        indices,
        atlas,
        queue,
        &breadcrumb_text,
        activity_bar_width + ui.sidebar_width + 15.0,
        (main_y + ui.tabbar_height + ui.breadcrumb_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
        ui.config.theme.breadcrumb_text,
        ui.ui_font_size,
        ui.ui_char_width,
    );
}
