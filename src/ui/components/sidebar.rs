use crate::renderer::atlas::FontAtlas;
use crate::renderer::gpu::Vertex;
use crate::ui::UiState;

pub fn draw_sidebar(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    main_y: f32,
    main_height: f32,
    mouse_x: f32,
    mouse_y: f32,
) {
    if ui.sidebar_width <= 0.0 {
        return;
    }

    let activity_bar_width = 0.0;
    let white_uv = atlas.white_pixel_uv();

    // Draw Sidebar Panel background
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width,
        main_y,
        ui.sidebar_width,
        main_height,
        white_uv,
        ui.config.theme.sidebar_bg,
    );

    // Draw Sidebar border line
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width - 1.0,
        main_y,
        1.0,
        main_height,
        white_uv,
        ui.config.theme.sidebar_border,
    );

    // Draw sidebar title header (root project directory name in original casing)
    let root_name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "Project".to_string());

    let icon_sz = (ui.ui_font_size * 1.05).round().max(13.0);
    let root_icon_x = activity_bar_width + 10.0;

    let total_rows = 1 + ui.visible_nodes.len();
    let visible_rows = (main_height / ui.ui_line_height).floor() as usize;
    let max_scroll = (total_rows as isize - visible_rows as isize).max(0) as usize;
    let sidebar_scroll = ui.sidebar_scroll.min(max_scroll);

    let start_r = sidebar_scroll;
    let end_r = (sidebar_scroll + visible_rows).min(total_rows);

    // Draw root folder if visible (r == 0)
    if start_r == 0 {
        let row_y = main_y;
        let root_text_baseline = (row_y + ui.ui_line_height / 2.0 + ui.ui_font_ascent / 2.0 - 1.0).round();
        let root_icon_y_center = root_text_baseline - (ui.ui_font_ascent * 0.33).round();
        let root_icon_y = root_icon_y_center - (icon_sz / 2.0).round();
        let root_text_x = root_icon_x + icon_sz + (ui.ui_char_width * 0.6).round().max(4.0);

        // Draw root folder icon
        ui.push_icon(
            vertices,
            indices,
            atlas,
            queue,
            "folder_open",
            root_icon_x,
            root_icon_y,
            ui.config.theme.sidebar_text_dir,
            icon_sz,
        );

        // Draw root folder name text (clipped to sidebar width)
        {
            let max_x = activity_bar_width + ui.sidebar_width - 4.0;
            let mut current_x = root_text_x;
            for c in root_name.chars() {
                if current_x + ui.ui_char_width > max_x {
                    break;
                }
                current_x += ui.push_char(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    c,
                    current_x,
                    root_text_baseline,
                    ui.config.theme.sidebar_text_dir,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
            }
        }

        // Draw line from root icon down to children if there are visible nodes
        if !ui.visible_nodes.is_empty() {
            let line_x = (root_icon_x + icon_sz / 2.0).floor();
            let start_line_y = (root_icon_y + icon_sz).round();
            ui.push_quad(
                vertices,
                indices,
                line_x - 0.5,
                start_line_y,
                1.0,
                (row_y + ui.ui_line_height - start_line_y).max(1.0),
                white_uv,
                ui.config.theme.tabbar_border,
            );
        }
    }

    // Pass 1: Draw all item highlights (hover & active states)
    for r in start_r.max(1)..end_r {
        let idx = r - 1;
        let node = &ui.visible_nodes[idx];
        let row_y = main_y + (r - start_r) as f32 * ui.ui_line_height;

        let is_hovered = ui.active_modal.is_none()
            && mouse_x >= activity_bar_width
            && mouse_x < activity_bar_width + ui.sidebar_width
            && mouse_y >= row_y
            && mouse_y < row_y + ui.ui_line_height;
        let is_selected = ui.selected_file.as_ref() == Some(&node.path);

        if is_hovered || is_selected {
            ui.push_quad(
                vertices,
                indices,
                activity_bar_width,
                row_y,
                ui.sidebar_width - 1.0,
                ui.ui_line_height,
                white_uv,
                if is_selected {
                    ui.config.theme.sidebar_selected_bg
                } else {
                    ui.config.theme.sidebar_hover_bg
                },
            );
        }
    }

    // Pass 2: Draw all guide lines, icons, and text labels
    for r in start_r.max(1)..end_r {
        let idx = r - 1;
        let node = &ui.visible_nodes[idx];
        let row_y = main_y + (r - start_r) as f32 * ui.ui_line_height;

        let effective_depth = node.depth + 1;
        let indent_step = 18.0f32; // Increased to 18px to prevent guide lines from crossing icons
        let indent_x = activity_bar_width + 10.0 + effective_depth as f32 * indent_step;
        let text_color = if node.is_dir {
            ui.config.theme.sidebar_text_dir
        } else {
            ui.config.theme.sidebar_text_file
        };

        let text_baseline = (row_y + ui.ui_line_height / 2.0 + ui.ui_font_ascent / 2.0 - 1.0).round();
        let icon_y_center = text_baseline - (ui.ui_font_ascent * 0.33).round();
        let icon_x = indent_x;
        let icon_y = icon_y_center - (icon_sz / 2.0).round();

        // Draw tree guide lines
        for i in 0..effective_depth {
            let mut has_later = false;
            for next_node in &ui.visible_nodes[idx + 1..] {
                let next_effective_depth = next_node.depth + 1;
                if next_effective_depth <= i {
                    break;
                }
                if next_effective_depth > i {
                    has_later = true;
                    break;
                }
            }

            let should_draw = (i < effective_depth - 1 && has_later) || (i == effective_depth - 1);
            if should_draw {
                let line_x = (activity_bar_width + 10.0 + i as f32 * indent_step + icon_sz / 2.0).floor();
                let end_y = if has_later {
                    row_y + ui.ui_line_height
                } else {
                    icon_y_center
                };
                ui.push_quad(
                    vertices,
                    indices,
                    line_x - 0.5,
                    row_y,
                    1.0,
                    end_y - row_y,
                    white_uv,
                    ui.config.theme.tabbar_border,
                );

                // Draw horizontal branch segment if it's the node's immediate column
                if i == effective_depth - 1 {
                    ui.push_quad(
                        vertices,
                        indices,
                        line_x,
                        icon_y_center - 0.5,
                        indent_x - line_x,
                        1.0,
                        white_uv,
                        ui.config.theme.tabbar_border,
                    );
                }
            }
        }

        if node.is_dir {
            // Draw Folder Outline Icon from SVGs
            let is_expanded = ui.expanded_dirs.contains(&node.path);
            let icon_path = if is_expanded {
                "folder_open"
            } else {
                "folder"
            };

            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                icon_path,
                icon_x,
                icon_y,
                text_color,
                icon_sz,
            );
        } else {
            // Check file extension for specific icon types and colors
            let ext = node.path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let (icon_path, icon_color) = match ext {
                "rs" => ("rust", [0.87, 0.29, 0.15, 1.0]), // Rust red-orange
                "toml" => ("toml", [0.65, 0.53, 0.43, 1.0]), // TOML beige
                "json" => ("json", [0.8, 0.68, 0.0, 1.0]), // JSON yellow
                "md" => ("md", [0.26, 0.53, 0.79, 1.0]), // Markdown blue
                _ => ("file", text_color),
            };

            ui.push_icon(
                vertices,
                indices,
                atlas,
                queue,
                icon_path,
                icon_x,
                icon_y,
                icon_color,
                icon_sz,
            );
        }

        let text_x = icon_x + icon_sz + (ui.ui_char_width * 0.6).round().max(4.0);
        // Draw file/directory name text (clipped to sidebar width)
        {
            let max_x = activity_bar_width + ui.sidebar_width - 4.0;
            let mut current_x = text_x;
            for c in node.name.chars() {
                if current_x + ui.ui_char_width > max_x {
                    break;
                }
                current_x += ui.push_char(
                    vertices,
                    indices,
                    atlas,
                    queue,
                    c,
                    current_x,
                    text_baseline,
                    text_color,
                    ui.ui_font_size,
                    ui.ui_char_width,
                );
            }
        }
    }
}
