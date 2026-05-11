// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use shared::log;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

use crate::logo;

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

struct AboutState {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    logo_rgba: Vec<u8>,
    logo_w: u32,
    logo_h: u32,
    start: Instant,
    angle: f32,
    phys_w: u32,
    phys_h: u32,
    scale: f32,
}

impl AboutState {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("About UDS Launcher")
                    .with_inner_size(winit::dpi::LogicalSize::new(420.0, 440.0))
                    .with_resizable(false),
            )?,
        );

        let context =
            softbuffer::Context::new(window.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
        let phys = window.inner_size();
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        surface
            .resize(
                NonZeroU32::new(phys.width).unwrap_or(NonZeroU32::new(1).unwrap()),
                NonZeroU32::new(phys.height).unwrap_or(NonZeroU32::new(1).unwrap()),
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let logo = logo::load_logo();
        let phys_size = window.inner_size();
        let win_scale = window.scale_factor() as f32;

        Ok(AboutState {
            window,
            surface,
            logo_rgba: logo.rgba,
            logo_w: logo.width,
            logo_h: logo.height,
            start: Instant::now(),
            angle: 0.0,
            phys_w: phys_size.width,
            phys_h: phys_size.height,
            scale: win_scale,
        })
    }

    fn paint(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f32();
        self.angle = (elapsed * std::f32::consts::PI).sin() * 0.3;

        let mut buffer = match self.surface.buffer_mut() {
            Ok(b) => b,
            Err(_) => return,
        };

        let w = self.phys_w;
        let h = self.phys_h;
        let s = self.scale;

        // Background gradient
        for row in 0..h {
            let t = row as f32 / h as f32;
            let r = (26.0 + t * 32.0) as u8;
            let g = (26.0 + t * 32.0) as u8;
            let b = (46.0 + t * 48.0) as u8;
            let color = u32::from_ne_bytes([b, g, r, 0xFF]);
            for col in 0..w {
                let idx = (row * w + col) as usize;
                if idx < buffer.len() {
                    buffer[idx] = color;
                }
            }
        }

        // Logo centered at top with rotation wobble
        {
            let logo_y = (30.0 * s) as i32;
            let cx = self.logo_w as f32 / 2.0;
            let cy = self.logo_h as f32 / 2.0;
            let scaled_ww = (self.logo_w as f32 * s) as u32;
            let scaled_hh = (self.logo_h as f32 * s) as u32;
            let logo_x = (w as i32 - scaled_ww as i32) / 2;

            for row in 0..self.logo_h {
                for col in 0..self.logo_w {
                    // Scale position for target
                    let dst_row = (row as f32 * s) as u32;
                    let dst_col = (col as f32 * s) as u32;
                    if dst_row >= scaled_hh || dst_col >= scaled_ww {
                        continue;
                    }

                    let sx = col as f32 - cx;
                    let sy = row as f32 - cy;
                    let rx = sx * self.angle.cos() - sy * self.angle.sin() + cx;
                    let ry = sx * self.angle.sin() + sy * self.angle.cos() + cy;
                    let px = logo_x + (rx * s) as i32;
                    let py = logo_y + (ry * s) as i32;
                    if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                        let si = (row * self.logo_w + col) as usize * 4;
                        if si + 3 < self.logo_rgba.len() {
                            let a = self.logo_rgba[si + 3];
                            if a > 0 {
                                let di = (py as u32 * w + px as u32) as usize;
                                if di < buffer.len() {
                                    let r = self.logo_rgba[si];
                                    let g = self.logo_rgba[si + 1];
                                    let b = self.logo_rgba[si + 2];
                                    let alpha = a as f32 / 255.0;
                                    let bg = buffer[di];
                                    let bg_b = bg as u8;
                                    let bg_g = (bg >> 8) as u8;
                                    let bg_r = (bg >> 16) as u8;
                                    let br = (r as f32 * alpha + bg_r as f32 * (1.0 - alpha)) as u8;
                                    let bg2 =
                                        (g as f32 * alpha + bg_g as f32 * (1.0 - alpha)) as u8;
                                    let bb = (b as f32 * alpha + bg_b as f32 * (1.0 - alpha)) as u8;
                                    buffer[di] = u32::from_ne_bytes([bb, bg2, br, 0xFF]);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Draw about text lines
        let base_y = self.logo_h as f32 * s + 80.0 * s;
        for (i, line) in ABOUT_LINES.iter().enumerate() {
            let y = base_y + i as f32 * (22.0 * s);
            draw_text_centered(&mut buffer, w, line, y, 14.0 * s, 0xFF_C0_C0_E0, s);
        }

        // Close button
        let btn_w = (80.0 * s) as i32;
        let btn_h = (35.0 * s) as i32;
        let btn_x = (w as f32 - btn_w as f32) / 2.0;
        let btn_y = base_y + ABOUT_LINES.len() as f32 * (22.0 * s) + 20.0 * s;
        draw_button_scaled(
            &mut buffer,
            w,
            btn_x as i32,
            btn_y as i32,
            btn_w,
            btn_h,
            "Close",
            s,
        );

        let _ = buffer.present();
    }
}

fn draw_text_centered(
    buffer: &mut [u32],
    screen_w: u32,
    text: &str,
    y: f32,
    size_px: f32,
    color: u32,
    _scale: f32,
) {
    let char_h = (size_px as i32).max(6);
    let char_w = char_h * 2 / 3;
    if char_w < 1 {
        return;
    }
    let x = (screen_w as i32 - text.len() as i32 * char_w) / 2;

    for (i, ch) in text.chars().enumerate() {
        let cx = x + i as i32 * char_w;
        let cy = y as i32;
        for row in 0..char_h {
            let src_row = (row * 12 / char_h).min(11);
            for col in 0..char_w {
                let src_col = (col * 8 / char_w).min(7);
                if char_pixel_simple(ch, src_col, src_row).is_some() {
                    let px = cx + col;
                    let py = cy + row;
                    if px >= 0 && py >= 0 && (px as u32) < screen_w {
                        let idx = (py as u32 * screen_w + px as u32) as usize;
                        if idx < buffer.len() {
                            buffer[idx] = color;
                        }
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn draw_button_scaled(
    buffer: &mut [u32],
    screen_w: u32,
    bx: i32,
    by: i32,
    bw: i32,
    bh: i32,
    label: &str,
    scale: f32,
) {
    let bg = 0xFF_50_50_70_u32;
    let border = 0xFF_80_80_A0_u32;
    for row in by..(by + bh) {
        for col in bx..(bx + bw) {
            if row >= 0 && col >= 0 && (col as u32) < screen_w {
                let idx = (row as u32 * screen_w + col as u32) as usize;
                if idx < buffer.len() {
                    let is_border =
                        row == by || row == by + bh - 1 || col == bx || col == bx + bw - 1;
                    buffer[idx] = if is_border { border } else { bg };
                }
            }
        }
    }
    draw_text_centered(
        buffer,
        screen_w,
        label,
        by as f32 + (bh as f32 - 12.0 * scale) / 2.0,
        12.0 * scale,
        0xFF_C0_C0_E0,
        scale,
    );
}

#[allow(dead_code)]
fn draw_button(buffer: &mut [u32], screen_w: u32, bx: i32, by: i32, bw: i32, bh: i32, label: &str) {
    draw_button_scaled(buffer, screen_w, bx, by, bw, bh, label, 1.0)
}

fn char_pixel_simple(ch: char, col: i32, row: i32) -> Option<()> {
    if ch == ' ' {
        return None;
    }
    if ch.is_alphanumeric() || matches!(ch, '.' | '-' | ':' | '/' | '%' | '\'' | ')' | '(') {
        // Simple representation: character fills a rectangle with basic shapes
        let ch = ch.to_ascii_uppercase();
        match (ch, col, row) {
            // Basic alphabet - simplified for readability
            ('A', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || r == 1 || r == 5) =>
            {
                Some(())
            }
            ('B', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('C', c, r)
                if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || r == 1 || r == 9) =>
            {
                Some(())
            }
            ('D', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || r == 1 || r == 9) =>
            {
                Some(())
            }
            ('E', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('F', c, r)
                if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || r == 1 || r == 5) =>
            {
                Some(())
            }
            ('G', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || r == 1 || r == 9 || (c >= 3 && r == 5) || (c == 5 && r >= 5)) =>
            {
                Some(())
            }
            ('H', c, r)
                if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || c == 5 || r == 5) =>
            {
                Some(())
            }
            ('I', c, _) if (2..=4).contains(&c) => Some(()),
            ('K', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || (r >= 5 && c == r - 5 + 2) || (r <= 5 && c == 6 - r)) =>
            {
                Some(())
            }
            ('L', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || r == 9) => {
                Some(())
            }
            ('M', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || (r <= 5 && (c == r || c == 6 - r))) =>
            {
                Some(())
            }
            ('N', c, r)
                if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || c == 5 || c == r) =>
            {
                Some(())
            }
            ('O', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || r == 1 || r == 9) =>
            {
                Some(())
            }
            ('P', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || (r <= 5 && c == 5) || r == 1 || r == 5) =>
            {
                Some(())
            }
            ('R', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1
                        || (r <= 5 && c == 5)
                        || r == 1
                        || r == 5
                        || (r > 5 && c == r - 5 + 2)) =>
            {
                Some(())
            }
            ('S', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && ((r <= 5 && c == 1) || (r >= 5 && c == 5) || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('T', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (r == 1 || c == 3) => {
                Some(())
            }
            ('U', c, r)
                if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || c == 5 || r == 9) =>
            {
                Some(())
            }
            ('V', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && ((r <= 5 && (c == 1 || c == 5)) || (r >= 5 && c == 3)) =>
            {
                Some(())
            }
            ('W', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || (r >= 5 && (c == 3 || c == 2 || c == 4))) =>
            {
                Some(())
            }
            ('X', c, r)
                if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == r || c == 6 - r) =>
            {
                Some(())
            }
            ('Y', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && ((r <= 5 && (c == r || c == 6 - r)) || (r >= 5 && c == 3)) =>
            {
                Some(())
            }
            ('0', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) => Some(()),
            ('1', 3, _) => Some(()),
            ('2', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (r == 1 || r == 5 || r == 9 || (r <= 5 && c == 5) || (r >= 5 && c == 1)) =>
            {
                Some(())
            }
            ('3', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (r == 1 || r == 5 || r == 9 || c == 5) =>
            {
                Some(())
            }
            ('4', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 5 || r == 5 || (r <= 5 && c == 1)) =>
            {
                Some(())
            }
            ('5', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (r == 1 || r == 5 || r == 9 || (r <= 5 && c == 1) || (r >= 5 && c == 5)) =>
            {
                Some(())
            }
            ('6', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || r == 5 || r == 9 || (r >= 5 && c == 5)) =>
            {
                Some(())
            }
            ('7', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (r == 1 || c == 5) => {
                Some(())
            }
            ('8', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 1 || c == 5 || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('9', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && (c == 5 || r == 1 || r == 5 || (r <= 5 && c == 1)) =>
            {
                Some(())
            }
            ('.', _, 8) => Some(()),
            ('-', _, 5) => Some(()),
            (':', 3, _) => Some(()),
            ('/', c, r) if c == 6 - r => Some(()),
            ('%', c, r)
                if (1..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && ((c == 1 && r <= 3) || (c == 5 && r >= 7) || c == r) =>
            {
                Some(())
            }
            ('\'', c, r) if c == 3 && r <= 3 => Some(()),
            (')', c, r)
                if (3..=5).contains(&c)
                    && (1..=9).contains(&r)
                    && ((c == 5 && (2..=8).contains(&r)) || (c == r + 2) && (r == 1 || r == 9)) =>
            {
                Some(())
            }
            ('(', c, r)
                if (1..=3).contains(&c)
                    && (1..=9).contains(&r)
                    && ((c == 1 && (2..=8).contains(&r)) || (c + r == 5) && (r == 1 || r == 9)) =>
            {
                Some(())
            }
            _ => Some(()),
        }
    } else {
        Some(())
    }
}

/// Show about window in its own event loop (blocking)
pub fn show_about_window() {
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            log::error!("Failed to create event loop for about: {}", e);
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut state: Option<AboutState> = None;

    let _ = event_loop.run_app(&mut AboutHandler {
        state: &mut state,
        last_frame: Instant::now(),
    });
}

#[allow(dead_code)]
struct AboutHandler<'a> {
    state: &'a mut Option<AboutState>,
    last_frame: Instant,
}

impl ApplicationHandler for AboutHandler<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match AboutState::new(event_loop) {
            Ok(s) => {
                *self.state = Some(s);
            }
            Err(e) => {
                log::error!("Failed to create about window: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = self.state.as_mut() {
                    state.paint();
                }
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                // Close button hit test
                if let Some(_s) = self.state.as_ref() {
                    // Close button at bottom center
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
    }
}
