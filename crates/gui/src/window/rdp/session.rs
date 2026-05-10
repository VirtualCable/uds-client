// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use crate::window::{AppWindow, rdp::connection::RdpConnectionState};
use eframe::egui;
use rdp::messaging::RdpMessage;
use shared::log;

impl AppWindow {
    pub fn update_rdp_session(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
        mut rdp_state: RdpConnectionState,
    ) {
        // Calculate relation between gdi size and egui content size
        let scale = {
            let egui_size = ui.ctx().content_rect().size();
            let gdi_width = unsafe { (*rdp_state.gdi).width as f32 };
            let gdi_height = unsafe { (*rdp_state.gdi).height as f32 };
            egui::Vec2::new(gdi_width / egui_size.x, gdi_height / egui_size.y)
        };

        self.handle_input(&mut rdp_state, ui, scale, egui::Vec2::ZERO);
        self.handle_screen_resize(ui.ctx(), ui.ctx().content_rect().size(), &mut rdp_state);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show_inside(ui, |ui| {
                let mut rects_to_update: Vec<rdp::geom::Rect> = Vec::new();
                while let Ok(message) = rdp_state.update_rx.try_recv() {
                    match message {
                        RdpMessage::UpdateRects(rects) => {
                            log::debug!("RDP UpdateRects: {} rects", rects.len());
                            rects_to_update.extend_from_slice(&rects);
                        }
                        RdpMessage::Disconnect => {
                            log::debug!("RDP Disconnected");
                            self.exit(ui.ctx());
                            break;
                        }
                        RdpMessage::Error(err) => {
                            log::error!("RDP Error: {}", err);
                            self.exit(ui.ctx());
                            break;
                        }
                        RdpMessage::SetCursorIcon(data, x, y, width, height) => {
                            self.set_custom_cursor(
                                ui.ctx(),
                                &mut rdp_state,
                                &data,
                                rdp::geom::Rect {
                                    x: x as i32,
                                    y: y as i32,
                                    w: width,
                                    h: height,
                                },
                            );
                        }
                        _ => {}
                    }
                }
                rdp_state.screen.update_screen_texture(
                    &rects_to_update,
                    rdp_state.gdi,
                    &rdp_state.gdi_lock,
                );

                // If the size of gdi is not equal to size of content, resize gdi and recreate texture
                let screen_rect = ui.available_rect_before_wrap();
                rdp_state
                    .screen
                    .paint(ui, screen_rect, rdp_state.fps.clone());

                self.handle_cursor(ui.ctx(), &rdp_state);
                self.show_pinbar(ui, &mut rdp_state);
            });

        rdp_state.fps.borrow_mut().record_frame();
    }
}
