use std::collections::HashMap;
use fontdue::{Font, FontSettings};
use resvg::usvg::TreeParsing;

pub struct GlyphInfo {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub width: f32,
    pub height: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

fn find_nerd_fonts_recursive(dir: &std::path::Path, fonts: &mut Vec<fontdue::Font>) {
    if fonts.len() >= 8 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_nerd_fonts_recursive(&path, fonts);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "ttf" || ext == "otf" {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        if filename.contains("Nerd") || filename.contains("NF") || filename.contains("Symbols") || filename.contains("Powerline") {
                            // Avoid adding duplicates (e.g. if we already loaded it or one with the same name)
                            if let Ok(bytes) = std::fs::read(&path) {
                                if let Ok(font) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                                    log::info!("Loaded Nerd Font from recursive search: {}", path.display());
                                    fonts.push(font);
                                    if fonts.len() >= 8 {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn load_fallback_nerd_fonts() -> Vec<fontdue::Font> {
    let mut fonts = Vec::new();
    let preferred = [
        "/usr/share/fonts/TTF/SymbolsNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Regular.ttf",
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/CascadiaMonoNF.ttf",
    ];

    for path in &preferred {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                log::info!("Loaded preferred fallback font from {}", path);
                fonts.push(font);
            }
        }
    }

    // Standard system and user font paths to scan recursively
    let mut dirs_to_scan = vec![
        std::path::PathBuf::from("/usr/share/fonts"),
        std::path::PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        let home_path = std::path::PathBuf::from(home);
        dirs_to_scan.push(home_path.join(".local/share/fonts"));
        dirs_to_scan.push(home_path.join(".fonts"));
    }

    for dir in dirs_to_scan {
        find_nerd_fonts_recursive(&dir, &mut fonts);
        if fonts.len() >= 8 {
            break;
        }
    }

    fonts
}

pub struct FontAtlas {
    pub font: Font,
    pub fallback_fonts: std::sync::Arc<std::sync::Mutex<Vec<Font>>>,
    pub texture: wgpu::Texture,
    pub sampler: wgpu::Sampler,
    pub atlas_width: u32,
    pub atlas_height: u32,
    glyphs: HashMap<(char, u32), GlyphInfo>,
    pub icons: HashMap<(String, u32), GlyphInfo>,
    current_x: u32,
    current_y: u32,
    max_row_height: u32,
    padding: u32,
    pub loaded_fallback_count: usize,
}

impl FontAtlas {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_bytes: &[u8],
    ) -> Result<Self, &'static str> {
        let font = Font::from_bytes(font_bytes, FontSettings::default())?;
        let fallback_fonts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let fallback_clone = fallback_fonts.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(2000));
            let loaded = load_fallback_nerd_fonts();
            if let Ok(mut lock) = fallback_clone.lock() {
                *lock = loaded;
            }
        });
        
