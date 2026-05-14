use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::sync::oneshot;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

use crate::draw::ui::{button::{self, ButtonStyle}, progress, text};

#[derive(Default)]
pub enum LauncherInner {
    #[default]
    Invisible,
    Test {
        buttons: Vec<(&'static str, LaunchAction)>,
        request: Option<LaunchAction>,
    },
    Progress {
        pct: u8,
        message: String,
        start: Instant,
        progress_duration_secs: u32,
        phase: ProgressPhase,
        auto_animate: bool,
    },
    Error(String),
    Warning(String),
    YesNo {
        message: String,
        response: Arc<RwLock<Option<oneshot::Sender<bool>>>>,
    },
}
#[derive(Default, PartialEq)]
pub enum ProgressPhase {
    #[default]
    Connecting,
    Connected,
}

#[derive(Clone)]
pub enum LaunchAction {
    ShowProgress,
    GoInvisible,
    ShowWarning,
    ShowError,
    ShowYesNo,
    ConnectRdp,
    ConnectRail,
}

impl LauncherInner {
    pub fn new_test() -> Self {
        LauncherInner::Test {
            buttons: vec![
                ("RDP Connect", LaunchAction::ConnectRdp),
                ("RDP RAIL Notepad", LaunchAction::ConnectRail),
                ("Progress", LaunchAction::ShowProgress),
                ("Invisible", LaunchAction::GoInvisible),
                ("Warning", LaunchAction::ShowWarning),
                ("Error", LaunchAction::ShowError),
                ("Yes/No", LaunchAction::ShowYesNo),
            ],
            request: None,
        }
    }
    pub fn handle_click(&mut self, x: f32, y: f32) -> Option<LaunchAction> {
        match self {
            LauncherInner::Error(_) | LauncherInner::Warning(_) => {
                if y > 230.0 && y < 270.0 && x > 140.0 && x < 260.0 {
                    *self = LauncherInner::Invisible;
                }
                None
            }
            LauncherInner::YesNo { response, .. } => {
                let r = response.clone();
                let mut c = false;
                if y > 230.0 && y < 270.0 && x > 100.0 && x < 180.0 {
                    if let Some(t) = r.write().unwrap().take() {
                        let _ = t.send(true);
                    }
                    c = true;
                }
                if !c && y > 230.0 && y < 270.0 && x > 220.0 && x < 300.0 {
                    if let Some(t) = r.write().unwrap().take() {
                        let _ = t.send(false);
                    }
                    c = true;
                }
                if c {
                    *self = LauncherInner::Invisible;
                }
                None
            }
            LauncherInner::Test { buttons, request } => {
                for (i, _) in buttons.iter().enumerate() {
                    let by = 42.0 + i as f32 * 34.0;
                    if y >= by && y <= by + 28.0 && (70.0..=330.0).contains(&x) {
                        *request = Some(buttons[i].1.clone());
                        return buttons[i].1.clone().into();
                    }
                }
                None
            }
            _ => None,
        }
    }
    pub fn take_request(&mut self) -> Option<LaunchAction> {
        match self {
            LauncherInner::Test { request, .. } => request.take(),
            _ => None,
        }
    }
}

pub struct TestingLauncherState {
    pub window: Option<Arc<winit::window::Window>>,
    pub renderer: Option<WgpuRenderer>,
    pub inner: LauncherInner,
    pub last_mouse_pos: Option<(f32, f32)>,
}

fn test_button_style() -> ButtonStyle {
    ButtonStyle {
        font_scale: monitor::scaled_val(14) as f32,
        ..ButtonStyle::default()
    }
}

fn dialog_button_style() -> ButtonStyle {
    ButtonStyle {
        font_scale: monitor::scaled_val(14) as f32,
        ..ButtonStyle::default()
    }
}

pub fn paint_launcher(state: &mut TestingLauncherState) {
    let renderer = match &mut state.renderer {
        Some(r) => r,
        None => return,
    };
    let logo = crate::logo::load_logo();
    let phys = state
        .window
        .as_ref()
        .map(|w| w.inner_size())
        .unwrap_or(winit::dpi::PhysicalSize::new(400, 300));
    let pw = phys.width;
    let ph = phys.height;
    let s = *monitor::SCALE_FACTOR as f32;
    let warn_color = [1.0f32, 0.5, 0.5, 1.0];
    let mut sections: Vec<OwnedSection> = Vec::new();
    let mut data: Vec<Vec<u8>> = Vec::new();
    let logo_idx = data.len();
    data.push(logo.rgba);

    struct OvDesc {
        data_idx: usize,
        w: u32,
        h: u32,
        x: f32,
        y: f32,
        scale: f32,
    }
    let mut ov_descs: Vec<OvDesc> = Vec::new();

    // Logo
    ov_descs.push(OvDesc {
        data_idx: logo_idx,
        w: logo.width,
        h: logo.height,
        x: (pw as f32 - logo.width as f32 * s) / 2.0,
        y: (8.0 * s).min(ph as f32 - logo.height as f32 * s),
        scale: s,
    });

    match &state.inner {
        LauncherInner::Invisible => {}
        LauncherInner::Test { .. } => {}
        LauncherInner::Progress { pct, message, start, progress_duration_secs, phase, auto_animate } => {
            let pct = if *auto_animate {
                let elapsed = start.elapsed().as_secs_f32();
                let total = *progress_duration_secs as f32;
                (elapsed / total * 100.0).min(100.0)
            } else {
                *pct as f32
            };
            let message = if *auto_animate {
                match phase {
                    ProgressPhase::Connecting => "Connecting to RDP server...",
                    ProgressPhase::Connected => "Connected.",
                }
            } else {
                message.as_str()
            };

            // Percentage above the bar
            let fs = monitor::scaled_val(18) as f32;
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(&format!("{}%", pct as u8))
                            .with_scale(fs)
                            .with_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .with_screen_position((pw as f32 / 2.0 - 30.0 * s, 160.0 * s))
                    .to_owned(),
            );

            // Progress bar
            let bw = monitor::scaled_val(280) as u32;
            let bh = monitor::scaled_val(16) as u32;
            let bx = (pw as f32 - bw as f32) / 2.0;
            let by = 190.0 * s;
            let i = data.len();
            data.push(progress::render(pct, bw, bh));
            ov_descs.push(OvDesc { data_idx: i, w: bw, h: bh, x: bx, y: by, scale: 1.0 });

            // Status message below the bar
            let msg_fs = monitor::scaled_val(11) as f32;
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(message)
                            .with_scale(msg_fs)
                            .with_color([0.8, 0.8, 1.0, 1.0]),
                    )
                    .with_screen_position((bx, by + bh as f32 + 6.0 * s))
                    .to_owned(),
            );
        }
        LauncherInner::Error(msg) | LauncherInner::Warning(msg) => {
            let title = if matches!(state.inner, LauncherInner::Error(_)) {
                "ERROR"
            } else {
                "WARNING"
            };
            let fs = monitor::scaled_val(14) as f32;
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(title)
                            .with_scale(fs)
                            .with_color(warn_color),
                    )
                    .with_screen_position((140.0 * s, 40.0 * s))
                    .to_owned(),
            );
            // Multiline wrapped message
            let msg_fs = monitor::scaled_val(12) as f32;
            let max_chars = (300.0 / (msg_fs * 0.5)) as usize;
            sections.extend(text::wrap(
                msg, max_chars, msg_fs, [1.0, 1.0, 1.0, 1.0],
                20.0 * s, 80.0 * s, msg_fs * 1.4,
            ));
            let style = dialog_button_style();
            let bw = monitor::scaled_val(100) as u32;
            let bh = monitor::scaled_val(35) as u32;
            let bx = 150.0 * s;
            let by = 235.0 * s;
            let (btn_data, btn_text) = button::render(bx, by, bw, bh, "OK", &style);
            let i = data.len();
            data.push(btn_data);
            ov_descs.push(OvDesc {
                data_idx: i,
                w: bw,
                h: bh,
                x: bx,
                y: by,
                scale: 1.0,
            });
            sections.push(btn_text);
        }
        LauncherInner::YesNo { message, .. } => {
            let msg_fs = monitor::scaled_val(12) as f32;
            let max_chars = (300.0 / (msg_fs * 0.5)) as usize;
            sections.extend(text::wrap(
                message, max_chars, msg_fs, [1.0, 1.0, 1.0, 1.0],
                20.0 * s, 70.0 * s, msg_fs * 1.4,
            ));
            let style = dialog_button_style();
            let bw = monitor::scaled_val(80) as u32;
            let bh = monitor::scaled_val(35) as u32;
            let y = 235.0 * s;

            let bx1 = 100.0 * s;
            let (yes_data, yes_text) = button::render(bx1, y, bw, bh, "Yes", &style);
            let i = data.len();
            data.push(yes_data);
            ov_descs.push(OvDesc {
                data_idx: i,
                w: bw,
                h: bh,
                x: bx1,
                y,
                scale: 1.0,
            });
            sections.push(yes_text);

            let bx2 = 220.0 * s;
            let (no_data, no_text) = button::render(bx2, y, bw, bh, "No", &style);
            let i = data.len();
            data.push(no_data);
            ov_descs.push(OvDesc {
                data_idx: i,
                w: bw,
                h: bh,
                x: bx2,
                y,
                scale: 1.0,
            });
            sections.push(no_text);
        }
    }

    // Test buttons
    if let LauncherInner::Test { buttons, .. } = &state.inner {
        let bh = monitor::scaled_val(28) as u32;
        let bw = monitor::scaled_val(260) as u32;
        let sy = 42.0 * s;
        let bx = 70.0 * s;
        for (i, (label, _)) in buttons.iter().enumerate() {
            let y = sy + i as f32 * (bh as f32 + 6.0);
            let (btn_data, btn_text) = button::render(bx, y, bw, bh, label, &test_button_style());
            let di = data.len();
            data.push(btn_data);
            ov_descs.push(OvDesc { data_idx: di, w: bw, h: bh, x: bx, y, scale: 1.0 });
            sections.push(btn_text);
        }
    }

    // Phase 2: build overlays from stable data
    let mut overlays = Vec::with_capacity(ov_descs.len());
    for d in &ov_descs {
        overlays.push(OverlayParams {
            rgba: &data[d.data_idx],
            width: d.w,
            height: d.h,
            x: d.x,
            y: d.y,
            scale: d.scale,
        });
    }

    renderer.update_and_render(&[], pw, ph, &overlays, &sections, None);
}
