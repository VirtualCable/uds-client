// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
pub use wgpu_text::glyph_brush::{OwnedSection, Section, Text};
use wgpu_text::{BrushBuilder, TextBrush};

// ── Overlay (cursor image) ──────────────────────────────────
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
struct OverlayUniforms {
    pos: [f32; 2],
    size: [f32; 2],
    scale: f32,
    _pad: f32,
    screen: [f32; 2],
}
pub struct OverlayParams<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

pub struct WgpuRenderer {
    pub(crate) _window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    gdi_bgl: wgpu::BindGroupLayout,
    gdi_pipeline: wgpu::RenderPipeline,
    gdi_sampler: wgpu::Sampler,
    gdi_cached: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    overlay_bgl: wgpu::BindGroupLayout,
    overlay_pipeline: wgpu::RenderPipeline,
    overlay_sampler: wgpu::Sampler,
    overlay_cache: HashMap<(u32, u32), (wgpu::Texture, wgpu::TextureView)>,
    text_brush: TextBrush<FontRef<'static>>,
    max_texture_size: u32,
}

impl WgpuRenderer {
    pub fn new(window: Arc<winit::window::Window>, _w: u32, _h: u32) -> Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let sw: &'static winit::window::Window = unsafe { &*Arc::as_ptr(&window) };
        let surface = instance.create_surface(wgpu::SurfaceTarget::from(sw))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;
        let max_texture_size = adapter.limits().max_texture_dimension_2d;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("UDS"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_texture_dimension_2d: max_texture_size,
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            }))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gdi"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/gdi.wgsl"
            ))),
        });
        let gdi_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gdi_bgl"),
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
        let gdi_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gdi_pl"),
            bind_group_layouts: &[Some(&gdi_bgl)],
            immediate_size: 0,
        });
        let gdi_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gdi"),
            layout: Some(&gdi_pl),
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
                    format,
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
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let ovsh = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ov"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/overlay.wgsl"
            ))),
        });
        let overlay_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ov_bgl"),
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
        let ov_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ov_pl"),
            bind_group_layouts: &[Some(&overlay_bgl)],
            immediate_size: 0,
        });
        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ov"),
            layout: Some(&ov_pl),
            vertex: wgpu::VertexState {
                module: &ovsh,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ovsh,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
        let overlay_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let text_brush = BrushBuilder::using_font_bytes_vec(vec![crate::draw::INTER_FONT_DATA])
            .map_err(|e| anyhow::anyhow!("BrushBuilder error: {e:?}"))?
            .build(&device, size.width, size.height, format);

        Ok(Self {
            _window: window,
            surface,
            device,
            queue,
            config,
            gdi_bgl,
            gdi_pipeline,
            gdi_sampler,
            gdi_cached: None,
            overlay_bgl,
            overlay_pipeline,
            overlay_sampler,
            overlay_cache: HashMap::new(),
            text_brush,
            max_texture_size,
        })
    }

    pub fn update_and_render(
        &mut self,
        rgba: &[u8],
        sw: u32,
        sh: u32,
        overlays: &[OverlayParams],
        sections: &[OwnedSection],
        cursor: Option<&OverlayParams>,
    ) {
        let sw = sw.min(self.max_texture_size);
        let sh = sh.min(self.max_texture_size);
        if sw == 0 || sh == 0 {
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
        let pw = self.config.width;
        let ph = self.config.height;

        // GDI — only when rgba data is provided
        let gdi_bg = if !rgba.is_empty() {
            let ts = wgpu::Extent3d {
                width: sw,
                height: sh,
                depth_or_array_layers: 1,
            };
            if !self
                .gdi_cached
                .as_ref()
                .is_some_and(|(_, _, tw, th)| *tw == sw && *th == sh)
            {
                let t = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("gdi_tex"),
                    size: ts,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let v = t.create_view(&wgpu::TextureViewDescriptor::default());
                self.gdi_cached = Some((t, v, sw, sh));
            }
            let (gtex, gview, _, _) = self.gdi_cached.as_ref().unwrap();
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: gtex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(sw * 4),
                    rows_per_image: Some(sh),
                },
                ts,
            );
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gdi_bg"),
                layout: &self.gdi_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(gview),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.gdi_sampler),
                    },
                ],
            });
            Some(bg)
        } else {
            None
        };

        // Overlays (cursor)
        let mut ov_bgs: Vec<wgpu::BindGroup> = Vec::new();
        for ov in overlays {
            if ov.rgba.is_empty() || ov.width == 0 || ov.height == 0 {
                continue;
            }
            let key = (ov.width, ov.height);
            let (tex, texv) = if let Some(c) = self.overlay_cache.get(&key) {
                (&c.0, &c.1)
            } else {
                let t = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("ov_tex"),
                    size: wgpu::Extent3d {
                        width: ov.width,
                        height: ov.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let v = t.create_view(&wgpu::TextureViewDescriptor::default());
                self.overlay_cache.insert(key, (t, v));
                let e = self.overlay_cache.get(&key).unwrap();
                (&e.0, &e.1)
            };
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                ov.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ov.width * 4),
                    rows_per_image: Some(ov.height),
                },
                wgpu::Extent3d {
                    width: ov.width,
                    height: ov.height,
                    depth_or_array_layers: 1,
                },
            );
            let u = OverlayUniforms {
                pos: [ov.x, ov.y],
                size: [ov.width as f32, ov.height as f32],
                scale: ov.scale,
                _pad: 0.0,
                screen: [pw as f32, ph as f32],
            };
            let ub = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ov_ub"),
                size: std::mem::size_of::<OverlayUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&ub, 0, bytemuck::bytes_of(&u));
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ov_bg"),
                layout: &self.overlay_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(ub.as_entire_buffer_binding()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(texv),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.overlay_sampler),
                    },
                ],
            });
            ov_bgs.push(bg);
        }

        // Queue text sections
        let _ = self.text_brush.queue(&self.device, &self.queue, sections);

        // Render
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rp"),
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
            if let Some(ref gdi_bg) = gdi_bg {
                rp.set_pipeline(&self.gdi_pipeline);
                rp.set_bind_group(0, gdi_bg, &[]);
                rp.draw(0..4, 0..1);
            }
            rp.set_pipeline(&self.overlay_pipeline);
            for bg in &ov_bgs {
                rp.set_bind_group(0, bg, &[]);
                rp.draw(0..4, 0..1);
            }
            self.text_brush.draw(&mut rp);

            // Cursor on top
            if let Some(cur) = cursor
                && !cur.rgba.is_empty()
                && cur.width > 0
                && cur.height > 0
            {
                let key = (cur.width, cur.height);
                let (tex, texv) = if let Some(c) = self.overlay_cache.get(&key) {
                    (&c.0, &c.1)
                } else {
                    let t = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("cur_tex"),
                        size: wgpu::Extent3d {
                            width: cur.width,
                            height: cur.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    let v = t.create_view(&wgpu::TextureViewDescriptor::default());
                    self.overlay_cache.insert(key, (t, v));
                    let e = self.overlay_cache.get(&key).unwrap();
                    (&e.0, &e.1)
                };
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    cur.rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(cur.width * 4),
                        rows_per_image: Some(cur.height),
                    },
                    wgpu::Extent3d {
                        width: cur.width,
                        height: cur.height,
                        depth_or_array_layers: 1,
                    },
                );
                let u = OverlayUniforms {
                    pos: [cur.x, cur.y],
                    size: [cur.width as f32, cur.height as f32],
                    scale: cur.scale,
                    _pad: 0.0,
                    screen: [pw as f32, ph as f32],
                };
                let ub = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("cur_ub"),
                    size: std::mem::size_of::<OverlayUniforms>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue.write_buffer(&ub, 0, bytemuck::bytes_of(&u));
                let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("cur_bg"),
                    layout: &self.overlay_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(ub.as_entire_buffer_binding()),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(texv),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.overlay_sampler),
                        },
                    ],
                });
                rp.set_pipeline(&self.overlay_pipeline);
                rp.set_bind_group(0, &bg, &[]);
                rp.draw(0..4, 0..1);
            }
        }
        self.queue.submit(std::iter::once(enc.finish()));
        output.present();
    }

    pub fn reconfigure(&mut self, w: u32, h: u32) {
        let w = w.min(self.max_texture_size);
        let h = h.min(self.max_texture_size);
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.text_brush.resize_view(w as f32, h as f32, &self.queue);
    }
}