        let atlas_width = 1024;
        let atlas_height = 1024;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Font Atlas Texture"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Font Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Initialize a 2x2 solid white pixel block at (0, 0) for drawing solid panels/rectangles.
        let white_pixels = [255u8, 255u8, 255u8, 255u8];
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );

        Ok(Self {
            font,
            fallback_fonts,
            texture,
            sampler,
            atlas_width,
            atlas_height,
            glyphs: HashMap::new(),
            icons: HashMap::new(),
            // Start allocating font glyphs after the 2x2 white region
            current_x: 4,
            current_y: 0,
            max_row_height: 4,
            padding: 2,
            loaded_fallback_count: 0,
        })
    }

    pub fn pre_rasterize_ascii(&mut self, queue: &wgpu::Queue, sizes: &[f32]) {
        for &size in sizes {
            for c in 32..=126 {
                let _ = self.get_or_rasterize(queue, c as u8 as char, size);
            }
        }
    }

    pub fn clear(&mut self, queue: &wgpu::Queue) {
        self.glyphs.clear();
        self.icons.clear();
        self.current_x = 4;
        self.current_y = 0;
        self.max_row_height = 4;

        // Rewrite the 2x2 solid white pixel block at (0, 0)
        let white_pixels = [255u8, 255u8, 255u8, 255u8];
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Retrieve the UV coordinate of the solid white pixel.
    pub fn white_pixel_uv(&self) -> [f32; 2] {
        // Point to the center of the first pixel at (0,0)
        [0.5 / self.atlas_width as f32, 0.5 / self.atlas_height as f32]
    }

    /// Retrieve glyph details, rasterizing and uploading it to the GPU texture if not cached.
    pub fn get_or_rasterize(&mut self, queue: &wgpu::Queue, c: char, size: f32) -> Option<&GlyphInfo> {
        let current_fb_count = if let Ok(lock) = self.fallback_fonts.lock() {
            lock.len()
        } else {
            0
        };
        if current_fb_count != self.loaded_fallback_count {
            self.clear(queue);
            self.loaded_fallback_count = current_fb_count;
        }

        let size_key = size.round() as u32;
        let key = (c, size_key);
        if self.glyphs.contains_key(&key) {
            return self.glyphs.get(&key);
        }

        let (metrics, bitmap) = {
            let mut found_fb = None;
            let fb_lock = self.fallback_fonts.lock().ok();
            if self.font.lookup_glyph_index(c) == 0 {
                if let Some(ref lock) = fb_lock {
                    for fb_font in &**lock {
                        if fb_font.lookup_glyph_index(c) != 0 {
                            found_fb = Some(fb_font);
                            break;
                        }
                    }
                }
            }
            if let Some(fb) = found_fb {
                fb.rasterize(c, size)
            } else {
                self.font.rasterize(c, size)
            }
        };
        
        // Handle empty glyphs (like spaces)
        if metrics.width == 0 || metrics.height == 0 {
            let info = GlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: metrics.advance_width,
                height: 0.0,
                bearing_x: 0.0,
                bearing_y: 0.0,
            };
            self.glyphs.insert(key, info);
            return self.glyphs.get(&key);
        }

        let w = metrics.width as u32;
        let h = metrics.height as u32;

        // Shelf packing algorithm check
        if self.current_x + w + self.padding > self.atlas_width {
            // Move to next row
            self.current_x = 4; // Skip the white pixel area on the left edge as well
            self.current_y += self.max_row_height + self.padding;
            self.max_row_height = 0;
        }

        if self.current_y + h + self.padding > self.atlas_height {
            log::warn!("Font atlas overflowed! Clearing cache.");
            self.clear(queue);
        }

        // Upload the new glyph to the GPU texture
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.current_x,
                    y: self.current_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let uv_min = [
            self.current_x as f32 / self.atlas_width as f32,
            self.current_y as f32 / self.atlas_height as f32,
        ];
        let uv_max = [
            (self.current_x + w) as f32 / self.atlas_width as f32,
            (self.current_y + h) as f32 / self.atlas_height as f32,
        ];

        let info = GlyphInfo {
            uv_min,
            uv_max,
            width: w as f32,
            height: h as f32,
            bearing_x: metrics.xmin as f32,
            bearing_y: metrics.ymin as f32,
        };

        // Advance layout pointer
        self.current_x += w + self.padding;
        self.max_row_height = self.max_row_height.max(h);

        self.glyphs.insert(key, info);
        self.glyphs.get(&key)
    }

    pub fn get_or_rasterize_icon(
        &mut self,
        queue: &wgpu::Queue,
        icon_name: &str,
        size: f32,
    ) -> Option<&GlyphInfo> {
        let size_key = size.round() as u32;
        let key = (icon_name.to_string(), size_key);
        if self.icons.contains_key(&key) {
            return self.icons.get(&key);
        }

        // Get SVG content from embedded assets
        let svg_content = match icon_name {
            "folder" => include_str!("../../assets/icons/folder.svg"),
            "folder_open" => include_str!("../../assets/icons/folder_open.svg"),
            "file" => include_str!("../../assets/icons/file.svg"),
            "rust" => include_str!("../../assets/icons/file_rust.svg"),
            "toml" => include_str!("../../assets/icons/file_toml.svg"),
            "json" => include_str!("../../assets/icons/json.svg"),
            "md" => include_str!("../../assets/icons/file_markdown.svg"),
            "python" => include_str!("../../assets/icons/file_icons/python.svg"),
            "javascript" => include_str!("../../assets/icons/file_icons/javascript.svg"),
            "typescript" => include_str!("../../assets/icons/file_icons/typescript.svg"),
            "html" => include_str!("../../assets/icons/file_icons/html.svg"),
            "css" => include_str!("../../assets/icons/file_icons/css.svg"),
            "c" => include_str!("../../assets/icons/file_icons/c.svg"),
            "cpp" => include_str!("../../assets/icons/file_icons/cpp.svg"),
            "go" => include_str!("../../assets/icons/file_icons/go.svg"),
            "binary" => include_str!("../../assets/icons/binary.svg"),
            "branch" => include_str!("../../assets/icons/git_branch.svg"),
            "info" => include_str!("../../assets/icons/info.svg"),
            "settings" => include_str!("../../assets/icons/settings.svg"),
            "search" => include_str!("../../assets/icons/magnifying_glass.svg"),
            "case_sensitive" => include_str!("../../assets/icons/case_sensitive.svg"),
            "whole_word" => include_str!("../../assets/icons/whole_word.svg"),
            "regex" => include_str!("../../assets/icons/regex.svg"),
            "replace" => include_str!("../../assets/icons/replace.svg"),
            "replace_all" => include_str!("../../assets/icons/replace_all.svg"),
            "replace_next" => include_str!("../../assets/icons/replace_next.svg"),
            "chevron_up" => include_str!("../../assets/icons/chevron_up.svg"),
            "chevron_down" => include_str!("../../assets/icons/chevron_down.svg"),
            "chevron_left" => include_str!("../../assets/icons/chevron_left.svg"),
            "chevron_right" => include_str!("../../assets/icons/chevron_right.svg"),
            "filter" => include_str!("../../assets/icons/filter.svg"),
            "circle" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><circle cx=\"8\" cy=\"8\" r=\"7.0\" fill=\"black\"/></svg>",
            "close" => include_str!("../../assets/icons/close.svg"),
            "list_collapse" => include_str!("../../assets/icons/list_collapse.svg"),
            "minimize" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path fill=\"none\" stroke=\"black\" stroke-width=\"1.5\" d=\"M3 8h10\"/></svg>",
            "maximize" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><rect width=\"10\" height=\"10\" x=\"3\" y=\"3\" rx=\"1.5\" fill=\"none\" stroke=\"black\" stroke-width=\"1.5\"/></svg>",
            "terminal" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><rect width=\"14\" height=\"11\" x=\"1\" y=\"2.5\" rx=\"2\" fill=\"none\" stroke=\"black\" stroke-width=\"1.5\"/><path fill=\"none\" stroke=\"black\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"M4.5 5.5L7 7.5l-2.5 2M8.5 9.5h3\"/></svg>",
            "plus" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path fill=\"none\" stroke=\"black\" stroke-width=\"1.5\" stroke-linecap=\"round\" d=\"M8 3v10M3 8h10\"/></svg>",
            "bug" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path fill=\"black\" d=\"M8 1.5a2.5 2.5 0 0 0-2.5 2.5V5H4.25a.75.75 0 0 0 0 1.5H5v1.25H3.75a.75.75 0 0 0 0 1.5H5V11H4.25a.75.75 0 0 0 0 1.5h1.25v.75c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5v-.75h1.25a.75.75 0 0 0 0-1.5H11V9.25h1.25a.75.75 0 0 0 0-1.5H11V6.5h.75a.75.75 0 0 0 0-1.5H11V4a2.5 2.5 0 0 0-2.5-2.5zM7 5H6.5V4a1.5 1.5 0 0 1 3 0v1H9V5H7zm-1 2.75h4V9H6V7.75zm0 2.75h4v1.5H6V10.5z\"/></svg>",
            "warning" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\" fill=\"none\"><path stroke=\"black\" stroke-linecap=\"round\" stroke-linejoin=\"round\" stroke-width=\"1.5\" d=\"M13.84 11.6 9.037 3.199a1.2 1.2 0 0 0-2.089 0l-4.802 8.403a1.2 1.2 0 0 0 1.05 1.8h9.604a1.201 1.201 0 0 0 1.038-1.8ZM8 6v2.667M8 11.333h.007\"/></svg>",
            "error" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><circle cx=\"8\" cy=\"8\" r=\"7\" fill=\"none\" stroke=\"black\" stroke-width=\"1.5\"/><path fill=\"none\" stroke=\"black\" stroke-width=\"1.5\" d=\"M5.5 5.5l5 5M10.5 5.5l-5 5\"/></svg>",
            _ => {
                log::warn!("Unknown embedded icon name: '{}'", icon_name);
                return None;
            }
        };

        // Parse and render SVG
        let opt = resvg::usvg::Options::default();
        let mut tree = resvg::usvg::Tree::from_str(svg_content, &opt)
            .map_err(|e| {
                log::warn!("Failed to parse embedded SVG icon '{}': {:?}", icon_name, e);
                e
            })
            .ok()?;
        tree.calculate_bounding_boxes();

        let w = size_key;
        let h = size_key;

        let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
        let size_obj = tree.size;
        let scale_x = w as f32 / size_obj.width() as f32;
        let scale_y = h as f32 / size_obj.height() as f32;
        let transform = resvg::tiny_skia::Transform::from_scale(scale_x, scale_y);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // Extract alpha channel
        let pixels = pixmap.data();
        let mut bitmap = Vec::with_capacity((w * h) as usize);
        for chunk in pixels.chunks_exact(4) {
            bitmap.push(chunk[3]);
        }

        // Shelf packing algorithm check
        if self.current_x + w + self.padding > self.atlas_width {
            // Move to next row
            self.current_x = 4;
            self.current_y += self.max_row_height + self.padding;
            self.max_row_height = 0;
        }

        if self.current_y + h + self.padding > self.atlas_height {
            log::warn!("Font atlas overflowed during icon rendering! Clearing cache.");
            self.clear(queue);
        }

        // Upload the new icon to the GPU texture
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.current_x,
                    y: self.current_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let uv_min = [
            self.current_x as f32 / self.atlas_width as f32,
            self.current_y as f32 / self.atlas_height as f32,
        ];
        let uv_max = [
            (self.current_x + w) as f32 / self.atlas_width as f32,
            (self.current_y + h) as f32 / self.atlas_height as f32,
        ];

        let info = GlyphInfo {
            uv_min,
            uv_max,
            width: w as f32,
            height: h as f32,
            bearing_x: 0.0,
            bearing_y: 0.0,
        };

        // Advance layout pointer
        self.current_x += w + self.padding;
        self.max_row_height = self.max_row_height.max(h);

        self.icons.insert(key, info);
        self.icons.get(&(icon_name.to_string(), size_key))
    }
}
