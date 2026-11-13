// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Software-rendered remote cursor for URP sessions (protocol §10): the
//! cursor never travels inside the video; the server sends its shape and we
//! paint it locally at the pointer position. Classic Windows masked-color
//! cursors (the I-beam) XOR-invert the background, which no OS cursor API
//! can express — hence the two-pass GPU sprite: normal pixels alpha-blend,
//! XOR pixels invert the background (ported from uds-rp's labclient).

/// What the cursor pass needs each frame: the two sprite textures (RGBA with
/// XOR pixels' alpha already zeroed, and the 1bpp XOR mask expanded to R8)
/// and the destination rect in surface pixels (origin top-left).
pub struct CursorDraw<'a> {
    /// RGBA sprite (XOR pixels carry alpha 0).
    pub texture: &'a wgpu::TextureView,
    /// R8 mask: 255 where the pixel XORs the background.
    pub mask: &'a wgpu::TextureView,
    /// Left edge, pixels.
    pub x: f32,
    /// Top edge, pixels.
    pub y: f32,
    /// Width, pixels.
    pub w: f32,
    /// Height, pixels.
    pub h: f32,
}

struct SpriteTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// One cursor shape uploaded to the GPU: the RGBA sprite with the XOR
/// pixels' alpha zeroed, plus the R8 mask marking them.
pub struct CursorSprite {
    tex: SpriteTexture,
    mask: SpriteTexture,
    pub width: u16,
    pub height: u16,
    pub hot_x: u16,
    pub hot_y: u16,
}

impl CursorSprite {
    /// Builds the two sprite textures from a wire cursor shape (MSB-first,
    /// row-major 1bpp XOR mask; empty mask = no XOR pixels).
    #[allow(
        clippy::too_many_arguments,
        reason = "parameters mirror the wire cursor shape verbatim"
    )]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u16,
        height: u16,
        hot_x: u16,
        hot_y: u16,
        rgba: &[u8],
        xor_mask: &[u8],
    ) -> Self {
        let mut pixels = rgba.to_vec();
        let mut mask = vec![0u8; usize::from(width) * usize::from(height)];
        for (i, m) in mask.iter_mut().enumerate() {
            let xor = xor_mask
                .get(i / 8)
                .is_some_and(|b| b & (0x80 >> (i % 8)) != 0);
            if xor {
                *m = 255;
                pixels[i * 4 + 3] = 0; // the invert pass owns this pixel
            }
        }
        let upload = |data: &[u8], bpp: u32, format: wgpu::TextureFormat, label: &str| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: u32::from(width).max(1),
                    height: u32::from(height).max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(u32::from(width) * bpp),
                    rows_per_image: Some(u32::from(height)),
                },
                wgpu::Extent3d {
                    width: u32::from(width).max(1),
                    height: u32::from(height).max(1),
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            SpriteTexture {
                _texture: texture,
                view,
            }
        };
        // Same sRGB convention as the display texture: sampled values decode
        // to linear and the surface write-back re-encodes, so blending matches.
        let tex = upload(&pixels, 4, wgpu::TextureFormat::Rgba8UnormSrgb, "cursor");
        let mask = upload(&mask, 1, wgpu::TextureFormat::R8Unorm, "cursor-mask");
        Self {
            tex,
            mask,
            width,
            height,
            hot_x,
            hot_y,
        }
    }

    /// Positions the sprite at the local pointer (surface pixels), honoring
    /// the remote hotspot and the display-to-window scale of `dest`.
    pub fn draw_at(
        &self,
        pointer: (f64, f64),
        dest: (f32, f32, f32, f32),
        disp: (u32, u32),
    ) -> Option<CursorDraw<'_>> {
        if disp.0 == 0 || disp.1 == 0 {
            return None;
        }
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let (sx, sy) = (dest.2 / disp.0 as f32, dest.3 / disp.1 as f32);
        Some(CursorDraw {
            texture: &self.tex.view,
            mask: &self.mask.view,
            x: pointer.0 as f32 - f32::from(self.hot_x) * sx,
            y: pointer.1 as f32 - f32::from(self.hot_y) * sy,
            w: f32::from(self.width) * sx,
            h: f32::from(self.height) * sy,
        })
    }
}

