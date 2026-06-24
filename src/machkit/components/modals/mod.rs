pub mod about;
pub mod command_palette;
pub mod dropdown;
pub mod global_search;
pub mod settings;
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
        let modal_w = match modal {
            ModalType::Settings => (45.0 * ui.ui_char_width).max(500.0).round(),
            ModalType::About => 520.0,
            ModalType::CommandPalette => (50.0 * ui.ui_char_width).max(500.0).round(),
            ModalType::UnsavedChanges => 520.0,
            ModalType::SidebarInput => 400.0,
            ModalType::GlobalSearch => 650.0,
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
                (header_h + visible_items as f32 * item_height).round()
            }
            ModalType::UnsavedChanges => 200.0,
            ModalType::SidebarInput => 150.0,
            ModalType::GlobalSearch => {
                let item_height = (ui.ui_line_height * 1.6).round().max(26.0);
                let count = ui.global_search_results.len().min(10).max(1);
                let header_h = 15.0 + ui.ui_line_height + 15.0 + 1.0;
                (header_h + count as f32 * item_height).round()
            }
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

                let title = ui
                    .sidebar_input_mode
                    .map(|mode| mode.title())
                    .unwrap_or("Input");

                let title_y = modal_y + 20.0;
                ctx.push_str(
                    title,
                    modal_x + 20.0,
                    title_y + ctx.ui_font_ascent,
                    ctx.theme.modal_text_title,
                    ctx.ui_font_size,
                );

                let input_x = modal_x + 20.0;
                let input_y = title_y + ctx.ui_line_height + 15.0;
                let input_w = modal_w - 40.0;
                let input_h = ctx.ui_line_height + 8.0;

                crate::machkit::Input::new()
                    .value(&ui.sidebar_input_value)
                    .focused(true)
                    .draw(&mut ctx, input_x, input_y, input_w, input_h);

                let btn_w = 80.0f32;
                let btn_h = 24.0f32;
                let cancel_x = modal_x + modal_w - 20.0 - btn_w * 2.0 - 10.0;
                let confirm_x = modal_x + modal_w - 20.0 - btn_w;
                let btn_y = input_y + input_h + 15.0;

                crate::machkit::Button::new()
                    .text("Cancel")
                    .border(true)
                    .bg_color(ctx.theme.button_bg)
                    .draw(&mut ctx, cancel_x, btn_y, btn_w, btn_h);

                crate::machkit::Button::new()
                    .text("OK")
                    .border(true)
                    .bg_color(ctx.theme.button_bg)
                    .draw(&mut ctx, confirm_x, btn_y, btn_w, btn_h);
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
