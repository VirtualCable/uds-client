// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

mod gdi;
mod overlay;

use std::sync::Arc;

use anyhow::Result;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
pub use wgpu_text::glyph_brush::{OwnedSection, Section, Text};
use wgpu_text::{BrushBuilder, TextBrush};

pub use overlay::{OverlayParams, OverlayRenderer};

/// Lightweight descriptor for an overlay queued for rendering.
#[derive(Debug, Clone)]
pub struct OverlayDesc {
    pub data_idx: usize,
    pub w: u32,
    pub h: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

pub struct TextRenderer {
    pub brush: TextBrush<FontRef<'static>>,
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        font_data: &'static [u8],
    ) -> Self {
        let brush = BrushBuilder::using_font_bytes_vec(vec![font_data])
            .expect("Failed to build text brush")
            .build(device, 800, 600, format);
        let _ = queue;
        TextRenderer { brush }
    }

    pub fn queue_and_draw<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'a>,
        sections: &[OwnedSection],
    ) {
        let _ = self.brush.queue(device, queue, sections);
        self.brush.draw(rpass);
    }

    pub fn resize(&mut self, w: u32, h: u32, queue: &wgpu::Queue) {
        self.brush.resize_view(w as f32, h as f32, queue);
    }
}

pub struct WgpuRenderer {
    _window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub gdi: gdi::GdiRenderer,
    pub overlay: OverlayRenderer,
    pub text: TextRenderer,
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
            alpha_mode: caps
                .alpha_modes
                .iter()
                .copied()
                .find(|m| {
                    matches!(
                        m,
                        wgpu::CompositeAlphaMode::PostMultiplied
                            | wgpu::CompositeAlphaMode::PreMultiplied
                            | wgpu::CompositeAlphaMode::Inherit
                    )
                })
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let gdi = gdi::GdiRenderer::new(&device, format);
        let overlay = OverlayRenderer::new(&device, format);
        let text = TextRenderer::new(&device, &queue, format, crate::draw::INTER_FONT_DATA);

        Ok(WgpuRenderer {
            _window: window,
            surface,
            device,
            queue,
            config,
            gdi,
            overlay,
            text,
            max_texture_size,
        })
    }

    pub fn upload_gdi(
        &mut self,
        rgba: &[u8],
        sw: u32,
        sh: u32,
        rects: Option<&[(u32, u32, u32, u32)]>,
    ) {
        self.gdi
            .upload(&self.device, &self.queue, rgba, sw, sh, rects);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_and_render(
        &mut self,
        rgba: &[u8],
        sw: u32,
        sh: u32,
        overlays: &[OverlayParams],
        sections: &[OwnedSection],
        cursor: Option<&OverlayParams>,
        rects: Option<&[(u32, u32, u32, u32)]>,
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

        let gdi_bg = self
            .gdi
            .upload(&self.device, &self.queue, rgba, sw, sh, rects);

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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if let Some(ref bg) = gdi_bg {
                self.gdi.draw(&mut rp, bg);
            }
            self.overlay
                .upload_and_draw(&self.device, &self.queue, &mut rp, overlays, pw, ph);
            self.text
                .queue_and_draw(&self.device, &self.queue, &mut rp, sections);
            if let Some(cur) = cursor {
                self.overlay
                    .draw_single(&self.device, &self.queue, &mut rp, cur, pw, ph);
            }
        }
        self.queue.submit(std::iter::once(enc.finish()));
        self._window.pre_present_notify();
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
        self.text.resize(w, h, &self.queue);
    }
}
