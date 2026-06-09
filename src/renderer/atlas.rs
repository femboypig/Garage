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

pub struct FontAtlas {
    pub font: Font,
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
}

impl FontAtlas {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_bytes: &[u8],
    ) -> Result<Self, &'static str> {
        let font = Font::from_bytes(font_bytes, FontSettings::default())?;
        
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
        })
    }

    /// Retrieve the UV coordinate of the solid white pixel.
    pub fn white_pixel_uv(&self) -> [f32; 2] {
        // Point to the center of the first pixel at (0,0)
        [0.5 / self.atlas_width as f32, 0.5 / self.atlas_height as f32]
    }

    /// Retrieve glyph details, rasterizing and uploading it to the GPU texture if not cached.
    pub fn get_or_rasterize(&mut self, queue: &wgpu::Queue, c: char, size: f32) -> Option<&GlyphInfo> {
        let size_key = size.round() as u32;
        let key = (c, size_key);
        if self.glyphs.contains_key(&key) {
            return self.glyphs.get(&key);
        }

        let (metrics, bitmap) = self.font.rasterize(c, size);
        
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
            log::warn!("Font atlas is completely full! Characters might render incorrectly.");
            return None;
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
            "rust" => include_str!("../../assets/icons/file_icons/rust.svg"),
            "toml" => include_str!("../../assets/icons/file_icons/toml.svg"),
            "json" => include_str!("../../assets/icons/json.svg"),
            "md" => include_str!("../../assets/icons/file_markdown.svg"),
            "branch" => include_str!("../../assets/icons/git_branch.svg"),
            "info" => include_str!("../../assets/icons/info.svg"),
            "settings" => include_str!("../../assets/icons/settings.svg"),
            "search" => include_str!("../../assets/icons/magnifying_glass.svg"),
            "circle" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><circle cx=\"8\" cy=\"8\" r=\"7.0\" fill=\"currentColor\"/></svg>",
            "close" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path fill=\"currentColor\" d=\"M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06z\"/></svg>",
            "minimize" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\" d=\"M3 8h10\"/></svg>",
            "maximize" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><rect width=\"10\" height=\"10\" x=\"3\" y=\"3\" rx=\"1.5\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\"/></svg>",
            "terminal" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><rect width=\"14\" height=\"12\" x=\"1\" y=\"2\" rx=\"1.5\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\"/><path fill=\"currentColor\" d=\"M4 5.5l2.5 2-2.5 2M8 9.5h4\"/></svg>",
            "bug" => "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path fill=\"currentColor\" d=\"M8 1.5a2.5 2.5 0 0 0-2.5 2.5V5H4.25a.75.75 0 0 0 0 1.5H5v1.25H3.75a.75.75 0 0 0 0 1.5H5V11H4.25a.75.75 0 0 0 0 1.5h1.25v.75c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5v-.75h1.25a.75.75 0 0 0 0-1.5H11V9.25h1.25a.75.75 0 0 0 0-1.5H11V6.5h.75a.75.75 0 0 0 0-1.5H11V4a2.5 2.5 0 0 0-2.5-2.5zM7 5H6.5V4a1.5 1.5 0 0 1 3 0v1H9V5H7zm-1 2.75h4V9H6V7.75zm0 2.75h4v1.5H6V10.5z\"/></svg>",
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
            log::warn!("Font atlas is completely full! Icons might render incorrectly.");
            return None;
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
