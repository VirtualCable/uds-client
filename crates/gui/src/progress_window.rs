// BSD 3-Clause License, Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::sync::Arc;
use std::time::Instant;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use crate::draw::ui::{button::{self, ButtonStyle}, progress, waves::{self, Wave}};

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
    pub hover_idx: Option<usize>,
    pub cancelled: bool,
    pub animation_time: f32,
    pub waves: Vec<Wave>,
    pub last_mouse_pos: Option<(f32, f32)>,
}

impl ProgressState {
    pub fn new(el: &winit::event_loop::ActiveEventLoop) -> anyhow::Result<Self> {
        let (dw, dh) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let ww = 400.0;
        let wh = 300.0;
        let sf = crate::monitor::scale(0) as f32;
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(el.create_window(
            winit::window::Window::default_attributes()
                .with_title("UDS Launcher")
                .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                .with_window_icon(Some(crate::logo::load_icon()))
                .with_resizable(false)
                .with_decorations(false)
                .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
        )?);
        
        let phys = window.inner_size();
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
        
        Ok(Self {
            window,
            renderer,
            pct: 0,
            message: String::new(),
            start: Instant::now(),
            progress_duration_secs: 30,
            phase: ProgressPhase::Connecting,
            auto_animate: false,
            hover_idx: None,
            cancelled: false,
            animation_time: 0.0,
            waves: vec![
                Wave { y_base: 0.4, amplitude: 25.0, speed: 0.04, offset: 0.0, thickness: 5.0, opacity: 0.5 },
                Wave { y_base: 0.42, amplitude: 20.0, speed: 0.06, offset: 2.0, thickness: 3.5, opacity: 0.3 },
                Wave { y_base: 0.38, amplitude: 22.0, speed: 0.03, offset: 4.0, thickness: 6.0, opacity: 0.25 },
            ],
            last_mouse_pos: None,
        })
    }

    pub fn handle_mouse_move(&mut self, logical_x: f32, logical_y: f32) -> bool {
        let old_hover = self.hover_idx;
        self.hover_idx = None;
        let s = *crate::monitor::SCALE_FACTOR as f32;
        let x = logical_x * s;
        let y = logical_y * s;
        
        // Cancel button position (same logic as in paint)
        let bw = crate::monitor::scaled_val(120) as f32;
        let bh = crate::monitor::scaled_val(32) as f32;
        let bx = (400.0 * s - bw) / 2.0;
        let by = (300.0 * s) - bh - 25.0 * s;

        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
            self.hover_idx = Some(0);
        }
        self.hover_idx != old_hover
    }

    pub fn handle_click(&mut self, logical_x: f32, logical_y: f32) {
        let s = *crate::monitor::SCALE_FACTOR as f32;
        let x = logical_x * s;
        let y = logical_y * s;
        let bw = crate::monitor::scaled_val(120) as f32;
        let bh = crate::monitor::scaled_val(32) as f32;
        let bx = (400.0 * s - bw) / 2.0;
        let by = (300.0 * s) - bh - 25.0 * s;

        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
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
                ProgressPhase::Connecting => "Connecting to RDP server...",
                ProgressPhase::Connected => "Connected.",
            }
        } else {
            self.message.as_str()
        };

        // 1. Draw Panel Background + Waves
        let panel_data = {
            let mut pixmap = tiny_skia::Pixmap::new(pw, ph).unwrap();
            let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, pw as f32, ph as f32).unwrap();
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(30, 30, 35, 255));
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            
            // Draw Waves using component
            let wave_data = waves::render(pw, ph, self.animation_time, s, &self.waves);
            if let Some(wave_pix) = tiny_skia::Pixmap::from_vec(wave_data, tiny_skia::IntSize::from_wh(pw, ph).unwrap()) {
                pixmap.draw_pixmap(0, 0, wave_pix.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            }

            // Subtle border
            let mut border = tiny_skia::Paint::default();
            border.set_color(tiny_skia::Color::from_rgba8(60, 60, 75, 255));
            let stroke = tiny_skia::Stroke { width: 2.0, ..Default::default() };
            let path = tiny_skia::PathBuilder::from_rect(rect);
            pixmap.stroke_path(&path, &border, &stroke, tiny_skia::Transform::identity(), None);
            pixmap.take()
        };
        let panel_idx = data.len();
        data.push(panel_data);
        ov_descs.push(OvDesc { data_idx: panel_idx, w: pw, h: ph, x: 0.0, y: 0.0, scale: 1.0 });

        // 2. Percentage text
        let fs = monitor::scaled_val(32) as f32;
        sections.push(
            Section::default()
                .with_layout(wgpu_text::glyph_brush::Layout::default().h_align(wgpu_text::glyph_brush::HorizontalAlign::Center))
                .add_text(
                    Text::new(&format!("{}%", pct as u8))
                        .with_scale(fs)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position((pw as f32 / 2.0, 140.0 * s))
                .to_owned(),
        );

        // 3. Progress bar
        let bw = monitor::scaled_val(280) as u32;
        let bh = monitor::scaled_val(12) as u32;
        let bx = (pw as f32 - bw as f32) / 2.0;
        let by = 190.0 * s;
        let i = data.len();
        data.push(progress::render(pct, bw, bh));
        ov_descs.push(OvDesc { data_idx: i, w: bw, h: bh, x: bx, y: by, scale: 1.0 });

        // 4. Status message
        let msg_fs = monitor::scaled_val(13) as f32;
        sections.push(
            Section::default()
                .with_layout(wgpu_text::glyph_brush::Layout::default().h_align(wgpu_text::glyph_brush::HorizontalAlign::Center))
                .add_text(
                    Text::new(message)
                        .with_scale(msg_fs)
                        .with_color([0.7, 0.7, 0.9, 1.0]),
                )
                .with_screen_position((pw as f32 / 2.0, by + bh as f32 + 10.0 * s))
                .to_owned(),
        );

        // 5. CANCEL Button
        let btn_w = monitor::scaled_val(120) as u32;
        let btn_h = monitor::scaled_val(32) as u32;
        let btn_x = (pw as f32 - btn_w as f32) / 2.0;
        let btn_y = (ph as f32) - btn_h as f32 - 25.0 * s;
        
        let btn_style = ButtonStyle {
            font_scale: monitor::scaled_val(13) as f32,
            bg_color: if self.hover_idx == Some(0) { [80, 45, 45, 255] } else { [60, 35, 35, 255] },
            border_color: if self.hover_idx == Some(0) { [150, 80, 80, 255] } else { [100, 60, 60, 255] },
            ..ButtonStyle::default()
        };
        let (btn_data, btn_text) = button::render(btn_x, btn_y, btn_w, btn_h, "CANCEL", &btn_style);
        let b_idx = data.len();
        data.push(btn_data);
        ov_descs.push(OvDesc { data_idx: b_idx, w: btn_w, h: btn_h, x: btn_x, y: btn_y, scale: 1.0 });
        sections.push(btn_text);

        // 6. Logo (Top)
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

        self.renderer.update_and_render(&[], pw, ph, &overlays, &sections, None);
    }
}
