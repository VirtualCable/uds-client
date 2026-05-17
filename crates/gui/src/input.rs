use std::sync::atomic::Ordering;

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;

use shared::log;

use super::{AppHandler, RawKey};
use crate::monitor;

impl AppHandler {
    pub(crate) fn handle_keyboard(&mut self, el: &ActiveEventLoop, event: &WindowEvent) -> bool {
        let WindowEvent::KeyboardInput { event: key_ev, .. } = event else {
            return false;
        };
        let PhysicalKey::Code(code) = key_ev.physical_key else {
            return false;
        };

        // Track Alt
        match code {
            winit::keyboard::KeyCode::AltLeft | winit::keyboard::KeyCode::AltRight => {
                self.alt_held = key_ev.state.is_pressed();
            }
            _ => {}
        }

        if !self.processing_events.load(Ordering::Relaxed) {
            return false;
        }

        // Hotkeys
        if self.alt_held && key_ev.state.is_pressed() && !key_ev.repeat {
            let is_rail = self.rdp.as_ref().is_some_and(|s| s.is_rail);
            match code {
                winit::keyboard::KeyCode::Enter if !is_rail => {
                    log::debug!("Alt+Enter → fullscreen");
                    self.toggle_fullscreen();
                    return true;
                }
                winit::keyboard::KeyCode::KeyF if !is_rail => {
                    if let Some(ref s) = self.rdp {
                        s.fps.toggle();
                    }
                    return true;
                }
                winit::keyboard::KeyCode::F4 => {
                    log::debug!("Alt+F4 → exit");
                    self.stop.trigger();
                    el.exit();
                    return true;
                }
                _ => {}
            }
        }

        let raw = RawKey {
            keycode: code,
            pressed: key_ev.state.is_pressed(),
            repeat: key_ev.repeat,
        };
        let _ = self.keys_tx.send(raw);
        true
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        let Some(s) = &mut self.rdp else { return };
        let is_fs = s.full_screen.load(Ordering::Relaxed);
        if !is_fs {
            s.window
                .window
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            s.full_screen.store(true, Ordering::Relaxed);
        } else {
            s.window.window.set_fullscreen(None);
            s.full_screen.store(false, Ordering::Relaxed);
            if s.last_windowed_size.is_none() {
                let phys = s.window.window.inner_size();
                let w = (phys.width as f64 * 2.0 / 3.0) as u32;
                let h = (phys.height as f64 * 2.0 / 3.0) as u32;
                let sf = s.window.window.scale_factor();
                let _ = s
                    .window
                    .window
                    .request_inner_size(winit::dpi::LogicalSize::new(w as f64 / sf, h as f64 / sf));
                s.last_windowed_size = Some((w, h));
            }
        }
    }

