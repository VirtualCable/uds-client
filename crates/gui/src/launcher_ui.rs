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
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("UDS Launcher")
                    .with_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
                    .with_window_icon(Some(logo::load_icon()))
                    .with_resizable(false),
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
                let sf = p.window.scale_factor() as f32;
                let (lx, ly) = (position.x as f32 / sf, position.y as f32 / sf);
                p.last_mouse_pos = Some((lx, ly));
                if p.handle_mouse_move(lx, ly) {
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
                self.stop.trigger();
                self.progress = None;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_launcher_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        let Some(ref mut l) = self.launcher else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                self.stop.trigger();
                el.exit();
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
                let sf = *monitor::SCALE_FACTOR as f32;
                let lx = position.x as f32 / sf;
                let ly = position.y as f32 / sf;
                l.last_mouse_pos = Some((lx, ly));
                if l.inner.handle_mouse_move(lx, ly) {
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
                let sf = popup.scale;
                let lx = position.x as f32 / sf;
                let ly = position.y as f32 / sf;
                popup.last_mouse_pos = Some((lx, ly));
                if popup.handle_mouse_move(lx, ly) {
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
        let Some(ref mut a) = self.about else { return };
        match event {
            WindowEvent::CloseRequested => {
                self.about = None;
            }
            WindowEvent::MouseInput { state, .. } if state.is_pressed() => {
                self.about = None;
            }
            WindowEvent::RedrawRequested => {
                a.paint();
            }
            _ => {}
        }
    }
}
