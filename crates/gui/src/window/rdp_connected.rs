#![allow(dead_code)]
use std::sync::{Arc, RwLock, atomic::Ordering};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use eframe::egui;

use crate::log;

use rdp::{
    connection::{Rdp, RdpMessage},
    settings::{RdpSettings, ScreenSize},
};

use crate::geom::RectExt; // For extracting rects from framebuffer

use super::{AppState, AppWindow, State};

const FRAMES_IN_FLIGHT: usize = 128;

pub struct RdpState {
    update_rx: crossbeam::channel::Receiver<RdpMessage>,
    gdi: *mut freerdp_sys::rdpGdi,
    gdi_lock: Arc<RwLock<()>>,
    stop_event: freerdp_sys::HANDLE,
    input: *mut freerdp_sys::rdpInput,
    texture: Option<egui::TextureHandle>,
}

impl AppWindow {
    pub fn switch_to_rdp_connected(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.processing_events.store(true, Ordering::Relaxed); // Start processing events
        let (tx, rx): (Sender<RdpMessage>, Receiver<RdpMessage>) = bounded(FRAMES_IN_FLIGHT);

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([1600.0, 900.0].into()));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([10.0, 10.0].into()));

        let mut rdp = Box::pin(Rdp::new(
            RdpSettings {
                server: "172.27.247.161".to_string(),
                user: "user".to_string(),
                password: "temporal".to_string(),
                screen_size: ScreenSize::Fixed(1600, 900),
                ..RdpSettings::default()
            },
            tx,
        ));
        // rdp.set_update_callbacks(vec![
        //     update_c::Callbacks::BeginPaint,
        //     update_c::Callbacks::EndPaint,
        //     update_c::Callbacks::DesktopResize,
        // ]);
        rdp.as_mut().build()?; // Build inserts "rdp" inside an struct for freedrp, must ensure that rdp does not move after this point

        log::debug!("** Rdp address: {:p}", &rdp);

        rdp.optimize();
        rdp.connect()?;

        #[cfg(debug_assertions)]
        {
            rdp.debug_assert_instance();
        }

        let rdpversion_str = rdp.get_rdp_version()?;

        log::debug!("Connected. RDP Version: {}", rdpversion_str);

        // Ge the gdi pointer
        let gdi = rdp
            .gdi()
            .ok_or_else(|| anyhow::anyhow!("GDI not initialized"))?;
        let input = rdp
            .input()
            .ok_or_else(|| anyhow::anyhow!("Input not initialized"))?;
        // And the lock
        let gdi_lock = rdp.gdi_lock();
        let stop_event = rdp
            .get_stop_event()
            .ok_or_else(|| anyhow::anyhow!("Stop event not available"))?;

        log::debug!("Obtained GDI pointer: {:?}", gdi);
        log::debug!("Gdi: {:?}", unsafe { *gdi });

        // Create a base texture of the right size
        self.app_state = AppState::RdpConnected;
        self.inner_state = State::Rdp(RdpState {
            update_rx: rx,
            gdi,
            input,
            gdi_lock,
            stop_event,
            texture: None,
        });

        // TODO: maybe add a trigger to allow proper shutdown
        std::thread::spawn(move || {
            let res = rdp.run();
            log::debug!("RDP thread exiting...");
            if let Err(e) = res {
                log::debug!("RDP thread ended with error: {}", e);
            } else {
                log::debug!("RDP thread ended.");
            }
        });
        Ok(())
    }
    // We have 2 states, 1 for the connection progress, another for the window

    pub(super) fn update_rdp_client(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Extract rdp state from inner state or switch back to connection state

            let rdp_state = if let State::Rdp(rdp_state) = &mut self.inner_state {
                rdp_state
            } else {
                self.switch_to(ctx, AppState::RdpConnecting);
                return;
            };
            ctx.request_repaint_after(std::time::Duration::from_millis(16));

            // If no texture yet, create one
            if rdp_state.texture.is_none() {
                // Stride in bytes
                let pitch = unsafe { (*rdp_state.gdi).stride } as usize;
                let height = unsafe { (*rdp_state.gdi).height } as usize;
                let buffer = unsafe {
                    std::slice::from_raw_parts(
                        (*rdp_state.gdi).primary_buffer as *const u8,
                        pitch * height,
                    )
                };
                let image = egui::ColorImage::from_rgba_unmultiplied([pitch / 4, height], buffer);
                let texture =
                    ui.ctx()
                        .load_texture("rdp_framebuffer", image, egui::TextureOptions::NEAREST);
                rdp_state.texture = Some(texture);
            }

            let mut switch_back_to_connection = false;
            while let Ok(message) = rdp_state.update_rx.try_recv() {
                match message {
                    RdpMessage::UpdateRects(rects) => {
                        let _guard = rdp_state.gdi_lock.write().unwrap();
                        for rect in rects {
                            let img = rect.extract(
                                unsafe {
                                    std::slice::from_raw_parts(
                                        (*rdp_state.gdi).primary_buffer as *const u8,
                                        ((*rdp_state.gdi).stride as usize)
                                            * (rdp_state.gdi.as_ref().unwrap().height as usize),
                                    )
                                },
                                unsafe { (*rdp_state.gdi).stride as usize },
                                unsafe { (*rdp_state.gdi).width as usize },
                                unsafe { (*rdp_state.gdi).height as usize },
                            );
                            if let Some(image) = img
                                && let Some(texture) = rdp_state.texture.as_mut()
                            {
                                texture.set_partial(
                                    [rect.x as usize, rect.y as usize],
                                    image,
                                    egui::TextureOptions::NEAREST,
                                );
                            }
                        }
                    }
                    RdpMessage::Resize(width, height) => {
                        // TODO: Handle resize
                        log::debug!("Received resize to {}x{}", width, height);
                    }
                    RdpMessage::Disconnect => {
                        log::debug!("RDP Disconnected");
                        // TODO: Handle disconnection properly
                        switch_back_to_connection = true;
                        break;
                    }
                    RdpMessage::Error(err) => {
                        log::debug!("RDP Error: {}", err);
                        switch_back_to_connection = true;
                        break;
                    }
                    RdpMessage::FocusRequired => {
                        log::debug!("RDP Focus Required");
                    }
                }
            }
            if switch_back_to_connection {
                self.switch_to(ctx, AppState::RdpConnecting);
                return;
            }
            // Show the texture on 0,0, full size
            if let Some(texture) = &rdp_state.texture {
                ui.image(texture);
            }

            let input = rdp_state.input;
            self.handle_input(ctx, frame, input);
        });
    }
}
