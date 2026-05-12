use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

pub enum PopupKind {
    YesNo {
        message: String,
        response: Arc<RwLock<Option<oneshot::Sender<bool>>>>,
    },
    Warning(String),
    Error(String),
}

pub struct PopupState {
    pub window: Arc<winit::window::Window>,
    pub renderer: WgpuRenderer,
    pub kind: PopupKind,
    pub phys_w: u32,
    pub phys_h: u32,
    pub scale: f32,
}

impl PopupState {
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        kind: PopupKind,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                winit::window::Window::default_attributes()
                    .with_inner_size(winit::dpi::LogicalSize::new(380.0, 180.0))
                    .with_resizable(false),
            )?,
        );
        let phys = window.inner_size();
        let scale = *monitor::SCALE_FACTOR as f32;
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
        Ok(PopupState {
            window,
            renderer,
            kind,
            phys_w: phys.width,
            phys_h: phys.height,
            scale,
        })
    }

    pub fn handle_click(&mut self, x: f32, y: f32) -> bool {
        // Returns true if the popup should close
        let s = self.scale;
        let ok_btn_x = self.phys_w as f32 / 2.0 - 50.0 * s;
        let ok_btn_y = 120.0 * s;
        let ok_w = 100.0 * s;
        let ok_h = 35.0 * s;

        match &self.kind {
            PopupKind::YesNo { response, .. } => {
                let yes_x = 70.0 * s;
                let no_x = 210.0 * s;
                if y >= ok_btn_y && y <= ok_btn_y + ok_h {
                    if x >= yes_x && x <= yes_x + 80.0 * s {
                        if let Some(tx) = response.write().unwrap().take() {
                            let _ = tx.send(true);
                        }
                        return true;
                    }
                    if x >= no_x && x <= no_x + 80.0 * s {
                        if let Some(tx) = response.write().unwrap().take() {
                            let _ = tx.send(false);
                        }
                        return true;
                    }
                }
            }
            PopupKind::Warning(_) | PopupKind::Error(_) => {
                if y >= ok_btn_y && y <= ok_btn_y + ok_h && x >= ok_btn_x && x <= ok_btn_x + ok_w {
                    return true;
                }
            }
        }
        false
    }

    pub fn paint(&mut self) {
        let s = self.scale;
        let pw = self.phys_w;
        let ph = self.phys_h;
        let white = [1.0f32, 1.0, 1.0, 1.0];

        let (title, message, is_yesno) = match &self.kind {
            PopupKind::Error(msg) => ("Error", msg.as_str(), false),
            PopupKind::Warning(msg) => ("Warning", msg.as_str(), false),
            PopupKind::YesNo { message, .. } => ("Question", message.as_str(), true),
        };

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new(); // (data_idx, w, h, x, y)

        // Title
        sections.push(
            Section::default()
                .add_text(
                    Text::new(title)
                        .with_scale(monitor::scaled_val(16) as f32)
                        .with_color(white),
                )
                .with_screen_position((pw as f32 / 2.0 - 40.0 * s, 12.0 * s))
                .to_owned(),
        );
        // Message (simple, no multiline for now)
        sections.push(
            Section::default()
                .add_text(
                    Text::new(message)
                        .with_scale(monitor::scaled_val(13) as f32)
                        .with_color(white),
                )
                .with_screen_position((20.0 * s, 50.0 * s))
                .to_owned(),
        );

        if is_yesno {
            // Yes button
            let bw = monitor::scaled_val(80) as u32;
            let bh = monitor::scaled_val(35) as u32;
            let mut yes_bg = Vec::with_capacity((bw * bh * 4) as usize);
            for _ in 0..bw * bh {
                yes_bg.extend_from_slice(&[0x50, 0x50, 0x70, 0xFF]);
            }
            data.push(yes_bg);
            let yi = data.len() - 1;
            ov_descs.push((yi, bw, bh, 70.0 * s, 120.0 * s));
            sections.push(
                Section::default()
                    .add_text(
                        Text::new("Yes")
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((90.0 * s, 126.0 * s))
                    .to_owned(),
            );
            // No button
            let mut no_bg = Vec::with_capacity((bw * bh * 4) as usize);
            for _ in 0..bw * bh {
                no_bg.extend_from_slice(&[0x50, 0x50, 0x70, 0xFF]);
            }
            data.push(no_bg);
            let ni = data.len() - 1;
            ov_descs.push((ni, bw, bh, 210.0 * s, 120.0 * s));
            sections.push(
                Section::default()
                    .add_text(
                        Text::new("No")
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((230.0 * s, 126.0 * s))
                    .to_owned(),
            );
        } else {
            // OK button
            let bw = monitor::scaled_val(100) as u32;
            let bh = monitor::scaled_val(35) as u32;
            let mut ok_bg = Vec::with_capacity((bw * bh * 4) as usize);
            for _ in 0..bw * bh {
                ok_bg.extend_from_slice(&[0x50, 0x50, 0x70, 0xFF]);
            }
            data.push(ok_bg);
            let oi = data.len() - 1;
            let ox = pw as f32 / 2.0 - bw as f32 / 2.0;
            ov_descs.push((oi, bw, bh, ox, 120.0 * s));
            sections.push(
                Section::default()
                    .add_text(
                        Text::new("OK")
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((ox + 35.0 * s, 126.0 * s))
                    .to_owned(),
            );
        }

        let mut overlays = Vec::with_capacity(ov_descs.len());
        for (di, w, h, x, y) in &ov_descs {
            overlays.push(OverlayParams {
                rgba: &data[*di],
                width: *w,
                height: *h,
                x: *x,
                y: *y,
                scale: 1.0,
            });
        }

        self.renderer
            .update_and_render(&[], pw, ph, &overlays, &sections, None);
    }
}
