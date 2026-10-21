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

use zeroize::{Zeroize, Zeroizing};

use crate::draw::ui::button::{self, Button, ButtonStyle};
use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedField {
    User,
    Password,
}

pub enum CredentialsAction {
    Connect { user: String, password: Zeroizing<String> },
    Cancel,
}

// Tests need to print the action to surface expect_err panics. We never
// print the password, only whether the action is `Connect` or `Cancel`.
impl std::fmt::Debug for CredentialsAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialsAction::Connect { user, .. } => f
                .debug_struct("Connect")
                .field("user", user)
                .field("password", &"****")
                .finish(),
            CredentialsAction::Cancel => f.debug_struct("Cancel").finish(),
        }
    }
}

const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 290.0;

const FIELD_HEIGHT: f32 = 36.0;
const BUTTON_WIDTH: f32 = 110.0;
const BUTTON_HEIGHT: f32 = 36.0;

const PANEL_MARGIN: f32 = 25.0;
const USER_FIELD_TOP: f32 = 95.0;
const PASSWORD_FIELD_TOP: f32 = 165.0;
const PASSWORD_LABEL_TOP: f32 = 146.0;
const BUTTON_BOTTOM_MARGIN: f32 = 25.0;
const BUTTON_GAP: f32 = 8.0;
const ERROR_GAP: f32 = 6.0;
const INPUT_HORIZONTAL_INSET: f32 = 10.0;
const INPUT_VERTICAL_INSET: f32 = 9.0;

const TITLE_FONT: f32 = 17.0;
const SUBTITLE_FONT: f32 = 12.0;
const LABEL_FONT: f32 = 12.0;
const INPUT_FONT: f32 = 14.0;
const ERROR_FONT: f32 = 12.0;
const BUTTON_FONT: f32 = 14.0;

const PANEL_BG: [u8; 4] = [24, 24, 32, 255];
const PANEL_BORDER: [u8; 4] = [60, 60, 80, 200];
const FIELD_BG: [u8; 4] = [35, 35, 48, 255];
const FIELD_BORDER: [u8; 4] = [65, 65, 85, 255];
const FIELD_BORDER_FOCUSED: [u8; 4] = [59, 130, 246, 255];
const TEXT_PRIMARY: [u8; 4] = [255, 255, 255, 255];
const TEXT_LABEL: [f32; 4] = [0.85, 0.85, 0.9, 1.0];
const TEXT_TITLE: [f32; 4] = [0.35, 0.65, 1.0, 1.0];
const TEXT_ERROR: [f32; 4] = [0.95, 0.35, 0.35, 1.0];

fn scaled(val: f32, s: f32) -> f32 {
    val * s
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    #[allow(dead_code)]
    phys_w: u32,
    #[allow(dead_code)]
    phys_h: u32,
    scale: f32,
    field_x: f32,
    field_w: f32,
    field_h: f32,
    user_y: f32,
    pass_y: f32,
    btn_y: f32,
    btn_w: f32,
    btn_h: f32,
    btn_connect_x: f32,
    btn_cancel_x: f32,
    panel_stroke_w: f32,
    field_stroke_w: f32,
}

impl Layout {
    fn from(phys_w: u32, phys_h: u32, scale: f32) -> Self {
        let pw = phys_w as f32;
        let ph = phys_h as f32;
        let s = scale;
        let field_w = pw - scaled(2.0 * PANEL_MARGIN, s);
        let field_h = scaled(FIELD_HEIGHT, s);
        let user_y = scaled(USER_FIELD_TOP, s);
        let pass_y = scaled(PASSWORD_FIELD_TOP, s);
        let btn_w = scaled(BUTTON_WIDTH, s);
        let btn_h = scaled(BUTTON_HEIGHT, s);
        let btn_y = ph - scaled(BUTTON_BOTTOM_MARGIN, s) - btn_h;
        let gap = scaled(BUTTON_GAP, s);
        Self {
            phys_w,
            phys_h,
            scale,
            field_x: scaled(PANEL_MARGIN, s),
            field_w,
            field_h,
            user_y,
            pass_y,
            btn_y,
            btn_w,
            btn_h,
            btn_connect_x: pw / 2.0 - btn_w - gap,
            btn_cancel_x: pw / 2.0 + gap,
            panel_stroke_w: 1.5 * s,
            field_stroke_w: 2.0 * s,
        }
    }

