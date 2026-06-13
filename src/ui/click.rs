use crate::editor::buffer::Buffer;
use crate::editor::cursor::Cursor;
use super::{UiState, UiAction};

pub mod modal;
pub mod menu;
pub mod workspace;

impl UiState {
    /// Handle click coordinates to determine if a menu, tree, or scroll item was clicked
    pub fn handle_click(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        tab_paths: &[Option<String>],
        tab_modified: &[bool],
        terminals: &[crate::terminal::TerminalInstance],
    ) -> UiAction {
        // 1. Delegate to modal click handler if a modal is open
        if let Some(modal) = self.active_modal {
            return self.handle_modal_click(mx, my, width, height, buffer, cursor, tab_paths, tab_modified, modal);
        }

        // 2. Delegate to menu click handler (titlebar menu, dropdown menu)
        if let Some(action) = self.handle_menu_click(mx, my, width, buffer, cursor) {
            return action;
        }

        // 3. Delegate to workspace clicks (tabs, sidebar file tree, terminal dock, status bar)
        self.handle_workspace_click(mx, my, width, height, tab_paths, tab_modified, terminals)
    }
}
