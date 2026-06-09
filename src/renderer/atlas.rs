use std::collections::HashMap;
use fontdue::{Font, FontSettings};

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
    pub font_size: f32,
    pub texture: wgpu::Texture,
    pub sampler: wgpu::Sampler,
    pub atlas_width: u32,
    pub atlas_height: u32,
    glyphs: HashMap<char, GlyphInfo>,
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
        font_size: f32,
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
            font_size,
            texture,
            sampler,
            atlas_width,
            atlas_height,
            glyphs: HashMap::new(),
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
    pub fn get_or_rasterize(&mut self, queue: &wgpu::Queue, c: char) -> Option<&GlyphInfo> {
        if self.glyphs.contains_key(&c) {
            return self.glyphs.get(&c);
        }

        let (metrics, bitmap) = self.font.rasterize(c, self.font_size);
        
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
            self.glyphs.insert(c, info);
            return self.glyphs.get(&c);
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
            bearing_x: metrics.bounds.xmin,
            bearing_y: metrics.bounds.ymin,
        };

        // Advance layout pointer
        self.current_x += w + self.padding;
        self.max_row_height = self.max_row_height.max(h);

        self.glyphs.insert(c, info);
        self.glyphs.get(&c)
    }
}
