use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Theme {
    pub name: String,
    
    // Titlebar
    pub titlebar_bg: [f32; 4],
    pub titlebar_border: [f32; 4],
    pub titlebar_text: [f32; 4],
    pub titlebar_hover_bg: [f32; 4],
    pub titlebar_brand_text: [f32; 4],
    
    // Sidebar
    pub sidebar_bg: [f32; 4],
    pub sidebar_border: [f32; 4],
    pub sidebar_text_dir: [f32; 4],
    pub sidebar_text_file: [f32; 4],
    pub sidebar_selected_bg: [f32; 4],
    pub sidebar_hover_bg: [f32; 4],
    
    // Tab bar
    pub tabbar_bg: [f32; 4],
    pub tabbar_border: [f32; 4],
    pub tab_active_bg: [f32; 4],
    pub tab_inactive_bg: [f32; 4],
    pub tab_text: [f32; 4],
    
    // Breadcrumbs
    pub breadcrumb_bg: [f32; 4],
    pub breadcrumb_border: [f32; 4],
    pub breadcrumb_text: [f32; 4],
    
    // Editor Area
    pub editor_bg: [f32; 4],
    pub gutter_bg: [f32; 4],
    pub gutter_border: [f32; 4],
    pub active_line_bg: [f32; 4],
    pub line_number_active: [f32; 4],
    pub line_number_inactive: [f32; 4],
    pub selection_bg: [f32; 4],
    pub cursor_color: [f32; 4],
    
    // Syntax Highlight Colors
    pub syntax_default: [f32; 4],
    pub syntax_keyword: [f32; 4],
    pub syntax_type: [f32; 4],
    pub syntax_number: [f32; 4],
    pub syntax_string: [f32; 4],
    pub syntax_comment: [f32; 4],
    pub syntax_attribute: [f32; 4],
    
    // Scrollbar
    pub scrollbar_track: [f32; 4],
    pub scrollbar_border: [f32; 4],
    pub scrollbar_thumb: [f32; 4],
    pub scrollbar_thumb_hover: [f32; 4],
    
    // Statusbar
    pub statusbar_bg: [f32; 4],
    pub statusbar_border: [f32; 4],
    pub statusbar_text: [f32; 4],
    
    // Dropdowns & Modals
    pub modal_bg: [f32; 4],
    pub modal_border: [f32; 4],
    pub modal_text_title: [f32; 4],
    pub modal_text_normal: [f32; 4],
    pub modal_text_muted: [f32; 4],
    pub button_bg: [f32; 4],
    pub button_hover_bg: [f32; 4],
    pub button_border: [f32; 4],
    pub button_text: [f32; 4],
    pub dropdown_hover_bg: [f32; 4],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "Light Theme".to_string(),
            titlebar_bg: [0.95, 0.95, 0.95, 1.0],
            titlebar_border: [0.82, 0.82, 0.82, 1.0],
            titlebar_text: [0.2, 0.2, 0.2, 1.0],
            titlebar_hover_bg: [0.88, 0.88, 0.9, 1.0],
            titlebar_brand_text: [0.12, 0.12, 0.12, 1.0],
            
            sidebar_bg: [0.95, 0.95, 0.95, 1.0],
            sidebar_border: [0.88, 0.88, 0.88, 1.0],
            sidebar_text_dir: [0.2, 0.2, 0.2, 1.0],
            sidebar_text_file: [0.12, 0.12, 0.12, 1.0],
            sidebar_selected_bg: [0.82, 0.82, 0.82, 1.0],
            sidebar_hover_bg: [0.9, 0.9, 0.9, 1.0],
            
            tabbar_bg: [0.93, 0.93, 0.93, 1.0],
            tabbar_border: [0.82, 0.82, 0.82, 1.0],
            tab_active_bg: [1.0, 1.0, 1.0, 1.0],
            tab_inactive_bg: [0.92, 0.92, 0.92, 1.0],
            tab_text: [0.12, 0.12, 0.12, 1.0],
            
            breadcrumb_bg: [0.98, 0.98, 0.98, 1.0],
            breadcrumb_border: [0.88, 0.88, 0.9, 1.0],
            breadcrumb_text: [0.4, 0.4, 0.45, 1.0],
            
            editor_bg: [1.0, 1.0, 1.0, 1.0],
            gutter_bg: [0.97, 0.97, 0.97, 1.0],
            gutter_border: [0.88, 0.88, 0.88, 1.0],
            active_line_bg: [0.95, 0.95, 0.95, 1.0],
            line_number_active: [0.15, 0.15, 0.15, 1.0],
            line_number_inactive: [0.6, 0.6, 0.6, 1.0],
            selection_bg: [0.68, 0.84, 1.0, 0.4],
            cursor_color: [0.0, 0.48, 0.8, 1.0],
            
            syntax_default: [0.12, 0.12, 0.12, 1.0],
            syntax_keyword: [0.68, 0.0, 0.85, 1.0],
            syntax_type: [0.15, 0.5, 0.6, 1.0],
            syntax_number: [0.09, 0.45, 0.27, 1.0],
            syntax_string: [0.64, 0.08, 0.08, 1.0],
            syntax_comment: [0.45, 0.45, 0.45, 1.0],
            syntax_attribute: [0.4, 0.4, 0.2, 1.0],
            
            scrollbar_track: [0.98, 0.98, 0.98, 1.0],
            scrollbar_border: [0.9, 0.9, 0.9, 1.0],
            scrollbar_thumb: [0.75, 0.75, 0.75, 1.0],
            scrollbar_thumb_hover: [0.65, 0.65, 0.65, 1.0],
            
            statusbar_bg: [0.95, 0.95, 0.95, 1.0],
            statusbar_border: [0.82, 0.82, 0.82, 1.0],
            statusbar_text: [0.3, 0.3, 0.35, 1.0],
            
            modal_bg: [1.0, 1.0, 1.0, 1.0],
            modal_border: [0.78, 0.78, 0.78, 1.0],
            modal_text_title: [0.12, 0.12, 0.12, 1.0],
            modal_text_normal: [0.2, 0.2, 0.2, 1.0],
            modal_text_muted: [0.45, 0.45, 0.45, 1.0],
            button_bg: [0.92, 0.92, 0.92, 1.0],
            button_hover_bg: [0.85, 0.85, 0.85, 1.0],
            button_border: [0.78, 0.78, 0.78, 1.0],
            button_text: [0.2, 0.2, 0.2, 1.0],
            dropdown_hover_bg: [0.9, 0.9, 0.92, 1.0],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub ui_font_size: f32,
    pub buffer_font_size: f32,
    pub sidebar_width: f32,
    pub backend: String, // "Vulkan" or "OpenGL"
    pub theme: Theme,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui_font_size: 11.0,
            buffer_font_size: 13.0,
            sidebar_width: 200.0,
            backend: "Vulkan".to_string(),
            theme: Theme::default(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("garage").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }
        
        // Return default if file doesn't exist or is corrupted
        let default_config = Self::default();
        let _ = default_config.save(); // Save default configuration
        default_config
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        fs::write(path, content)?;
        Ok(())
    }
}
