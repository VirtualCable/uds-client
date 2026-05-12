// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
pub struct OverlayUniforms {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub scale: f32,
    pub _pad: f32,
    pub screen: [f32; 2],
}

pub struct OverlayParams<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

pub struct OverlayRenderer {
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    cache: std::collections::HashMap<(u32, u32), (wgpu::Texture, wgpu::TextureView)>,
}

impl OverlayRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ovsh = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shaders/overlay.wgsl"
            ))),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ov_pl"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&pl),
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        OverlayRenderer {
            bgl,
            pipeline,
            sampler,
            cache: std::collections::HashMap::new(),
        }
    }

    pub fn upload_and_draw<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'a>,
        overlays: &[OverlayParams],
        surf_w: u32,
        surf_h: u32,
    ) {
        for ov in overlays {
            if ov.rgba.is_empty() || ov.width == 0 || ov.height == 0 {
                continue;
            }
            let key = (ov.width, ov.height);
            let (tex, texv) = if let Some(c) = self.cache.get(&key) {
                (&c.0, &c.1)
            } else {
                let t = device.create_texture(&wgpu::TextureDescriptor {
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
                self.cache.insert(key, (t, v));
                let e = self.cache.get(&key).unwrap();
                (&e.0, &e.1)
            };
            queue.write_texture(
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
                screen: [surf_w as f32, surf_h as f32],
            };
            let ub = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ov_ub"),
                size: std::mem::size_of::<OverlayUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&ub, 0, bytemuck::bytes_of(&u));
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ov_bg"),
                layout: &self.bgl,
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
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &bg, &[]);
            rpass.draw(0..4, 0..1);
        }
    }

    pub fn draw_single<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'a>,
        ov: &OverlayParams,
        surf_w: u32,
        surf_h: u32,
    ) {
        self.upload_and_draw(
            device,
            queue,
            rpass,
            std::slice::from_ref(ov),
            surf_w,
            surf_h,
        );
    }
}
