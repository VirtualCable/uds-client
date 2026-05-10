// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

pub enum LauncherInner {
    Invisible,
    Test,
    Progress {
        pct: u8,
        message: String,
    },
    Error(String),
    Warning(String),
    YesNo {
        message: String,
        response: Arc<RwLock<Option<oneshot::Sender<bool>>>>,
    },
}

impl Default for LauncherInner {
    fn default() -> Self {
        LauncherInner::Invisible
    }
}

impl LauncherInner {
    pub fn handle_click(&mut self, x: f32, y: f32) {
        match self {
            LauncherInner::Error(_) | LauncherInner::Warning(_) => {
                if y > 230.0 && y < 270.0 && x > 140.0 && x < 260.0 {
                    *self = LauncherInner::Invisible;
                }
            }
            LauncherInner::YesNo { response, .. } => {
                let resp = response.clone();
                let mut clicked = false;
                if y > 230.0 && y < 270.0 && x > 100.0 && x < 180.0 {
                    if let Some(tx) = resp.write().unwrap().take() {
                        let _ = tx.send(true);
                    }
                    clicked = true;
                }
                if !clicked && y > 230.0 && y < 270.0 && x > 220.0 && x < 300.0 {
                    if let Some(tx) = resp.write().unwrap().take() {
                        let _ = tx.send(false);
                    }
                    clicked = true;
                }
                if clicked {
                    *self = LauncherInner::Invisible;
                }
            }
            _ => {}
        }
    }
}

pub struct LauncherState {
    pub window: Option<Arc<winit::window::Window>>,
    pub surface:
        Option<softbuffer::Surface<Arc<winit::window::Window>, Arc<winit::window::Window>>>,
    pub context: Option<softbuffer::Context<Arc<winit::window::Window>>>,
    pub logo_rgba: Vec<u8>,
    pub logo_width: u32,
    pub logo_height: u32,
    pub inner: LauncherInner,
    pub last_mouse_pos: Option<(f32, f32)>,
}

impl LauncherState {
    pub fn new() -> Self {
        LauncherState {
            window: None,
            surface: None,
            context: None,
            logo_rgba: Vec::new(),
            logo_width: 0,
            logo_height: 0,
            inner: LauncherInner::Invisible,
            last_mouse_pos: None,
        }
    }
}

/// Paint the launcher UI into the softbuffer surface
pub fn paint_launcher(state: &mut LauncherState) {
    let surface = match &mut state.surface {
        Some(s) => s,
        None => return,
    };

    let mut buffer = match surface.buffer_mut() {
        Ok(b) => b,
        Err(_) => return,
    };

    let width = 400u32;
    let height = 300u32;

    // Clear to dark background
    for pixel in buffer.iter_mut() {
        *pixel = 0xFF_1A_1A_2E;
    }

    match &state.inner {
        LauncherInner::Invisible => {
            draw_logo(
                &mut buffer,
                width,
                height,
                &state.logo_rgba,
                state.logo_width,
                state.logo_height,
            );
        }
        LauncherInner::Test => {
            draw_logo(
                &mut buffer,
                width,
                height,
                &state.logo_rgba,
                state.logo_width,
                state.logo_height,
            );
            draw_text(&mut buffer, width, "Test Mode", 140.0, 30.0, 14.0);
        }
        LauncherInner::Progress { pct, message } => {
            draw_logo(
                &mut buffer,
                width,
                height,
                &state.logo_rgba,
                state.logo_width,
                state.logo_height,
            );
            draw_text(&mut buffer, width, &format!("{}%", pct), 180.0, 200.0, 16.0);
            draw_text(&mut buffer, width, message, 40.0, 220.0, 11.0);
            let bar_width = ((*pct as f32 / 100.0) * 320.0) as i32;
            draw_rect(
                &mut buffer,
                width,
                40,
                210,
                320,
                18,
                0xFF_40_40_60,
                0xFF_60_C0_FF,
                Some(bar_width),
            );
        }
        LauncherInner::Error(msg) | LauncherInner::Warning(msg) => {
            let is_error = matches!(state.inner, LauncherInner::Error(_));
            if is_error {
                draw_text(&mut buffer, width, "ERROR", 150.0, 50.0, 14.0);
            } else {
                draw_text(&mut buffer, width, "WARNING", 140.0, 50.0, 14.0);
            }
            draw_text(&mut buffer, width, msg, 20.0, 100.0, 12.0);
            draw_button(&mut buffer, width, 150, 235, 100, 35, "OK");
        }
        LauncherInner::YesNo { message, .. } => {
            draw_text(&mut buffer, width, message, 20.0, 80.0, 12.0);
            draw_button(&mut buffer, width, 100, 235, 80, 35, "Yes");
            draw_button(&mut buffer, width, 220, 235, 80, 35, "No");
        }
    }

    let _ = buffer.present();
}

