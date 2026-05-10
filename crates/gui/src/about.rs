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
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        surface
            .resize(NonZeroU32::new(420).unwrap(), NonZeroU32::new(440).unwrap())
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let logo = logo::load_logo();

        Ok(AboutState {
            window,
            surface,
            logo_rgba: logo.rgba,
            logo_w: logo.width,
            logo_h: logo.height,
            start: Instant::now(),
            angle: 0.0,
        })
    }

    fn paint(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f32();
        self.angle = (elapsed * std::f32::consts::PI).sin() * 0.3;

        let mut buffer = match self.surface.buffer_mut() {
            Ok(b) => b,
            Err(_) => return,
        };

        let w = 420u32;
        let h = 440u32;

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
            let logo_x = (w as i32 - self.logo_w as i32) / 2;
            let logo_y = 30i32;
            let cx = self.logo_w as f32 / 2.0;
            let cy = self.logo_h as f32 / 2.0;

            for row in 0..self.logo_h {
                for col in 0..self.logo_w {
                    let sx = col as f32 - cx;
                    let sy = row as f32 - cy;
                    let rx = sx * self.angle.cos() - sy * self.angle.sin() + cx;
                    let ry = sx * self.angle.sin() + sy * self.angle.cos() + cy;
                    let px = logo_x + rx as i32;
                    let py = logo_y + ry as i32;
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
        let base_y = (self.logo_h as i32 + 50) as f32;
        for (i, line) in ABOUT_LINES.iter().enumerate() {
            let y = base_y + i as f32 * 22.0;
            draw_text_centered(&mut buffer, w, line, y, 14.0, 0xFF_C0_C0_E0);
        }

        // Close button
        let btn_x = (w as f32 - 80.0) / 2.0;
        let btn_y = base_y + ABOUT_LINES.len() as f32 * 22.0 + 20.0;
        draw_button(&mut buffer, w, btn_x as i32, btn_y as i32, 80, 35, "Close");

        let _ = buffer.present();
    }
}

fn draw_text_centered(
    buffer: &mut [u32],
    screen_w: u32,
    text: &str,
    y: f32,
    _size: f32,
    color: u32,
) {
    let char_w = 8;
    let char_h = 12;
    let x = (screen_w as f32 - text.len() as f32 * char_w as f32) / 2.0;

    for (i, ch) in text.chars().enumerate() {
        let cx = x as i32 + i as i32 * char_w;
        let cy = y as i32;
        for row in 0..char_h {
            for col in 0..char_w {
                if let Some(_) = char_pixel_simple(ch, col, row) {
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

fn draw_button(buffer: &mut [u32], screen_w: u32, bx: i32, by: i32, bw: i32, bh: i32, label: &str) {
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
        by as f32 + (bh as f32 - 12.0) / 2.0,
        12.0,
        0xFF_C0_C0_E0,
    );
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
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || r == 1 || r == 5) =>
            {
                Some(())
            }
            ('B', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('C', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == 1 || r == 1 || r == 9) => {
                Some(())
            }
            ('D', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || r == 1 || r == 9) =>
            {
                Some(())
            }
            ('E', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('F', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == 1 || r == 1 || r == 5) => {
                Some(())
            }
            ('G', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || r == 1 || r == 9 || (c >= 3 && r == 5) || (c == 5 && r >= 5)) =>
            {
                Some(())
            }
            ('H', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == 1 || c == 5 || r == 5) => {
                Some(())
            }
            ('I', c, _) if c >= 2 && c <= 4 => Some(()),
            ('K', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || (r >= 5 && c == r - 5 + 2) || (r <= 5 && c == 6 - r)) =>
            {
                Some(())
            }
            ('L', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == 1 || r == 9) => Some(()),
            ('M', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || (r <= 5 && (c == r || c == 6 - r))) =>
            {
                Some(())
            }
            ('N', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == 1 || c == 5 || c == r) => {
                Some(())
            }
            ('O', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || r == 1 || r == 9) =>
            {
                Some(())
            }
            ('P', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || (r <= 5 && c == 5) || r == 1 || r == 5) =>
            {
                Some(())
            }
            ('R', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1
                        || (r <= 5 && c == 5)
                        || r == 1
                        || r == 5
                        || (r > 5 && c == r - 5 + 2)) =>
            {
                Some(())
            }
            ('S', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && ((r <= 5 && c == 1) || (r >= 5 && c == 5) || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('T', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (r == 1 || c == 3) => Some(()),
            ('U', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == 1 || c == 5 || r == 9) => {
                Some(())
            }
            ('V', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && ((r <= 5 && (c == 1 || c == 5)) || (r >= 5 && c == 3)) =>
            {
                Some(())
            }
            ('W', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || (r >= 5 && (c == 3 || c == 2 || c == 4))) =>
            {
                Some(())
            }
            ('X', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (c == r || c == 6 - r) => {
                Some(())
            }
            ('Y', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && ((r <= 5 && (c == r || c == 6 - r)) || (r >= 5 && c == 3)) =>
            {
                Some(())
            }
            ('0', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 => Some(()),
            ('1', c, _) if c == 3 => Some(()),
            ('2', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (r == 1 || r == 5 || r == 9 || (r <= 5 && c == 5) || (r >= 5 && c == 1)) =>
            {
                Some(())
            }
            ('3', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (r == 1 || r == 5 || r == 9 || c == 5) =>
            {
                Some(())
            }
            ('4', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 5 || r == 5 || (r <= 5 && c == 1)) =>
            {
                Some(())
            }
            ('5', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (r == 1 || r == 5 || r == 9 || (r <= 5 && c == 1) || (r >= 5 && c == 5)) =>
            {
                Some(())
            }
            ('6', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || r == 5 || r == 9 || (r >= 5 && c == 5)) =>
            {
                Some(())
            }
            ('7', c, r) if c >= 1 && c <= 5 && r >= 1 && r <= 9 && (r == 1 || c == 5) => Some(()),
            ('8', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 1 || c == 5 || r == 1 || r == 5 || r == 9) =>
            {
                Some(())
            }
            ('9', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && (c == 5 || r == 1 || r == 5 || (r <= 5 && c == 1)) =>
            {
                Some(())
            }
            ('.', _, r) if r == 8 => Some(()),
            ('-', _, r) if r == 5 => Some(()),
            (':', c, _) if c == 3 => Some(()),
            ('/', c, r) if c == 6 - r => Some(()),
            ('%', c, r)
                if c >= 1
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && ((c == 1 && r <= 3) || (c == 5 && r >= 7) || c == r) =>
            {
                Some(())
            }
            ('\'', c, r) if c == 3 && r <= 3 => Some(()),
            (')', c, r)
                if c >= 3
                    && c <= 5
                    && r >= 1
                    && r <= 9
                    && ((c == 5 && (r >= 2 && r <= 8)) || (c == r + 2) && (r == 1 || r == 9)) =>
            {
                Some(())
            }
            ('(', c, r)
                if c >= 1
                    && c <= 3
                    && r >= 1
                    && r <= 9
                    && ((c == 1 && (r >= 2 && r <= 8)) || (c + r == 5) && (r == 1 || r == 9)) =>
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
            WindowEvent::MouseInput { state, button, .. } => {
                if state.is_pressed() && button == winit::event::MouseButton::Left {
                    // Close button hit test
                    if let Some(_s) = self.state.as_ref() {
                        // Close button at bottom center
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            let _ = state.window.request_redraw();
        }
    }
}
