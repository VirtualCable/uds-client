use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::Arc;
use wgpu_text::glyph_brush::OwnedSection;

#[derive(Default)]
#[allow(dead_code)]
pub enum LauncherInner {
    #[default]
    None,
    #[cfg(feature = "test-ui")]
    Test {
        buttons: Vec<(crate::draw::ui::button::Button, LaunchAction)>,
        request: Option<LaunchAction>,
    },
}

#[cfg(feature = "test-ui")]
#[derive(Clone)]
pub enum LaunchAction {
    ShowProgress,
    ShowAbout,
    ShowWarning,
    ShowError,
    ShowYesNo,
    ConnectRdp,
    ConnectRail,
}

impl LauncherInner {
    #[cfg(feature = "test-ui")]
    pub fn new_test() -> Self {
        let s = *crate::monitor::SCALE_FACTOR as f32;
        let bh = crate::monitor::scaled_val(28) as u32;
        let bw = crate::monitor::scaled_val(260) as u32;
        let sy = 42.0 * s;
        let bx = 70.0 * s;

        let mut buttons = Vec::new();
        let labels_and_actions = vec![
            ("RDP Connect", LaunchAction::ConnectRdp),
            ("RDP RAIL Notepad", LaunchAction::ConnectRail),
            ("Progress", LaunchAction::ShowProgress),
            ("About", LaunchAction::ShowAbout),
            ("Warning", LaunchAction::ShowWarning),
            ("Error", LaunchAction::ShowError),
            ("Yes/No", LaunchAction::ShowYesNo),
        ];

        for (i, (label, action)) in labels_and_actions.into_iter().enumerate() {
            let by = sy + i as f32 * (bh as f32 + 6.0 * s);
            let btn = crate::draw::ui::button::Button::new(
                bx,
                by,
                bw,
                bh,
                label.to_string(),
                crate::draw::ui::button::ButtonStyle {
                    font_scale: crate::monitor::scaled_val(14) as f32,
                    ..Default::default()
                },
            );
            buttons.push((btn, action));
        }

        LauncherInner::Test {
            buttons,
            request: None,
        }
    }

    pub fn handle_mouse_move(&mut self, phys_x: f32, phys_y: f32) -> bool {
        match self {
            #[cfg(feature = "test-ui")]
            LauncherInner::Test { buttons, .. } => {
                let mut changed = false;
                for (btn, _) in buttons.iter_mut() {
                    if btn.handle_mouse_move(phys_x, phys_y) {
                        changed = true;
                    }
                }
                changed
            }
            _ => false,
        }
    }

    pub fn handle_click(&mut self, phys_x: f32, phys_y: f32) {
        match self {
            #[cfg(feature = "test-ui")]
            LauncherInner::Test {
                buttons, request, ..
            } => {
                for (btn, action) in buttons.iter() {
                    if btn.contains(phys_x, phys_y) {
                        *request = Some(action.clone());
                        break;
                    }
                }
            }
            _ => {}
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

#[derive(Default)]
pub struct TestingLauncherState {
    pub window: Option<Arc<winit::window::Window>>,
    pub renderer: Option<WgpuRenderer>,
    pub inner: LauncherInner,
    pub last_mouse_pos: Option<(f32, f32)>,
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
    ov_descs.push(OvDesc {
        data_idx: logo_idx,
        w: logo.width,
        h: logo.height,
        x: (pw as f32 - logo.width as f32 * s) / 2.0,
        y: (35.0 * s).min(ph as f32 - logo.height as f32 * s),
        scale: s,
    });

    match &state.inner {
        LauncherInner::None => {}
        #[cfg(feature = "test-ui")]
        LauncherInner::Test { buttons, .. } => {
            for (btn, _) in buttons.iter() {
                let (btn_data, btn_text) = btn.render();
                let di = data.len();
                data.push(btn_data);
                ov_descs.push(OvDesc {
                    data_idx: di,
                    w: btn.w,
                    h: btn.h,
                    x: btn.x,
                    y: btn.y,
                    scale: 1.0,
                });
                sections.push(btn_text);
            }
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

    renderer.update_and_render(&[], pw, ph, &overlays, &sections, None, None);
}
