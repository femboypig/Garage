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

impl Theme {
    pub fn get_by_name(name: &str) -> Self {
        match name {
            "Dark Theme" => Self::dark(),
            "Dracula" => Self::dracula(),
            _ => Self::default(), // "Light Theme"
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "Dark Theme".to_string(),
            titlebar_bg: [0.035, 0.035, 0.043, 1.0], // zinc-950
            titlebar_border: [0.153, 0.153, 0.165, 1.0], // zinc-800
            titlebar_text: [0.8, 0.8, 0.8, 1.0],
            titlebar_hover_bg: [0.153, 0.153, 0.165, 0.5],
            titlebar_brand_text: [0.231, 0.514, 0.965, 1.0], // electric blue
            
            sidebar_bg: [0.035, 0.035, 0.043, 1.0],
            sidebar_border: [0.153, 0.153, 0.165, 1.0],
            sidebar_text_dir: [0.957, 0.957, 0.961, 1.0],
            sidebar_text_file: [0.635, 0.635, 0.663, 1.0], // zinc-400
            sidebar_selected_bg: [0.153, 0.153, 0.165, 1.0],
            sidebar_hover_bg: [0.153, 0.153, 0.165, 0.5],
            
            tabbar_bg: [0.035, 0.035, 0.043, 1.0],
            tabbar_border: [0.153, 0.153, 0.165, 1.0],
            tab_active_bg: [0.094, 0.094, 0.102, 1.0], // zinc-900
            tab_inactive_bg: [0.035, 0.035, 0.043, 1.0],
            tab_text: [0.957, 0.957, 0.961, 1.0],
            
            breadcrumb_bg: [0.094, 0.094, 0.102, 1.0],
            breadcrumb_border: [0.153, 0.153, 0.165, 1.0],
            breadcrumb_text: [0.443, 0.443, 0.478, 1.0], // zinc-500
            
            editor_bg: [0.094, 0.094, 0.102, 1.0],
            gutter_bg: [0.094, 0.094, 0.102, 1.0],
            gutter_border: [0.153, 0.153, 0.165, 0.0],
            active_line_bg: [0.153, 0.153, 0.165, 0.3],
            line_number_active: [0.957, 0.957, 0.961, 1.0],
            line_number_inactive: [0.322, 0.322, 0.353, 1.0], // zinc-700
            selection_bg: [0.153, 0.153, 0.165, 0.8],
            cursor_color: [0.231, 0.514, 0.965, 1.0], // electric blue
            
            syntax_default: [0.957, 0.957, 0.961, 1.0],
            syntax_keyword: [0.231, 0.514, 0.965, 1.0], // blue
            syntax_type: [0.18, 0.8, 0.44, 1.0], // green
            syntax_number: [0.95, 0.61, 0.07, 1.0], // orange
            syntax_string: [0.9, 0.73, 0.09, 1.0], // yellow
            syntax_comment: [0.443, 0.443, 0.478, 1.0], // comment grey
            syntax_attribute: [0.61, 0.35, 0.71, 1.0], // purple
            
            scrollbar_track: [0.094, 0.094, 0.102, 1.0],
            scrollbar_border: [0.153, 0.153, 0.165, 1.0],
            scrollbar_thumb: [0.153, 0.153, 0.165, 0.6],
            scrollbar_thumb_hover: [0.153, 0.153, 0.165, 0.9],
            
            statusbar_bg: [0.035, 0.035, 0.043, 1.0],
            statusbar_border: [0.153, 0.153, 0.165, 1.0],
            statusbar_text: [0.443, 0.443, 0.478, 1.0],
            
            modal_bg: [0.094, 0.094, 0.102, 1.0],
            modal_border: [0.153, 0.153, 0.165, 1.0],
            modal_text_title: [0.957, 0.957, 0.961, 1.0],
            modal_text_normal: [0.8, 0.8, 0.8, 1.0],
            modal_text_muted: [0.443, 0.443, 0.478, 1.0],
            button_bg: [0.153, 0.153, 0.165, 1.0],
            button_hover_bg: [0.27, 0.27, 0.3, 1.0], // zinc-700
            button_border: [0.153, 0.153, 0.165, 1.0],
            button_text: [0.957, 0.957, 0.961, 1.0],
            dropdown_hover_bg: [0.153, 0.153, 0.165, 1.0],
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "Dracula".to_string(),
            titlebar_bg: [0.098, 0.102, 0.133, 1.0], // #191a21
            titlebar_border: [0.204, 0.216, 0.275, 1.0], // #343746
            titlebar_text: [0.973, 0.973, 0.949, 1.0], // #f8f8f2
            titlebar_hover_bg: [0.267, 0.278, 0.353, 1.0], // #44475a
            titlebar_brand_text: [1.0, 0.475, 0.776, 1.0], // #ff79c6 (pink)
            
            sidebar_bg: [0.098, 0.102, 0.133, 1.0], // #191a21
            sidebar_border: [0.204, 0.216, 0.275, 1.0], // #343746
            sidebar_text_dir: [0.741, 0.576, 0.976, 1.0], // #bd93f9 (purple)
            sidebar_text_file: [0.973, 0.973, 0.949, 1.0], // #f8f8f2
            sidebar_selected_bg: [0.267, 0.278, 0.353, 1.0], // #44475a
            sidebar_hover_bg: [0.267, 0.278, 0.353, 0.5],
            
            tabbar_bg: [0.098, 0.102, 0.133, 1.0], // #191a21
            tabbar_border: [0.204, 0.216, 0.275, 1.0], // #343746
            tab_active_bg: [0.157, 0.165, 0.212, 1.0], // #282a36 (editor bg)
            tab_inactive_bg: [0.098, 0.102, 0.133, 1.0],
            tab_text: [0.973, 0.973, 0.949, 1.0],
            
            breadcrumb_bg: [0.157, 0.165, 0.212, 1.0], // #282a36
            breadcrumb_border: [0.204, 0.216, 0.275, 1.0], // #343746
            breadcrumb_text: [0.545, 0.914, 0.992, 1.0], // #8be9fd (cyan)
            
            editor_bg: [0.157, 0.165, 0.212, 1.0], // #282a36
            gutter_bg: [0.157, 0.165, 0.212, 1.0], // #282a36
            gutter_border: [0.204, 0.216, 0.275, 0.0],
            active_line_bg: [0.267, 0.278, 0.353, 0.3], // #44475a with transparency
            line_number_active: [1.0, 0.722, 0.424, 1.0], // #ffb86c (orange)
            line_number_inactive: [0.384, 0.447, 0.643, 1.0], // #6272a4 (comment)
            selection_bg: [0.267, 0.278, 0.353, 0.8],
            cursor_color: [1.0, 0.475, 0.776, 1.0], // #ff79c6 (pink)
            
            syntax_default: [0.973, 0.973, 0.949, 1.0], // #f8f8f2
            syntax_keyword: [1.0, 0.475, 0.776, 1.0], // #ff79c6 (pink)
            syntax_type: [0.545, 0.914, 0.992, 1.0], // #8be9fd (cyan)
            syntax_number: [0.741, 0.576, 0.976, 1.0], // #bd93f9 (purple)
            syntax_string: [0.945, 0.98, 0.549, 1.0], // #f1fa8c (yellow)
            syntax_comment: [0.384, 0.447, 0.643, 1.0], // #6272a4
            syntax_attribute: [0.314, 0.98, 0.482, 1.0], // #50fa7b (green)
            
            scrollbar_track: [0.157, 0.165, 0.212, 1.0],
            scrollbar_border: [0.204, 0.216, 0.275, 1.0],
            scrollbar_thumb: [0.267, 0.278, 0.353, 0.6],
            scrollbar_thumb_hover: [0.267, 0.278, 0.353, 0.9],
            
            statusbar_bg: [0.098, 0.102, 0.133, 1.0], // #191a21
            statusbar_border: [0.204, 0.216, 0.275, 1.0], // #343746
            statusbar_text: [0.384, 0.447, 0.643, 1.0], // #6272a4
            
            modal_bg: [0.157, 0.165, 0.212, 1.0], // #282a36
            modal_border: [0.204, 0.216, 0.275, 1.0], // #343746
            modal_text_title: [1.0, 0.475, 0.776, 1.0], // #ff79c6
            modal_text_normal: [0.973, 0.973, 0.949, 1.0], // #f8f8f2
            modal_text_muted: [0.384, 0.447, 0.643, 1.0], // #6272a4
            button_bg: [0.267, 0.278, 0.353, 1.0], // #44475a
            button_hover_bg: [0.384, 0.447, 0.643, 0.7],
            button_border: [0.204, 0.216, 0.275, 1.0],
            button_text: [0.973, 0.973, 0.949, 1.0],
            dropdown_hover_bg: [0.267, 0.278, 0.353, 1.0],
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub ui_font_size: f32,
    pub buffer_font_size: f32,
    pub sidebar_width: f32,
    pub backend: String, // "Vulkan" or "OpenGL"
    pub theme: Theme,
    #[serde(default = "default_true")]
    pub show_git_blame: bool,
    #[serde(default = "default_true")]
    pub show_git_branch: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui_font_size: 11.0,
            buffer_font_size: 13.0,
            sidebar_width: 200.0,
            backend: "Vulkan".to_string(),
            theme: Theme::default(),
            show_git_blame: true,
            show_git_branch: true,
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

    pub fn save_in_background(&self) {
        let config_clone = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = config_clone.save() {
                eprintln!("Failed to save config in background: {:?}", e);
            }
        });
    }
}
