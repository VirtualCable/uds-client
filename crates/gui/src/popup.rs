use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};
use crate::draw::ui::{button::{self, ButtonStyle}, text};
use tiny_skia::{Color, Paint, Pixmap, Stroke, Transform, PathBuilder, FillRule};

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
    pub hover_idx: Option<usize>,
    pub last_mouse_pos: Option<(f32, f32)>,
}

impl PopupState {
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        kind: PopupKind,
    ) -> anyhow::Result<Self> {
        let (dw, dh) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let ww = 400.0;
        let wh = 200.0;
        let sf = crate::monitor::scale(0) as f32;
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            event_loop.create_window(
                winit::window::Window::default_attributes()
                    .with_title("UDS Alert")
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_resizable(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
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
            hover_idx: None,
            last_mouse_pos: None,
        })
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32) -> bool {
        let old_hover = self.hover_idx;
        self.hover_idx = None;

        let s = self.scale;
        let px = x * s;
        let py = y * s;

        let bh = 40.0 * s;
        let by = self.phys_h as f32 - bh - 20.0 * s;

        match &self.kind {
            PopupKind::YesNo { .. } => {
                let bw = 100.0 * s;
                let bx_yes = (self.phys_w as f32 / 2.0) - bw - 10.0 * s;
                let bx_no = (self.phys_w as f32 / 2.0) + 10.0 * s;

                if py >= by && py <= by + bh {
                    if px >= bx_yes && px <= bx_yes + bw {
                        self.hover_idx = Some(0);
                    } else if px >= bx_no && px <= bx_no + bw {
                        self.hover_idx = Some(1);
                    }
                }
            }
            PopupKind::Warning(_) | PopupKind::Error(_) => {
                let bw = 120.0 * s;
                let bx = self.phys_w as f32 / 2.0 - bw / 2.0;
                if py >= by && py <= by + bh && px >= bx && px <= bx + bw {
                    self.hover_idx = Some(0);
                }
            }
        }
        self.hover_idx != old_hover
    }

    pub fn handle_click(&mut self, x: f32, y: f32) -> bool {
        let s = self.scale;
        let px = x * s;
        let py = y * s;

        let bh = 40.0 * s;
        let by = self.phys_h as f32 - bh - 20.0 * s;

        match &self.kind {
            PopupKind::YesNo { response, .. } => {
                let bw_yn = 100.0 * s;
                let bx_yes = (self.phys_w as f32 / 2.0) - bw_yn - 10.0 * s;
                let bx_no = (self.phys_w as f32 / 2.0) + 10.0 * s;

                if py >= by && py <= by + bh {
                    if px >= bx_yes && px <= bx_yes + bw_yn {
                        if let Some(tx) = response.write().unwrap().take() {
                            let _ = tx.send(true);
                        }
                        return true;
                    }
                    if px >= bx_no && px <= bx_no + bw_yn {
                        if let Some(tx) = response.write().unwrap().take() {
                            let _ = tx.send(false);
                        }
                        return true;
                    }
                }
            }
            PopupKind::Warning(_) | PopupKind::Error(_) => {
                let bw_ok = 120.0 * s;
                let bx_ok = self.phys_w as f32 / 2.0 - bw_ok / 2.0;
                if py >= by && py <= by + bh && px >= bx_ok && px <= bx_ok + bw_ok {
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

        // Ensure renderer is configured for this size (fixes text alignment)
        self.renderer.reconfigure(pw, ph);

        let (title, message, is_yesno, color) = match &self.kind {
            PopupKind::Error(msg) => ("ERROR", msg.as_str(), false, [0.9, 0.2, 0.2, 1.0]),
            PopupKind::Warning(msg) => ("WARNING", msg.as_str(), false, [1.0, 0.7, 0.1, 1.0]),
            PopupKind::YesNo { message, .. } => ("CONFIRM", message.as_str(), true, [0.2, 0.6, 1.0, 1.0]),
        };

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new();

        // 1. Draw Background Panel
        let mut panel_pixmap = Pixmap::new(pw, ph).unwrap();
        let rect = button::rounded_rect_path(2.0, 2.0, pw as f32 - 4.0, ph as f32 - 4.0, 10.0 * s);
        
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(30, 30, 35, 255));
        panel_pixmap.fill_path(&rect, &paint, FillRule::Winding, Transform::identity(), None);
        
        let stroke = Stroke {
            width: 2.0 * s,
            ..Default::default()
        };
        paint.set_color(Color::from_rgba(color[0], color[1], color[2], 0.6).unwrap());
        panel_pixmap.stroke_path(&rect, &paint, &stroke, Transform::identity(), None);
        
        data.push(panel_pixmap.take());
        ov_descs.push((0, pw, ph, 0.0, 0.0));

        // 2. Draw Icon Circle and Symbol
        let icon_size_px = monitor::scaled_val(40) as u32;
        let mut icon_pixmap = Pixmap::new(icon_size_px, icon_size_px).unwrap();
        let icon_center = icon_size_px as f32 / 2.0;
        let icon_radius = icon_center - 2.0 * s;
        
        let mut pb = PathBuilder::new();
        pb.push_circle(icon_center, icon_center, icon_radius);
        let icon_path = pb.finish().unwrap();
        
        paint.set_color(Color::from_rgba(color[0], color[1], color[2], 0.15).unwrap());
        icon_pixmap.fill_path(&icon_path, &paint, FillRule::Winding, Transform::identity(), None);
        paint.set_color(Color::from_rgba(color[0], color[1], color[2], 1.0).unwrap());
        icon_pixmap.stroke_path(&icon_path, &paint, &stroke, Transform::identity(), None);
        
        // Symbol inside icon
        let mut sym_pb = PathBuilder::new();
        if is_yesno {
            // Draw a bold '?'
            sym_pb.move_to(icon_center - 4.0 * s, icon_center - 6.0 * s);
            sym_pb.cubic_to(icon_center - 4.0 * s, icon_center - 12.0 * s, icon_center + 6.0 * s, icon_center - 12.0 * s, icon_center + 6.0 * s, icon_center - 6.0 * s);
            sym_pb.cubic_to(icon_center + 6.0 * s, icon_center - 2.0 * s, icon_center, icon_center - 2.0 * s, icon_center, icon_center + 2.0 * s);
            sym_pb.move_to(icon_center, icon_center + 6.0 * s);
            sym_pb.push_circle(icon_center, icon_center + 7.0 * s, 1.5 * s);
        } else {
            // Draw a bold '!'
            sym_pb.move_to(icon_center, icon_center - 10.0 * s);
            sym_pb.line_to(icon_center, icon_center + 2.0 * s);
            sym_pb.move_to(icon_center, icon_center + 6.0 * s);
            sym_pb.push_circle(icon_center, icon_center + 7.0 * s, 1.5 * s);
        }
        if let Some(sym_path) = sym_pb.finish() {
            let sym_stroke = Stroke {
                width: 3.0 * s,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            icon_pixmap.stroke_path(&sym_path, &paint, &sym_stroke, Transform::identity(), None);
        }

        data.push(icon_pixmap.take());
        ov_descs.push((1, icon_size_px, icon_size_px, 20.0 * s, 20.0 * s));

        // 3. Texts
        let title_fs = monitor::scaled_val(18) as f32;
        sections.push(
            Section::default()
                .add_text(Text::new(title).with_scale(title_fs).with_color(color))
                .with_screen_position((20.0 * s + icon_size_px as f32 + 15.0 * s, 25.0 * s))
                .to_owned(),
        );

        let msg_fs = monitor::scaled_val(14) as f32;
        let msg_x = 20.0 * s;
        let msg_y = 20.0 * s + icon_size_px as f32 + 10.0 * s;
        let max_chars = ((pw as f32 - 40.0 * s) / (msg_fs * 0.55)) as usize;
        sections.extend(text::wrap(
            message,
            max_chars,
            msg_fs,
            [0.9, 0.9, 0.9, 1.0],
            msg_x,
            msg_y,
            msg_fs * 1.5,
        ));

        // 4. Buttons
        let h_idx = self.hover_idx;
        let bh = monitor::scaled_val(40) as u32;
        let by = ph as f32 - bh as f32 - 20.0 * s;

        if is_yesno {
            let bw = monitor::scaled_val(100) as u32;
            let bx1 = (pw as f32 / 2.0) - bw as f32 - 10.0 * s;
            let style1 = ButtonStyle {
                font_scale: monitor::scaled_val(15) as f32,
                bg_color: if h_idx == Some(0) { [65, 65, 80, 255] } else { [45, 45, 55, 255] },
                border_color: if h_idx == Some(0) { [120, 120, 150, 255] } else { [80, 80, 100, 255] },
                radius: 8.0,
                ..ButtonStyle::default()
            };
            let (yes_data, yes_text) = button::render(bx1, by, bw, bh, "YES", &style1);
            data.push(yes_data);
            ov_descs.push((data.len() - 1, bw, bh, bx1, by));
            sections.push(yes_text);

            let bx2 = (pw as f32 / 2.0) + 10.0 * s;
            let mut style2 = style1;
            style2.bg_color = if h_idx == Some(1) { [65, 65, 80, 255] } else { [45, 45, 55, 255] };
            style2.border_color = if h_idx == Some(1) { [120, 120, 150, 255] } else { [80, 80, 100, 255] };
            
            let (no_data, no_text) = button::render(bx2, by, bw, bh, "NO", &style2);
            data.push(no_data);
            ov_descs.push((data.len() - 1, bw, bh, bx2, by));
            sections.push(no_text);
        } else {
            let bw = monitor::scaled_val(120) as u32;
            let bx = pw as f32 / 2.0 - bw as f32 / 2.0;
            let style = ButtonStyle {
                font_scale: monitor::scaled_val(15) as f32,
                bg_color: if h_idx == Some(0) { [65, 65, 80, 255] } else { [45, 45, 55, 255] },
                border_color: if h_idx == Some(0) { [120, 120, 150, 255] } else { [80, 80, 100, 255] },
                radius: 8.0,
                ..ButtonStyle::default()
            };
            let (ok_data, ok_text) = button::render(bx, by, bw, bh, "GOT IT", &style);
            data.push(ok_data);
            ov_descs.push((data.len() - 1, bw, bh, bx, by));
            sections.push(ok_text);
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

        self.renderer.update_and_render(&[], pw, ph, &overlays, &sections, None);
    }
}