const CURSOR_SHADER: &str = r"
struct Uniforms {
    rect: vec4<f32>,   // x, y, w, h in pixels (origin top-left)
    screen: vec4<f32>, // w, h in pixels, unused, unused
    mode: vec4<f32>,   // x: 0 = alpha pass, 1 = invert (XOR) pass
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var xor_mask: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Two triangles covering the rect.
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    var out: VsOut;
    let px = u.rect.xy + pos[vi] * u.rect.zw;
    out.pos = vec4<f32>(
        px.x / u.screen.x * 2.0 - 1.0,
        1.0 - px.y / u.screen.y * 2.0,
        0.0,
        1.0,
    );
    out.uv = pos[vi];
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    let m = textureSample(xor_mask, samp, in.uv).r;
    if u.mode.x > 0.5 {
        // Invert pass: only where the mask says XOR; the blend state turns
        // src - dst with src = 1 into 1 - dst (inversion). White is the only
        // XOR color classic cursors use.
        if m < 0.5 {
            discard;
        }
        return vec4<f32>(c.rgb * m, 1.0);
    }
    // Alpha pass: draw everything EXCEPT the XOR pixels (their alpha was
    // already zeroed at upload; the mask multiply is belt and braces for
    // linear-filter bleeding at the edges).
    return vec4<f32>(c.rgb, c.a * (1.0 - m));
}
";

/// Two-pass cursor pipeline (alpha + invert), one per [`super::WgpuRenderer`].
pub struct CursorRenderer {
    alpha: wgpu::RenderPipeline,
    invert: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl CursorRenderer {
    #[allow(
        clippy::too_many_lines,
        reason = "two near-identical pipeline builds; splitting adds indirection"
    )]
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cursor"),
            source: wgpu::ShaderSource::Wgsl(CURSOR_SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cursor"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cursor"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |label: &str, blend: Option<wgpu::BlendState>, write_mask: wgpu::ColorWrites| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let alpha = make(
            "cursor-alpha",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            wgpu::ColorWrites::ALL,
        );
        // Invert = src - dst with src = 1 → 1 - dst (true inversion). wgpu's
        // Subtract is src*sf - dst*df; ReverseSubtract would be dst - src,
        // which with src=1 clamps to black instead of inverting.
        // Alpha untouched (the surface alpha is meaningless on screen).
        let invert = make(
            "cursor-invert",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Subtract,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
            wgpu::ColorWrites::RED
                .union(wgpu::ColorWrites::GREEN)
                .union(wgpu::ColorWrites::BLUE),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cursor"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            alpha,
            invert,
            layout,
            sampler,
        }
    }

    pub fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'_>,
        cursor: &CursorDraw<'_>,
        screen_w: u32,
        screen_h: u32,
    ) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Uniforms {
            rect: [f32; 4],
            screen: [f32; 4],
            mode: [f32; 4],
        }
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let base = Uniforms {
            rect: [cursor.x, cursor.y, cursor.w, cursor.h],
            screen: [screen_w as f32, screen_h as f32, 0.0, 0.0],
            mode: [0.0; 4],
        };
        let bind = |device: &wgpu::Device, buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cursor"),
                layout: &self.layout,
                entries: [
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(cursor.texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(cursor.mask),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ]
                .as_slice(),
            })
        };
        // Same texture pair, two draws: alpha first, invert on top.
        for (pipeline, mode) in [(&self.alpha, 0.0f32), (&self.invert, 1.0f32)] {
            let mut uniforms = base;
            uniforms.mode[0] = mode;
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cursor"),
                size: u64::try_from(std::mem::size_of::<Uniforms>()).unwrap_or(64),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&uniforms));
            let bind = bind(device, &buffer);
            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &bind, &[]);
            rpass.draw(0..6, 0..1);
        }
    }
}
