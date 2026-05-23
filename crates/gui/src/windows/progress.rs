// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use std::sync::Arc;
use std::time::Instant;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;

use crate::draw::ui::{
    progress,
    waves::{self, Wave},
};
use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};

#[derive(Default, PartialEq, Debug)]
pub enum ProgressPhase {
    #[default]
    Connecting,
    Connected,
}

pub struct ProgressState {
    pub window: Arc<winit::window::Window>,
    pub renderer: WgpuRenderer,
    pub pct: u8,
    pub message: String,
    pub start: Instant,
    pub progress_duration_secs: u32,
    pub phase: ProgressPhase,
    pub auto_animate: bool,
    pub cancel_btn: crate::draw::ui::button::Button,
    pub cancelled: bool,
    pub animation_time: f32,
    pub waves: Vec<Wave>,
    pub last_mouse_pos: Option<(f32, f32)>,
    pub connecting_text: String,
    pub connected_text: String,
}

impl ProgressState {
    pub fn new(
        el: &winit::event_loop::ActiveEventLoop,
        title: String,
        cancel_text: String,
        connecting_text: String,
        connected_text: String,
    ) -> anyhow::Result<Self> {
        let (dw, dh) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let ww = 400.0;
        let wh = 300.0;
        let sf = crate::monitor::scale(0) as f32;
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            el.create_window(
                winit::window::Window::default_attributes()
                    .with_title(title)
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_window_icon(Some(crate::logo::load_icon()))
                    .with_resizable(false)
                    .with_decorations(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
            )?,
        );

        let phys = window.inner_size();
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;

        let pw = phys.width as f32;
        let ph = phys.height as f32;
        let s = *crate::monitor::SCALE_FACTOR as f32;
        let bw = crate::monitor::scaled_val(120) as f32;
        let bh = crate::monitor::scaled_val(32) as f32;
        let bx = (pw - bw) / 2.0;
        let by = ph - bh - 25.0 * s;

        let cancel_btn = crate::draw::ui::button::Button::new(
            bx,
            by,
            bw as u32,
            bh as u32,
            cancel_text,
            crate::draw::ui::button::ButtonStyle {
                font_scale: crate::monitor::scaled_val(13) as f32,
                bg_color: [60, 35, 35, 255],
                border_color: [100, 60, 60, 255],
                hover_bg_color: [80, 45, 45, 255],
                hover_border_color: [150, 80, 80, 255],
                ..Default::default()
            },
        );

        Ok(Self {
            window,
            renderer,
            pct: 0,
            message: String::new(),
            start: Instant::now(),
            progress_duration_secs: 30,
            phase: ProgressPhase::Connecting,
            auto_animate: false,
            cancel_btn,
            cancelled: false,
            animation_time: 0.0,
            waves: vec![
                Wave {
                    y_base: 0.4,
                    amplitude: 25.0,
                    speed: 0.04,
                    offset: 0.0,
                    thickness: 5.0,
                    opacity: 0.5,
                },
                Wave {
                    y_base: 0.42,
                    amplitude: 20.0,
                    speed: 0.06,
                    offset: 2.0,
                    thickness: 3.5,
                    opacity: 0.3,
                },
                Wave {
                    y_base: 0.38,
                    amplitude: 22.0,
                    speed: 0.03,
                    offset: 4.0,
                    thickness: 6.0,
                    opacity: 0.25,
                },
            ],
            last_mouse_pos: None,
            connecting_text,
            connected_text,
        })
    }

    pub fn handle_mouse_move(&mut self, phys_x: f32, phys_y: f32) -> bool {
        self.cancel_btn.handle_mouse_move(phys_x, phys_y)
    }

    pub fn handle_click(&mut self, phys_x: f32, phys_y: f32) {
        if self.cancel_btn.contains(phys_x, phys_y) {
            self.cancelled = true;
        }
    }

