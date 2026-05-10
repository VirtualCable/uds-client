// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use crate::window::{
    AppWindow,
    rdp::connection::{RdpConnectionState, RemoteWindow},
};
use eframe::egui;
use flume::Receiver;
use rdp::messaging::RdpMessage;
use shared::log;
use std::sync::{Arc, Mutex, RwLock};

impl AppWindow {
    pub fn update_rdp_rail(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
        rdp_state: RdpConnectionState,
    ) {
        // Global mouse handling for RAIL (to handle drags outside viewports)
        let mut mouse_capture = rdp_state.mouse_capture.lock().unwrap();
        let command_tx = &rdp_state.command_tx;
        let command_event = &rdp_state.command_event;

        ui.ctx().input(|i| {
            if let Some(_window_id) = *mouse_capture {
                // If button is released, stop capture
                if !i.pointer.any_down() {
                    *mouse_capture = None;
                }
                // Even if we stop capture now, we should send this last event
                crate::window::rdp::input::handle_mouse(
                    ui.ctx(),
                    command_tx,
                    command_event,
                    i,
                    egui::Vec2::splat(rdp_state.scale_factor as f32),
                    egui::Vec2::ZERO,
                    rdp_state.desktop_size,
                    None,
                );
            }
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show_inside(ui, |ui| {
                while let Ok(message) = rdp_state.update_rx.try_recv() {
                    match message {
                        RdpMessage::UpdateRects(_) => {
                            // In RAIL mode, we usually don't use the main screen texture
                            // but we might need it for some fallback.
                            // For now, we ignore it to save CPU.
                        }
                        RdpMessage::WindowCreate {
                            window_id,
                            owner_id,
                            style,
                            ext_style,
                            taskbar_button,
                            title,
                            show_state,
                            is_offscreen,
                            pos,
                            size,
                        } => {
                            log::info!("RAIL: WindowCreate id={}", window_id);
                            let scale_factor = rdp_state.scale_factor;
                            let (x, y) = pos.unwrap_or((0, 0));
                            let (width, height) = size.unwrap_or((0, 0));

                            let rect = rdp::geom::Rect::new(
                                ((x as f64 / scale_factor) as i32).max(0),
                                ((y as f64 / scale_factor) as i32).max(0),
                                ((width as f64 / scale_factor) as u32)
                                    .min(rdp_state.desktop_size.0),
                                ((height as f64 / scale_factor) as u32)
                                    .min(rdp_state.desktop_size.1),
                            );

                            let w = RemoteWindow {
                                id: window_id,
                                owner_id,
                                style,
                                extended_style: ext_style,
                                taskbar_button,
                                title,
                                show_state,
                                is_offscreen: is_offscreen.unwrap_or(false),
                                rect,
                                resize_requested: true,
                                move_requested: true,
                                last_focused: false,
                                texture: None,
                            };
                            rdp_state
                                .remote_windows
                                .write()
                                .unwrap()
                                .insert(window_id, w);
                            ui.ctx().request_repaint();
                        }
                        RdpMessage::WindowUpdate {
                            window_id,
                            owner_id,
                            style,
                            ext_style,
                            taskbar_button,
                            title,
                            show_state,
                            is_offscreen,
                            pos,
                            size,
                        } => {
                            let mut remote_windows = rdp_state.remote_windows.write().unwrap();
                            let scale_factor = rdp_state.scale_factor;

                            if let Some(w) = remote_windows.get_mut(&window_id) {
                                if let Some(oid) = owner_id {
                                    w.owner_id = Some(oid);
                                }
                                if let Some(s) = style {
                                    w.style = Some(s);
                                }
                                if let Some(s) = ext_style {
                                    w.extended_style = Some(s);
                                }
                                if taskbar_button.is_some() {
                                    w.taskbar_button = taskbar_button;
                                }
                                if !title.is_empty() {
                                    w.title = title;
                                }
                                if let Some(s) = show_state {
                                    w.show_state = Some(s);
                                }
                                if let Some(o) = is_offscreen {
                                    w.is_offscreen = o;
                                }
                                if let Some((x, y)) = pos {
                                    let lx = (x as f64 / scale_factor) as i32;
                                    let ly = (y as f64 / scale_factor) as i32;
                                    if w.rect.x != lx || w.rect.y != ly {
                                        w.rect.x = lx.max(0);
                                        w.rect.y = ly.max(0);
                                        w.move_requested = true;
                                    }
                                }
                                if let Some((width, height)) = size {
                                    let lw = ((width as f64 / scale_factor) as u32)
                                        .min(rdp_state.desktop_size.0);
                                    let lh = ((height as f64 / scale_factor) as u32)
                                        .min(rdp_state.desktop_size.1);
                                    if w.rect.w != lw || w.rect.h != lh {
                                        w.rect.w = lw;
                                        w.rect.h = lh;
                                        w.resize_requested = true;
                                    }
                                }
                                if w.move_requested || w.resize_requested {
                                    ui.ctx().request_repaint();
                                }
                            }
                        }
                        RdpMessage::WindowDelete(window_id) => {
                            log::info!("RAIL: WindowDelete id={}", window_id);
                            rdp_state.remote_windows.write().unwrap().remove(&window_id);
                        }
                        RdpMessage::WindowPixels {
                            window_id,
                            width,
                            height,
                            data,
                        } => {
                            log::debug!("RAIL WindowPixels: id={}, {}x{}", window_id, width, height);
                            let mut windows = rdp_state.remote_windows.write().unwrap();
                            let scale_factor = rdp_state.scale_factor;
                            let lw = (width as f64 / scale_factor) as u32;
                            let lh = (height as f64 / scale_factor) as u32;

                            if let Some(w) = windows.get_mut(&window_id) {
                                if w.rect.w != lw || w.rect.h != lh {
                                    w.rect.w = lw;
                                    w.rect.h = lh;
                                    w.resize_requested = true;
                                    ui.ctx().request_repaint();
                                }
                                let image = egui::ColorImage::from_rgba_unmultiplied(
                                    [width as usize, height as usize],
                                    &data,
                                );
                                if let Some(tex) = &mut w.texture {
                                    tex.set(image, egui::TextureOptions::LINEAR);
                                } else {
                                    w.texture = Some(ui.ctx().load_texture(
                                        format!("window_{}", window_id),
                                        image,
                                        egui::TextureOptions::LINEAR,
                                    ));
                                }
                                ui.ctx()
                                    .request_repaint_of(egui::ViewportId::from_hash_of(window_id));
                            }
                        }
                        RdpMessage::Disconnect => {
                            self.exit(ui.ctx());
                            break;
                        }
                        RdpMessage::Error(err) => {
                            log::error!("RAIL Error: {}", err);
                            self.exit(ui.ctx());
                            break;
                        }
                        _ => {}
                    }
                }

                // Draw main window UI for RAIL mode
                ui.centered_and_justified(|ui| {
                    ui.heading("UDS RemoteApp Connection Active");
                    ui.add_space(10.0);
                    if ui.button("Disconnect").clicked() {
                        self.exit(ui.ctx());
                    }
                });

                draw_rail_windows(
                    ui,
                    rdp_state.remote_windows.clone(),
                    rdp_state.mouse_capture.clone(),
                    rdp_state.channels.read().unwrap().rail(),
                    &rdp_state.command_tx,
                    &rdp_state.command_event,
                    &self.keys_rx,
                    rdp_state.scale_factor,
                    rdp_state.desktop_size,
                );
            });
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rail_windows(
    ui: &mut egui::Ui,
    remote_windows: Arc<RwLock<std::collections::HashMap<u32, RemoteWindow>>>,
    mouse_capture: Arc<Mutex<Option<u32>>>,
    rail_channel: Option<rdp::channels::rail::RailChannel>,
    command_tx: &rdp::commands::Sender,
    command_event: &rdp::utils::SafeHandle,
    keys_rx: &Receiver<crate::RawKey>,
    scale_factor: f64,
    desktop_size: (u32, u32),
) {
    let windows_map = remote_windows.read().unwrap();
    let windows: Vec<_> = windows_map.values().cloned().collect();
    drop(windows_map);

    for window in windows {
        if window.is_offscreen || window.rect.w == 0 || window.rect.h == 0 {
            continue;
        }
        if window.show_state == Some(0) {
            continue;
        }

        // Filter out shadow/overlay windows which are purely for visual effects
        // and render as black rectangles. They always have WS_EX_TRANSPARENT (0x20).
        if window.extended_style.is_some_and(|s| (s & 0x20) != 0) {
            continue;
        }

        let texture_id = if let Some(tex) = &window.texture {
            tex.id()
        } else {
            continue;
        };

        let id = egui::ViewportId::from_hash_of(window.id);
        let rect = window.rect;
        let pos = egui::pos2((rect.x as f32).max(0.0), (rect.y as f32).max(0.0));
        let offset = egui::Vec2::new(pos.x, pos.y);

        if window.resize_requested || window.move_requested {
            let mut windows_map = remote_windows.write().unwrap();
            if let Some(w) = windows_map.get_mut(&window.id) {
                w.resize_requested = false;
                w.move_requested = false;
            }
        }

        if window.resize_requested {
            ui.ctx().send_viewport_cmd_to(
                id,
                egui::ViewportCommand::InnerSize([rect.w as f32, rect.h as f32].into()),
            );
        }
        
        // Force the position every frame to prevent Windows from applying
        // cascading logic (+20, +20) to new RAIL windows like menus/dialogs.
        ui.ctx().send_viewport_cmd_to(
            id,
            egui::ViewportCommand::OuterPosition(egui::pos2(rect.x as f32, rect.y as f32)),
        );

        let window_rail = rail_channel.clone();
        let window_capture = mouse_capture.clone();
        let window_id = window.id;
        let window_title = window.title.clone();
        let cloned_tx = command_tx.clone();
        let cloned_event = *command_event;

        let is_tool_window = window.extended_style.is_some_and(|s| (s & 0x80) != 0);
        let has_real_owner = window.owner_id.is_some() && window.owner_id != Some(0);
        let show_in_taskbar = if let Some(tb) = window.taskbar_button {
            tb
        } else {
            !(is_tool_window || has_real_owner)
        };

        let can_activate = show_in_taskbar;

        let builder = egui::ViewportBuilder::default()
            .with_title(window_title)
            .with_inner_size([rect.w as f32, rect.h as f32])
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(true)
            .with_taskbar(show_in_taskbar)
            .with_position(egui::pos2(rect.x as f32, rect.y as f32));

        let windows_for_closure = remote_windows.clone();
        let keys_rx = keys_rx.clone();
        ui.ctx()
            .show_viewport_deferred(id, builder, move |ctx, _class| {
                let is_focused = ctx.input(|i| i.viewport().focused == Some(true));
                let mut should_activate = false;

                {
                    let mut windows = windows_for_closure.write().unwrap();
                    if let Some(w) = windows.get_mut(&window_id) {
                        if is_focused && !w.last_focused {
                            should_activate = true;
                        }
                        w.last_focused = is_focused;
                    }
                }

                if can_activate && should_activate && let Some(rail) = &window_rail {
                    rail.send_activate(window_id, true);
                }

                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::default()
                            .inner_margin(0.0)
                            .fill(egui::Color32::TRANSPARENT),
                    )
                    .show_inside(ctx, |ui| {
                        ui.add_sized(
                            [rect.w as f32, rect.h as f32],
                            egui::Image::new(egui::load::SizedTexture::new(
                                texture_id,
                                [rect.w as f32, rect.h as f32],
                            )),
                        );
                    });

                ctx.input(|i| {
                    let rail_for_click = window_rail.clone();
                    let mut on_click = move || {
                        if can_activate {
                            if let Some(rail) = &rail_for_click {
                                rail.send_activate(window_id, true);
                            }
                        }
                    };
                    let mut capture = window_capture.lock().unwrap();
                    if i.pointer.any_pressed() {
                        *capture = Some(window_id);
                    }
                    crate::window::rdp::input::handle_mouse(
                        ctx,
                        &cloned_tx,
                        &cloned_event,
                        i,
                        egui::Vec2::splat(scale_factor as f32),
                        offset,
                        desktop_size,
                        Some(&mut on_click),
                    );
                    crate::window::rdp::input::handle_keyboard(
                        ctx,
                        &cloned_tx,
                        &cloned_event,
                        i,
                        &keys_rx,
                        None,
                    );
                });

                if ctx.input(|i| i.viewport().close_requested()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    if let Some(rail) = &window_rail {
                        rail.send_system_command(window_id, rdp::consts::SC_CLOSE as u16);
                    }
                }
            });
    }
}
