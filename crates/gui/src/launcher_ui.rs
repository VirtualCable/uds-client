use std::sync::Arc;

use anyhow::Result;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use super::{AppHandler, WindowKind};
use crate::launcher::{LauncherInner, TestingLauncherState, paint_launcher};
use crate::logo;
use crate::monitor;
use crate::wgpu_render::WgpuRenderer;

impl AppHandler {
    pub(crate) fn open_launcher(
        &mut self,
        el: &ActiveEventLoop,
        inner: LauncherInner,
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
        self.launcher = Some(TestingLauncherState {
            window: Some(window),
            renderer: Some(renderer),
            inner,
            ..Default::default()
        });
        self.register_window(wid, WindowKind::Launcher);
        Ok(())
    }

    pub(crate) fn close_launcher(&mut self) {
        if let Some(ref l) = self.launcher
            && let Some(w) = &l.window
        {
            self.unregister_window(w.id());
        }
        self.launcher = None;
    }

    pub(crate) fn handle_progress_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        let Some(ref mut p) = self.progress else {
            return;
        };

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                p.last_mouse_pos = Some((px, py));
                if p.handle_mouse_move(px, py) {
                    p.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(pos) = p.last_mouse_pos {
                    p.handle_click(pos.0, pos.1);
                    if p.cancelled {
                        self.stop.trigger();
                        el.exit();
                    }
                }
                p.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                p.paint();
            }
            WindowEvent::CloseRequested => {
                self.close_progress();
                self.stop.trigger();
            }
            _ => {}
        }
    }

    pub(crate) fn close_progress(&mut self) {
        if let Some(ref p) = self.progress {
            self.unregister_window(p.window.id());
        }
        self.progress = None;
    }

    pub(crate) fn handle_launcher_event(&mut self, _el: &ActiveEventLoop, event: WindowEvent) {
        let Some(ref mut l) = self.launcher else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                self.close_launcher();
                self.stop.trigger();
            }
            WindowEvent::RedrawRequested => {
                paint_launcher(l);
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
                if l.inner.handle_mouse_move(px, py) {
                    if let Some(w) = &l.window {
                        w.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_popup_event(&mut self, event: WindowEvent) {
        if self.popup.is_none() {
            return;
        }
        let popup = self.popup.as_mut().unwrap();
        let mut close = false;
        match event {
            WindowEvent::CloseRequested => close = true,
            WindowEvent::RedrawRequested => {
                popup.paint();
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                if let Some(pos) = popup.last_mouse_pos {
                    close = popup.handle_click(pos.0, pos.1);
                }
                if !close {
                    popup.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                popup.last_mouse_pos = Some((px, py));
                if popup.handle_mouse_move(px, py) {
                    popup.window.request_redraw();
                }
            }
            _ => {}
        }
        if close {
            let wid = popup.window.id();
            self.unregister_window(wid);
            self.popup = None;
        }
    }

    pub(crate) fn handle_about_event(&mut self, event: WindowEvent) {
        let Some(_) = self.about else { return };
        match event {
            WindowEvent::CloseRequested => {
                self.close_about();
            }
            WindowEvent::MouseInput { state, .. } if state.is_pressed() => {
                self.close_about();
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut a) = self.about {
                    a.paint();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn close_about(&mut self) {
        if let Some(ref a) = self.about {
            self.unregister_window(a.window().id());
        }
        self.about = None;
    }
}
