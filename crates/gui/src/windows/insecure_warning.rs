// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::Arc;

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
use wgpu_text::glyph_brush::OwnedSection;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

use shared::log;

use crate::draw::ui::button::{self, Button, ButtonStyle};
use crate::draw::ui::text;
use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};

// Logical (unscaled) layout of the dialog, shared by the window sizing and the painting
const MARGIN: f32 = 20.0;
const ICON_SIZE: f32 = 40.0;
const MESSAGE_FONT_SIZE: f32 = 14.0;
const MESSAGE_LINE_HEIGHT: f32 = MESSAGE_FONT_SIZE * 1.5;
const MESSAGE_TOP_GAP: f32 = 10.0;
const BUTTON_HEIGHT: f32 = 40.0;
const BUTTON_TOP_GAP: f32 = 15.0;
const WIDTH: f32 = 440.0;
const MIN_HEIGHT: f32 = 220.0;

// Red-ish tone: this is not an advisory warning, it is a "do not use this" one
const COLOR: [f32; 4] = [0.9, 0.25, 0.2, 1.0];

fn max_chars_per_line(width: f32, font_size: f32) -> usize {
    ((width - 2.0 * MARGIN) / (font_size * 0.55)) as usize
}

fn message_top(margin: f32, icon_size: f32, top_gap: f32) -> f32 {
    margin + icon_size + top_gap
}

fn scaled(value: f32) -> f32 {
    value * *monitor::SCALE_FACTOR as f32
}

fn height_for(line_count: usize) -> f32 {
    let height = message_top(MARGIN, ICON_SIZE, MESSAGE_TOP_GAP)
        + line_count as f32 * MESSAGE_LINE_HEIGHT
        + BUTTON_TOP_GAP
        + BUTTON_HEIGHT
        + MARGIN;

    height.max(MIN_HEIGHT)
}

fn danger_button_style() -> ButtonStyle {
    ButtonStyle {
        bg_color: [130, 35, 35, 255],
        border_color: [170, 55, 55, 255],
        hover_bg_color: [160, 45, 45, 255],
        hover_border_color: [210, 70, 70, 255],
        font_scale: monitor::scaled_val(15) as f32,
        radius: 8.0,
        ..Default::default()
    }
}

fn neutral_button_style() -> ButtonStyle {
    ButtonStyle {
        bg_color: [45, 45, 55, 255],
        border_color: [80, 80, 100, 255],
        hover_bg_color: [65, 65, 80, 255],
        hover_border_color: [120, 120, 150, 255],
        font_scale: monitor::scaled_val(15) as f32,
        radius: 8.0,
        ..Default::default()
    }
}

fn layout_buttons(ok_label: &str, cancel_label: &str, pw: f32, ph: f32) -> (Button, Button) {
    let scale = *monitor::SCALE_FACTOR as f32;
    let bw = monitor::scaled_val(120) as f32;
    let bh = scaled(BUTTON_HEIGHT);
    let by = ph - bh - scaled(MARGIN);
    // OK (danger) on the left, Cancel on the right, mirroring the YesNo popup
    let bx_ok = (pw / 2.0) - bw - 10.0 * scale;
    let bx_cancel = (pw / 2.0) + 10.0 * scale;

    (
        Button::new(
            bx_ok,
            by,
            bw as u32,
            bh as u32,
            ok_label.to_string(),
            danger_button_style(),
        ),
        Button::new(
            bx_cancel,
            by,
            bw as u32,
            bh as u32,
            cancel_label.to_string(),
            neutral_button_style(),
        ),
    )
}

struct WarningState<'a> {
    window: Arc<Window>,
    renderer: WgpuRenderer,
    message: &'a str,
    ok_btn: Button,
    cancel_btn: Button,
    phys_w: u32,
    phys_h: u32,
    scale: f32,
    last_mouse_pos: Option<(f32, f32)>,
}

