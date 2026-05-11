// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::sync::Arc;

use anyhow::Result;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
struct CursorUniforms {
    pos: [f32; 2],
    size: [f32; 2],
    scale: f32,
    _pad: f32,
    screen: [f32; 2],
}

/// Cursor render params: (rgba_data, width, height, draw_x, draw_y, scale_factor)
pub type CursorParams<'a> = (&'a [u8], u32, u32, f32, f32, f32);

pub struct WgpuRenderer {
    _window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    // GDI layer
    gdi_bind_group_layout: wgpu::BindGroupLayout,
    gdi_pipeline: wgpu::RenderPipeline,
    gdi_sampler: wgpu::Sampler,
    gdi_cached_texture: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,

    // Cursor layer
    cursor_bind_group_layout: wgpu::BindGroupLayout,
    cursor_pipeline: wgpu::RenderPipeline,
    cursor_sampler: wgpu::Sampler,
    cursor_uniform_buf: wgpu::Buffer,
    cursor_cached_texture: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
}

impl WgpuRenderer {
    pub fn new(window: Arc<winit::window::Window>, _width: u32, _height: u32) -> Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let static_win: &'static winit::window::Window = unsafe { &*Arc::as_ptr(&window) };
        let surface = instance.create_surface(wgpu::SurfaceTarget::from(static_win))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("UDS RDP Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            }))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // ── GDI pipeline ──────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UDS RDP Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/gdi.wgsl"
            ))),
        });

        let gdi_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gdi_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let gdi_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GDI Pipeline Layout"),
            bind_group_layouts: &[Some(&gdi_bind_group_layout)],
            immediate_size: 0,
        });

        let gdi_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GDI Pipeline"),
            layout: Some(&gdi_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let gdi_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // ── Cursor pipeline ──────────────────────
        let cursor_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cursor Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/cursor.wgsl"
            ))),
        });

        let cursor_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("cursor_bind_group_layout"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let cursor_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cursor Pipeline Layout"),
                bind_group_layouts: &[Some(&cursor_bind_group_layout)],
                immediate_size: 0,
            });

        let cursor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cursor Pipeline"),
            layout: Some(&cursor_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cursor_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &cursor_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let cursor_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let cursor_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cursor Uniforms"),
            size: std::mem::size_of::<CursorUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            _window: window,
            surface,
            device,
            queue,
            config,
            gdi_bind_group_layout,
            gdi_pipeline,
            gdi_sampler,
            gdi_cached_texture: None,
            cursor_bind_group_layout,
            cursor_pipeline,
            cursor_sampler,
            cursor_uniform_buf,
            cursor_cached_texture: None,
        })
    }

    pub fn update_and_render(
        &mut self,
        rgba_data: &[u8],
        src_width: u32,
        src_height: u32,
        cursor: Option<CursorParams>,
    ) {
        if src_width == 0 || src_height == 0 {
            return;
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            e => {
                shared::log::error!("Surface error: {e:?}");
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let surf_w = self.config.width;
        let surf_h = self.config.height;

        // ── GDI texture ───
        let tex_size = wgpu::Extent3d {
            width: src_width,
            height: src_height,
            depth_or_array_layers: 1,
        };

        let cache_valid = self
            .gdi_cached_texture
            .as_ref()
            .is_some_and(|(_, _, tw, th)| *tw == src_width && *th == src_height);

        if !cache_valid {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("GDI Texture"),
                size: tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let v = tex.create_view(&wgpu::TextureViewDescriptor::default());
            self.gdi_cached_texture = Some((tex, v, src_width, src_height));
        }

        let gdi_texture = &self.gdi_cached_texture.as_ref().unwrap().0;
        let gdi_texture_view = &self.gdi_cached_texture.as_ref().unwrap().1;

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: gdi_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(src_width * 4),
                rows_per_image: Some(src_height),
            },
            tex_size,
        );

        let gdi_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gdi_bind_group"),
            layout: &self.gdi_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(gdi_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.gdi_sampler),
                },
            ],
        });

        // ── Cursor texture ───
        let cursor_bind_group = cursor.and_then(
            |(data, cw, ch, draw_x, draw_y, cursor_scale): CursorParams| {
                if data.is_empty() || cw == 0 || ch == 0 {
                    return None;
                }
                let cursor_cache_valid = self
                    .cursor_cached_texture
                    .as_ref()
                    .is_some_and(|(_, _, tw, th)| *tw == cw && *th == ch);

                if !cursor_cache_valid {
                    let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Cursor Texture"),
                        size: wgpu::Extent3d {
                            width: cw,
                            height: ch,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    let v = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    self.cursor_cached_texture = Some((tex, v, cw, ch));
                }

                let cursor_tex = &self.cursor_cached_texture.as_ref().unwrap().0;
                let cursor_view = &self.cursor_cached_texture.as_ref().unwrap().1;

                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: cursor_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(cw * 4),
                        rows_per_image: Some(ch),
                    },
                    wgpu::Extent3d {
                        width: cw,
                        height: ch,
                        depth_or_array_layers: 1,
                    },
                );

                // Update uniform buffer
                let uniforms = CursorUniforms {
                    pos: [draw_x, draw_y],
                    size: [cw as f32, ch as f32],
                    scale: cursor_scale,
                    _pad: 0.0,
                    screen: [surf_w as f32, surf_h as f32],
                };
                self.queue
                    .write_buffer(&self.cursor_uniform_buf, 0, bytemuck::bytes_of(&uniforms));

                let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("cursor_bind_group"),
                    layout: &self.cursor_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(
                                self.cursor_uniform_buf.as_entire_buffer_binding(),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(cursor_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.cursor_sampler),
                        },
                    ],
                });
                Some(bg)
            },
        );

        // ── Render ───
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.gdi_pipeline);
            render_pass.set_bind_group(0, &gdi_bind_group, &[]);
            render_pass.draw(0..4, 0..1);

            if let Some(ref cursor_bg) = cursor_bind_group {
                render_pass.set_pipeline(&self.cursor_pipeline);
                render_pass.set_bind_group(0, cursor_bg, &[]);
                render_pass.draw(0..4, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    pub fn reconfigure(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}
