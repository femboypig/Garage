use crate::ui::{UiState, Vertex, FontAtlas};
use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;

pub fn draw_gutter(
    ui: &UiState,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    atlas: &mut FontAtlas,
    queue: &wgpu::Queue,
    _buffer: &Buffer,
    cursor: &Cursor,
    editor_y: f32,
    total_editor_height: f32,
    gutter_width: f32,
    text_area_x: f32,
    activity_bar_width: f32,
    start_idx: usize,
    end_idx: usize,
    max_line_digits: usize,
    active_path: Option<&str>,
) {
    let white_uv = atlas.white_pixel_uv();
    
    // Draw Gutter background
    ui.push_quad(
        vertices,
        indices,
        activity_bar_width + ui.sidebar_width,
        editor_y,
        gutter_width,
        total_editor_height,
        white_uv,
        ui.config.theme.gutter_bg,
    );
    // Draw Gutter border separator
    ui.push_quad(
        vertices,
        indices,
        text_area_x - 1.0,
        editor_y,
        1.0,
        total_editor_height,
        white_uv,
        ui.config.theme.gutter_border,
    );

    let abs_path = active_path.map(crate::editor::lsp::get_absolute_path);

    // Draw line numbers and Git diff indicators
    for line_idx in start_idx..end_idx {
        let row_y = editor_y + (line_idx - start_idx) as f32 * ui.buffer_line_height;
        let baseline_y = (row_y + ui.buffer_font_ascent).round();

        let line_num_str = format!("{:>width$}", line_idx + 1, width = max_line_digits);
        let num_color = if line_idx == cursor.line {
            ui.config.theme.line_number_active
        } else {
            ui.config.theme.line_number_inactive
        };
        ui.push_str(
            vertices,
            indices,
            atlas,
            queue,
            &line_num_str,
            activity_bar_width + ui.sidebar_width + ui.buffer_char_width,
            baseline_y,
            num_color,
            ui.buffer_font_size,
            ui.buffer_char_width,
        );

        // Git Diff Gutter Markers
        let mut line_status = None;
        if let Some(hunks) = active_path.and_then(|p| ui.git_diffs.get(p)) {
            for hunk in hunks {
                match hunk {
                    crate::ui::types::GitDiffHunk::Added { line, count } => {
                        if line_idx >= *line && line_idx < *line + *count {
                            line_status = Some("Added");
                            break;
                        }
                    }
                    crate::ui::types::GitDiffHunk::Modified { line, count } => {
                        if line_idx >= *line && line_idx < *line + *count {
                            line_status = Some("Modified");
                            break;
                        }
                    }
                    crate::ui::types::GitDiffHunk::Deleted { line } => {
                        if line_idx == *line {
                            line_status = Some("Deleted");
                            break;
                        }
                    }
                }
            }
        }

        if let Some(status) = line_status {
            let color = match status {
                "Added" => [0.18, 0.65, 0.43, 1.0],     // green
                "Modified" => [0.86, 0.49, 0.18, 1.0],  // orange
                "Deleted" => [0.90, 0.30, 0.30, 1.0],   // red
                _ => [0.0, 0.0, 0.0, 0.0],
            };
            if status == "Deleted" {
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x - 4.0,
                    row_y,
                    3.0,
                    2.0,
                    white_uv,
                    color,
                );
            } else {
                ui.push_quad(
                    vertices,
                    indices,
                    text_area_x - 4.0,
                    row_y,
                    3.0,
                    ui.buffer_line_height,
                    white_uv,
                    color,
                );
            }
        }

        // LSP Diagnostic Gutter Indicators
        if let Some(ref abs) = abs_path {
            if let Some(diags) = ui.lsp_diagnostics_details.get(abs) {
                // Find the highest severity diagnostic on this line
                let mut max_severity = None;
                for d in diags {
                    if line_idx >= d.line && line_idx <= d.end_line {
                        let sev = d.severity;
                        if max_severity.is_none() || sev < max_severity.unwrap() {
                            max_severity = Some(sev);
                        }
                    }
                }
                if let Some(sev) = max_severity {
                    let indicator_color = match sev {
                        1 => [0.95, 0.25, 0.25, 1.0],  // Error: Red
                        2 => [0.95, 0.6, 0.1, 1.0],    // Warning: Orange
                        3 => [0.2, 0.6, 0.9, 0.8],     // Info: Blue
                        _ => [0.5, 0.5, 0.5, 0.6],      // Hint: Gray
                    };
                    let dot_size = 4.0;
                    let dot_x = activity_bar_width + ui.sidebar_width + 3.0;
                    let dot_y = row_y + (ui.buffer_line_height - dot_size) * 0.5;
                    ui.push_quad(
                        vertices,
                        indices,
                        dot_x,
                        dot_y,
                        dot_size,
                        dot_size,
                        white_uv,
                        indicator_color,
                    );
                }
            }
        }
    }
}