fn draw_logo(
    buffer: &mut [u32],
    screen_w: u32,
    screen_h: u32,
    rgba: &[u8],
    logo_w: u32,
    logo_h: u32,
) {
    let x = (screen_w as i32 - logo_w as i32) / 2;
    let y = 40;
    if x < 0 {
        return;
    }

    for row in 0..logo_h {
        for col in 0..logo_w {
            let px = x + col as i32;
            let py = y + row as i32;
            if px >= 0 && py >= 0 && (px as u32) < screen_w && (py as u32) < screen_h {
                let src_idx = (row * logo_w + col) as usize * 4;
                let dst_idx = (py as u32 * screen_w + px as u32) as usize;
                if src_idx + 3 < rgba.len() && dst_idx < buffer.len() {
                    let r = rgba[src_idx];
                    let g = rgba[src_idx + 1];
                    let b = rgba[src_idx + 2];
                    let a = rgba[src_idx + 3];
                    if a > 0 {
                        let bg = buffer[dst_idx];
                        let bg_r = (bg >> 16) as u8;
                        let bg_g = (bg >> 8) as u8;
                        let bg_b = bg as u8;
                        let alpha = a as f32 / 255.0;
                        let br = (r as f32 * alpha + bg_r as f32 * (1.0 - alpha)) as u8;
                        let bg2 = (g as f32 * alpha + bg_g as f32 * (1.0 - alpha)) as u8;
                        let bb = (b as f32 * alpha + bg_b as f32 * (1.0 - alpha)) as u8;
                        buffer[dst_idx] = u32::from_ne_bytes([bb, bg2, br, 0xFF]);
                    }
                }
            }
        }
    }
}

fn draw_rect(
    buffer: &mut [u32],
    screen_w: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    bg: u32,
    fg: u32,
    fill_width: Option<i32>,
) {
    for row in y..(y + h) {
        for col in x..(x + w) {
            if row >= 0 && col >= 0 && (col as u32) < screen_w {
                let idx = (row as u32 * screen_w + col as u32) as usize;
                if idx < buffer.len() {
                    if let Some(fw) = fill_width {
                        if col < x + fw {
                            buffer[idx] = fg;
                        } else {
                            buffer[idx] = bg;
                        }
                    } else {
                        buffer[idx] = bg;
                    }
                }
            }
        }
    }
}

