// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;
use std::sync::Arc;
use wgpu_text::glyph_brush::OwnedSection;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::AppHandler;
use crate::WindowKind;
use crate::logo;
use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};

#[derive(Default)]
#[allow(dead_code)]
pub enum TestingLauncherInner {
    #[default]
    None,
    Test {
        buttons: Vec<(crate::draw::ui::button::Button, TestingLaunchAction)>,
        request: Option<TestingLaunchAction>,
    },
}

#[derive(Clone)]
pub enum TestingLaunchAction {
    ShowProgress,
    ShowAbout,
    ShowWarning,
    ShowError,
    ShowYesNo,
    ConnectRdp,
    ConnectRail,
}

impl TestingLauncherInner {
    pub fn new_test() -> Self {
        let s = *crate::monitor::SCALE_FACTOR as f32;
        let bh = crate::monitor::scaled_val(28) as u32;
        let bw = crate::monitor::scaled_val(260) as u32;
        let sy = 42.0 * s;
        let bx = 70.0 * s;

        let mut buttons = Vec::new();
        let labels_and_actions = vec![
            ("RDP Connect", TestingLaunchAction::ConnectRdp),
            ("RDP RAIL Notepad", TestingLaunchAction::ConnectRail),
            ("Progress", TestingLaunchAction::ShowProgress),
            ("About", TestingLaunchAction::ShowAbout),
            ("Warning", TestingLaunchAction::ShowWarning),
            ("Error", TestingLaunchAction::ShowError),
            ("Yes/No", TestingLaunchAction::ShowYesNo),
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

        TestingLauncherInner::Test {
            buttons,
            request: None,
        }
    }

    pub fn handle_mouse_move(&mut self, phys_x: f32, phys_y: f32) -> bool {
        match self {
            TestingLauncherInner::Test { buttons, .. } => {
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
        if let TestingLauncherInner::Test {
            buttons, request, ..
        } = self
        {
            for (btn, action) in buttons.iter() {
                if btn.contains(phys_x, phys_y) {
                    *request = Some(action.clone());
                    break;
                }
            }
        }
    }

    pub fn take_request(&mut self) -> Option<TestingLaunchAction> {
        match self {
            TestingLauncherInner::Test { request, .. } => request.take(),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct TestingLauncherState {
    pub window: Option<Arc<winit::window::Window>>,
    pub renderer: Option<WgpuRenderer>,
    pub inner: TestingLauncherInner,
    pub last_mouse_pos: Option<(f32, f32)>,
}

pub fn paint_testing_launcher(state: &mut TestingLauncherState) {
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
        TestingLauncherInner::None => {}
        TestingLauncherInner::Test { buttons, .. } => {
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

impl AppHandler {
    pub(crate) fn open_testing_launcher(
        &mut self,
        el: &ActiveEventLoop,
        inner: TestingLauncherInner,
    ) -> Result<()> {
        let (dw, dh) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let ww = 400.0;
        let wh = 300.0;
        let sf = crate::monitor::scale(0) as f32;
        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_visible(false)
                    .with_title("UDS Launcher")
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_window_icon(Some(logo::load_icon()))
                    .with_resizable(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
            )?,
        );
        let wid = window.id();
        let phys = window.inner_size();

        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;

        window.set_visible(true);
        window.request_redraw();

        self.testing_launcher = Some(TestingLauncherState {
            window: Some(window),
            renderer: Some(renderer),
            inner,
            ..Default::default()
        });
        self.register_window(wid, WindowKind::TestingLauncher);
        Ok(())
    }

    pub(crate) fn close_testing_launcher(&mut self) {
        if let Some(ref l) = self.testing_launcher
            && let Some(w) = &l.window
        {
            self.unregister_window(w.id());
        }
        self.testing_launcher = None;
    }

    pub(crate) fn handle_testing_launcher_event(
        &mut self,
        _el: &ActiveEventLoop,
        event: WindowEvent,
    ) {
        let Some(ref mut l) = self.testing_launcher else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                self.close_testing_launcher();
                self.stop.trigger();
            }
            WindowEvent::RedrawRequested => {
                paint_testing_launcher(l);
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                if let Some(pos) = l.last_mouse_pos {
                    l.inner.handle_click(pos.0, pos.1);
                }
                if let Some(w) = &l.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                l.last_mouse_pos = Some((px, py));
                if l.inner.handle_mouse_move(px, py)
                    && let Some(w) = &l.window
                {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}
