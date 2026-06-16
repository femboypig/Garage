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
    pub syntax_namespace: [f32; 4],
    pub syntax_enum_member: [f32; 4],
    pub syntax_parameter: [f32; 4],
    pub syntax_variable: [f32; 4],
    pub syntax_property: [f32; 4],
    pub syntax_macro: [f32; 4],
    pub syntax_operator: [f32; 4],
    
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

const LIGHT_THEME_JSON: &str = include_str!("../../assets/themes/light.json");
const DARK_THEME_JSON: &str = include_str!("../../assets/themes/dark.json");

impl Default for Theme {
    fn default() -> Self {
        serde_json::from_str(LIGHT_THEME_JSON).expect("Failed to parse built-in Light Theme JSON")
    }
}

impl Theme {
    pub fn get_by_name(name: &str) -> Self {
        match name {
            "Dark Theme" => serde_json::from_str(DARK_THEME_JSON).unwrap_or_else(|e| {
                log::error!("Failed to parse built-in Dark Theme JSON: {}", e);
                Self::default()
            }),
            _ => Self::default(),
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
    #[serde(default)]
    pub override_tiling_wm: Option<bool>,
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
            override_tiling_wm: None,
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
