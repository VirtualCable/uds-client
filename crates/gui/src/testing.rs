use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

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
    pub fn take_request(&mut self) -> Option<TestAction> {
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

fn rect_rgba(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        v.extend_from_slice(&[r, g, b, a]);
    }
    v
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
    let white = [1.0f32, 1.0, 1.0, 1.0];
    let warn = [1.0, 0.5, 0.5, 1.0];
    let mut sections: Vec<OwnedSection> = Vec::new();

    // Phase 1: build all RGBA data into a stable Vec
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
        LauncherInner::Progress { pct, message } => {
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(&format!("{}%", pct))
                            .with_scale(monitor::scaled_val(16) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((pw as f32 / 2.0 - 40.0 * s, 200.0 * s))
                    .to_owned(),
            );
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(message.as_str())
                            .with_scale(monitor::scaled_val(11) as f32)
                            .with_color([0.8, 0.8, 1.0, 1.0]),
                    )
                    .with_screen_position((40.0 * s, 220.0 * s))
                    .to_owned(),
            );
            let bw = monitor::scaled_val(320) as u32;
            let bh = monitor::scaled_val(18) as u32;
            let fw = ((*pct as f32 / 100.0) * bw as f32) as u32;
            if fw > 0 {
                let i = data.len();
                data.push(rect_rgba(fw, bh, 0x60, 0xC0, 0xFF, 0xFF));
                ov_descs.push(OvDesc {
                    data_idx: i,
                    w: fw,
                    h: bh,
                    x: 40.0 * s,
                    y: 210.0 * s,
                    scale: 1.0,
                });
            }
            let i = data.len();
            data.push(rect_rgba(bw, bh, 0x40, 0x40, 0x60, 0xFF));
            ov_descs.push(OvDesc {
                data_idx: i,
                w: bw,
                h: bh,
                x: 40.0 * s,
                y: 210.0 * s,
                scale: 1.0,
            });
        }
        LauncherInner::Error(msg) | LauncherInner::Warning(msg) => {
            let title = if matches!(state.inner, LauncherInner::Error(_)) {
                "ERROR"
            } else {
                "WARNING"
            };
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(title)
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(warn),
                    )
                    .with_screen_position((140.0 * s, 50.0 * s))
                    .to_owned(),
            );
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(msg.as_str())
                            .with_scale(monitor::scaled_val(12) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((20.0 * s, 100.0 * s))
                    .to_owned(),
            );
            let i = data.len();
            data.push(rect_rgba(
                monitor::scaled_val(100) as u32,
                monitor::scaled_val(35) as u32,
                0x50,
                0x50,
                0x70,
                0xFF,
            ));
            ov_descs.push(OvDesc {
                data_idx: i,
                w: monitor::scaled_val(100) as u32,
                h: monitor::scaled_val(35) as u32,
                x: 150.0 * s,
                y: 235.0 * s,
                scale: 1.0,
            });
            sections.push(
                Section::default()
                    .add_text(
                        Text::new("OK")
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((180.0 * s, 240.0 * s))
                    .to_owned(),
            );
        }
        LauncherInner::YesNo { message, .. } => {
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(message.as_str())
                            .with_scale(monitor::scaled_val(12) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((20.0 * s, 80.0 * s))
                    .to_owned(),
            );
            let i = data.len();
            data.push(rect_rgba(
                monitor::scaled_val(80) as u32,
                monitor::scaled_val(35) as u32,
                0x50,
                0x50,
                0x70,
                0xFF,
            ));
            ov_descs.push(OvDesc {
                data_idx: i,
                w: monitor::scaled_val(80) as u32,
                h: monitor::scaled_val(35) as u32,
                x: 100.0 * s,
                y: 235.0 * s,
                scale: 1.0,
            });
            let i = data.len();
            data.push(rect_rgba(
                monitor::scaled_val(80) as u32,
                monitor::scaled_val(35) as u32,
                0x50,
                0x50,
                0x70,
                0xFF,
            ));
            ov_descs.push(OvDesc {
                data_idx: i,
                w: monitor::scaled_val(80) as u32,
                h: monitor::scaled_val(35) as u32,
                x: 220.0 * s,
                y: 235.0 * s,
                scale: 1.0,
            });
            sections.push(
                Section::default()
                    .add_text(
                        Text::new("Yes")
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((120.0 * s, 240.0 * s))
                    .to_owned(),
            );
            sections.push(
                Section::default()
                    .add_text(
                        Text::new("No")
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((240.0 * s, 240.0 * s))
                    .to_owned(),
            );
        }
    }
    if let LauncherInner::Test { buttons, .. } = &state.inner {
        let bh = monitor::scaled_val(28) as u32;
        let bw = monitor::scaled_val(260) as u32;
        let sy = 42.0 * s;
        let bx = 70.0 * s;
        for (i, (label, _)) in buttons.iter().enumerate() {
            let y = sy + i as f32 * (bh as f32 + 6.0);
            let di = data.len();
            data.push(rect_rgba(bw, bh, 0x50, 0x50, 0x70, 0xFF));
            ov_descs.push(OvDesc {
                data_idx: di,
                w: bw,
                h: bh,
                x: bx,
                y,
                scale: 1.0,
            });
            sections.push(
                Section::default()
                    .add_text(
                        Text::new(label)
                            .with_scale(monitor::scaled_val(14) as f32)
                            .with_color(white),
                    )
                    .with_screen_position((bx + 8.0 * s, y + 4.0 * s))
                    .to_owned(),
            );
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
