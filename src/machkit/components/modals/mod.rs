pub mod about;
pub mod command_palette;
pub mod dropdown;
pub mod global_search;
pub mod settings;
pub mod sidebar_input;
pub mod unsaved_changes;

use crate::machkit::{ModalType, UiState};
use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;

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
        let modal_rect = ui.modal_rect(modal, width, height);
        let modal_x = modal_rect.x;
        let modal_y = modal_rect.y;
        let modal_w = modal_rect.w;
        let modal_h = modal_rect.h;

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
                    ui, vertices, indices, atlas, queue, modal_x, modal_y, modal_w, modal_h,
                    white_uv,
                );
            }
            ModalType::GlobalSearch => {
                global_search::draw_global_search(
                    ui, vertices, indices, atlas, queue, modal_x, modal_y, modal_w, modal_h,
                    white_uv,
                );
            }
            ModalType::UnsavedChanges => {
                unsaved_changes::draw_unsaved_changes(
                    ui, vertices, indices, atlas, queue, mouse_x, mouse_y, modal_x, modal_y,
                    modal_w, modal_h, white_uv, tab_paths,
                );
            }
            ModalType::About => {
                about::draw_about(
                    ui, vertices, indices, atlas, queue, modal_x, modal_y, modal_w, modal_h,
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
            ModalType::SidebarInput => {
                sidebar_input::draw_sidebar_input(
                    ui, vertices, indices, atlas, queue, mouse_x, mouse_y, modal_x, modal_y,
                    modal_w, white_uv,
                );
            }
        }

        if modal != ModalType::CommandPalette
            && modal != ModalType::UnsavedChanges
            && modal != ModalType::SidebarInput
        {
            let mut ctx = crate::machkit::UiContext {
                vertices,
                indices,
                atlas,
                queue,
                mouse_x,
                mouse_y,
                theme: &ui.config.theme,
                white_uv,
                ui_font_size: ui.ui_font_size,
                ui_char_width: ui.ui_char_width,
                ui_font_ascent: ui.ui_font_ascent,
                ui_line_height: ui.ui_line_height,
                buffer_font_size: ui.buffer_font_size,
                buffer_font_ascent: ui.buffer_font_ascent,
                buffer_line_height: ui.buffer_line_height,
            };

            // Draw generic Close Button (centered horizontally)
            let btn_w = (12.0 * ctx.ui_char_width).max(100.0).round();
            let btn_h = (ctx.ui_line_height * 1.6).max(30.0).round();
            let btn_x = modal_x + ((modal_w - btn_w) / 2.0).round();
            let btn_y = modal_y + modal_h - btn_h - (ctx.ui_line_height * 1.0).round();

            crate::machkit::Button::new()
                .text("Close")
                .border(true)
                .bg_color(ctx.theme.button_bg)
                .draw(&mut ctx, btn_x, btn_y, btn_w, btn_h);
        }
    }
}