    pub fn paint(&mut self) {
        let logo = crate::logo::load_logo();
        let phys = self.window.inner_size();
        let pw = phys.width;
        let ph = phys.height;
        let s = *monitor::SCALE_FACTOR as f32;

        self.renderer.reconfigure(pw, ph);

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();

        let logo_idx = data.len();
        data.push(logo.rgba);

        struct OvDesc {
            data_idx: usize,
            w: u32,
            h: u32,
            x: f32,
            y: f32,
            scale: f32,
        }
        let mut ov_descs: Vec<OvDesc> = Vec::new();

        let elapsed = self.start.elapsed().as_secs_f32();
        let pct = if self.auto_animate {
            (elapsed / self.progress_duration_secs as f32 * 100.0).min(100.0)
        } else {
            self.pct as f32
        };
        let message = if self.auto_animate {
            match self.phase {
                ProgressPhase::Connecting => self.connecting_text.as_str(),
                ProgressPhase::Connected => self.connected_text.as_str(),
            }
        } else {
            self.message.as_str()
        };

        let panel_data = {
            let mut pixmap = tiny_skia::Pixmap::new(pw, ph).unwrap();
            let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, pw as f32, ph as f32).unwrap();
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(30, 30, 35, 255));
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);

            let wave_data = waves::render(pw, ph, self.animation_time, s, &self.waves);
            if let Some(wave_pix) =
                tiny_skia::Pixmap::from_vec(wave_data, tiny_skia::IntSize::from_wh(pw, ph).unwrap())
            {
                pixmap.draw_pixmap(
                    0,
                    0,
                    wave_pix.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
            }

            let mut border = tiny_skia::Paint::default();
            border.set_color(tiny_skia::Color::from_rgba8(60, 60, 75, 255));
            let stroke = tiny_skia::Stroke {
                width: 2.0,
                ..Default::default()
            };
            let path = tiny_skia::PathBuilder::from_rect(rect);
            pixmap.stroke_path(
                &path,
                &border,
                &stroke,
                tiny_skia::Transform::identity(),
                None,
            );
            pixmap.take()
        };
        let panel_idx = data.len();
        data.push(panel_data);
        ov_descs.push(OvDesc {
            data_idx: panel_idx,
            w: pw,
            h: ph,
            x: 0.0,
            y: 0.0,
            scale: 1.0,
        });

        let fs = monitor::scaled_val(32) as f32;
        sections.push(
            Section::default()
                .with_layout(
                    wgpu_text::glyph_brush::Layout::default()
                        .h_align(wgpu_text::glyph_brush::HorizontalAlign::Center),
                )
                .add_text(
                    Text::new(&format!("{}%", pct as u8))
                        .with_scale(fs)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position((pw as f32 / 2.0, 140.0 * s))
                .to_owned(),
        );

        let bw = monitor::scaled_val(280) as u32;
        let bh = monitor::scaled_val(12) as u32;
        let bx = (pw as f32 - bw as f32) / 2.0;
        let by = 190.0 * s;
        let i = data.len();
        data.push(progress::render(pct, bw, bh));
        ov_descs.push(OvDesc {
            data_idx: i,
            w: bw,
            h: bh,
            x: bx,
            y: by,
            scale: 1.0,
        });

        let msg_fs = monitor::scaled_val(13) as f32;
        sections.push(
            Section::default()
                .with_layout(
                    wgpu_text::glyph_brush::Layout::default()
                        .h_align(wgpu_text::glyph_brush::HorizontalAlign::Center),
                )
                .add_text(
                    Text::new(message)
                        .with_scale(msg_fs)
                        .with_color([0.7, 0.7, 0.9, 1.0]),
                )
                .with_screen_position((pw as f32 / 2.0, by + bh as f32 + 10.0 * s))
                .to_owned(),
        );

        let (btn_data, btn_text) = self.cancel_btn.render();
        let b_idx = data.len();
        data.push(btn_data);
        ov_descs.push(OvDesc {
            data_idx: b_idx,
            w: self.cancel_btn.w,
            h: self.cancel_btn.h,
            x: self.cancel_btn.x,
            y: self.cancel_btn.y,
            scale: 1.0,
        });
        sections.push(btn_text);

        ov_descs.push(OvDesc {
            data_idx: logo_idx,
            w: logo.width,
            h: logo.height,
            x: (pw as f32 - logo.width as f32 * s) / 2.0,
            y: (35.0 * s).min(ph as f32 - logo.height as f32 * s),
            scale: s,
        });

        let mut overlays = Vec::with_capacity(ov_descs.len());
        for d in &ov_descs {
            overlays.push(OverlayParams {
                rgba: &data[d.data_idx],
                width: d.w,
                height: d.h,
                x: d.x,
                y: d.y,
                scale: d.scale,
            });
        }

        self.renderer
            .update_and_render(&[], pw, ph, &overlays, &sections, None, None);
    }
}

impl crate::AppHandler {
    pub(crate) fn handle_progress_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        let Some(ref mut p) = self.progress else {
            return;
        };

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                p.last_mouse_pos = Some((px, py));
                if p.handle_mouse_move(px, py) {
                    p.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(pos) = p.last_mouse_pos {
                    p.handle_click(pos.0, pos.1);
                    if p.cancelled {
                        self.stop.trigger();
                        el.exit();
                    }
                }
                p.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                p.paint();
            }
            WindowEvent::CloseRequested => {
                self.close_progress();
                self.stop.trigger();
                el.exit();
            }
            _ => {}
        }
    }

    pub(crate) fn close_progress(&mut self) {
        if let Some(ref p) = self.progress {
            self.unregister_window(p.window.id());
        }
        self.progress = None;
    }
}
