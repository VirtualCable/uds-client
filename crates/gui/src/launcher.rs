use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::Arc;
use wgpu_text::glyph_brush::OwnedSection;

#[cfg(feature = "test-ui")]
use crate::draw::ui::button::{self, ButtonStyle};

#[derive(Default)]
#[allow(dead_code)]
pub enum LauncherInner {
    #[default]
    Invisible,
    #[cfg(feature = "test-ui")]
    Test {
        buttons: Vec<(&'static str, LaunchAction)>,
        request: Option<LaunchAction>,
        hover_idx: Option<usize>,
    },
}

#[derive(Default, PartialEq, Debug)]
pub enum ProgressPhase {
    #[default]
    Connecting,
    Connected,
}

#[cfg(feature = "test-ui")]
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
    #[cfg(feature = "test-ui")]
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
            hover_idx: None,
        }
    }

    pub fn handle_mouse_move(&mut self, logical_x: f32, logical_y: f32) -> bool {
        match self {
            #[cfg(feature = "test-ui")]
            LauncherInner::Test { buttons, hover_idx, .. } => {
                let old_hover = *hover_idx;
                *hover_idx = None;
                let s = *crate::monitor::SCALE_FACTOR as f32;
                let x = logical_x * s;
                let y = logical_y * s;
                let bh = crate::monitor::scaled_val(28) as f32;
                let bw = crate::monitor::scaled_val(260) as f32;
                let sy = 42.0 * s;
                let bx = 70.0 * s;
                
                for (i, _) in buttons.iter().enumerate() {
                    let by = sy + i as f32 * (bh + 6.0 * s);
                    if y >= by && y <= by + bh && x >= bx && x <= bx + bw {
                        *hover_idx = Some(i);
                        break;
                    }
                }
                *hover_idx != old_hover
            }
            _ => {
                let _ = (logical_x, logical_y);
                false
            }
        }
    }

    pub fn handle_click(&mut self, logical_x: f32, logical_y: f32) {
        match self {
            #[cfg(feature = "test-ui")]
            LauncherInner::Test { buttons, request, .. } => {
                let s = *crate::monitor::SCALE_FACTOR as f32;
                let x = logical_x * s;
                let y = logical_y * s;
                let bh = crate::monitor::scaled_val(28) as f32;
                let bw = crate::monitor::scaled_val(260) as f32;
                let sy = 42.0 * s;
                let bx = 70.0 * s;
                
                for (i, _) in buttons.iter().enumerate() {
                    let by = sy + i as f32 * (bh + 6.0 * s);
                    if y >= by && y <= by + bh && x >= bx && x <= bx + bw {
                        *request = Some(buttons[i].1.clone());
                        break;
                    }
                }
            }
            _ => {
                let _ = (logical_x, logical_y);
            }
        }
    }
    #[cfg(feature = "test-ui")]
    pub fn take_request(&mut self) -> Option<LaunchAction> {
        match self {
            LauncherInner::Test { request, .. } => request.take(),
            _ => None,
        }
    }
}

pub struct Wave {
    pub y_base: f32,
    pub amplitude: f32,
    pub speed: f32,
    pub offset: f32,
    pub thickness: f32,
    pub opacity: f32,
}

pub struct TestingLauncherState {
    pub window: Option<Arc<winit::window::Window>>,
    pub renderer: Option<WgpuRenderer>,
    pub inner: LauncherInner,
    pub last_mouse_pos: Option<(f32, f32)>,
}

impl Default for TestingLauncherState {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            inner: LauncherInner::Invisible,
            last_mouse_pos: None,
        }
    }
}

#[cfg(feature = "test-ui")]
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

    // Ensure renderer is configured for this size (fixes text alignment)
    renderer.reconfigure(pw, ph);

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

    match &state.inner {
        LauncherInner::Invisible => {}
        #[cfg(feature = "test-ui")]
        LauncherInner::Test { buttons, hover_idx, .. } => {
            let bh = monitor::scaled_val(28) as u32;
            let bw = monitor::scaled_val(260) as u32;
            let sy = 42.0 * s;
            let bx = 70.0 * s;
            for (i, (label, _)) in buttons.iter().enumerate() {
                let y = sy + i as f32 * (bh as f32 + 6.0 * s);
                let style = ButtonStyle {
                    font_scale: monitor::scaled_val(14) as f32,
                    bg_color: if hover_idx == &Some(i) { [0x70, 0x70, 0x90, 0xFF] } else { [0x50, 0x50, 0x70, 0xFF] },
                    border_color: if hover_idx == &Some(i) { [0x90, 0x90, 0xB0, 0xFF] } else { [0x70, 0x70, 0x90, 0xFF] },
                    ..ButtonStyle::default()
                };
                let (btn_data, btn_text) = button::render(bx, y, bw, bh, label, &style);
                let di = data.len();
                data.push(btn_data);
                ov_descs.push(OvDesc {
                    data_idx: di,
                    w: bw,
                    h: bh,
                    x: bx,
                    y,
                    scale: 1.0,
                });
                sections.push(btn_text);
            }
        }
    }

    // Logo (ALWAYS on top)
    ov_descs.push(OvDesc {
        data_idx: logo_idx,
        w: logo.width,
        h: logo.height,
        x: (pw as f32 - logo.width as f32 * s) / 2.0,
        y: (35.0 * s).min(ph as f32 - logo.height as f32 * s),
        scale: s,
    });

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
