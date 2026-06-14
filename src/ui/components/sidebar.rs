use crate::renderer::atlas::FontAtlas;
use crate::renderer::wgpu::Vertex;
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
    tab_paths: &[Option<String>],
    tab_modified: &[bool],
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

    let icon_sz = (ui.ui_font_size * 1.25).round().max(16.0);
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

        let relative_path = node.path.strip_prefix("./").unwrap_or(&node.path);
        let mut git_badge = None;
        let mut file_color = if node.is_dir {
            ui.config.theme.sidebar_text_dir
        } else {
            ui.config.theme.sidebar_text_file
        };

        if node.is_dir {
            let mut has_modified = false;
            let mut has_untracked = false;
            for (path, status) in &ui.git_statuses {
                if path.starts_with(relative_path) {
                    if status.contains('M') {
                        has_modified = true;
                    } else if status.contains('?') || status.contains('A') {
                        has_untracked = true;
                    }
                }
            }
            // Also check open unsaved modified tabs under this directory
            if !has_modified {
                for (t_idx, t_path_opt) in tab_paths.iter().enumerate() {
                    if let Some(t_path) = t_path_opt {
                        let t_path_buf = std::path::Path::new(t_path);
                        let t_path_rel = t_path_buf.strip_prefix("./").unwrap_or(t_path_buf);
                        if t_path_rel.starts_with(relative_path) && tab_modified.get(t_idx).copied().unwrap_or(false) {
                            has_modified = true;
                            break;
                        }
                    }
                }
            }
            if has_modified {
                file_color = [0.86, 0.49, 0.18, 0.85]; // muted orange
            } else if has_untracked {
                file_color = [0.18, 0.65, 0.43, 0.85]; // muted green
            }
        } else {
            let mut is_unsaved_modified = false;
            let relative_path = node.path.strip_prefix(".").unwrap_or(&node.path);
            for (t_idx, t_path) in tab_paths.iter().flatten().enumerate() {
                if !t_path.is_empty() {
                    let t_path_buf = std::path::Path::new(t_path);
                    let matches = t_path == relative_path.to_str().unwrap_or("")
                        || t_path == node.path.to_str().unwrap_or("")
                        || crate::editor::normalize_path(t_path_buf) == crate::editor::normalize_path(&node.path);
                    if matches && tab_modified.get(t_idx).copied().unwrap_or(false) {
                        is_unsaved_modified = true;
                        break;
                    }
                }
            }

            if is_unsaved_modified {
                file_color = [0.95, 0.45, 0.1, 1.0]; // bright unsaved orange
            }

            if let Some(status) = ui.git_statuses.get(relative_path) {
                if status.contains('M') {
                    file_color = if is_unsaved_modified { [0.95, 0.45, 0.1, 1.0] } else { [0.86, 0.49, 0.18, 0.85] };
                    git_badge = Some("M");
                } else if status.contains('?') || status.contains('A') {
                    file_color = if is_unsaved_modified { [0.95, 0.45, 0.1, 1.0] } else { [0.18, 0.65, 0.43, 0.85] };
                    git_badge = Some(if status.contains('A') { "A" } else { "U" });
                }
            }
        }

        let text_color = file_color;

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
                if ext == "rs" || ext == "toml" || ext == "json" || ext == "md" { icon_color } else { text_color },
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

            // Draw Git status badge if space permits
            if let Some(badge) = git_badge {
                if current_x + 12.0 < max_x {
                    ui.push_str(
                        vertices,
                        indices,
                        atlas,
                        queue,
                        badge,
                        current_x + 8.0,
                        text_baseline,
                        text_color,
                        ui.ui_font_size * 0.85,
                        ui.ui_char_width * 0.85,
                    );
                }
            }
        }
    }

    // Draw Sidebar Scrollbar if needed
    if total_rows > visible_rows {
        let track_x = activity_bar_width + ui.sidebar_width - 6.0;
        let track_w = 3.0f32;
        let track_y = main_y;
        let track_h = main_height;

        // Draw track
        ui.push_quad(
            vertices,
            indices,
            track_x,
            track_y,
            track_w,
            track_h,
            white_uv,
            ui.config.theme.scrollbar_track,
        );

        let ratio = visible_rows as f32 / total_rows as f32;
        let thumb_h = (track_h * ratio).clamp(15.0_f32.min(track_h), track_h);
        let max_scroll_f = max_scroll as f32;
        let scroll_ratio = if max_scroll_f > 0.0 {
            sidebar_scroll as f32 / max_scroll_f
        } else {
            0.0
        };
        let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

        // Draw thumb
        ui.push_quad(
            vertices,
            indices,
            track_x,
            thumb_y,
            track_w,
            thumb_h,
            white_uv,
            ui.config.theme.scrollbar_thumb,
        );
    }

    if let Some((menu_x, menu_y, _, _)) = ui.sidebar_context_menu {
        let items = &["New File", "New Folder", "Rename", "Delete"];
        let item_height = ui.ui_line_height;
        let menu_w = 120.0f32;
        let menu_h = items.len() as f32 * item_height;
        
        ui.push_quad(vertices, indices, menu_x, menu_y, menu_w, menu_h, white_uv, ui.config.theme.modal_bg);
        ui.push_quad(vertices, indices, menu_x, menu_y, menu_w, 1.0, white_uv, ui.config.theme.modal_border);
        ui.push_quad(vertices, indices, menu_x, menu_y + menu_h - 1.0, menu_w, 1.0, white_uv, ui.config.theme.modal_border);
        ui.push_quad(vertices, indices, menu_x, menu_y, 1.0, menu_h, white_uv, ui.config.theme.modal_border);
        ui.push_quad(vertices, indices, menu_x + menu_w - 1.0, menu_y, 1.0, menu_h, white_uv, ui.config.theme.modal_border);
        
        for (idx, label) in items.iter().enumerate() {
            let item_y = menu_y + idx as f32 * item_height;
            let is_hovered = mouse_x >= menu_x && mouse_x < menu_x + menu_w && mouse_y >= item_y && mouse_y < item_y + item_height;
            
            if is_hovered {
                ui.push_quad(vertices, indices, menu_x + 1.0, item_y + 1.0, menu_w - 2.0, item_height - 2.0, white_uv, ui.config.theme.dropdown_hover_bg);
            }
            
            let text_baseline = (item_y + item_height / 2.0 + ui.ui_font_ascent / 2.0 - 1.0).round();
            ui.push_str(vertices, indices, atlas, queue, label, menu_x + 10.0, text_baseline, ui.config.theme.modal_text_normal, ui.ui_font_size, ui.ui_char_width);
        }
    }
}