fn draw_text(buffer: &mut [u32], screen_w: u32, text: &str, x: f32, y: f32, _size: f32) {
    let char_w = 8;
    let char_h = 12;
    let color = 0xFF_C0_C0_E0_u32;

    for (i, ch) in text.chars().enumerate() {
        let cx = x as i32 + (i as i32 * char_w);
        let cy = y as i32;
        for row in 0..char_h {
            for col in 0..char_w {
                if char_pixel(ch, col, row) {
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

fn char_pixel(ch: char, col: i32, row: i32) -> bool {
    let ch = ch.to_ascii_uppercase();
    match (ch, col, row) {
        ('A', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || r == 1 || r == 5) =>
        {
            true
        }
        ('B', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || r == 1 || r == 5 || r == 9) =>
        {
            true
        }
        ('C', c, r)
            if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || r == 1 || r == 9) =>
        {
            true
        }
        ('D', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || r == 1 || r == 9) =>
        {
            true
        }
        ('E', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || r == 1 || r == 5 || r == 9) =>
        {
            true
        }
        ('F', c, r)
            if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || r == 1 || r == 5) =>
        {
            true
        }
        ('G', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || r == 1 || r == 9 || (c >= 3 && r == 5) || (c == 5 && r >= 5)) =>
        {
            true
        }
        ('H', c, r)
            if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || c == 5 || r == 5) =>
        {
            true
        }
        ('I', c, _) if (2..=4).contains(&c) => true,
        ('K', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || (r >= 5 && c == r - 5 + 2) || (r <= 5 && c == 6 - r)) =>
        {
            true
        }
        ('L', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || r == 9) => true,
        ('M', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || (r <= 5 && (c == r || c == 6 - r))) =>
        {
            true
        }
        ('N', c, r)
            if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || c == 5 || c == r) =>
        {
            true
        }
        ('O', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || r == 1 || r == 9) =>
        {
            true
        }
        ('P', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || (r <= 5 && c == 5) || r == 1 || r == 5) =>
        {
            true
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
            true
        }
        ('S', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && ((r <= 5 && c == 1) || (r >= 5 && c == 5) || r == 1 || r == 5 || r == 9) =>
        {
            true
        }
        ('T', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (r == 1 || c == 3) => true,
        ('U', c, r)
            if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == 1 || c == 5 || r == 9) =>
        {
            true
        }
        ('W', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || (r >= 5 && (c == 3 || c == r - 2 || c == 8 - r))) =>
        {
            true
        }
        ('X', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (c == r || c == 6 - r) => {
            true
        }
        ('Y', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && ((r <= 5 && (c == r || c == 6 - r)) || (r >= 5 && c == 3)) =>
        {
            true
        }
        ('0', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || r == 1 || r == 9) =>
        {
            true
        }
        ('1', c, _) if c == 3 => true,
        ('2', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (r == 1 || r == 5 || r == 9 || (r <= 5 && c == 5) || (r >= 5 && c == 1)) =>
        {
            true
        }
        ('3', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (r == 1 || r == 5 || r == 9 || c == 5) =>
        {
            true
        }
        ('4', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 5 || r == 5 || (r <= 5 && c == 1)) =>
        {
            true
        }
        ('5', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (r == 1 || r == 5 || r == 9 || (r <= 5 && c == 1) || (r >= 5 && c == 5)) =>
        {
            true
        }
        ('6', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || r == 5 || r == 9 || (r >= 5 && c == 5)) =>
        {
            true
        }
        ('7', c, r) if (1..=5).contains(&c) && (1..=9).contains(&r) && (r == 1 || c == 5) => true,
        ('8', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 1 || c == 5 || r == 1 || r == 5 || r == 9) =>
        {
            true
        }
        ('9', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && (c == 5 || r == 1 || r == 5 || (r <= 5 && c == 1)) =>
        {
            true
        }
        ('.', _, 8) => true,
        ('%', c, r)
            if (1..=5).contains(&c)
                && (1..=9).contains(&r)
                && ((c == 1 && r <= 3) || (c == 5 && r >= 7) || c == r) =>
        {
            true
        }
        ('-', _, 5) => true,
        (':', 3, 3) | (':', 3, 7) => true,
        ('/', c, r) if c == 6 - r => true,
        (' ', _, _) => false,
        _ => true,
    }
}

fn draw_button(buffer: &mut [u32], screen_w: u32, x: i32, y: i32, w: i32, h: i32, label: &str) {
    let bg = 0xFF_50_50_70_u32;
    let border = 0xFF_80_80_A0_u32;
    for row in y..(y + h) {
        for col in x..(x + w) {
            if row >= 0 && col >= 0 && (col as u32) < screen_w {
                let idx = (row as u32 * screen_w + col as u32) as usize;
                if idx < buffer.len() {
                    let is_border = row == y || row == y + h - 1 || col == x || col == x + w - 1;
                    buffer[idx] = if is_border { border } else { bg };
                }
            }
        }
    }
    let text_x = x as f32 + (w as f32 - label.len() as f32 * 8.0) / 2.0;
    let text_y = y as f32 + (h as f32 - 12.0) / 2.0;
    draw_text(buffer, screen_w, label, text_x, text_y, 12.0);
}