impl<'a> WarningState<'a> {
    fn new(
        event_loop: &ActiveEventLoop,
        message: &'a str,
        ok_label: &str,
        cancel_label: &str,
    ) -> anyhow::Result<Self> {
        let (dw, dh) = monitor::size(0).unwrap_or((1920, 1080));
        let sf = monitor::scale(0) as f32;
        let line_count = text::lines(message, max_chars_per_line(WIDTH, MESSAGE_FONT_SIZE)).len();
        let ww = WIDTH;
        let wh = height_for(line_count).min(dh as f32 / sf);
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_visible(false)
                    .with_title("UDS Security Warning")
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_resizable(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
            )?,
        );
        let phys = window.inner_size();
        let scale = *monitor::SCALE_FACTOR as f32;
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
        let (ok_btn, cancel_btn) = layout_buttons(
            ok_label,
            cancel_label,
            phys.width as f32,
            phys.height as f32,
        );

        Ok(WarningState {
            window,
            renderer,
            message,
            ok_btn,
            cancel_btn,
            phys_w: phys.width,
            phys_h: phys.height,
            scale,
            last_mouse_pos: None,
        })
    }

    /// The surface is mapped after creation on Linux, so the first real size arrives here.
    fn resize(&mut self, ok_label: &str, cancel_label: &str, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.phys_w = width;
        self.phys_h = height;
        self.scale = *monitor::SCALE_FACTOR as f32;
        (self.ok_btn, self.cancel_btn) =
            layout_buttons(ok_label, cancel_label, width as f32, height as f32);
    }

    fn paint(&mut self) {
        let s = self.scale;
        let pw = self.phys_w;
        let ph = self.phys_h;

        self.renderer.reconfigure(pw, ph);

        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new();
        let mut sections: Vec<OwnedSection> = Vec::new();

        let mut panel_pixmap = Pixmap::new(pw, ph).unwrap();
        let rect = button::rounded_rect_path(2.0, 2.0, pw as f32 - 4.0, ph as f32 - 4.0, 10.0 * s);

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(30, 30, 35, 255));
        panel_pixmap.fill_path(
            &rect,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let stroke = Stroke {
            width: 2.0 * s,
            ..Default::default()
        };
        paint.set_color(Color::from_rgba(COLOR[0], COLOR[1], COLOR[2], 0.6).unwrap());
        panel_pixmap.stroke_path(&rect, &paint, &stroke, Transform::identity(), None);

        data.push(panel_pixmap.take());
        ov_descs.push((0, pw, ph, 0.0, 0.0));

        // Warning icon: red circle with an exclamation mark
        let icon_size_px = scaled(ICON_SIZE) as u32;
        let mut icon_pixmap = Pixmap::new(icon_size_px, icon_size_px).unwrap();
        let icon_center = icon_size_px as f32 / 2.0;
        let icon_radius = icon_center - 2.0 * s;

        let mut pb = PathBuilder::new();
        pb.push_circle(icon_center, icon_center, icon_radius);

        let circle_path = pb.finish().unwrap();
        let mut icon_paint = Paint::default();
        icon_paint.set_color(Color::from_rgba(COLOR[0], COLOR[1], COLOR[2], 0.15).unwrap());
        icon_pixmap.fill_path(
            &circle_path,
            &icon_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        icon_paint.set_color(Color::from_rgba(COLOR[0], COLOR[1], COLOR[2], 1.0).unwrap());
        icon_pixmap.stroke_path(
            &circle_path,
            &icon_paint,
            &stroke,
            Transform::identity(),
            None,
        );

        let mut sym_pb = PathBuilder::new();
        sym_pb.move_to(icon_center, icon_center - 10.0 * s);
        sym_pb.line_to(icon_center, icon_center + 2.0 * s);
        sym_pb.push_circle(icon_center, icon_center + 7.0 * s, 1.5 * s);
        if let Some(sym_path) = sym_pb.finish() {
            let sym_stroke = Stroke {
                width: 3.0 * s,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            icon_pixmap.stroke_path(
                &sym_path,
                &icon_paint,
                &sym_stroke,
                Transform::identity(),
                None,
            );
        }

        data.push(icon_pixmap.take());
        ov_descs.push((1, icon_size_px, icon_size_px, 20.0 * s, 20.0 * s));

        sections.extend(text::wrap(
            self.message,
            max_chars_per_line(WIDTH, MESSAGE_FONT_SIZE),
            scaled(MESSAGE_FONT_SIZE),
            [0.9, 0.9, 0.9, 1.0],
            scaled(MARGIN),
            scaled(message_top(MARGIN, ICON_SIZE, MESSAGE_TOP_GAP)),
            scaled(MESSAGE_LINE_HEIGHT),
        ));

        for btn in [&self.ok_btn, &self.cancel_btn] {
            let (btn_data, btn_text) = btn.render();
            data.push(btn_data);
            ov_descs.push((data.len() - 1, btn.w, btn.h, btn.x, btn.y));
            sections.push(btn_text);
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
            .update_and_render(&[], pw, ph, &overlays, &sections, None, None);
    }
}

/// Blocking, standalone warning dialog for insecure builds.
///
/// Shown by the launcher before anything else happens: the user is told that
/// this build does not verify TLS certificates and must explicitly accept it.
/// Returns `true` only if the OK button is pressed; Cancel, closing the
/// window, or any failure to show the dialog returns `false` (fail closed).
pub fn show_insecure_warning(message: &str, ok_label: &str, cancel_label: &str) -> bool {
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            log::error!("Cannot show insecure client warning: {e}");
            return false;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut state: Option<WarningState> = None;
    let mut handler = WarningHandler {
        message,
        ok_label,
        cancel_label,
        accepted: false,
        state: &mut state,
    };
    let _ = event_loop.run_app(&mut handler);
    handler.accepted
}

struct WarningHandler<'a> {
    message: &'a str,
    ok_label: &'a str,
    cancel_label: &'a str,
    accepted: bool,
    state: &'a mut Option<WarningState<'a>>,
}

impl ApplicationHandler for WarningHandler<'_> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        // Populate monitor info so SCALE_FACTOR reflects the actual DPI.
        // Without this, SCALE_FACTOR defaults to 1.0 and text is tiny on high-DPI.
        monitor::populate(el);
        match WarningState::new(el, self.message, self.ok_label, self.cancel_label) {
            Ok(s) => {
                s.window.set_visible(true);
                *self.state = Some(s);
            }
            Err(e) => {
                log::error!("Cannot create insecure client warning window: {e}");
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
        let Some(s) = self.state else { return };
        match event {
            WindowEvent::CloseRequested => {
                // Cancelled
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                s.paint();
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                if let Some(pos) = s.last_mouse_pos {
                    if s.ok_btn.contains(pos.0, pos.1) {
                        self.accepted = true;
                        el.exit();
                    } else if s.cancel_btn.contains(pos.0, pos.1) {
                        el.exit();
                    }
                }
            }
            WindowEvent::Resized(size) => {
                s.resize(self.ok_label, self.cancel_label, size.width, size.height);
                s.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = s.window.inner_size();
                s.resize(self.ok_label, self.cancel_label, size.width, size.height);
                s.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                s.last_mouse_pos = Some((position.x as f32, position.y as f32));
                let ok_hover = s
                    .ok_btn
                    .handle_mouse_move(position.x as f32, position.y as f32);
                let cancel_hover = s
                    .cancel_btn
                    .handle_mouse_move(position.x as f32, position.y as f32);
                if ok_hover || cancel_hover {
                    s.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(s) = self.state {
            s.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MESSAGE_LINE_HEIGHT, MIN_HEIGHT, height_for, max_chars_per_line};

    #[test]
    fn short_messages_keep_the_default_size() {
        assert_eq!(height_for(1), MIN_HEIGHT);
    }

    #[test]
    fn the_window_grows_with_the_message() {
        assert!(height_for(8) > height_for(4));
        assert_eq!(
            height_for(8) - height_for(4),
            4.0 * MESSAGE_LINE_HEIGHT,
            "each extra line adds exactly one line height"
        );
    }

    #[test]
    fn the_message_never_reaches_the_buttons() {
        for line_count in 1..20 {
            let message_bottom =
                super::message_top(super::MARGIN, super::ICON_SIZE, super::MESSAGE_TOP_GAP)
                    + line_count as f32 * MESSAGE_LINE_HEIGHT;
            let buttons_top = height_for(line_count) - super::BUTTON_HEIGHT - super::MARGIN;

            assert!(
                message_bottom <= buttons_top,
                "{} lines: message ends at {}, buttons start at {}",
                line_count,
                message_bottom,
                buttons_top
            );
        }
    }

    #[test]
    fn chars_per_line_shrinks_with_font() {
        assert!(max_chars_per_line(440.0, 14.0) > max_chars_per_line(440.0, 28.0));
    }
}