    pub(crate) fn handle_rdp_input(&mut self, event: &WindowEvent) -> bool {
        let Some(s) = &mut self.rdp else { return true };
        if s.is_rail {
            match event {
                WindowEvent::CloseRequested => return false,
                WindowEvent::CursorEntered { .. } => {
                    s.window.window.set_cursor_visible(true);
                }
                WindowEvent::CursorLeft { .. } => {
                    s.window.window.set_cursor_visible(false);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let px = position.x as f32;
                    let py = position.y as f32;
                    s.cursor.x = px;
                    s.cursor.y = py;
                    if let Some(ref mut rc) = s.rail_control
                        && rc.handle_mouse_move(px, py)
                    {
                        s.window.window.request_redraw();
                    }
                }
                WindowEvent::MouseInput { state, button, .. }
                    if state.is_pressed() && *button == winit::event::MouseButton::Left =>
                {
                    if let Some(ref mut rc) = s.rail_control {
                        if rc.handle_click(s.cursor.x, s.cursor.y) {
                            // Clicked exit!
                            return false; // This will close the application
                        } else {
                            // Clicked outside button -> DRAG!
                            let _ = s.window.window.drag_window();
                        }
                    }
                }
                _ => {}
            }
            return true;
        }
        match event {
            WindowEvent::CloseRequested => return false,
            WindowEvent::Resized(_) => {
                s.request_screen_resize();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_pointer = Some(*position);
                s.cursor.x = position.x as f32;
                s.cursor.y = position.y as f32;
                s.window.window.request_redraw();
                let gdi_w = unsafe { (*s.gdi).width as u32 };
                let gdi_h = unsafe { (*s.gdi).height as u32 };
                let phys_w = s.window.window.inner_size().width;
                let phys_h = s.window.window.inner_size().height;
                let x = ((position.x * gdi_w as f64) / phys_w as f64)
                    .round()
                    .clamp(0.0, (gdi_w - 1) as f64) as u16;
                let y = ((position.y * gdi_h as f64) / phys_h as f64)
                    .round()
                    .clamp(0.0, (gdi_h - 1) as f64) as u16;
                let _ = s.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                    rdp_ffi::commands::InputEvent::Mouse {
                        flags: rdp_ffi::sys::PTR_FLAGS_MOVE as u16,
                        x,
                        y,
                    },
                ));
                unsafe {
                    rdp_ffi::sys::SetEvent(s.command_event.as_handle());
                }
                // Pinbar
                let is_fs = s.full_screen.load(Ordering::Relaxed);
                if position.y < 5.0
                    && position.x > std::cmp::max(phys_w, 1) as f64 * 0.4
                    && position.x < phys_w as f64 * 0.6
                {
                    s.pinbar.visible = is_fs;
                }
                if position.y > 32.0 {
                    s.pinbar.visible = false;
                }
            }
            WindowEvent::MouseInput {
                state: btn, button, ..
            } => {
                // Pinbar click — only on press
                if btn.is_pressed()
                    && let Some(pos) = self.last_pointer
                    && s.pinbar.visible
                    && *button == winit::event::MouseButton::Left
                {
                    let px = pos.x as f32;
                    if s.pinbar.btn_fs_x.contains(&px) {
                        self.toggle_fullscreen();
                        return true;
                    }
                    if s.pinbar.btn_close_x.contains(&px) {
                        return false;
                    }
                }

                if let Some(pos) = self.last_pointer {
                    let gdi_w = unsafe { (*s.gdi).width as u32 };
                    let gdi_h = unsafe { (*s.gdi).height as u32 };
                    let phys_w = s.window.window.inner_size().width;
                    let phys_h = s.window.window.inner_size().height;
                    let x = ((pos.x * gdi_w as f64) / phys_w as f64)
                        .round()
                        .clamp(0.0, (gdi_w - 1) as f64) as u16;
                    let y = ((pos.y * gdi_h as f64) / phys_h as f64)
                        .round()
                        .clamp(0.0, (gdi_h - 1) as f64) as u16;
                    let flags = match *button {
                        winit::event::MouseButton::Left => rdp_ffi::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp_ffi::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp_ffi::sys::PTR_FLAGS_BUTTON3,
                        _ => 0,
                    } as u16;
                    if flags != 0 {
                        let f = flags
                            | if btn.is_pressed() {
                                rdp_ffi::sys::PTR_FLAGS_DOWN as u16
                            } else {
                                0
                            };
                        let _ = s.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                            rdp_ffi::commands::InputEvent::Mouse { flags: f, x, y },
                        ));
                        unsafe {
                            rdp_ffi::sys::SetEvent(s.command_event.as_handle());
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let mut wd = (dy as f32 * 120.0) as i32;
                let flags = (rdp_ffi::sys::PTR_FLAGS_WHEEL as u16)
                    | if wd < 0 {
                        wd = -wd;
                        rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                    } else {
                        0
                    };
                while wd > 0 {
                    let step: u16 = if wd > 0xFF { 0xFF } else { (wd & 0xFF) as u16 };
                    wd -= step as i32;
                    let cflags = if flags & (rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = s.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(s.command_event.as_handle());
                    }
                }
            }
            _ => {}
        }
        true
    }

    pub(crate) fn handle_rail_control_redraw(&mut self) {
        if let Some(ref mut state) = self.rdp
            && let Some(ref mut rc) = state.rail_control
        {
            let phys = state.window.window.inner_size();
            let scale = *crate::monitor::SCALE_FACTOR as f32;
            rc.paint(&mut state.window.renderer, phys.width, phys.height, scale);
        }
    }

    pub(crate) fn handle_rail_redraw(&mut self, rail_id: u32) {
        if let Some(ref mut state) = self.rdp
            && let Some(rw) = state.rail_windows.get_mut(&rail_id)
        {
            // Force position every frame to prevent Windows cascading offset
            let sf = state.coords_scale.max(1.0);
            let (px, py) = monitor::logic_2_phys_pos((rw.rect.x, rw.rect.y), sf);
            rw.window
                .set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
            if let (Some(rgba), Some(ref mut renderer)) = (&rw.rgba_data, rw.renderer.as_mut()) {
                renderer.update_and_render(
                    rgba.as_slice(),
                    rw.width,
                    rw.height,
                    &[],
                    &[],
                    None,
                    None,
                );
            }
        }
    }

    pub(crate) fn handle_rail_event(&mut self, rail_id: u32, event: WindowEvent) {
        let Some(ref mut state) = self.rdp else {
            return;
        };
        let Some(rail_channel) = state.rail_channel.clone() else {
            return;
        };
        let cmd_tx = state.command_tx.clone();
        let cmd_ev = state.command_event;

        if let WindowEvent::MouseInput { state: btn, .. } = &event {
            self.rail_button_down = if btn.is_pressed() {
                Some(rail_id)
            } else {
                None
            };
        }

        match event {
            WindowEvent::CloseRequested => {
                rail_channel.send_system_command(rail_id, rdp_ffi::consts::SC_CLOSE as u16);
            }
            WindowEvent::Focused(focused) => {
                if let Some(rw) = state.rail_windows.get_mut(&rail_id) {
                    if focused && !rw.last_focused && rw.show_in_taskbar {
                        rail_channel.send_activate(rail_id, true);
                    }
                    rw.last_focused = focused;
                }
            }
            WindowEvent::CursorMoved { ref position, .. } => {
                self.last_pointer = Some(*position);
                if let Some(rw) = state.rail_windows.get(&rail_id) {
                    let sf = state.coords_scale;
                    let (gx, gy) = monitor::phys_2_logic(
                        (
                            (position.x + rw.rect.x as f64 * sf).round() as i32,
                            (position.y + rw.rect.y as f64 * sf).round() as i32,
                        ),
                        sf,
                    );
                    let dw = state.desktop_size.0.saturating_sub(1) as i32;
                    let dh = state.desktop_size.1.saturating_sub(1) as i32;
                    let gx = gx.clamp(0, dw) as u16;
                    let gy = gy.clamp(0, dh) as u16;
                    shared::log::trace!(
                        "RAIL[{rail_id}] MoveSend pos=({:.0},{:.0}) sf={sf} rect=({},{})+{}x{} → g=({gx},{gy}) clamp=({dw:.0},{dh:.0})",
                        position.x,
                        position.y,
                        rw.rect.x,
                        rw.rect.y,
                        rw.rect.w,
                        rw.rect.h,
                    );
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: rdp_ffi::sys::PTR_FLAGS_MOVE as u16,
                            x: gx,
                            y: gy,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                // Do NOT synthesize release — Windows SetCapture will deliver
                // the real MouseInput release even when cursor is outside.
                shared::log::trace!(
                    "RAIL[{rail_id}] CursorLeft (button_down={})",
                    self.rail_button_down.is_some()
                );
            }
            WindowEvent::MouseInput {
                button, state: btn, ..
            } => {
                shared::log::trace!(
                    "RAIL[{rail_id}] MouseInput({button:?} pressed={})",
                    btn.is_pressed()
                );
                if btn.is_pressed()
                    && state
                        .rail_windows
                        .get(&rail_id)
                        .is_some_and(|rw| rw.show_in_taskbar)
                {
                    rail_channel.send_activate(rail_id, true);
                }
                if let Some(pos) = self.last_pointer
                    && let Some(rw) = state.rail_windows.get(&rail_id)
                {
                    let bm = match button {
                        winit::event::MouseButton::Left => rdp_ffi::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp_ffi::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp_ffi::sys::PTR_FLAGS_BUTTON3,
                        _ => return,
                    } as u16;
                    let f = bm
                        | if btn.is_pressed() {
                            rdp_ffi::sys::PTR_FLAGS_DOWN as u16
                        } else {
                            0
                        };
                    let sf = state.coords_scale;
                    let (gx, gy) = monitor::phys_2_logic(
                        (
                            (pos.x + rw.rect.x as f64 * sf).round() as i32,
                            (pos.y + rw.rect.y as f64 * sf).round() as i32,
                        ),
                        sf,
                    );
                    let dw = state.desktop_size.0.saturating_sub(1) as i32;
                    let dh = state.desktop_size.1.saturating_sub(1) as i32;
                    let gx = gx.clamp(0, dw) as u16;
                    let gy = gy.clamp(0, dh) as u16;
                    log::trace!(
                        "RAIL[{rail_id}] MouseClick → flags={f} x={gx} y={gy} (phys=({:.0},{:.0}) sf={sf} rect=({},{})+{}x{})",
                        pos.x,
                        pos.y,
                        rw.rect.x,
                        rw.rect.y,
                        rw.rect.w,
                        rw.rect.h
                    );
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: f,
                            x: gx,
                            y: gy,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let mut wd = (dy as f32 * 120.0) as i32;
                let flags = (rdp_ffi::sys::PTR_FLAGS_WHEEL as u16)
                    | if wd < 0 {
                        wd = -wd;
                        rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                    } else {
                        0
                    };
                while wd > 0 {
                    let step: u16 = if wd > 0xFF { 0xFF } else { (wd & 0xFF) as u16 };
                    wd -= step as i32;
                    let cflags = if flags & (rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                }
            }
            _ => {}
        }
    }
}
