pub mod dropdown;
pub mod command_palette;
pub mod unsaved_changes;
pub mod about;
pub mod settings;

use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::ui::{UiState, MenuType, ModalType};

pub fn draw_modals(
    ui: &mut UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    width: f32,
    height: f32,
    mouse_x: f32,
    mouse_y: f32,
    current_backend: wgpu::Backend,
    tab_paths: &[Option<String>],
) {
    let white_uv = atlas.white_pixel_uv();

    // --- 6. Draw Context Dropdown Menus (On top of everything) ---
    if let Some(menu) = ui.active_menu {
        dropdown::draw_dropdown(ui, vertices, indices, atlas, queue, mouse_x, mouse_y, menu);
    }

    // --- 7. Draw Modal Dialogs (On top of dropdowns/everything) ---
    if let Some(modal) = ui.active_modal {
        // Semi-transparent black background overlay
        ui.push_quad(
            vertices,
            indices,
            0.0,
            0.0,
            width,
            height,
            white_uv,
            [0.0, 0.0, 0.0, 0.4],
        );
        let modal_w = match modal {
            ModalType::Settings => (45.0 * ui.ui_char_width).max(500.0).round(),
            ModalType::About => 520.0,
            ModalType::CommandPalette => (50.0 * ui.ui_char_width).max(500.0).round(),
            ModalType::UnsavedChanges => 520.0,
        };
        let modal_h = match modal {
            ModalType::Settings => {
                let row_height = (ui.ui_line_height * 2.2).round();
                (row_height * 8.2).max(430.0).round()
            }
            ModalType::About => 190.0,
            ModalType::CommandPalette => {
                let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
                let filtered_len = ui.get_filtered_commands().len();
                let visible_items = filtered_len.min(10);
                let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
                (header_h + visible_items as f32 * item_height + 15.0).round()
            }
            ModalType::UnsavedChanges => 200.0,
        };
        let modal_x = ((width - modal_w) / 2.0).round();
        let modal_y = ((height - modal_h) / 2.0).round();

        // Draw Modal Box Background
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            white_uv,
            ui.config.theme.modal_bg,
        );
        // Draw modal borders
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y,
            modal_w,
            1.0,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y + modal_h - 1.0,
            modal_w,
            1.0,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            modal_x,
            modal_y,
            1.0,
            modal_h,
            white_uv,
            ui.config.theme.modal_border,
        );
        ui.push_quad(
            vertices,
            indices,
            modal_x + modal_w - 1.0,
            modal_y,
            1.0,
            modal_h,
            white_uv,
            ui.config.theme.modal_border,
        );

        match modal {
            ModalType::CommandPalette => {
                command_palette::draw_command_palette(
                    ui,
                    vertices,
                    indices,
                    atlas,
                    queue,
                    modal_x,
                    modal_y,
                    modal_w,
                    modal_h,
                    white_uv,
                );
            }
            ModalType::UnsavedChanges => {
                unsaved_changes::draw_unsaved_changes(
                    ui,
                    vertices,
                    indices,
                    atlas,
                    queue,
                    mouse_x,
                    mouse_y,
                    modal_x,
                    modal_y,
                    modal_w,
                    modal_h,
                    white_uv,
                    tab_paths,
                );
            }
            ModalType::About => {
                about::draw_about(
                    ui,
                    vertices,
                    indices,
                    atlas,
                    queue,
                    modal_x,
                    modal_y,
                    modal_w,
                    modal_h,
                    white_uv,
                );
            }
            ModalType::Settings => {
                settings::draw_settings(
                    ui,
                    vertices,
                    indices,
                    atlas,
                    queue,
                    mouse_x,
                    mouse_y,
                    current_backend,
                    modal_x,
                    modal_y,
                    modal_w,
                    modal_h,
                    white_uv,
                );
            }
        }

        if modal != ModalType::CommandPalette && modal != ModalType::UnsavedChanges {
            // Draw generic Close Button (centered horizontally)
            let btn_w = (12.0 * ui.ui_char_width).max(100.0).round();
            let btn_h = (ui.ui_line_height * 1.6).max(30.0).round();
            let btn_x = modal_x + ((modal_w - btn_w) / 2.0).round();
            let btn_y = modal_y + modal_h - btn_h - (ui.ui_line_height * 1.0).round();

            let close_btn_hover = mouse_x >= btn_x && mouse_x <= btn_x + btn_w && mouse_y >= btn_y && mouse_y <= btn_y + btn_h;
            ui.push_quad(
                vertices,
                indices,
                btn_x,
                btn_y,
                btn_w,
                btn_h,
                white_uv,
                if close_btn_hover { ui.config.theme.button_hover_bg } else { ui.config.theme.button_bg },
            );
            // Draw borders
            ui.push_quad(vertices, indices, btn_x, btn_y, btn_w, 1.0, white_uv, ui.config.theme.button_border);
            ui.push_quad(vertices, indices, btn_x, btn_y + btn_h - 1.0, btn_w, 1.0, white_uv, ui.config.theme.button_border);
            ui.push_quad(vertices, indices, btn_x, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);
            ui.push_quad(vertices, indices, btn_x + btn_w - 1.0, btn_y, 1.0, btn_h, white_uv, ui.config.theme.button_border);

            let close_text = "Close";
            let close_text_w = close_text.chars().count() as f32 * ui.ui_char_width;
            let close_text_x = btn_x + ((btn_w - close_text_w) / 2.0).round();
            let close_text_y = (btn_y + btn_h / 2.0 + ui.ui_font_ascent / 2.0 - 2.0).round();

            ui.push_str(
                vertices,
                indices,
                atlas,
                queue,
                close_text,
                close_text_x,
                close_text_y,
                ui.config.theme.button_text,
                ui.ui_font_size,
                ui.ui_char_width,
            );
        }
    }
}