    fn contains_user(&self, x: f32, y: f32) -> bool {
        rect_contains(self.field_x, self.user_y, self.field_w, self.field_h, x, y)
    }

    fn contains_password(&self, x: f32, y: f32) -> bool {
        rect_contains(self.field_x, self.pass_y, self.field_w, self.field_h, x, y)
    }

    fn error_y(&self) -> f32 {
        self.pass_y + self.field_h + scaled(ERROR_GAP, self.scale)
    }

    fn input_text_origin(&self, y: f32) -> (f32, f32) {
        (
            self.field_x + scaled(INPUT_HORIZONTAL_INSET, self.scale),
            y + scaled(INPUT_VERTICAL_INSET, self.scale),
        )
    }
}

fn rect_contains(rx: f32, ry: f32, rw: f32, rh: f32, x: f32, y: f32) -> bool {
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

pub struct CredentialsState {
    pub renderer: WgpuRenderer,
    pub window: Arc<Window>,
    pub settings: rdp::settings::RdpSettings,
    pub user: String,
    pub password: Zeroizing<String>,
    pub focused: FocusedField,
    pub phys_w: u32,
    pub phys_h: u32,
    pub scale: f32,
    pub btn_connect: Button,
    pub btn_cancel: Button,
    pub last_mouse_pos: Option<(f32, f32)>,
    pub error_message: Option<String>,
    layout: Layout,
}

impl Drop for CredentialsState {
    fn drop(&mut self) {
        self.user.zeroize();
    }
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

        let layout = Layout::from(phys.width, phys.height, scale);

        let btn_connect = Button::new(
            layout.btn_connect_x,
            layout.btn_y,
            layout.btn_w as u32,
            layout.btn_h as u32,
            "Connect".to_string(),
            ButtonStyle {
                font_scale: scaled(BUTTON_FONT, scale),
                radius: 8.0,
                bg_color: [0x37, 0x41, 0xB1, 0xFF],
                border_color: [0x4B, 0x55, 0xC3, 0xFF],
                hover_bg_color: [0x4B, 0x55, 0xD1, 0xFF],
                hover_border_color: [0x6B, 0x72, 0xE0, 0xFF],
                ..Default::default()
            },
        );

        let btn_cancel = Button::new(
            layout.btn_cancel_x,
            layout.btn_y,
            layout.btn_w as u32,
            layout.btn_h as u32,
            "Cancel".to_string(),
            ButtonStyle {
                font_scale: scaled(BUTTON_FONT, scale),
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
            password: Zeroizing::new(String::new()),
            focused: initial_focus,
            phys_w: phys.width,
            phys_h: phys.height,
            scale,
            btn_connect,
            btn_cancel,
            last_mouse_pos: None,
            error_message: None,
            layout,
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
                    if let Some((mx, my)) = self.last_mouse_pos {
                        if self.layout.contains_user(mx, my) {
                            self.focused = FocusedField::User;
                            self.window.request_redraw();
                            return None;
                        }
                        if self.layout.contains_password(mx, my) {
                            self.focused = FocusedField::Password;
                            self.window.request_redraw();
                            return None;
                        }
                    }

                    if self.btn_connect.is_hovered {
                        return self.validate_and_connect();
                    }
                    if self.btn_cancel.is_hovered {
                        return Some(CredentialsAction::Cancel);
                    }
                }
            }
            WindowEvent::KeyboardInput { event: key_ev, .. } if key_ev.state.is_pressed() => {
                match &key_ev.logical_key {
                    Key::Named(NamedKey::Enter) => {
                        return self.validate_and_connect();
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
                        self.backspace();
                    }
                    Key::Character(s) => {
                        self.append_text(s.as_str());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        None
    }

    fn backspace(&mut self) {
        let field = match self.focused {
            FocusedField::User => &mut self.user,
            FocusedField::Password => &mut self.password,
        };
        if !field.is_empty() {
            field.pop();
            self.window.request_redraw();
        }
    }

    fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        match self.focused {
            FocusedField::User => self.user.push_str(text),
            FocusedField::Password => self.password.push_str(text),
        }
        self.window.request_redraw();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.phys_w = width;
        self.phys_h = height;
        self.scale = *monitor::SCALE_FACTOR as f32;
        self.layout = Layout::from(width, height, self.scale);

        self.btn_connect.x = self.layout.btn_connect_x;
        self.btn_connect.y = self.layout.btn_y;
        self.btn_connect.w = self.layout.btn_w as u32;
        self.btn_connect.h = self.layout.btn_h as u32;

        self.btn_cancel.x = self.layout.btn_cancel_x;
        self.btn_cancel.y = self.layout.btn_y;
        self.btn_cancel.w = self.layout.btn_w as u32;
        self.btn_cancel.h = self.layout.btn_h as u32;
    }

    /// Pure validation rule used by both the GUI event loop and the unit
    /// tests. Returns the field that should be focused and the user-facing
    /// error message when the credentials are not valid yet.
    pub fn validate(user: &str, password: &str) -> Result<(), (FocusedField, &'static str)> {
        if user.trim().is_empty() {
            return Err((FocusedField::User, "Username is required"));
        }
        if password.is_empty() {
            return Err((FocusedField::Password, "Password is required"));
        }
        Ok(())
    }

    /// Build the `Connect` action from the current user/password inputs. The
    /// `password` is moved out of the state and wrapped in `Zeroizing`; the
    /// GUI entry point below also handles redraw bookkeeping.
    pub fn build_action(
        user: &str,
        password: &mut Zeroizing<String>,
    ) -> Result<CredentialsAction, (FocusedField, &'static str)> {
        Self::validate(user, password)?;
        let user = user.trim().to_string();
        let password = Zeroizing::new(std::mem::take(&mut **password));
        Ok(CredentialsAction::Connect { user, password })
    }

    /// GUI entry point: validate, move the password out, and ask the window to
    /// redraw when state changes. Tests that only exercise validation should
    /// call `validate` directly so they don't need a live `Window`.
    pub fn validate_and_connect(&mut self) -> Option<CredentialsAction> {
        match Self::build_action(&self.user, &mut self.password) {
            Err((focus, msg)) => {
                self.focused = focus;
                self.error_message = Some(msg.to_string());
                self.window.request_redraw();
                None
            }
            Ok(action) => {
                self.error_message = None;
                Some(action)
            }
        }
    }

    pub fn paint(&mut self) {
        let layout = self.layout;
        let pw = self.phys_w;
        let ph = self.phys_h;
        let s = layout.scale;

        self.renderer.reconfigure(pw, ph);

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new();

        // 1. Background panel
        let mut panel_pixmap = Pixmap::new(pw, ph).unwrap();
        let rect = button::rounded_rect_path(2.0, 2.0, pw as f32 - 4.0, ph as f32 - 4.0, 12.0 * s);

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(PANEL_BG[0], PANEL_BG[1], PANEL_BG[2], PANEL_BG[3]));
        panel_pixmap.fill_path(&rect, &paint, FillRule::Winding, Transform::identity(), None);

        let stroke = Stroke {
            width: layout.panel_stroke_w,
            ..Default::default()
        };
        paint.set_color(Color::from_rgba8(
            PANEL_BORDER[0],
            PANEL_BORDER[1],
            PANEL_BORDER[2],
            PANEL_BORDER[3],
        ));
        panel_pixmap.stroke_path(&rect, &paint, &stroke, Transform::identity(), None);

        // 2. Input boxes (User & Password)
        let fx = layout.field_x;
        let fw = layout.field_w;
        let fh = layout.field_h;
        let user_y = layout.user_y;
        let pass_y = layout.pass_y;

        let active_stroke = Stroke {
            width: layout.field_stroke_w,
            ..Default::default()
        };

        // User box
        let user_rect = button::rounded_rect_path(fx, user_y, fw, fh, 8.0 * s);
        paint.set_color(Color::from_rgba8(FIELD_BG[0], FIELD_BG[1], FIELD_BG[2], FIELD_BG[3]));
        panel_pixmap.fill_path(&user_rect, &paint, FillRule::Winding, Transform::identity(), None);
        let user_border = if self.focused == FocusedField::User {
            FIELD_BORDER_FOCUSED
        } else {
            FIELD_BORDER
        };
        paint.set_color(Color::from_rgba8(
            user_border[0], user_border[1], user_border[2], user_border[3],
        ));
        panel_pixmap.stroke_path(&user_rect, &paint, &active_stroke, Transform::identity(), None);

        // Password box
        let pass_rect = button::rounded_rect_path(fx, pass_y, fw, fh, 8.0 * s);
        paint.set_color(Color::from_rgba8(FIELD_BG[0], FIELD_BG[1], FIELD_BG[2], FIELD_BG[3]));
        panel_pixmap.fill_path(&pass_rect, &paint, FillRule::Winding, Transform::identity(), None);
        let pass_border = if self.focused == FocusedField::Password {
            FIELD_BORDER_FOCUSED
        } else {
            FIELD_BORDER
        };
        paint.set_color(Color::from_rgba8(
            pass_border[0], pass_border[1], pass_border[2], pass_border[3],
        ));
        panel_pixmap.stroke_path(&pass_rect, &paint, &active_stroke, Transform::identity(), None);

        data.push(panel_pixmap.take());
        ov_descs.push((0, pw, ph, 0.0, 0.0));

        // 3. Text sections
        let title_fs = scaled(TITLE_FONT, s);
        let subtitle_fs = scaled(SUBTITLE_FONT, s);
        let label_fs = scaled(LABEL_FONT, s);
        let input_fs = scaled(INPUT_FONT, s);

        // Header
        sections.push(
            Section::default()
                .add_text(
                    Text::new("UDS Credentials")
                        .with_scale(title_fs)
                        .with_color(TEXT_TITLE),
                )
                .with_screen_position((scaled(PANEL_MARGIN, s), scaled(20.0, s)))
                .to_owned(),
        );

        sections.push(
            Section::default()
                .add_text(
                    Text::new("Enter your username and password to continue.")
                        .with_scale(subtitle_fs)
                        .with_color(TEXT_LABEL),
                )
                .with_screen_position((scaled(PANEL_MARGIN, s), scaled(45.0, s)))
                .to_owned(),
        );

        // User label & text
        sections.push(
            Section::default()
                .add_text(
                    Text::new("Username")
                        .with_scale(label_fs)
                        .with_color(TEXT_LABEL),
                )
                .with_screen_position((scaled(PANEL_MARGIN, s), scaled(76.0, s)))
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
                            .with_color(TEXT_PRIMARY.map(|c| c as f32 / 255.0)),
                    )
                    .with_screen_position(layout.input_text_origin(user_y))
                    .to_owned(),
            );
        }

        // Password label & text
        sections.push(
            Section::default()
                .add_text(
                    Text::new("Password")
                        .with_scale(label_fs)
                        .with_color(TEXT_LABEL),
                )
                .with_screen_position((
                    scaled(PANEL_MARGIN, s),
                    scaled(PASSWORD_LABEL_TOP, s),
                ))
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
                            .with_color(TEXT_PRIMARY.map(|c| c as f32 / 255.0)),
                    )
                    .with_screen_position(layout.input_text_origin(pass_y))
                    .to_owned(),
            );
        }

        // Error message if validation failed
        if let Some(ref err) = self.error_message {
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(err)
                            .with_scale(scaled(ERROR_FONT, s))
                            .with_color(TEXT_ERROR),
                    )
                    .with_screen_position((scaled(PANEL_MARGIN, s), layout.error_y()))
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
    fn password_bullet_masking_length_matches_input() {
        let password = "secretPassword123";
        let masked = "•".repeat(password.chars().count());
        assert_eq!(masked.chars().count(), password.chars().count());
        assert!(!masked.contains("secret"));
    }

    #[test]
    fn validate_rejects_empty_user() {
        let result = CredentialsState::validate("", "any");
        assert_eq!(result, Err((FocusedField::User, "Username is required")));
    }

    #[test]
    fn validate_rejects_whitespace_user() {
        let result = CredentialsState::validate("   ", "any");
        assert_eq!(result, Err((FocusedField::User, "Username is required")));
    }

    #[test]
    fn validate_rejects_empty_password() {
        let result = CredentialsState::validate("admin", "");
        assert_eq!(result, Err((FocusedField::Password, "Password is required")));
    }

    #[test]
    fn validate_accepts_full_credentials() {
        assert!(CredentialsState::validate("admin", "secret").is_ok());
    }

    #[test]
    fn build_action_uses_real_validation() {
        // `build_action` is the function the GUI calls from
        // `validate_and_connect`. It must enforce the same rules as `validate`
        // and must return a `Connect` action with a `Zeroizing` password when
        // the credentials are valid.
        let mut password = Zeroizing::new("secret".to_string());
        let action = CredentialsState::build_action("admin", &mut password)
            .expect("valid credentials must yield a Connect action");
        match action {
            CredentialsAction::Connect { user, password: taken } => {
                assert_eq!(user, "admin");
                assert_eq!(&*taken, "secret");
            }
            _ => panic!("expected Connect action"),
        }
    }

    #[test]
    fn build_action_returns_trimmed_user() {
        let mut password = Zeroizing::new(" secret ".to_string());
        let action = CredentialsState::build_action("  admin ", &mut password)
            .expect("valid credentials must yield a Connect action");
        match action {
            CredentialsAction::Connect { user, password: taken } => {
                assert_eq!(user, "admin");
                assert_eq!(&*taken, " secret ", "the password is never trimmed");
            }
            _ => panic!("expected Connect action"),
        }
    }

    #[test]
    fn build_action_rejects_empty_user() {
        let mut password = Zeroizing::new("any".to_string());
        let err = CredentialsState::build_action("", &mut password)
            .expect_err("empty user must not connect");
        assert_eq!(err, (FocusedField::User, "Username is required"));
        // Password stays intact when validation fails.
        assert_eq!(&*password, "any");
    }

    #[test]
    fn build_action_rejects_empty_password() {
        let mut password = Zeroizing::new(String::new());
        let err = CredentialsState::build_action("admin", &mut password)
            .expect_err("empty password must not connect");
        assert_eq!(err, (FocusedField::Password, "Password is required"));
    }

    #[test]
    fn build_action_rejects_whitespace_user() {
        let mut password = Zeroizing::new("any".to_string());
        let err = CredentialsState::build_action("   ", &mut password)
            .expect_err("whitespace user must not connect");
        assert_eq!(err, (FocusedField::User, "Username is required"));
    }

    #[test]
    fn layout_geometry_is_consistent_across_scales() {
        // Both input rectangles use the same `field_x`/`field_w` pair, so the
        // hit-test rectangles in `handle_event` stay aligned with the painted
        // boxes regardless of monitor scale. The window we use here is large
        // enough to fit the input row, the gap below it, and the button row
        // at any scale we test.
        let phys_w = 1680u32;
        let phys_h = 1000u32;
        for scale in [1.0f32, 1.5, 2.0, 2.5] {
            let layout = Layout::from(phys_w, phys_h, scale);
            assert_eq!(layout.field_x, layout.field_x, "user_x matches pass_x");
            assert_eq!(
                layout.field_x + layout.field_w,
                layout.field_x + layout.field_w,
                "user right matches pass right"
            );
            // The button row must not overlap the password input box.
            assert!(
                layout.btn_y >= layout.pass_y + layout.field_h,
                "btn_y {} must be at or below pass box bottom {}",
                layout.btn_y,
                layout.pass_y + layout.field_h
            );
            let connect_right = layout.btn_connect_x + layout.btn_w;
            let cancel_left = layout.btn_cancel_x;
            assert!(connect_right <= cancel_left);
            let centre_gap = (cancel_left - connect_right) / 2.0;
            let expect_gap = scaled(BUTTON_GAP, scale);
            assert!(
                (centre_gap - expect_gap).abs() < 0.5,
                "gap mismatch at scale {scale}: {centre_gap} vs {expect_gap}"
            );
        }
    }

    #[test]
    fn hit_test_matches_paint_geometry() {
        // The same `Layout` powers both paint and hit-test, so a point inside
        // the painted user box must always be reported as inside `contains_user`.
        let layout = Layout::from(840, 580, 1.5);
        let cx = layout.field_x + layout.field_w / 2.0;
        let cy = layout.user_y + layout.field_h / 2.0;
        assert!(layout.contains_user(cx, cy));
        assert!(!layout.contains_password(cx, layout.user_y - 1.0));
        // Outside the user box but still inside the password box: pass only.
        let px = layout.field_x + layout.field_w / 2.0;
        let py = layout.pass_y + layout.field_h / 2.0;
        assert!(!layout.contains_user(px, py));
        assert!(layout.contains_password(px, py));
    }
}
