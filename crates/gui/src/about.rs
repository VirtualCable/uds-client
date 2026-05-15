use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use anyhow::Result;
use shared::log;
use std::sync::Arc;
use std::time::Instant;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

const ABOUT_LINES: &[&str] = &[
    "UDS Launcher",
    "Version: 5.0.0",
    "UDS Client Launcher",
    "",
    "Developed by Virtual Cable S.L.",
    "https://www.udsenterprise.com",
    "",
    "This software is provided 'as-is',",
    "without any express or implied warranty.",
    "In no event will the authors be held liable",
    "for any damages arising from the use of this software.",
];

pub struct AboutState {
    window: Arc<Window>,
    renderer: WgpuRenderer,
    logo: crate::logo::LogoImage,
    start: Instant,
    angle: f32,
    phys_w: u32,
    phys_h: u32,
    pub scale: f32,
    pub animation_time: f32,
    pub waves: Vec<crate::draw::ui::waves::Wave>,
    pub is_hovered: bool,
}

impl AboutState {
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        let (dw, dh) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let ww = 460.0;
        let wh = 500.0;
        let sf = crate::monitor::scale(0) as f32;
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("About UDS Launcher")
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_resizable(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
            )?,
        );
        let phys = window.inner_size();
        let scale = *monitor::SCALE_FACTOR as f32;
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
        let logo = crate::logo::load_logo();
        Ok(AboutState {
            window,
            renderer,
            logo,
            start: Instant::now(),
            angle: 0.0,
            phys_w: phys.width,
            phys_h: phys.height,
            scale,
            animation_time: 0.0,
            waves: crate::draw::ui::waves::Wave::default_set(),
            is_hovered: false,
        })
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn handle_mouse_move(&mut self, logical_x: f32, logical_y: f32) -> bool {
        let old_hover = self.is_hovered;
        self.is_hovered = false;
        let s = self.scale;
        let pw = self.phys_w as f32;
        let ph = self.phys_h as f32;
        
        let bw = monitor::scaled_val(80) as f32;
        let bh = monitor::scaled_val(35) as f32;
        
        let bx = (pw - bw) / 2.0;
        let by = ph - bh - 20.0 * s;

        let x = logical_x * s;
        let y = logical_y * s;

        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
            self.is_hovered = true;
        }
        
        self.is_hovered != old_hover
    }

    pub fn paint(&mut self) {
        self.angle = (self.start.elapsed().as_secs_f32() * std::f32::consts::PI).sin() * 0.3;
        let s = self.scale;
        let pw = self.phys_w;
        let ph = self.phys_h;

        self.renderer.reconfigure(pw, ph);

        let mut data: Vec<Vec<u8>> = Vec::new();

        // 1. Draw Panel Background + Waves
        let panel_data = {
            let mut pixmap = tiny_skia::Pixmap::new(pw, ph).unwrap();
            let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, pw as f32, ph as f32).unwrap();
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(30, 30, 35, 255));
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            
            // Draw Waves
            let wave_data = crate::draw::ui::waves::render(pw, ph, self.animation_time, s, &self.waves);
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
        data.push(panel_data);

        let logo_x = (pw as f32 - self.logo.width as f32 * s) / 2.0;
        let logo_y = 30.0 * s;

        // Overlay index 0 is background
        // We'll push logo and others after
        let mut sections: Vec<OwnedSection> = Vec::new();
        let base_y = self.logo.height as f32 * s + 60.0 * s;
        for (i, line) in ABOUT_LINES.iter().enumerate() {
            let y = base_y + i as f32 * (22.0 * s);
            sections.push(
                Section::default()
                    .with_layout(
                        wgpu_text::glyph_brush::Layout::default()
                            .h_align(wgpu_text::glyph_brush::HorizontalAlign::Center)
                    )
                    .add_text(
                        Text::new(line)
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color([0.75, 0.75, 0.88, 1.0]),
                    )
                    .with_screen_position((pw as f32 / 2.0, y))
                    .to_owned(),
            );
        }
        let bw = monitor::scaled_val(80) as u32;
        let bh = monitor::scaled_val(35) as u32;
        let close_y = (ph as f32) - (bh as f32) - 20.0 * s;
        
        let style = crate::draw::ui::button::ButtonStyle {
            font_scale: monitor::scaled_val(14) as f32,
            bg_color: if self.is_hovered { [0x70, 0x70, 0x90, 0xFF] } else { [0x50, 0x50, 0x70, 0xFF] },
            border_color: if self.is_hovered { [0x90, 0x90, 0xB0, 0xFF] } else { [0x70, 0x70, 0x90, 0xFF] },
            ..crate::draw::ui::button::ButtonStyle::default()
        };
        
        let close_x = (pw as f32 - bw as f32) / 2.0;
        let (close_data, close_text) = crate::draw::ui::button::render(close_x, close_y, bw, bh, "Close", &style);
        data.push(close_data);
        sections.push(close_text.to_owned());

        let overlays = vec![
            OverlayParams {
                rgba: &data[0],
                width: pw,
                height: ph,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
            },
            OverlayParams {
                rgba: &self.logo.rgba,
                width: self.logo.width,
                height: self.logo.height,
                x: logo_x,
                y: logo_y,
                scale: s,
            },
            OverlayParams {
                rgba: &data[1],
                width: bw,
                height: bh,
                x: (pw as f32 - bw as f32) / 2.0,
                y: close_y,
                scale: 1.0,
            },
        ];
        self.renderer
            .update_and_render(&[], pw, ph, &overlays, &sections, None);
    }
}

pub fn show_about_window() {
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            log::error!("{e}");
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut state: Option<AboutState> = None;
    let _ = event_loop.run_app(&mut AboutHandler { state: &mut state });
}

struct AboutHandler<'a> {
    state: &'a mut Option<AboutState>,
}

impl ApplicationHandler for AboutHandler<'_> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        match AboutState::new(el) {
            Ok(s) => *self.state = Some(s),
            Err(e) => {
                log::error!("{e}");
                el.exit();
            }
        }
    }
    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => {
                if let Some(s) = self.state.as_mut() {
                    s.paint();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(s) = self.state.as_mut() {
                    let logical = position.to_logical::<f32>(s.scale as f64);
                    if s.handle_mouse_move(logical.x, logical.y) {
                        s.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: bs, button, ..
            } if bs.is_pressed() && button == winit::event::MouseButton::Left => {
                el.exit();
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(s) = self.state.as_ref() {
            s.window.request_redraw();
        }
    }
}
