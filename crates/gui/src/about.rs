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
    scale: f32,
}

impl AboutState {
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("About UDS Launcher")
                    .with_inner_size(winit::dpi::LogicalSize::new(420.0, 440.0))
                    .with_resizable(false),
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
        })
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn paint(&mut self) {
        self.angle = (self.start.elapsed().as_secs_f32() * std::f32::consts::PI).sin() * 0.3;
        let s = self.scale;
        let pw = self.phys_w;
        let ph = self.phys_h;
        let logo_x = (pw as f32 - self.logo.width as f32 * s) / 2.0;
        let logo_y = 30.0 * s;
        // Logo has rotation applied, but for simplicity just show it centered
        let ov = OverlayParams {
            rgba: &self.logo.rgba,
            width: self.logo.width,
            height: self.logo.height,
            x: logo_x,
            y: logo_y,
            scale: s,
        };
        let mut sections: Vec<OwnedSection> = Vec::new();
        let base_y = self.logo.height as f32 * s + 80.0 * s;
        for (i, line) in ABOUT_LINES.iter().enumerate() {
            let y = base_y + i as f32 * (22.0 * s);
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(line)
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color([0.75, 0.75, 0.88, 1.0]),
                    )
                    .with_screen_position(((pw as f32 - line.len() as f32 * 8.0 * s) / 2.0, y))
                    .to_owned(),
            );
        }
        let close_y = base_y + ABOUT_LINES.len() as f32 * 22.0 * s + 20.0 * s;
        let bw = monitor::scaled_val(80) as u32;
        let bh = monitor::scaled_val(35) as u32;
        let mut btn = Vec::with_capacity((bw * bh * 4) as usize);
        for _ in 0..bw * bh {
            btn.extend_from_slice(&[0x50, 0x50, 0x70, 0xFF]);
        }
        let ovb = OverlayParams {
            rgba: &btn,
            width: bw,
            height: bh,
            x: (pw as f32 - bw as f32) / 2.0,
            y: close_y,
            scale: 1.0,
        };
        sections.push(
            Section::default()
                .add_text(
                    Text::new("Close")
                        .with_scale(monitor::scaled_val(14) as f32)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position(((pw as f32 - 40.0 * s) / 2.0, close_y + 8.0 * s))
                .to_owned(),
        );
        let overlays = vec![ov, ovb];
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
    let _ = event_loop.run_app(&mut AboutHandler {
        state: &mut state,
    });
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
