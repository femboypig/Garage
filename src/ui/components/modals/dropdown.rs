use crate::ui::{UiState, Vertex, FontAtlas, MenuType};

pub fn draw_dropdown(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    mouse_x: f32,
    mouse_y: f32,
    menu: MenuType,
) {
    let white_uv = atlas.white_pixel_uv();
    let items = match menu {
        MenuType::Garage => vec!["Settings", "About", "Exit"],
        MenuType::File => vec!["Save (Ctrl+S)", "Toggle Sidebar", "Exit"],
        MenuType::Edit => vec![
            "Undo (Ctrl+Z)",
            "Redo (Ctrl+Y)",
            "Find (Ctrl+F)",
            "Find in Project (Ctrl+Shift+F)",
        ],
        MenuType::Selection => vec!["Select All", "Clear Selection"],
        MenuType::View => vec!["Toggle Sidebar", "Command Palette (Ctrl+Shift+P)"],
    };
    
    // Calculate dynamic menu_x matching the header position
    let menu_items_raw = [
        ("Garage", MenuType::Garage),
        ("File", MenuType::File),
        ("Edit", MenuType::Edit),
        ("Selection", MenuType::Selection),
        ("View", MenuType::View),
    ];
    let mut menu_x = 0.0;
    let mut current_x = 0.0;
    for (i, (label, m_type)) in menu_items_raw.iter().enumerate() {
        let label_len = label.chars().count() as f32;
        let text_w = label_len * ui.ui_char_width;
        let (left_pad, right_pad) = if i == 0 {
            (14.0, 10.0)
        } else {
            (10.0, 10.0)
        };
        let item_w = text_w + left_pad + right_pad;
        if m_type == &menu {
            menu_x = current_x;
            break;
        }
        current_x = current_x + item_w;
    }

    let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
    let dropdown_h = items.len() as f32 * item_height;
    let max_chars = items.iter().map(|s| s.chars().count()).max().unwrap_or(10) as f32;
    let dropdown_w = (max_chars * ui.ui_char_width + 30.0).round();

    // Draw Dropdown Card Background
    ui.push_quad(
        vertices,
        indices,
        menu_x,
        ui.titlebar_height,
        dropdown_w,
        dropdown_h,
        white_uv,
        ui.config.theme.modal_bg,
    );

    // Draw Item Hovers and text
    for (idx, label) in items.iter().enumerate() {
        let row_y = ui.titlebar_height + idx as f32 * item_height;
        let is_hovered = mouse_x >= menu_x && mouse_x < menu_x + dropdown_w && mouse_y >= row_y && mouse_y < row_y + item_height;

        if is_hovered {
            ui.push_quad(
                vertices,
                indices,
                menu_x,
                row_y,
                dropdown_w,
                item_height,
                white_uv,
                ui.config.theme.dropdown_hover_bg,
            );
        }

        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            label,
            menu_x + 12.0,
            (row_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round(),
            if is_hovered {
                if ui.config.theme.name.contains("Dark") {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                }
            } else {
                ui.config.theme.modal_text_normal
            },
            ui.ui_font_size,
            ui.ui_char_width,
        );
    }

    // Draw Card Borders on top of everything (left, right, bottom)
    ui.push_quad(
        vertices,
        indices,
        menu_x,
        ui.titlebar_height,
        1.0,
        dropdown_h,
        white_uv,
        ui.config.theme.modal_border,
    );
    ui.push_quad(
        vertices,
        indices,
        menu_x + dropdown_w - 1.0,
        ui.titlebar_height,
        1.0,
        dropdown_h,
        white_uv,
        ui.config.theme.modal_border,
    );
    ui.push_quad(
        vertices,
        indices,
        menu_x,
        ui.titlebar_height + dropdown_h - 1.0,
        dropdown_w,
        1.0,
        white_uv,
        ui.config.theme.modal_border,
    );
}
