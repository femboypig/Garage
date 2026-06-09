use std::sync::Arc;
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 2]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    screen_size: [f32; 2],
}

pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,
    
    pub backend: wgpu::Backend,
    pub device_name: String,
    globals_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_capacity: usize,
    index_buffer: Option<wgpu::Buffer>,
    index_buffer_capacity: usize,
}

impl GpuContext {
    pub async fn new(window: Arc<Window>, forced_backend: Option<wgpu::Backends>) -> Self {
        let size = window.inner_size();

        // Helper to try creating surface, adapter, device, queue for a given instance and backend
        async fn try_create(
            window: &Arc<Window>,
            backends: wgpu::Backends,
            flags: wgpu::InstanceFlags,
        ) -> Option<(wgpu::Surface<'static>, wgpu::Device, wgpu::Queue, wgpu::Adapter)> {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                flags,
                ..Default::default()
            });

            let surface = match instance.create_surface(window.clone()) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("try_create (backend={:?}): create_surface failed: {:?}", backends, e);
                    return None;
                }
            };

            let mut adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }).await;

            if adapter.is_none() {
                log::warn!("try_create (backend={:?}): request_adapter with compatible_surface returned None, retrying without compatible_surface...", backends);
                adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                }).await;
            }

            let adapter = match adapter {
                Some(a) => a,
                None => {
                    log::warn!("try_create (backend={:?}): request_adapter returned None", backends);
                    return None;
                }
            };

            let required_limits = if backends == wgpu::Backends::GL {
                wgpu::Limits::downlevel_webgl2_defaults()
            } else {
                wgpu::Limits::default()
            };

            log::warn!("try_create (backend={:?}): requesting device with limits...", backends);
            let device_result = adapter.request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            ).await;

            let (device, queue) = match device_result {
                Ok(res) => res,
                Err(e) => {
                    log::warn!("try_create (backend={:?}): request_device failed: {:?}", backends, e);
                    return None;
                }
            };

            Some((surface, device, queue, adapter))
        }

        fn restore_env(
            orig_wgpu: Option<String>,
            orig_libgl: Option<String>,
            orig_gles_override: Option<String>,
            orig_gl_override: Option<String>,
            orig_driver_override: Option<String>,
        ) {
            unsafe {
                if let Some(val) = orig_wgpu {
                    std::env::set_var("WGPU_GL_BACKEND", val);
                } else {
                    std::env::remove_var("WGPU_GL_BACKEND");
                }
                if let Some(val) = orig_libgl {
                    std::env::set_var("LIBGL_ALWAYS_SOFTWARE", val);
                } else {
                    std::env::remove_var("LIBGL_ALWAYS_SOFTWARE");
                }
                if let Some(val) = orig_gles_override {
                    std::env::set_var("MESA_GLES_VERSION_OVERRIDE", val);
                } else {
                    std::env::remove_var("MESA_GLES_VERSION_OVERRIDE");
                }
                if let Some(val) = orig_gl_override {
                    std::env::set_var("MESA_GL_VERSION_OVERRIDE", val);
                } else {
                    std::env::remove_var("MESA_GL_VERSION_OVERRIDE");
                }
                if let Some(val) = orig_driver_override {
                    std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", val);
                } else {
                    std::env::remove_var("MESA_LOADER_DRIVER_OVERRIDE");
                }
            }
        }

        // Helper to try GL context creation with various fallbacks
        async fn try_create_gl(
            window: &Arc<Window>,
            flags: wgpu::InstanceFlags,
        ) -> Option<(wgpu::Surface<'static>, wgpu::Device, wgpu::Queue, wgpu::Adapter)> {
            // Save current environment variables
            let orig_wgpu_backend = std::env::var("WGPU_GL_BACKEND").ok();
            let orig_libgl_software = std::env::var("LIBGL_ALWAYS_SOFTWARE").ok();
            let orig_gles_override = std::env::var("MESA_GLES_VERSION_OVERRIDE").ok();
            let orig_gl_override = std::env::var("MESA_GL_VERSION_OVERRIDE").ok();
            let orig_driver_override = std::env::var("MESA_LOADER_DRIVER_OVERRIDE").ok();

            // Set Mesa version overrides to force GLES 3.0 and GL 3.3 compatibility
            unsafe {
                std::env::set_var("MESA_GLES_VERSION_OVERRIDE", "3.0");
                std::env::set_var("MESA_GL_VERSION_OVERRIDE", "3.3");
            }

            // Try different Intel Mesa drivers sequentially to see if context creation succeeds
            let drivers = [Some("crocus"), Some("i965"), None];

            for driver in drivers {
                if let Some(d) = driver {
                    unsafe { std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", d); }
                } else {
                    unsafe { std::env::remove_var("MESA_LOADER_DRIVER_OVERRIDE"); }
                }

                // 1. Try default (EGL hardware)
                if let Some(res) = try_create(window, wgpu::Backends::GL, flags).await {
                    restore_env(orig_wgpu_backend, orig_libgl_software, orig_gles_override, orig_gl_override, orig_driver_override);
                    return Some(res);
                }

                // 2. Try GLX hardware
                log::warn!("OpenGL with default EGL failed (driver={:?}). Retrying OpenGL with GLX backend...", driver);
                unsafe { std::env::set_var("WGPU_GL_BACKEND", "glx"); }
                if let Some(res) = try_create(window, wgpu::Backends::GL, flags).await {
                    restore_env(orig_wgpu_backend, orig_libgl_software, orig_gles_override, orig_gl_override, orig_driver_override);
                    return Some(res);
                }
                
                // Reset WGPU_GL_BACKEND to default for the next driver iteration
                if let Some(ref val) = orig_wgpu_backend {
                    unsafe { std::env::set_var("WGPU_GL_BACKEND", val); }
                } else {
                    unsafe { std::env::remove_var("WGPU_GL_BACKEND"); }
                }
            }

            // Fallback to software rendering (no driver overrides)
            unsafe {
                std::env::remove_var("MESA_LOADER_DRIVER_OVERRIDE");
            }

            // 3. Try EGL software
            log::warn!("OpenGL with hardware GLX failed. Retrying OpenGL with EGL software rendering (llvmpipe)...");
            unsafe {
                std::env::set_var("WGPU_GL_BACKEND", "egl");
                std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            }
            if let Some(res) = try_create(window, wgpu::Backends::GL, flags).await {
                restore_env(orig_wgpu_backend, orig_libgl_software, orig_gles_override, orig_gl_override, orig_driver_override);
                return Some(res);
            }

            // 4. Try GLX software
            log::warn!("OpenGL with software EGL failed. Retrying OpenGL with GLX software rendering (llvmpipe)...");
            unsafe {
                std::env::set_var("WGPU_GL_BACKEND", "glx");
                std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            }
            if let Some(res) = try_create(window, wgpu::Backends::GL, flags).await {
                restore_env(orig_wgpu_backend, orig_libgl_software, orig_gles_override, orig_gl_override, orig_driver_override);
                return Some(res);
            }

            // Clean up and restore env on failure
            restore_env(orig_wgpu_backend, orig_libgl_software, orig_gles_override, orig_gl_override, orig_driver_override);
            None
        }

        let mut creation_result = None;

        if let Some(backend) = forced_backend {
            if backend == wgpu::Backends::GL {
                let flags = wgpu::InstanceFlags::default() & !wgpu::InstanceFlags::VALIDATION & !wgpu::InstanceFlags::DEBUG;
                creation_result = try_create_gl(&window, flags).await;
            } else {
                let flags = if backend == wgpu::Backends::VULKAN {
                    wgpu::InstanceFlags::default() | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER
                } else {
                    wgpu::InstanceFlags::default()
                };
                creation_result = try_create(&window, backend, flags).await;
            }
        }

        if creation_result.is_none() {
            // Try Vulkan first (including non-compliant adapters for older GPUs/drivers)
            creation_result = try_create(
                &window,
                wgpu::Backends::VULKAN,
                wgpu::InstanceFlags::default() | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER,
            ).await;
        }

        if creation_result.is_none() {
            log::warn!("Failed to initialize Vulkan. Falling back to OpenGL/GL backend...");
            // Try GL/GLES next
            let flags = wgpu::InstanceFlags::default() & !wgpu::InstanceFlags::VALIDATION & !wgpu::InstanceFlags::DEBUG;
            creation_result = try_create_gl(&window, flags).await;
        }

        if creation_result.is_none() {
            log::warn!("Failed to initialize GL. Trying any available backend...");
            // Try any backend with fallback allowed
            creation_result = try_create(
                &window,
                wgpu::Backends::all(),
                wgpu::InstanceFlags::default() | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER,
            ).await;
        }

        let (surface, device, queue, adapter) = creation_result.expect(
            "Failed to find an appropriate adapter using Vulkan, GL, or any other backend."
        );

        let adapter_info = adapter.get_info();
        log::warn!("Selected Graphics Backend: {:?}, Device: {}", adapter_info.backend, adapter_info.name);

        let surface_caps = surface.get_capabilities(&adapter);
        // Find an sRGB surface format
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // Cap at monitor refresh rate (VSync)
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Globals uniform buffer (screen size)
        let globals = Globals {
            screen_size: [size.width as f32, size.height as f32],
        };
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Globals Buffer"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&globals_buffer, 0, bytemuck::bytes_of(&globals));

        // Create Bind Group Layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Main Bind Group Layout"),
            entries: &[
                // Globals Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Texture Atlas
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create Dummy texture and sampler for initial bind group configuration
        let dummy_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // Create Bind Group
        let atlas_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Main Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&dummy_sampler),
                },
            ],
        });


        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            backend: adapter_info.backend,
            device_name: adapter_info.name,
            globals_buffer,
            bind_group_layout,
            bind_group,
            vertex_buffer: None,
            vertex_buffer_capacity: 0,
            index_buffer: None,
            index_buffer_capacity: 0,
        }
    }

    /// Update the Bind Group with a new texture view (useful if texture atlas changes).
    pub fn update_bind_group(&mut self, atlas_texture: &wgpu::Texture, atlas_sampler: &wgpu::Sampler) {
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Main Bind Group (Updated)"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        });
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Update Globals uniform buffer
            let globals = Globals {
                screen_size: [new_size.width as f32, new_size.height as f32],
            };
            self.queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        }
    }

    /// Upload and render the given vertices and indices.
    pub fn render(&mut self, vertices: &[Vertex], indices: &[u16]) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Update vertex buffer, resizing if necessary
        if !vertices.is_empty() {
            if self.vertex_buffer.is_none() || vertices.len() > self.vertex_buffer_capacity {
                self.vertex_buffer_capacity = vertices.len().next_power_of_two();
                self.vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Vertex Buffer"),
                    size: (self.vertex_buffer_capacity * std::mem::size_of::<Vertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            self.queue.write_buffer(self.vertex_buffer.as_ref().unwrap(), 0, bytemuck::cast_slice(vertices));
        }

        // Update index buffer, resizing if necessary
        if !indices.is_empty() {
            if self.index_buffer.is_none() || indices.len() > self.index_buffer_capacity {
                self.index_buffer_capacity = indices.len().next_power_of_two();
                self.index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Index Buffer"),
                    size: (self.index_buffer_capacity * std::mem::size_of::<u16>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            self.queue.write_buffer(self.index_buffer.as_ref().unwrap(), 0, bytemuck::cast_slice(indices));
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05, // Dark gray background (sleek tech startup vibe)
                            g: 0.05,
                            b: 0.07,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if !vertices.is_empty() && !indices.is_empty() {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.as_ref().unwrap().slice(..));
                render_pass.set_index_buffer(self.index_buffer.as_ref().unwrap().slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
