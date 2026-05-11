// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

#[derive(Default)]
pub enum LauncherInner {
    #[default]
    Invisible,
    Test {
        buttons: Vec<(&'static str, TestAction)>,
        request: Option<TestAction>,
    },
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

#[derive(Clone)]
pub enum TestAction {
    ShowProgress,
    GoInvisible,
    ShowWarning,
    ShowError,
    ShowYesNo,
    ConnectRdp,
    ConnectRdpPreconnection,
    ConnectRail,
}

impl LauncherInner {
    pub fn new_test() -> Self {
        LauncherInner::Test {
            buttons: vec![
                ("RDP Connecting", TestAction::ConnectRdpPreconnection),
                ("RDP Connect", TestAction::ConnectRdp),
                ("RDP RAIL Notepad", TestAction::ConnectRail),
                ("Progress", TestAction::ShowProgress),
                ("Invisible", TestAction::GoInvisible),
                ("Warning", TestAction::ShowWarning),
                ("Error", TestAction::ShowError),
                ("Yes/No", TestAction::ShowYesNo),
            ],
            request: None,
        }
    }

    pub fn handle_click(&mut self, x: f32, y: f32) -> Option<TestAction> {
        match self {
            LauncherInner::Error(_) | LauncherInner::Warning(_) => {
                if y > 230.0 && y < 270.0 && x > 140.0 && x < 260.0 {
                    *self = LauncherInner::Invisible;
                }
                None
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
                None
            }
            LauncherInner::Test { buttons, request } => {
                let btn_h = 28.0;
                let btn_w = 260.0;
                let start_y = 42.0;
                let btn_x = 70.0;

                for (i, _) in buttons.iter().enumerate() {
                    let btn_y = start_y + i as f32 * (btn_h + 6.0);
                    if y >= btn_y && y <= btn_y + btn_h && x >= btn_x && x <= btn_x + btn_w {
                        *request = Some(buttons[i].1.clone());
                        return buttons[i].1.clone().into();
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn take_request(&mut self) -> Option<TestAction> {
        match self {
            LauncherInner::Test { request, .. } => request.take(),
            _ => None,
        }
    }
}

#[allow(dead_code)]
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
    pub phys_w: u32,
    pub phys_h: u32,
    pub scale_factor: f32,
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
            phys_w: 400,
            phys_h: 300,
            scale_factor: 1.0,
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

    let width = state.phys_w;
    let height = state.phys_h;
    let s = state.scale_factor; // Scale all logical coords → physical

    // Clear
    for pixel in buffer.as_mut().iter_mut() {
        *pixel = 0xFF_1A_1A_2E;
    }

    match &state.inner {
        LauncherInner::Invisible | LauncherInner::Test { .. } => {
            draw_logo(
                &mut buffer,
                width,
                height,
                &state.logo_rgba,
                state.logo_width,
                state.logo_height,
                s,
            );
        }
        LauncherInner::Progress { pct, message } => {
            draw_logo(
                &mut buffer,
                width,
                height,
                &state.logo_rgba,
                state.logo_width,
                state.logo_height,
                s,
            );
            draw_text(
                &mut buffer,
                width,
                &format!("{}%", pct),
                (180.0 * s) as i32,
                (200.0 * s) as i32,
                (14.0 * s) as i32,
            );
            draw_text(
                &mut buffer,
                width,
                message,
                (40.0 * s) as i32,
                (220.0 * s) as i32,
                (11.0 * s) as i32,
            );
            let bw = ((*pct as f32 / 100.0) * 320.0 * s) as i32;
            draw_rect(
                &mut buffer,
                width,
                (40.0 * s) as i32,
                (210.0 * s) as i32,
                (320.0 * s) as i32,
                (18.0 * s) as i32,
                0xFF_40_40_60,
                0xFF_60_C0_FF,
                Some(bw),
            );
        }
        LauncherInner::Error(msg) | LauncherInner::Warning(msg) => {
            let is_err = matches!(state.inner, LauncherInner::Error(_));
            draw_text(
                &mut buffer,
                width,
                if is_err { "ERROR" } else { "WARNING" },
                (140.0 * s) as i32,
                (50.0 * s) as i32,
                (14.0 * s) as i32,
            );
            draw_text(
                &mut buffer,
                width,
                msg,
                (20.0 * s) as i32,
                (100.0 * s) as i32,
                (12.0 * s) as i32,
            );
            draw_button(
                &mut buffer,
                width,
                (150.0 * s) as i32,
                (235.0 * s) as i32,
                (100.0 * s) as i32,
                (35.0 * s) as i32,
                "OK",
            );
        }
        LauncherInner::YesNo { message, .. } => {
            draw_text(
                &mut buffer,
                width,
                message,
                (20.0 * s) as i32,
                (80.0 * s) as i32,
                (12.0 * s) as i32,
            );
            draw_button(
                &mut buffer,
                width,
                (100.0 * s) as i32,
                (235.0 * s) as i32,
                (80.0 * s) as i32,
                (35.0 * s) as i32,
                "Yes",
            );
            draw_button(
                &mut buffer,
                width,
                (220.0 * s) as i32,
                (235.0 * s) as i32,
                (80.0 * s) as i32,
                (35.0 * s) as i32,
                "No",
            );
        }
    }

    // Draw test buttons if in Test mode
    if let LauncherInner::Test { buttons, .. } = &state.inner {
        let btn_h = (28.0 * s) as i32;
        let btn_w = (260.0 * s) as i32;
        let start_y = (42.0 * s) as i32;
        let btn_x = (70.0 * s) as i32;

        for (i, (label, _)) in buttons.iter().enumerate() {
            let y = start_y + i as i32 * (32.0 * s) as i32;
            draw_button(&mut buffer, width, btn_x, y, btn_w, btn_h, label);
        }
    }

    let _ = buffer.present();
}

// ── Drawing helpers ────────────────────────────────────────

fn draw_logo(
    buffer: &mut [u32],
    screen_w: u32,
    screen_h: u32,
    rgba: &[u8],
    logo_w: u32,
    logo_h: u32,
    scale: f32,
) {
    let x = (screen_w as i32 - (logo_w as f32 * scale) as i32) / 2;
    let y = (8.0 * scale) as i32;
    if x < 0 {
        return;
    }
    let scaled_w = (logo_w as f32 * scale) as u32;
    let scaled_h = (logo_h as f32 * scale) as u32;
    for row in 0..scaled_h {
        for col in 0..scaled_w {
            let px = x + col as i32;
            let py = y + row as i32;
            if px >= 0 && py >= 0 && (px as u32) < screen_w && (py as u32) < screen_h {
                // Sample source pixel (nearest neighbor)
                let src_col = ((col as f32 / scale) as u32).min(logo_w - 1);
                let src_row = ((row as f32 / scale) as u32).min(logo_h - 1);
                let si = (src_row * logo_w + src_col) as usize * 4;
                let di = (py as u32 * screen_w + px as u32) as usize;
                if si + 3 < rgba.len() && di < buffer.len() {
                    let r = rgba[si];
                    let g = rgba[si + 1];
                    let b = rgba[si + 2];
                    let a = rgba[si + 3];
                    if a > 0 {
                        let bg = buffer[di];
                        let bg_r = (bg >> 16) as u8;
                        let bg_g = (bg >> 8) as u8;
                        let bg_b = bg as u8;
                        let alpha = a as f32 / 255.0;
                        let br = (r as f32 * alpha + bg_r as f32 * (1.0 - alpha)) as u8;
                        let bg2 = (g as f32 * alpha + bg_g as f32 * (1.0 - alpha)) as u8;
                        let bb = (b as f32 * alpha + bg_b as f32 * (1.0 - alpha)) as u8;
                        buffer[di] = u32::from_ne_bytes([bb, bg2, br, 0xFF]);
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
                        buffer[idx] = if col < x + fw { fg } else { bg };
                    } else {
                        buffer[idx] = bg;
                    }
                }
            }
        }
    }
}

fn draw_text(buffer: &mut [u32], screen_w: u32, text: &str, x: i32, y: i32, size: i32) {
    let char_h = size.max(6);
    let char_w = char_h * 2 / 3; // 8:12 ratio → 2:3
    if char_w < 1 {
        return;
    }
    let color = 0xFF_C0_C0_E0_u32;
    for (i, ch) in text.chars().enumerate() {
        let cx = x + i as i32 * char_w;
        let cy = y;
        for row in 0..char_h {
            let src_row = (row * 12 / char_h).min(11);
            for col in 0..char_w {
                let src_col = (col * 8 / char_w).min(7);
                if char_pixel(ch, src_col, src_row) {
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
    // Compute text size from button height (12px base font, scaled to fit)
    let font_h = ((h as f32 * 0.55) as i32).max(6);
    let font_w = font_h * 2 / 3;
    let text_w = label.len() as i32 * font_w;
    let text_x = x + (w - text_w).max(0) / 2;
    let text_y = y + (h - font_h) / 2;
    draw_text(buffer, screen_w, label, text_x, text_y, font_h);
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
        ('1', 3, _) => true,
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
