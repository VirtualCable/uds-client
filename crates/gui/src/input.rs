// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::atomic::Ordering;

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;

use shared::log;

use super::{AppHandler, RawKey};
use crate::monitor;
use crate::windows::rdp_window::RdpMode;

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
            let is_rail = self
                .rdp
                .as_ref()
                .is_some_and(|s| matches!(s.mode, RdpMode::Rail(_)));
            match code {
                winit::keyboard::KeyCode::Enter if !is_rail => {
                    log::debug!("Alt+Enter → fullscreen");
                    self.toggle_fullscreen();
                    return true;
                }
                winit::keyboard::KeyCode::KeyF if !is_rail => {
                    if let Some(ref s) = self.rdp
                        && let RdpMode::Desktop { ref fps, .. } = s.mode
                    {
                        fps.toggle();
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
        if let RdpMode::Desktop {
            ref full_screen,
            ref mut last_windowed_size,
            ..
        } = s.mode
        {
            let is_fs = full_screen.load(Ordering::Relaxed);
            if !is_fs {
                s.window
                    .window
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                full_screen.store(true, Ordering::Relaxed);
            } else {
                s.window.window.set_fullscreen(None);
                full_screen.store(false, Ordering::Relaxed);
                if last_windowed_size.is_none() {
                    let phys = s.window.window.inner_size();
                    let w = (phys.width as f64 * 2.0 / 3.0) as u32;
                    let h = (phys.height as f64 * 2.0 / 3.0) as u32;
                    let sf = s.window.window.scale_factor();
                    let _ = s
                        .window
                        .window
                        .request_inner_size(winit::dpi::LogicalSize::new(
                            w as f64 / sf,
                            h as f64 / sf,
                        ));
                    *last_windowed_size = Some((w, h));
                }
            }
        }
    }

    pub(crate) fn handle_rdp_input(&mut self, event: &WindowEvent) -> bool {
        let Some(s) = &mut self.rdp else { return true };
        if let RdpMode::Rail(ref mut rail) = s.mode {
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
                    if let Some(ref mut rc) = rail.control.as_mut()
                        && rc.handle_mouse_move(px, py)
                    {
                        s.window.window.request_redraw();
                    }
                }
                WindowEvent::MouseInput { state, button, .. }
                    if state.is_pressed() && *button == winit::event::MouseButton::Left =>
                {
                    if let Some(ref mut rc) = rail.control.as_mut() {
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
                let _ = s.command_tx.send(rdp::messaging::RdpCommand::Input(
                    rdp::messaging::InputEvent::Mouse {
                        flags: rdp::sys::PTR_FLAGS_MOVE as u16,
                        x,
                        y,
                    },
                ));
                rdp::Rdp::set_command_event(&s.command_event);
                // Pinbar
                if let RdpMode::Desktop {
                    ref mut pinbar,
                    ref full_screen,
                    ..
                } = s.mode
                {
                    let is_fs = full_screen.load(Ordering::Relaxed);
                    let trigger_y = crate::monitor::scaled_val(5) as f64;
                    let close_y = crate::monitor::scaled_val(32) as f64;
                    if position.y < trigger_y
                        && position.x > std::cmp::max(phys_w, 1) as f64 * 0.4
                        && position.x < phys_w as f64 * 0.6
                    {
                        pinbar.visible = is_fs;
                    }
                    if position.y > close_y {
                        pinbar.visible = false;
                    }
                }
            }
            WindowEvent::MouseInput {
                state: btn, button, ..
            } => {
                // Pinbar click — only on press
                if let RdpMode::Desktop { ref pinbar, .. } = s.mode
                    && btn.is_pressed()
                    && let Some(pos) = self.last_pointer
                    && pinbar.visible
                    && *button == winit::event::MouseButton::Left
                {
                    let px = pos.x as f32;
                    if pinbar.btn_fs_x.contains(&px) {
                        self.toggle_fullscreen();
                        return true;
                    }
                    if pinbar.btn_close_x.contains(&px) {
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
                        winit::event::MouseButton::Left => rdp::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp::sys::PTR_FLAGS_BUTTON3,
                        _ => 0,
                    } as u16;
                    if flags != 0 {
                        let f = flags
                            | if btn.is_pressed() {
                                rdp::sys::PTR_FLAGS_DOWN as u16
                            } else {
                                0
                            };
                        let _ = s.command_tx.send(rdp::messaging::RdpCommand::Input(
                            rdp::messaging::InputEvent::Mouse { flags: f, x, y },
                        ));
                        rdp::Rdp::set_command_event(&s.command_event);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let mut wd = (dy as f32 * 120.0) as i32;
                let flags = (rdp::sys::PTR_FLAGS_WHEEL as u16)
                    | if wd < 0 {
                        wd = -wd;
                        rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                    } else {
                        0
                    };
                while wd > 0 {
                    let step: u16 = if wd > 0xFF { 0xFF } else { (wd & 0xFF) as u16 };
                    wd -= step as i32;
                    let cflags = if flags & (rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = s.command_tx.send(rdp::messaging::RdpCommand::Input(
                        rdp::messaging::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    rdp::Rdp::set_command_event(&s.command_event);
                }
            }
            _ => {}
        }
        true
    }

    pub(crate) fn handle_rail_control_redraw(&mut self) {
        if let Some(ref mut state) = self.rdp
            && let RdpMode::Rail(ref mut rail) = state.mode
            && let Some(ref mut rc) = rail.control
        {
            let phys = state.window.window.inner_size();
            let scale = *crate::monitor::SCALE_FACTOR as f32;
            rc.paint(&mut state.window.renderer, phys.width, phys.height, scale);
        }
    }

    pub(crate) fn handle_rail_redraw(&mut self, rail_id: u32) {
        if let Some(ref mut state) = self.rdp
            && let RdpMode::Rail(ref mut rail) = state.mode
            && let Some(rw) = rail.windows.get_mut(&rail_id)
        {
            // Force position every frame to prevent Windows cascading offset.
            // Only for on-screen windows — off-screen coords are for minimized/offscreen.
            const OFFSCREEN: i32 = rdp::consts::OFFSCREEN_THRESHOLD;
            if rw.rect.x > OFFSCREEN && rw.rect.y > OFFSCREEN {
                let sf = state.coords_scale.max(1.0);
                let (px, py) = monitor::logic_2_phys_pos((rw.rect.x, rw.rect.y), sf);
                rw.window
                    .set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
            }

            // Detect minimize/restore transitions via polling (Occluded is unsupported
            // on Windows/Android/Wayland). When the OS restores a window, tell the server.
            let is_minimized = rw.window.is_minimized().unwrap_or(false);
            if is_minimized != rw.was_minimized {
                if let Some(ref channel) = rail.channel {
                    let cmd = if is_minimized {
                        rdp::windows_types::SystemCommand::Minimize
                    } else {
                        rdp::windows_types::SystemCommand::Restore
                    };
                    channel.send_system_command(rail_id, cmd);
                }
                rw.was_minimized = is_minimized;
            }
            if let Some(ref mut renderer) = rw.renderer.as_mut() {
                // Ensure the WGPU surface matches the actual window size.
                // request_inner_size() is async — the window may have resized
                // since the last reconfigure, but we never handle Resized for
                // Rail windows, so the surface config can be stale.
                let phys = rw.window.inner_size();
                if phys.width > 0 && phys.height > 0 {
                    renderer.reconfigure(phys.width, phys.height);
                }
                let upload_data = if rw.rgba_dirty {
                    rw.rgba_dirty = false;
                    rw.rgba_data.as_deref().unwrap_or(&[])
                } else {
                    &[]
                };
                renderer.update_and_render(upload_data, rw.width, rw.height, &[], &[], None, None);
            }
        }
    }

    pub(crate) fn handle_rail_event(&mut self, rail_id: u32, event: WindowEvent) {
        let Some(ref mut state) = self.rdp else {
            return;
        };
        let RdpMode::Rail(ref mut rail) = state.mode else {
            return;
        };
        let Some(rail_channel) = rail.channel.clone() else {
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
                rail_channel.send_system_command(rail_id, rdp::windows_types::SystemCommand::Close);
            }
            WindowEvent::Occluded(_) => {
                // Occluded unsupported on Windows/Android/Wayland/Orbital.
                // Minimize/restore handled via is_minimized() polling in handle_rail_redraw.
            }
            WindowEvent::Focused(focused) => {
                if let Some(rw) = rail.windows.get_mut(&rail_id) {
                    if focused && !rw.last_focused && rw.show_in_taskbar {
                        rail_channel.send_activate(rail_id, true);
                    }
                    rw.last_focused = focused;
                }
            }
            WindowEvent::CursorMoved { ref position, .. } => {
                self.last_pointer = Some(*position);
                if let Some(rw) = rail.windows.get(&rail_id) {
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
                    let _ = cmd_tx.send(rdp::messaging::RdpCommand::Input(
                        rdp::messaging::InputEvent::Mouse {
                            flags: rdp::sys::PTR_FLAGS_MOVE as u16,
                            x: gx,
                            y: gy,
                        },
                    ));
                    rdp::Rdp::set_command_event(&cmd_ev);
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
                    && rail
                        .windows
                        .get(&rail_id)
                        .is_some_and(|rw| rw.show_in_taskbar)
                {
                    rail_channel.send_activate(rail_id, true);
                }
                if let Some(pos) = self.last_pointer
                    && let Some(rw) = rail.windows.get(&rail_id)
                {
                    let bm = match button {
                        winit::event::MouseButton::Left => rdp::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp::sys::PTR_FLAGS_BUTTON3,
                        _ => return,
                    } as u16;
                    let f = bm
                        | if btn.is_pressed() {
                            rdp::sys::PTR_FLAGS_DOWN as u16
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
                    let _ = cmd_tx.send(rdp::messaging::RdpCommand::Input(
                        rdp::messaging::InputEvent::Mouse {
                            flags: f,
                            x: gx,
                            y: gy,
                        },
                    ));
                    rdp::Rdp::set_command_event(&cmd_ev);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let mut wd = (dy as f32 * 120.0) as i32;
                let flags = (rdp::sys::PTR_FLAGS_WHEEL as u16)
                    | if wd < 0 {
                        wd = -wd;
                        rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                    } else {
                        0
                    };
                while wd > 0 {
                    let step: u16 = if wd > 0xFF { 0xFF } else { (wd & 0xFF) as u16 };
                    wd -= step as i32;
                    let cflags = if flags & (rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = cmd_tx.send(rdp::messaging::RdpCommand::Input(
                        rdp::messaging::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    rdp::Rdp::set_command_event(&cmd_ev);
                }
            }
            _ => {}
        }
    }
}
