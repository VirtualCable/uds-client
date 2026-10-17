// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Author: Andres Schumann, aschumann at virtualcable dot es

use std::sync::Arc;
use tiny_skia::{Color, FillRule, Paint, Pixmap, Stroke, Transform};
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

use crate::draw::ui::button::{self, Button, ButtonStyle};
use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedField {
    User,
    Password,
}

pub enum CredentialsAction {
    Connect { user: String, password: String },
    Cancel,
}

const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 290.0;

const FIELD_HEIGHT: f32 = 36.0;
const BUTTON_WIDTH: f32 = 110.0;
const BUTTON_HEIGHT: f32 = 36.0;

fn scaled(val: f32, s: f32) -> f32 {
    val * s
}

pub struct CredentialsState {
    pub renderer: WgpuRenderer,
    pub window: Arc<Window>,
    pub settings: rdp::settings::RdpSettings,
    pub user: String,
    pub password: String,
    pub focused: FocusedField,
    pub phys_w: u32,
    pub phys_h: u32,
    pub scale: f32,
    pub btn_connect: Button,
    pub btn_cancel: Button,
    pub last_mouse_pos: Option<(f32, f32)>,
}

impl CredentialsState {
    pub fn new(
        event_loop: &ActiveEventLoop,
        settings: rdp::settings::RdpSettings,
    ) -> anyhow::Result<Self> {
        let (dw, dh) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let sf = crate::monitor::scale(0) as f32;
        let ww = WIDTH;
        let wh = HEIGHT;
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            event_loop.create_window(
                winit::window::Window::default_attributes()
                    .with_visible(false)
                    .with_title("UDS Credentials")
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_resizable(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
            )?,
        );

        let phys = window.inner_size();
        let scale = *monitor::SCALE_FACTOR as f32;
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;

        let initial_user = settings.user.clone();
        let initial_focus = if initial_user.is_empty() {
            FocusedField::User
        } else {
            FocusedField::Password
        };

        let pw = phys.width as f32;
        let ph = phys.height as f32;
        let s = scale;

        let bw = scaled(BUTTON_WIDTH, s);
        let bh = scaled(BUTTON_HEIGHT, s);
        let by = ph - scaled(25.0, s) - bh;
        let bx_connect = pw / 2.0 - bw - scaled(8.0, s);
        let bx_cancel = pw / 2.0 + scaled(8.0, s);

        let btn_connect = Button::new(
            bx_connect,
            by,
            bw as u32,
            bh as u32,
            "Connect".to_string(),
            ButtonStyle {
                font_scale: scaled(14.0, s),
                radius: 8.0,
                bg_color: [0x25, 0x63, 0xEB, 0xFF],
                border_color: [0x3B, 0x82, 0xF6, 0xFF],
                hover_bg_color: [0x1D, 0x4E, 0xD8, 0xFF],
                hover_border_color: [0x60, 0xA5, 0xFA, 0xFF],
                ..Default::default()
            },
        );

        let btn_cancel = Button::new(
            bx_cancel,
            by,
            bw as u32,
            bh as u32,
            "Cancel".to_string(),
            ButtonStyle {
                font_scale: scaled(14.0, s),
                radius: 8.0,
                bg_color: [0x37, 0x41, 0x51, 0xFF],
                border_color: [0x4B, 0x55, 0x63, 0xFF],
                hover_bg_color: [0x4B, 0x55, 0x63, 0xFF],
                hover_border_color: [0x6B, 0x72, 0x80, 0xFF],
                ..Default::default()
            },
        );

        Ok(Self {
            renderer,
            window,
            settings,
            user: initial_user,
            password: String::new(),
            focused: initial_focus,
            phys_w: phys.width,
            phys_h: phys.height,
            scale,
            btn_connect,
            btn_cancel,
            last_mouse_pos: None,
        })
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> Option<CredentialsAction> {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                self.last_mouse_pos = Some((px, py));
                let c_hover = self.btn_connect.handle_mouse_move(px, py);
                let x_hover = self.btn_cancel.handle_mouse_move(px, py);
                if c_hover || x_hover {
                    self.window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Left && state.is_pressed() {
                    let s = self.scale;
                    let fx = scaled(25.0, s);
                    let fw = self.phys_w as f32 - scaled(50.0, s);
                    let fh = scaled(FIELD_HEIGHT, s);
                    let user_y = scaled(95.0, s);
                    let pass_y = scaled(165.0, s);

                    if let Some((mx, my)) = self.last_mouse_pos {
                        if mx >= fx && mx <= fx + fw && my >= user_y && my <= user_y + fh {
                            self.focused = FocusedField::User;
                            self.window.request_redraw();
                            return None;
                        }
                        if mx >= fx && mx <= fx + fw && my >= pass_y && my <= pass_y + fh {
                            self.focused = FocusedField::Password;
                            self.window.request_redraw();
                            return None;
                        }
                    }

                    if self.btn_connect.is_hovered {
                        return Some(CredentialsAction::Connect {
                            user: self.user.clone(),
                            password: self.password.clone(),
                        });
                    }
                    if self.btn_cancel.is_hovered {
                        return Some(CredentialsAction::Cancel);
                    }
                }
            }
            WindowEvent::KeyboardInput { event: key_ev, .. } => {
                if key_ev.state.is_pressed() {
                    match &key_ev.logical_key {
                        Key::Named(NamedKey::Enter) => {
                            return Some(CredentialsAction::Connect {
                                user: self.user.clone(),
                                password: self.password.clone(),
                            });
                        }
                        Key::Named(NamedKey::Escape) => {
                            return Some(CredentialsAction::Cancel);
                        }
                        Key::Named(NamedKey::Tab) => {
                            self.focused = match self.focused {
                                FocusedField::User => FocusedField::Password,
                                FocusedField::Password => FocusedField::User,
                            };
                            self.window.request_redraw();
                        }
                        Key::Named(NamedKey::Backspace) => {
                            match self.focused {
                                FocusedField::User => {
                                    self.user.pop();
                                }
                                FocusedField::Password => {
                                    self.password.pop();
                                }
                            }
                            self.window.request_redraw();
                        }
                        Key::Character(s) => {
                            for c in s.chars() {
                                if !c.is_control() {
                                    match self.focused {
                                        FocusedField::User => self.user.push(c),
                                        FocusedField::Password => self.password.push(c),
                                    }
                                }
                            }
                            self.window.request_redraw();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        None
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.phys_w = width;
        self.phys_h = height;
        self.scale = *monitor::SCALE_FACTOR as f32;

        let pw = width as f32;
        let ph = height as f32;
        let s = self.scale;
        let bw = scaled(BUTTON_WIDTH, s);
        let bh = scaled(BUTTON_HEIGHT, s);
        let by = ph - scaled(25.0, s) - bh;

        self.btn_connect.x = pw / 2.0 - bw - scaled(8.0, s);
        self.btn_connect.y = by;
        self.btn_connect.w = bw as u32;
        self.btn_connect.h = bh as u32;

        self.btn_cancel.x = pw / 2.0 + scaled(8.0, s);
        self.btn_cancel.y = by;
        self.btn_cancel.w = bw as u32;
        self.btn_cancel.h = bh as u32;
    }

    pub fn paint(&mut self) {
        let s = self.scale;
        let pw = self.phys_w;
        let ph = self.phys_h;

        self.renderer.reconfigure(pw, ph);

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new();

        // 1. Background panel
        let mut panel_pixmap = Pixmap::new(pw, ph).unwrap();
        let rect = button::rounded_rect_path(2.0, 2.0, pw as f32 - 4.0, ph as f32 - 4.0, 12.0 * s);

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(24, 24, 32, 255));
        panel_pixmap.fill_path(&rect, &paint, FillRule::Winding, Transform::identity(), None);

        let stroke = Stroke {
            width: 1.5 * s,
            ..Default::default()
        };
        paint.set_color(Color::from_rgba8(60, 60, 80, 200));
        panel_pixmap.stroke_path(&rect, &paint, &stroke, Transform::identity(), None);

        // 2. Input boxes (User & Password)
        let fx = scaled(25.0, s);
        let fw = pw as f32 - scaled(50.0, s);
        let fh = scaled(FIELD_HEIGHT, s);
        let user_y = scaled(95.0, s);
        let pass_y = scaled(165.0, s);

        let active_stroke = Stroke {
            width: 2.0 * s,
            ..Default::default()
        };

        // User box
        let user_rect = button::rounded_rect_path(fx, user_y, fw, fh, 8.0 * s);
        paint.set_color(Color::from_rgba8(35, 35, 48, 255));
        panel_pixmap.fill_path(&user_rect, &paint, FillRule::Winding, Transform::identity(), None);

        if self.focused == FocusedField::User {
            paint.set_color(Color::from_rgba8(59, 130, 246, 255));
            panel_pixmap.stroke_path(&user_rect, &paint, &active_stroke, Transform::identity(), None);
        } else {
            paint.set_color(Color::from_rgba8(65, 65, 85, 255));
            panel_pixmap.stroke_path(&user_rect, &paint, &stroke, Transform::identity(), None);
        }

        // Password box
        let pass_rect = button::rounded_rect_path(fx, pass_y, fw, fh, 8.0 * s);
        paint.set_color(Color::from_rgba8(35, 35, 48, 255));
        panel_pixmap.fill_path(&pass_rect, &paint, FillRule::Winding, Transform::identity(), None);

        if self.focused == FocusedField::Password {
            paint.set_color(Color::from_rgba8(59, 130, 246, 255));
            panel_pixmap.stroke_path(&pass_rect, &paint, &active_stroke, Transform::identity(), None);
        } else {
            paint.set_color(Color::from_rgba8(65, 65, 85, 255));
            panel_pixmap.stroke_path(&pass_rect, &paint, &stroke, Transform::identity(), None);
        }

        data.push(panel_pixmap.take());
        ov_descs.push((0, pw, ph, 0.0, 0.0));

        // 3. Text sections
        let title_fs = scaled(17.0, s);
        let subtitle_fs = scaled(12.0, s);
        let label_fs = scaled(12.0, s);
        let input_fs = scaled(14.0, s);

        // Header
        sections.push(
            Section::default()
                .add_text(
                    Text::new("UDS Credentials")
                        .with_scale(title_fs)
                        .with_color([0.35, 0.65, 1.0, 1.0]),
                )
                .with_screen_position((scaled(25.0, s), scaled(20.0, s)))
                .to_owned(),
        );

        let server_text = format!("Connect to {}", self.settings.server);
        sections.push(
            Section::default()
                .add_text(
                    Text::new(&server_text)
                        .with_scale(subtitle_fs)
                        .with_color([0.7, 0.7, 0.8, 1.0]),
                )
                .with_screen_position((scaled(25.0, s), scaled(46.0, s)))
                .to_owned(),
        );

        // User label & text
        sections.push(
            Section::default()
                .add_text(
                    Text::new("Username")
                        .with_scale(label_fs)
                        .with_color([0.85, 0.85, 0.9, 1.0]),
                )
                .with_screen_position((scaled(25.0, s), scaled(76.0, s)))
                .to_owned(),
        );

        let user_display = if self.user.is_empty() && self.focused != FocusedField::User {
            "".to_string()
        } else if self.focused == FocusedField::User {
            format!("{}|", self.user)
        } else {
            self.user.clone()
        };

        if !user_display.is_empty() {
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(&user_display)
                            .with_scale(input_fs)
                            .with_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .with_screen_position((scaled(35.0, s), user_y + scaled(9.0, s)))
                    .to_owned(),
            );
        }

        // Password label & text
        sections.push(
            Section::default()
                .add_text(
                    Text::new("Password")
                        .with_scale(label_fs)
                        .with_color([0.85, 0.85, 0.9, 1.0]),
                )
                .with_screen_position((scaled(25.0, s), scaled(146.0, s)))
                .to_owned(),
        );

        let bullets = "•".repeat(self.password.chars().count());
        let pass_display = if self.password.is_empty() && self.focused != FocusedField::Password {
            "".to_string()
        } else if self.focused == FocusedField::Password {
            format!("{}|", bullets)
        } else {
            bullets
        };

        if !pass_display.is_empty() {
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(&pass_display)
                            .with_scale(input_fs)
                            .with_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .with_screen_position((scaled(35.0, s), pass_y + scaled(9.0, s)))
                    .to_owned(),
            );
        }

        // 4. Buttons
        for btn in [&self.btn_connect, &self.btn_cancel] {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focused_field_toggle() {
        let mut focus = FocusedField::User;
        focus = match focus {
            FocusedField::User => FocusedField::Password,
            FocusedField::Password => FocusedField::User,
        };
        assert_eq!(focus, FocusedField::Password);

        focus = match focus {
            FocusedField::User => FocusedField::Password,
            FocusedField::Password => FocusedField::User,
        };
        assert_eq!(focus, FocusedField::User);
    }

    #[test]
    fn test_password_bullet_masking() {
        let password = "secretPassword123";
        let masked = "•".repeat(password.chars().count());
        assert_eq!(masked.chars().count(), 17);
        assert!(!masked.contains("secret"));
    }
}
